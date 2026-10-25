//! Fixed-size SDL2 display for the ferrovgl desktop simulator.
//!
//! Rendering uses a single full-size XRGB8888 buffer and synchronous SDL
//! presentation. Mouse, keyboard and wheel feed real LVGL input devices.
//! The software SDL
//! renderer also works with `SDL_VIDEODRIVER=dummy` for headless smoke checks.

#![warn(missing_docs)]

mod input;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use ferrovgl_core::display::{DisplayThemeInit, FlushResult, Rotation};
use ferrovgl_core::draw::draw_buf::{DRAW_BUF_ALIGN, DrawBuf, STRIDE_AUTO};
use ferrovgl_core::group::GroupId;
use ferrovgl_core::indev::IndevId;
use ferrovgl_core::{Area, ColorFormat, DisplayDriver, DisplayId, Obj, RenderMode, Ui, UiConfig};
use sdl2::event::{Event, WindowEvent};
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{BlendMode, Canvas};
use sdl2::video::Window;

/// Default window title used by the examples.
pub const DEFAULT_TITLE: &str = "ferrovgl";

struct Presenter {
    canvas: Canvas<Window>,
    width: u32,
    height: u32,
    pitch: usize,
    frames: u64,
    error: Option<String>,
}

impl Presenter {
    fn present(&mut self, pixels: &[u8]) -> Result<(), String> {
        let required = self.pitch * self.height as usize;
        if pixels.len() < required {
            return Err("SDL flush received a buffer smaller than the display".into());
        }
        // A scoped texture avoids self-referential lifetimes or unsafe SDL
        // handles. BGRA32 describes byte order on both endian variants; the
        // unused X byte must not become alpha when copying to the window.
        let creator = self.canvas.texture_creator();
        let mut texture = creator
            .create_texture_streaming(PixelFormatEnum::BGRA32, self.width, self.height)
            .map_err(|error| error.to_string())?;
        texture.set_blend_mode(BlendMode::None);
        texture
            .update(None, &pixels[..required], self.pitch)
            .map_err(|error| error.to_string())?;
        self.canvas.copy(&texture, None, None)?;
        self.canvas.present();
        self.frames += 1;
        Ok(())
    }
}

struct SdlDriver(Rc<RefCell<Presenter>>);

impl DisplayDriver for SdlDriver {
    fn flush(&mut self, _disp: DisplayId, _area: &Area, pixels: &mut [u8]) -> FlushResult {
        // Direct mode passes the full buffer even when only a small area was
        // invalidated. Presenting it preserves all pixels outside that area.
        let mut presenter = self.0.borrow_mut();
        if presenter.error.is_none()
            && let Err(error) = presenter.present(pixels)
        {
            presenter.error = Some(error);
        }
        // Errors are returned by step(); never leave the UI waiting on a
        // transfer that has already failed synchronously.
        FlushResult::Done
    }
}

/// A UI context, fixed-size SDL window, draw unit and event pump.
///
/// Call [`Self::step`] on the creating thread, then sleep between iterations.
/// Mouse, keypad and encoder devices are installed automatically; attach a
/// focus group using [`Self::set_group`]. Changing its display's
/// resolution, rotation, offsets, format or buffering mode is unsupported.
pub struct SdlDisplay {
    /// Build and update the application's objects here.
    pub ui: Ui,
    display: DisplayId,
    presenter: Rc<RefCell<Presenter>>,
    events: sdl2::EventPump,
    inputs: input::Inputs,
    // Keep SDL alive until the UI driver, canvas and event pump are dropped.
    sdl: sdl2::Sdl,
    last_tick: Instant,
    running: bool,
}

impl core::fmt::Debug for SdlDisplay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SdlDisplay")
            .field("ui", &self.ui)
            .field("display", &self.display)
            .field("running", &self.running)
            .finish_non_exhaustive()
    }
}

impl SdlDisplay {
    /// Create a single-buffered XRGB8888 window and software draw unit.
    ///
    /// `config.font_default` supplies the text font. An optional theme hook
    /// runs before screen creation, as in [`Ui::display_create_ex`]. SDL errors
    /// and dimensions that cannot fit the draw buffer header return `Err`.
    pub fn new(
        title: &str,
        width: u32,
        height: u32,
        config: UiConfig,
        theme_init: Option<DisplayThemeInit>,
    ) -> Result<Self, String> {
        let pitch = checked_pitch(width, height)?;
        let buffer = DrawBuf::create(width, height, ColorFormat::Xrgb8888, STRIDE_AUTO)
            .ok_or_else(|| "could not allocate the SDL draw buffer".to_owned())?;
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let window = video
            .window(title, width, height)
            .position_centered()
            .build()
            .map_err(|error| error.to_string())?;
        let canvas = window
            .into_canvas()
            .software()
            .build()
            .map_err(|error| error.to_string())?;
        let presenter = Rc::new(RefCell::new(Presenter {
            canvas,
            width,
            height,
            pitch,
            frames: 0,
            error: None,
        }));
        video.text_input().start();
        let events = sdl.event_pump()?;
        let mut ui = Ui::new(config);
        ferrovgl_draw_sw::init(&mut ui);
        let display = ui
            .display_create_ex(width as i32, height as i32, theme_init)
            .ok_or_else(|| "could not create the SDL display".to_owned())?;
        ui.display_set_color_format(display, ColorFormat::Xrgb8888);
        ui.display_set_draw_buffers(display, Some(buffer), None);
        ui.display_set_render_mode(display, RenderMode::Direct);
        ui.display_set_driver(display, Some(Box::new(SdlDriver(presenter.clone()))));
        if let Some(timer) = ui.display_get_refr_timer(Some(display)) {
            let presenter = presenter.clone();
            // Another timer can change the display before this refresh timer
            // runs in the same handler pass. Validate before entering drawing,
            // as well as before the explicit refresh in step().
            ui.timer_set_cb(
                timer,
                Some(Rc::new(move |ui, timer| {
                    let error = validate_configuration(ui, display, &presenter.borrow()).err();
                    if let Some(error) = error {
                        presenter.borrow_mut().error.get_or_insert(error);
                        return;
                    }
                    ui.display_refr_timer(Some(timer));
                })),
            );
        }
        let inputs = input::Inputs::new(&mut ui, display);
        Ok(Self {
            ui,
            display,
            presenter,
            events,
            inputs,
            sdl,
            last_tick: Instant::now(),
            running: true,
        })
    }

    /// The display attached to the SDL window.
    #[must_use]
    pub fn display(&self) -> DisplayId {
        self.display
    }

    /// The active screen, or `None` after the screen/display was deleted.
    #[must_use]
    pub fn screen(&self) -> Option<Obj> {
        self.ui
            .display(self.display)?
            .act_scr
            .and_then(|id| self.ui.resolve(id))
    }

    /// Mouse pointer device, owned by the UI.
    #[must_use]
    pub fn pointer(&self) -> IndevId {
        self.inputs.pointer
    }

    /// Keyboard device, owned by the UI.
    #[must_use]
    pub fn keypad(&self) -> IndevId {
        self.inputs.keypad
    }

    /// Mouse wheel / middle-button encoder, owned by the UI.
    #[must_use]
    pub fn encoder(&self) -> IndevId {
        self.inputs.encoder
    }

    /// Attach the keyboard and wheel encoder to an application's focus group.
    pub fn set_group(&mut self, group: Option<GroupId>) {
        self.ui.indev_set_group(self.inputs.keypad, group);
        self.ui.indev_set_group(self.inputs.encoder, group);
    }

    /// Number of successful SDL presentations, including separate dirty areas.
    #[must_use]
    pub fn presented_frames(&self) -> u64 {
        self.presenter.borrow().frames
    }

    /// Handle window events, advance wall-clock ticks, run timers and refresh.
    ///
    /// Returns `false` after quit, window close or display deletion. SDL
    /// presentation failures and unsupported display changes return `Err`.
    /// There is no sleep inside this method; callers choose their frame pacing.
    pub fn step(&mut self) -> Result<bool, String> {
        if !self.running || self.ui.display(self.display).is_none() {
            self.running = false;
            return Ok(false);
        }
        self.check_configuration()?;
        let elapsed_ms = self.last_tick.elapsed().as_millis() as u64;
        self.last_tick += Duration::from_millis(elapsed_ms);
        self.ui.tick_inc(elapsed_ms as u32);
        let window_id = self.presenter.borrow().canvas.window().id();
        let mut redraw = false;
        while let Some(event) = self.events.poll_event() {
            match event {
                Event::Quit { .. } => self.running = false,
                Event::Window {
                    window_id: id,
                    win_event: WindowEvent::Close,
                    ..
                } if id == window_id => self.running = false,
                Event::Window {
                    window_id: id,
                    win_event: WindowEvent::Exposed | WindowEvent::Restored,
                    ..
                } if id == window_id => redraw = true,
                _ => self.inputs.handle(&mut self.ui, &event, window_id),
            }
            if !self.running || self.ui.display(self.display).is_none() {
                self.running = false;
                return Ok(false);
            }
            self.check_configuration()?;
        }
        if !self.running {
            return Ok(false);
        }
        if redraw && let Some(screen) = self.screen() {
            self.ui.obj_invalidate(screen);
        }
        self.ui.timer_handler();
        if let Some(error) = self.presenter.borrow_mut().error.take() {
            return Err(error);
        }
        if self.ui.display(self.display).is_none() {
            self.running = false;
            return Ok(false);
        }
        self.check_configuration()?;
        self.ui.refr_now(Some(self.display));
        if let Some(error) = self.presenter.borrow_mut().error.take() {
            return Err(error);
        }
        if self.ui.display(self.display).is_none() {
            self.running = false;
        } else {
            self.check_configuration()?;
        }
        Ok(self.running)
    }

    /// Request a normal SDL quit event. Useful for embedding a custom loop.
    pub fn request_quit(&self) -> Result<(), String> {
        self.sdl.event()?.push_event(Event::Quit { timestamp: 0 })
    }

    fn check_configuration(&self) -> Result<(), String> {
        validate_configuration(&self.ui, self.display, &self.presenter.borrow())
    }
}

fn validate_configuration(ui: &Ui, display: DisplayId, presenter: &Presenter) -> Result<(), String> {
    let data = ui.display(display).ok_or("the SDL display was deleted")?;
    if data.act_scr.is_none_or(|id| ui.resolve(id).is_none()) {
        return Err("the SDL display has no active screen".into());
    }
    let buffer = data.buf_1.as_ref().ok_or("the SDL draw buffer was removed")?;
    if data.hor_res != presenter.width as i32
        || data.ver_res != presenter.height as i32
        || data.rotation != Rotation::R0
        || data.offset_x != 0
        || data.offset_y != 0
        || data.color_format != ColorFormat::Xrgb8888
        || data.render_mode != RenderMode::Direct
        || data.buf_act != 1
        || data.buf_2.is_some()
        || data.buf_3.is_some()
        || u32::from(buffer.header.w) != presenter.width
        || u32::from(buffer.header.h) != presenter.height
        || buffer.header.cf != ColorFormat::Xrgb8888
        || usize::from(buffer.header.stride) != presenter.pitch
        || buffer.data().len() < presenter.pitch * presenter.height as usize
    {
        return Err("SDL requires its original full-size, single XRGB8888 direct buffer".into());
    }
    Ok(())
}

fn checked_pitch(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 || width > u32::from(u16::MAX) || height > u32::from(u16::MAX) {
        return Err("SDL display dimensions must fit nonzero u16 image header fields".into());
    }
    let pitch = DrawBuf::width_to_stride(width, ColorFormat::Xrgb8888) as usize;
    if pitch > usize::from(u16::MAX)
        || pitch
            .checked_mul(height as usize)
            .and_then(|size| size.checked_add(DRAW_BUF_ALIGN - 1))
            .is_none_or(|size| size > isize::MAX as usize)
    {
        return Err("SDL display stride or buffer size is too large".into());
    }
    Ok(pitch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::{Color, StyleSelector};

    #[test]
    fn dimensions_are_validated_before_sdl_initialization() {
        for (w, h) in [(0, 1), (1, 0), (16384, 1), (65536, 1), (1, 65536)] {
            assert!(SdlDisplay::new("invalid", w, h, UiConfig::default(), None).is_err());
        }
    }

    #[test]
    fn dummy_window_displays_rgb_bytes_and_handles_quit() {
        // SDL's process-global video driver must be selected before init. A
        // child process avoids unsafe set_var and test-thread interference.
        if std::env::var_os("FERROVGL_SDL_SMOKE_CHILD").is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .env("SDL_VIDEODRIVER", "dummy")
                .env("FERROVGL_SDL_SMOKE_CHILD", "1")
                .args([
                    "--exact",
                    "tests::dummy_window_displays_rgb_bytes_and_handles_quit",
                    "--nocapture",
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let mut window = SdlDisplay::new("SDL smoke", 16, 12, UiConfig::default(), None).unwrap();
        let screen = window.screen().unwrap();
        window
            .ui
            .obj_set_style_bg_color(screen, Color::hex(0x123456), StyleSelector::DEFAULT);
        window
            .ui
            .obj_set_style_bg_opa(screen, 255, StyleSelector::DEFAULT);
        assert!(window.step().unwrap());
        assert!(window.presented_frames() >= 1);
        let rgb = window
            .presenter
            .borrow()
            .canvas
            .read_pixels(None, PixelFormatEnum::RGB24)
            .unwrap();
        assert!(
            rgb.as_chunks::<3>()
                .0
                .iter()
                .all(|pixel| pixel == &[0x12, 0x34, 0x56])
        );

        let before = window.presented_frames();
        let window_id = window.presenter.borrow().canvas.window().id();
        window
            .sdl
            .event()
            .unwrap()
            .push_event(Event::Window {
                timestamp: 0,
                window_id,
                win_event: WindowEvent::Exposed,
            })
            .unwrap();
        assert!(window.step().unwrap());
        assert!(window.presented_frames() > before);
        window.request_quit().unwrap();
        assert!(!window.step().unwrap());
        window.ui.deinit();
        assert!(window.ui.objs.is_empty());
        assert!(!window.step().unwrap());
        drop(window);

        for deinit in [false, true] {
            let mut window = SdlDisplay::new("timer teardown", 8, 8, UiConfig::default(), None).unwrap();
            let display = window.display();
            let timer = window.ui.timer_create(
                Rc::new(move |ui, _| {
                    if deinit {
                        ui.deinit();
                    } else {
                        ui.display_delete(display);
                    }
                }),
                0,
                None,
            );
            window.ui.timer_set_repeat_count(timer, 1);
            assert!(!window.step().unwrap());
        }

        let mut window = SdlDisplay::new("timer configuration", 8, 8, UiConfig::default(), None).unwrap();
        let display = window.display();
        let timer = window.ui.timer_create(
            Rc::new(move |ui, _| {
                ui.display_mut(display)
                    .unwrap()
                    .buf_1
                    .as_mut()
                    .unwrap()
                    .header
                    .stride = 1;
            }),
            0,
            None,
        );
        window.ui.timer_set_repeat_count(timer, 1);
        assert!(window.step().unwrap_err().contains("XRGB8888 direct buffer"));
        assert_eq!(window.presented_frames(), 0);
        window.ui.deinit();
    }
}

#[cfg(test)]
mod input_tests;

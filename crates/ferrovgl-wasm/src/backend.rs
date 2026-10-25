//! Platform-independent part of the browser backend: the [`Ui`], its
//! display and flush driver, the input devices and the frame step.
//!
//! [`Backend`] does not touch any browser API. Pixels leave through a
//! [`Surface`] (a Canvas2D context in [`crate::web`], a recorder in tests),
//! input arrives as [`InputEvent`]s and time as `performance.now()`
//! milliseconds, so the whole flow runs under `cargo test` on the host.

use std::cell::RefCell;
use std::rc::Rc;

use ferrovgl_core::display::{DisplayThemeInit, FlushResult, Rotation};
use ferrovgl_core::draw::draw_buf::{DRAW_BUF_ALIGN, DrawBuf, STRIDE_AUTO};
use ferrovgl_core::group::GroupId;
use ferrovgl_core::indev::IndevId;
use ferrovgl_core::{Area, ColorFormat, DisplayDriver, DisplayId, FontRef, Obj, RenderMode, Ui, UiConfig};

use crate::clock::FrameClock;
use crate::convert::{self, SUPPORTED_FORMATS};
use crate::input::{InputEvent, Inputs};

/// Receives converted pixels: a block of non-premultiplied RGBA8, `w * h * 4`
/// bytes without row padding, for display position `(x, y)`.
pub trait Surface {
    /// Draws the block, e.g. with `putImageData(imageData, x, y)`.
    fn put_rgba(&mut self, x: i32, y: i32, w: u32, h: u32, rgba: &[u8]) -> Result<(), String>;
}

/// How flushed buffers are laid out, sampled from the display before each
/// refresh so changes made by the application are followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlushConfig {
    /// `lv_display_get_render_mode()`
    pub render_mode: RenderMode,
    /// `lv_display_get_color_format()`
    pub color_format: ColorFormat,
    /// `lv_display_get_horizontal_resolution()`
    pub hor_res: i32,
    /// `lv_display_get_vertical_resolution()`
    pub ver_res: i32,
    /// Row stride of a screen-sized buffer (direct and full mode).
    pub screen_stride: usize,
}

impl FlushConfig {
    /// Reads the flush layout of `display`.
    ///
    /// Rotation, display offsets and color formats outside
    /// [`SUPPORTED_FORMATS`] are rejected: the canvas is the unrotated
    /// physical screen.
    pub fn sample(ui: &Ui, display: DisplayId) -> Result<Self, String> {
        let d = ui.display(display).ok_or("the canvas display was deleted")?;
        if d.rotation != Rotation::R0 || d.offset_x != 0 || d.offset_y != 0 {
            return Err("the canvas backend does not support display rotation or offsets".into());
        }
        let cf = d.color_format;
        if !SUPPORTED_FORMATS.contains(&cf) {
            return Err(format!("the canvas backend cannot show {cf:?} pixels"));
        }
        // `refr_area()`: direct/full mode reshape with the buffer's own
        // stride unless the stride is automatic.
        let auto = DrawBuf::width_to_stride(d.hor_res.max(0) as u32, cf) as usize;
        let screen_stride = if d.stride_is_auto {
            auto
        } else {
            [&d.buf_1, &d.buf_2]
                .into_iter()
                .flatten()
                .next()
                .map_or(auto, |b| usize::from(b.header.stride))
        };
        Ok(Self {
            render_mode: d.render_mode,
            color_format: cf,
            hor_res: d.hor_res,
            ver_res: d.ver_res,
            screen_stride,
        })
    }
}

struct Target {
    config: FlushConfig,
    surface: Box<dyn Surface>,
    scratch: Vec<u8>,
    flushes: u64,
    error: Option<String>,
}

impl Target {
    fn flush(&mut self, area: &Area, px_map: &[u8]) -> Result<(), String> {
        let c = self.config;
        convert::check_area(area, c.hor_res, c.ver_res).map_err(|e| e.to_string())?;
        let layout = convert::source_layout(c.render_mode, c.color_format, area, c.screen_stride);
        let (w, h) = (area.width() as usize, area.height() as usize);
        convert::area_to_rgba(c.color_format, px_map, layout, w, h, &mut self.scratch)
            .map_err(|e| e.to_string())?;
        self.surface
            .put_rgba(area.x1, area.y1, w as u32, h as u32, &self.scratch)?;
        self.flushes += 1;
        Ok(())
    }
}

/// The `flush_cb`: converts `area` of `px_map` and hands it to the surface.
struct CanvasDriver(Rc<RefCell<Target>>);

impl DisplayDriver for CanvasDriver {
    fn flush(&mut self, _disp: DisplayId, area: &Area, px_map: &mut [u8]) -> FlushResult {
        let mut target = self.0.borrow_mut();
        if target.error.is_none()
            && let Err(e) = target.flush(area, px_map)
        {
            target.error = Some(e);
        }
        // Drawing is synchronous: `lv_display_flush_ready()` right away.
        FlushResult::Done
    }
}

/// Parameters of [`Backend::new`].
#[derive(Clone)]
pub struct BackendConfig {
    /// Horizontal resolution in pixels.
    pub width: u32,
    /// Vertical resolution in pixels.
    pub height: u32,
    /// Display color format; one of [`SUPPORTED_FORMATS`].
    pub color_format: ColorFormat,
    /// `lv_display_set_render_mode()`
    pub render_mode: RenderMode,
    /// Rows of the partial-mode buffer; 0 picks a tenth of the screen
    /// (the size LVGL's porting guide suggests). Ignored in direct/full mode.
    pub partial_rows: u32,
    /// Use two draw buffers (`lv_display_set_draw_buffers(disp, buf1, buf2)`).
    pub double_buffered: bool,
    /// `lv_display_set_dpi()`; `None` keeps `config.dpi_def`.
    pub dpi: Option<i32>,
    /// Context configuration (`lv_conf.h` values).
    pub ui: UiConfig,
    /// Theme hook of `lv_display_create()`.
    pub theme_init: Option<DisplayThemeInit>,
}

impl Default for BackendConfig {
    /// 800x480 XRGB8888, partial mode, Montserrat 14 and the default theme.
    fn default() -> Self {
        Self {
            width: 800,
            height: 480,
            color_format: ColorFormat::Xrgb8888,
            render_mode: RenderMode::Partial,
            partial_rows: 0,
            double_buffered: false,
            dpi: None,
            ui: UiConfig {
                font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
                ..UiConfig::default()
            },
            theme_init: Some(ferrovgl_widgets::theme::default::init_display),
        }
    }
}

fn checked_dimensions(cfg: &BackendConfig) -> Result<usize, String> {
    let (w, h) = (cfg.width, cfg.height);
    if w == 0 || h == 0 || w > u32::from(u16::MAX) || h > u32::from(u16::MAX) {
        return Err("display dimensions must be between 1 and 65535".into());
    }
    if !SUPPORTED_FORMATS.contains(&cfg.color_format) {
        return Err(format!("unsupported color format {:?}", cfg.color_format));
    }
    let stride = DrawBuf::width_to_stride(w, cfg.color_format) as usize;
    if stride > usize::from(u16::MAX)
        || stride
            .checked_mul(h as usize)
            .and_then(|s| s.checked_add(DRAW_BUF_ALIGN - 1))
            .is_none_or(|s| s > isize::MAX as usize)
    {
        return Err("display stride or buffer size is too large".into());
    }
    Ok(stride)
}

/// `MODE_DARK` of `lv_theme_default.c` (`theme->base.flags`).
const THEME_MODE_DARK: u32 = 1;

/// After `lv_display_set_dpi()`: `lv_display_set_dpi()` only stores the value
/// and `lv_theme_default_init()` reads it once, so a C application changing
/// the DPI calls the init again. If `display` uses the default theme, re-run
/// it with the theme's own colors, mode and font, as
/// `resolution_change_event_cb` does (a no-op when nothing changed).
fn reinit_default_theme(ui: &mut Ui, display: DisplayId) {
    use ferrovgl_widgets::theme::default;
    let Some(theme) = default::get(ui) else {
        return;
    };
    if ui.display_get_theme(Some(display)) != Some(theme) {
        return;
    }
    let Some(base) = ui.theme(theme).cloned() else {
        return;
    };
    if let Some(font) = base.font_normal {
        default::init(
            ui,
            display,
            base.color_primary,
            base.color_secondary,
            base.flags & THEME_MODE_DARK != 0,
            font,
        );
    }
}

/// A UI context bound to one canvas-like [`Surface`].
pub struct Backend {
    /// Build and update the application's objects here.
    pub ui: Ui,
    display: DisplayId,
    group: GroupId,
    target: Rc<RefCell<Target>>,
    inputs: Inputs,
    clock: FrameClock,
}

impl Backend {
    /// Creates the context, software draw unit, display, draw buffers and
    /// input devices.
    ///
    /// A default focus group is created and set with `lv_group_set_default()`
    /// so focusable widgets join it, and the keypad and encoder use it.
    pub fn new(config: BackendConfig, surface: Box<dyn Surface>) -> Result<Self, String> {
        checked_dimensions(&config)?;
        let (w, h, cf) = (config.width, config.height, config.color_format);
        let rows = match config.render_mode {
            RenderMode::Partial if config.partial_rows == 0 => (h / 10).max(1),
            RenderMode::Partial => config.partial_rows.min(h),
            RenderMode::Direct | RenderMode::Full => h,
        };
        let create = || {
            DrawBuf::create(w, rows, cf, STRIDE_AUTO)
                .ok_or_else(|| "could not allocate a draw buffer".to_owned())
        };
        let buf1 = create()?;
        let buf2 = if config.double_buffered {
            Some(create()?)
        } else {
            None
        };

        let mut ui = Ui::new(config.ui);
        ferrovgl_draw_sw::init(&mut ui);
        let display = ui
            .display_create_ex(w as i32, h as i32, config.theme_init)
            .ok_or("could not create the display")?;
        ui.display_set_color_format(display, cf);
        if let Some(dpi) = config.dpi {
            ui.display_set_dpi(display, dpi);
            reinit_default_theme(&mut ui, display);
        }
        ui.display_set_draw_buffers(display, Some(buf1), buf2);
        ui.display_set_render_mode(display, config.render_mode);
        let target = Rc::new(RefCell::new(Target {
            config: FlushConfig::sample(&ui, display)?,
            surface,
            scratch: Vec::new(),
            flushes: 0,
            error: None,
        }));
        ui.display_set_driver(display, Some(Box::new(CanvasDriver(target.clone()))));
        if let Some(timer) = ui.display_get_refr_timer(Some(display)) {
            let target = target.clone();
            // Re-sample the layout right before each refresh: another timer
            // of the same handler pass may have changed the display.
            ui.timer_set_cb(
                timer,
                Some(Rc::new(move |ui, timer| {
                    match FlushConfig::sample(ui, display) {
                        Ok(c) => target.borrow_mut().config = c,
                        Err(e) => {
                            target.borrow_mut().error.get_or_insert(e);
                            return;
                        }
                    }
                    ui.display_refr_timer(Some(timer));
                })),
            );
        }
        let inputs = Inputs::new(&mut ui, display);
        let group = ui.group_create();
        ui.group_set_default(Some(group));
        ui.indev_set_group(inputs.keypad, Some(group));
        ui.indev_set_group(inputs.encoder, Some(group));
        Ok(Self {
            ui,
            display,
            group,
            target,
            inputs,
            clock: FrameClock::new(),
        })
    }

    /// The display drawn to the surface.
    pub fn display(&self) -> DisplayId {
        self.display
    }

    /// The default focus group used by the keypad and encoder.
    pub fn group(&self) -> GroupId {
        self.group
    }

    /// The active screen, or `None` after the display was deleted.
    pub fn screen(&self) -> Option<Obj> {
        self.ui.display_get_screen_active(Some(self.display))
    }

    /// Pointer device, owned by the UI.
    pub fn pointer(&self) -> IndevId {
        self.inputs.pointer
    }

    /// Keyboard device, owned by the UI.
    pub fn keypad(&self) -> IndevId {
        self.inputs.keypad
    }

    /// Wheel / middle-button encoder, owned by the UI.
    pub fn encoder(&self) -> IndevId {
        self.inputs.encoder
    }

    /// Number of areas delivered to the surface so far.
    pub fn flushed_areas(&self) -> u64 {
        self.target.borrow().flushes
    }

    /// The flush layout used by the last refresh.
    pub fn flush_config(&self) -> FlushConfig {
        self.target.borrow().config
    }

    /// Whether the display still exists.
    pub fn is_alive(&self) -> bool {
        self.ui.display(self.display).is_some()
    }

    /// Applies one input event right away (`lv_indev_read()` in event mode).
    pub fn handle_input(&mut self, event: InputEvent) {
        if self.is_alive() {
            self.inputs.handle(&mut self.ui, event);
        }
    }

    /// One animation frame at `now_ms` (`performance.now()`):
    /// `lv_tick_inc()` with the elapsed whole milliseconds, the queued input
    /// events in order, then `lv_timer_handler()` (which refreshes the
    /// display when its refresh period has passed).
    ///
    /// Returns `Ok(false)` once the display was deleted and `Err` when a
    /// flush failed or the display was reconfigured to something the canvas
    /// cannot show.
    pub fn frame(
        &mut self,
        now_ms: f64,
        events: impl IntoIterator<Item = InputEvent>,
    ) -> Result<bool, String> {
        if !self.is_alive() {
            return Ok(false);
        }
        let elapsed = self.clock.advance(now_ms);
        self.ui.tick_inc(elapsed);
        for event in events {
            self.handle_input(event);
            if !self.is_alive() {
                return Ok(false);
            }
        }
        self.ui.timer_handler();
        self.take_error()?;
        Ok(self.is_alive())
    }

    /// `lv_refr_now(disp)`: refresh pending areas immediately.
    pub fn refresh_now(&mut self) -> Result<(), String> {
        if !self.is_alive() {
            return Ok(());
        }
        let config = FlushConfig::sample(&self.ui, self.display)?;
        self.target.borrow_mut().config = config;
        self.ui.refr_now(Some(self.display));
        self.take_error()
    }

    /// Invalidates the active screen so the next refresh redraws everything
    /// (e.g. after the canvas was cleared by a resize).
    pub fn invalidate(&mut self) {
        if let Some(screen) = self.screen() {
            let _ = self.ui.obj_invalidate(screen);
        }
    }

    /// `lv_display_set_dpi()`, then `lv_theme_default_init()` again when the
    /// display uses the default theme (with its own colors, mode and font), which
    /// rebuilds its styles in place and refreshes every object. Sizes the
    /// application computed with `lv_dpx()` keep their values.
    pub fn set_dpi(&mut self, dpi: i32) {
        if self.is_alive() {
            self.ui.display_set_dpi(self.display, dpi);
            reinit_default_theme(&mut self.ui, self.display);
        }
    }

    fn take_error(&mut self) -> Result<(), String> {
        match self.target.borrow_mut().error.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests;

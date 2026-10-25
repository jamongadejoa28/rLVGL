//! Test harness ported from LVGL's `src/debugging/test` and
//! `tests/src/lv_test_init.c`: an 800x480 XRGB8888 headless display in direct
//! render mode, upstream's mouse, keypad and encoder test devices, the
//! `lv_test_wait()` family and byte-exact screenshot comparison against
//! upstream reference images (`tests/upstream/lvgl/tests/ref_imgs`).
//!
//! `TestDisplay::new` leaves styles to the scene; `TestDisplay::with_default_theme`
//! installs the light theme, as `lv_display_create()` does with
//! `LV_USE_THEME_DEFAULT`, for upstream parity. Every constructor creates the
//! three test input devices right after the display, like `lv_test_init()`.
//!
//! # Differences from `lv_test_init()`
//!
//! The reference images come from the `full` + `depth_32` + `run_tests`
//! configuration, where `lv_test_init()` also does three things this harness
//! leaves out:
//!
//! - `lv_test_fs_init()`: there is no test file system.
//! - `lv_test_indev_gesture_create()` (`LV_USE_GESTURE_RECOGNITION=y`): a fourth
//!   `LV_INDEV_TYPE_POINTER` device, created after the encoder with its own read
//!   timer, that reports a released pointer at (0, 0). ferrovgl-core has no
//!   multitouch gesture recognition (see `ferrovgl_core::indev`), so upstream
//!   sees four devices from `lv_indev_get_next()` where this harness has three,
//!   and upstream's extra pointer sends its own `HOVER_OVER`/`HOVER_LEAVE`
//!   events to whatever object is at (0, 0). A port that depends on device
//!   count, device or timer order, or hover events must create a fourth
//!   released pointer at (0, 0) right after the encoder.
//! - `lv_sysmon_hide_performance()` (`LV_USE_SYSMON`, `LV_USE_PERF_MONITOR`):
//!   there is no system monitor, so there is no performance label to hide.
//!
//! None of these changes the pixels of the matched reference images.
//!
//! # One display per `#[test]`
//!
//! Upstream runs `lv_test_init()` once per test file (`tests/config.yml`
//! `:suite_setup:`); the tests of a file share the tick, the input devices and
//! their internal state (`last_short_click_timestamp`/`point`, `last_hovered`,
//! `prev_state`, the test devices' pointer position, pressed state, key and
//! encoder diff) and the screen, and `tearDown()` only calls
//! `lv_obj_clean(lv_screen_active())`. A fresh [`TestDisplay`] corresponds to
//! the *first* test of an upstream suite. State carried from earlier tests is
//! not modelled: for example two consecutive upstream tests that each end and
//! start with `lv_test_mouse_click_at()` on the same point are less than
//! `LV_INDEV_DEF_DOUBLE_CLICK_TIME` apart, so the second one gets
//! `LV_EVENT_DOUBLE_CLICKED`. A port that depends on such state must run the
//! earlier tests' input sequence on the same [`TestDisplay`], calling
//! [`TestDisplay::clean_screen`] between them as upstream's `tearDown()` does.

#![warn(missing_docs)]

mod input;
mod screenshot;

pub use input::{SyntheticInput, TestInputs};

use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ferrovgl_core::display::DisplayThemeInit;
use ferrovgl_core::display::FlushResult;
use ferrovgl_core::draw::draw_buf::{
    BufStorage, DRAW_BUF_ALIGN, DRAW_BUF_STRIDE_ALIGN, DrawBuf, STRIDE_AUTO,
};
use ferrovgl_core::indev::{IndevId, IndevType};
use ferrovgl_core::misc::event::EventTarget;
use ferrovgl_core::{
    Area, ColorFormat, DisplayDriver, DisplayId, EventCode, FontRef, Obj, RenderMode, Ui, UiConfig,
};

pub use screenshot::{RgbImage, compare_screenshot};

/// Horizontal resolution of the upstream test display (`LV_TEST_DISPLAY_HOR_RES`).
pub const DISPLAY_HOR_RES: i32 = 800;
/// Vertical resolution of the upstream test display (`LV_TEST_DISPLAY_VER_RES`).
pub const DISPLAY_VER_RES: i32 = 480;
/// DPI used to render upstream reference PNGs. `tests/configs/run_tests.defconfig`
/// overrides the compile-only `depth_32.defconfig` value of 160 with 130.
pub const DISPLAY_DPI: i32 = 130;
/// Path of the upstream sparse checkout relative to the workspace root.
pub const UPSTREAM_DIR: &str = "tests/upstream/lvgl";

/// Cargo workspace root, independent of the test process's working directory.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("ferrovgl-test is in crates/")
        .to_path_buf()
}

/// `dummy_flush_cb()` of `lv_test_display.c`.
struct HeadlessDriver;

impl DisplayDriver for HeadlessDriver {
    fn flush(&mut self, _disp: DisplayId, _area: &Area, _px_map: &mut [u8]) -> FlushResult {
        // `lv_display_flush_ready(disp)`: the direct-mode buffer is the
        // screenshot; nothing is copied.
        FlushResult::Done
    }
}

/// The draw buffer of `lv_test_display_create()` and `buf_changed_event_cb()`:
/// `4 * (hor_res + LV_DRAW_BUF_STRIDE_ALIGN - 1) * ver_res + LV_DRAW_BUF_ALIGN`
/// bytes, initialised by `lv_draw_buf_init()` with an automatic stride.
///
/// Unlike `malloc` the memory is zeroed. Returns `None` if the size does not
/// fit or the allocation fails.
fn test_draw_buf(hor_res: i32, ver_res: i32, cf: ColorFormat) -> Option<DrawBuf> {
    let w = u32::try_from(hor_res).ok()?;
    let h = u32::try_from(ver_res).ok()?;
    let buf_size = w
        .checked_add(DRAW_BUF_STRIDE_ALIGN - 1)?
        .checked_mul(4)?
        .checked_mul(h)?
        .checked_add(DRAW_BUF_ALIGN as u32)?;
    let words = (buf_size as usize).div_ceil(size_of::<u64>());
    let mut storage: Vec<u64> = Vec::new();
    storage.try_reserve_exact(words).ok()?;
    storage.resize(words, 0);
    /* `lv_draw_buf_align(buf, cf)`: the `u64` storage is already aligned. */
    DrawBuf::init(
        w,
        h,
        cf,
        STRIDE_AUTO,
        BufStorage::Owned(storage.into_boxed_slice()),
        buf_size,
    )
    .ok()
}

/// `buf_changed_event_cb()` of `lv_test_display.c`: reallocate the draw
/// buffer for the display's new color format or original resolution.
fn buf_changed_event_cb(ui: &mut Ui, e: &mut ferrovgl_core::Event<'_>) {
    let EventTarget::Display(disp) = e.target() else {
        return;
    };
    let cf = ui.display_get_color_format(Some(disp));
    let hor_res = ui.display_get_original_horizontal_resolution(Some(disp));
    let ver_res = ui.display_get_original_vertical_resolution(Some(disp));
    /* `free(_state.draw_buf.unaligned_data)`: the old buffer is dropped by
     * `lv_display_set_draw_buffers()`. */
    let buf = test_draw_buf(hor_res, ver_res, cf).expect("LV_ASSERT_MALLOC(buf)");
    ui.display_set_draw_buffers(disp, Some(buf), None);
}

/// A real Ui, software draw unit and single-buffered headless display with
/// upstream's three test input devices.
///
/// Construction follows the parts of `lv_test_init()` that ferrovgl
/// represents: `lv_init()`, then `lv_test_display_create()`, then
/// `lv_test_indev_create_all()`. `lv_test_fs_init()`, the gesture pointer of
/// `lv_test_indev_gesture_create()` and `lv_sysmon_hide_performance()` are
/// left out (see the [crate docs](crate#differences-from-lv_test_init)).
/// Pixel format, size, DPI and font match the upstream 32-bit screenshot
/// harness. The theme is opt-in. Dropping the display is `lv_test_deinit()`.
///
/// Each value starts like the first test of an upstream suite; tick, input
/// device and screen state carried over from earlier tests of the same
/// upstream file is not modelled unless the scenario replays those tests on
/// the same display (see the [crate docs](crate#one-display-per-test)).
pub struct TestDisplay {
    /// The UI under test. Tests build scenes using its public methods.
    pub ui: Ui,
    display: DisplayId,
    inputs: TestInputs,
}

impl TestDisplay {
    /// Create the upstream 800×480, 130 DPI display with Montserrat 14.
    pub fn new() -> io::Result<Self> {
        Self::with_resolution(DISPLAY_HOR_RES, DISPLAY_VER_RES)
    }

    /// Create the upstream screenshot configuration with the light default
    /// theme's styles and Montserrat 14.
    pub fn with_default_theme() -> io::Result<Self> {
        Self::create(
            DISPLAY_HOR_RES,
            DISPLAY_VER_RES,
            Some(ferrovgl_widgets::theme::default::init_display),
        )
    }

    /// Create a smaller display for integration tests, preserving pixel format,
    /// DPI, font, rendering configuration and input devices.
    pub fn with_resolution(width: i32, height: i32) -> io::Result<Self> {
        Self::create(width, height, None)
    }

    fn create(width: i32, height: i32, theme_init: Option<DisplayThemeInit>) -> io::Result<Self> {
        if width <= 0 || height <= 0 || width > i32::from(u16::MAX) || height > i32::from(u16::MAX) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "display dimensions must fit nonzero u16 image header fields",
            ));
        }
        let stride = DrawBuf::width_to_stride(width as u32, ColorFormat::Xrgb8888);
        if stride > u32::from(u16::MAX)
            || (stride as usize)
                .checked_mul(height as usize)
                .and_then(|size| size.checked_add(DRAW_BUF_ALIGN - 1))
                .is_none_or(|size| size > isize::MAX as usize)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "display stride or byte size is too large",
            ));
        }
        // `lv_init()`
        let mut ui = Ui::new(UiConfig {
            dpi_def: DISPLAY_DPI,
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        });
        ferrovgl_draw_sw::init(&mut ui);
        // `lv_test_display_create(hor_res, ver_res)`
        let display = ui
            .display_create_ex(width, height, theme_init)
            .ok_or_else(|| io::Error::other("could not create the test display"))?;
        ui.display_set_color_format(display, ColorFormat::Xrgb8888);
        let buffer = test_draw_buf(width, height, ColorFormat::Xrgb8888)
            .ok_or_else(|| io::Error::other("could not allocate the test display buffer"))?;
        ui.display_set_draw_buffers(display, Some(buffer), None);
        ui.display_set_render_mode(display, RenderMode::Direct);
        ui.display_set_driver(display, Some(Box::new(HeadlessDriver)));
        ui.display_add_event_cb(
            display,
            Rc::new(buf_changed_event_cb),
            EventCode::COLOR_FORMAT_CHANGED,
            None,
        );
        ui.display_add_event_cb(
            display,
            Rc::new(buf_changed_event_cb),
            EventCode::RESOLUTION_CHANGED,
            None,
        );
        /* `delete_event_cb()` frees the buffer; the display owns it here. */
        // `lv_test_indev_create_all()`
        let inputs = TestInputs::create_all(&mut ui);
        Ok(Self { ui, display, inputs })
    }

    /// Handles of upstream's mouse, keypad and encoder test devices, which
    /// every constructor creates (`lv_test_indev_create_all()`). Clones share
    /// the device state; calling this again returns the same devices.
    pub fn create_inputs(&mut self) -> TestInputs {
        self.inputs.clone()
    }

    /// The three test input devices.
    pub fn inputs(&self) -> &TestInputs {
        &self.inputs
    }

    /// `lv_test_indev_get_indev()`
    pub fn indev_get_indev(&self, kind: IndevType) -> Option<IndevId> {
        self.inputs.get_indev(kind)
    }

    /// The headless display handle.
    pub fn display(&self) -> DisplayId {
        self.display
    }

    /// The active screen, if the caller has not deleted it or the display.
    pub fn screen(&self) -> Option<Obj> {
        self.ui.display_get_screen_active(Some(self.display))
    }

    /// The `tearDown()` of most upstream test files:
    /// `lv_obj_clean(lv_screen_active())`.
    ///
    /// Only the screen's children are deleted; the tick and the input
    /// devices' state carry over to the next scenario on this display, as
    /// they do between the tests of one upstream suite.
    pub fn clean_screen(&mut self) {
        if let Some(screen) = self.ui.screen_active() {
            self.ui.obj_clean(screen);
        }
    }

    /// The first half of `lv_test_screenshot_compare()`:
    /// `lv_obj_invalidate(lv_screen_active())` and `lv_refr_now(NULL)`. This
    /// does not advance animation time.
    pub fn refresh(&mut self) {
        if let Some(screen) = self.ui.screen_active() {
            self.ui.obj_invalidate(screen);
        }
        self.ui.refr_now(None);
    }

    /// The trailing `lv_indev_read()` calls of `lv_test_wait()` and
    /// `lv_test_fast_forward()`: pointer, encoder, then keypad.
    fn read_inputs(&mut self) {
        self.ui.indev_read(self.inputs.mouse.id());
        self.ui.indev_read(self.inputs.encoder.id());
        self.ui.indev_read(self.inputs.keypad.id());
    }

    /// `lv_test_wait()`: advance the clock and run the timers once per
    /// millisecond, read the pointer, encoder and keypad once more and
    /// refresh every display, even when `ms` is zero.
    pub fn wait(&mut self, ms: u32) {
        let mut ms = ms;
        while ms != 0 {
            self.ui.tick_inc(1);
            self.ui.timer_handler();
            ms -= 1;
        }
        self.read_inputs();
        self.ui.refr_now(None);
    }

    /// `lv_test_fast_forward()`: advance the clock by `ms` at once, run the
    /// timers a single time, read the pointer, encoder and keypad and refresh
    /// every display.
    pub fn fast_forward(&mut self, ms: u32) {
        self.ui.tick_inc(ms);
        self.ui.timer_handler();
        self.read_inputs();
        self.ui.refr_now(None);
    }

    /// `lv_test_mouse_move_to()`
    pub fn mouse_move_to(&mut self, x: i32, y: i32) {
        self.inputs.mouse.move_to(x, y);
    }

    /// `lv_test_mouse_move_to_obj()`: the object's current center, without a
    /// layout update.
    pub fn mouse_move_to_obj(&mut self, obj: Obj) {
        self.inputs.mouse.move_to_obj(&self.ui, obj);
    }

    /// `lv_test_mouse_move_by()`
    pub fn mouse_move_by(&mut self, x: i32, y: i32) {
        self.inputs.mouse.move_by(x, y);
    }

    /// `lv_test_mouse_press()`
    pub fn mouse_press(&mut self) {
        self.inputs.mouse.press();
    }

    /// `lv_test_mouse_release()`
    pub fn mouse_release(&mut self) {
        self.inputs.mouse.release();
    }

    /// `lv_test_mouse_click_at()`
    pub fn mouse_click_at(&mut self, x: i32, y: i32) {
        let inputs = self.inputs.clone();
        inputs.mouse_click_at(self, x, y);
    }

    /// `lv_test_key_press()`
    pub fn key_press(&mut self, key: u32) {
        self.inputs.keypad.key_press(key);
    }

    /// `lv_test_key_release()`
    pub fn key_release(&mut self) {
        self.inputs.keypad.release();
    }

    /// `lv_test_key_hit()`
    pub fn key_hit(&mut self, key: u32) {
        let inputs = self.inputs.clone();
        inputs.key_hit(self, key);
    }

    /// `lv_test_encoder_add_diff()`
    pub fn encoder_add_diff(&mut self, diff: i32) {
        self.inputs.encoder.add_diff(diff);
    }

    /// `lv_test_encoder_turn()`
    pub fn encoder_turn(&mut self, diff: i32) {
        let inputs = self.inputs.clone();
        inputs.encoder_turn(self, diff);
    }

    /// `lv_test_encoder_press()`
    pub fn encoder_press(&mut self) {
        self.inputs.encoder.press();
    }

    /// `lv_test_encoder_release()`
    pub fn encoder_release(&mut self) {
        self.inputs.encoder.release();
    }

    /// `lv_test_encoder_click()`
    pub fn encoder_click(&mut self) {
        let inputs = self.inputs.clone();
        inputs.encoder_click(self);
    }

    /// Refresh like `lv_test_screenshot_compare()` and convert the default
    /// display's active draw buffer like `buf_to_xrgb8888()`, keeping RGB.
    ///
    /// As upstream, an RGB565_SWAPPED buffer is byte-swapped in place
    /// (`lv_draw_rgb565_swap()`) before it is read. Returns an error when
    /// there is no default display, active screen or active draw buffer, or
    /// the buffer's format is not one `buf_to_xrgb8888()` converts.
    pub fn snapshot(&mut self) -> io::Result<RgbImage> {
        self.default_display()?;
        self.refresh();
        // A callback may delete the display or change its configuration.
        let disp = self.default_display()?;
        let display = self.ui.display_mut(disp).expect("default display");
        let buffer = match display.buf_act {
            1 => display.buf_1.as_mut(),
            2 => display.buf_2.as_mut(),
            3 => display.buf_3.as_mut(),
            _ => None,
        }
        .ok_or_else(|| io::Error::other("the test display has no active draw buffer"))?;
        if buffer.header.cf == ColorFormat::Rgb565Swapped {
            let px = u32::from(buffer.header.w) * u32::from(buffer.header.h);
            if let Some(data) = buffer.data_mut() {
                ferrovgl_core::draw::utils::rgb565_swap(data, px);
            }
            return RgbImage::from_buffer(
                buffer.header.w.into(),
                buffer.header.h.into(),
                buffer.header.stride.into(),
                ColorFormat::Rgb565,
                buffer.data(),
            );
        }
        RgbImage::from_draw_buf(buffer)
    }

    /// `lv_display_get_default()` with an active screen.
    fn default_display(&self) -> io::Result<DisplayId> {
        let disp = self
            .ui
            .display_get_default()
            .ok_or_else(|| io::Error::other("the test display was deleted"))?;
        if self
            .ui
            .screen_active()
            .is_none_or(|obj| !self.ui.obj_is_valid(obj))
        {
            return Err(io::Error::other("the test display has no active screen"));
        }
        Ok(disp)
    }

    /// `lv_test_screenshot_compare()` against an upstream relative PNG path.
    /// Mismatches write actual, expected and diff PNGs under `target/parity`
    /// instead of upstream's `<name>_err.png` next to the reference.
    /// A missing reference is an error, never a skipped or passing comparison.
    pub fn screenshot_compare(&mut self, name: impl AsRef<Path>) -> io::Result<()> {
        let root = workspace_root();
        self.screenshot_compare_in(
            root.join(UPSTREAM_DIR).join("tests/ref_imgs"),
            root.join("target/parity"),
            name,
        )
    }

    /// Compare using explicit reference and artifact directories, for custom
    /// fixtures or isolated tests. The result is marked incomplete before any
    /// rendering, so a rendering error cannot retain an earlier passing result.
    pub fn screenshot_compare_in(
        &mut self,
        reference_dir: impl AsRef<Path>,
        artifact_dir: impl AsRef<Path>,
        name: impl AsRef<Path>,
    ) -> io::Result<()> {
        let name = name.as_ref();
        let base = screenshot::begin_comparison(artifact_dir.as_ref(), name)?;
        let actual = match self.snapshot() {
            Ok(image) => image,
            Err(error) => {
                std::fs::write(base.with_extension("result"), format!("error\n{error}\n"))?;
                return Err(error);
            }
        };
        compare_screenshot(&actual, reference_dir, artifact_dir, name)
    }
}

#[cfg(test)]
mod tests {
    use ferrovgl_core::{State, StyleSelector};

    use super::*;

    fn clickable(display: &mut TestDisplay) -> Obj {
        let obj = display.ui.obj_create(display.screen());
        display.ui.obj_set_pos(obj, 10, 10);
        display.ui.obj_set_size(obj, 20, 20);
        display.ui.obj_set_style_bg_opa(obj, 255, StyleSelector::DEFAULT);
        display.ui.obj_update_layout(obj);
        obj
    }

    #[test]
    fn test_init_creates_the_three_devices_once() {
        let mut display = TestDisplay::with_resolution(32, 32).unwrap();
        assert_eq!(display.ui.indevs.len(), 3);
        let inputs = display.create_inputs();
        assert_eq!(display.ui.indevs.len(), 3);
        assert_eq!(inputs.mouse.id(), display.inputs().mouse.id());
        assert_eq!(display.ui.indev_get_type(inputs.mouse.id()), IndevType::Pointer);
        assert_eq!(display.ui.indev_get_type(inputs.keypad.id()), IndevType::Keypad);
        assert_eq!(display.ui.indev_get_type(inputs.encoder.id()), IndevType::Encoder);
        assert_eq!(display.indev_get_indev(IndevType::Button), None);
        // `lv_indev_create()` keeps them on the default display.
        assert_eq!(
            display.ui.indev_get_display(inputs.keypad.id()),
            Some(display.display())
        );
    }

    #[test]
    fn wait_zero_reads_the_devices_after_the_timers() {
        let mut display = TestDisplay::with_resolution(64, 64).unwrap();
        let obj = clickable(&mut display);
        display.mouse_move_to(15, 15);
        display.mouse_press();
        // No timer runs, but `lv_test_wait()` still reads the pointer.
        display.wait(0);
        assert!(display.ui.obj_has_state(obj, State::PRESSED));
        display.mouse_release();
        display.fast_forward(0);
        assert!(!display.ui.obj_has_state(obj, State::PRESSED));
    }

    #[test]
    fn fast_forward_runs_the_timers_once() {
        let mut display = TestDisplay::with_resolution(16, 16).unwrap();
        let runs = Rc::new(std::cell::Cell::new(0));
        let counter = runs.clone();
        display
            .ui
            .timer_create(Rc::new(move |_, _| counter.set(counter.get() + 1)), 1, None);
        display.fast_forward(100);
        assert_eq!((display.ui.tick_get(), runs.get()), (100, 1));
        display.wait(5);
        assert_eq!((display.ui.tick_get(), runs.get()), (105, 6));
    }

    #[test]
    fn move_to_obj_uses_the_coordinates_without_a_layout_update() {
        let mut display = TestDisplay::with_resolution(64, 64).unwrap();
        let obj = clickable(&mut display);
        display.ui.obj_set_pos(obj, 30, 30);
        display.mouse_move_to_obj(obj);
        display.wait(0);
        let mouse = display.inputs().mouse.id();
        // The stale (10, 10) 20x20 rectangle, not the pending (30, 30) one.
        assert_eq!(
            display.ui.indev_get_point(mouse),
            ferrovgl_core::Point::new(20, 20)
        );
        display.ui.obj_delete(obj);
        display.mouse_move_to_obj(obj);
        display.wait(0);
        assert_eq!(
            display.ui.indev_get_point(mouse),
            ferrovgl_core::Point::new(20, 20)
        );
    }

    #[test]
    fn clean_screen_deletes_the_children_of_the_active_screen() {
        let mut display = TestDisplay::with_resolution(16, 16).unwrap();
        let obj = clickable(&mut display);
        display.clean_screen();
        assert!(!display.ui.obj_is_valid(obj));
        assert!(display.screen().is_some_and(|s| display.ui.obj_is_valid(s)));
    }

    #[test]
    fn suite_state_carries_over_clean_screen() {
        // Two consecutive upstream tests of one suite, each clicking (15, 15):
        // the second click lands within `LV_INDEV_DEF_DOUBLE_CLICK_TIME`.
        let mut display = TestDisplay::with_resolution(64, 64).unwrap();
        let mouse = display.inputs().mouse.id();
        clickable(&mut display);
        display.mouse_click_at(15, 15);
        assert_eq!(display.ui.indev_get_short_click_streak(mouse), 1);
        display.clean_screen();
        clickable(&mut display);
        display.mouse_click_at(15, 15);
        assert_eq!(display.ui.indev_get_short_click_streak(mouse), 2);
        // A fresh display starts like the first test of a suite.
        let mut fresh = TestDisplay::with_resolution(64, 64).unwrap();
        let mouse = fresh.inputs().mouse.id();
        clickable(&mut fresh);
        fresh.mouse_click_at(15, 15);
        assert_eq!(fresh.ui.indev_get_short_click_streak(mouse), 1);
    }
}

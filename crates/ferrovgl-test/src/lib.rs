//! Test harness mirroring LVGL's `src/debugging/test`: an 800x480 XRGB8888
//! headless display in direct render mode and byte-exact screenshot comparison against upstream reference
//! images (`tests/upstream/lvgl/tests/ref_imgs`).
//!
//! `TestDisplay::new` leaves styles to the scene; `TestDisplay::with_default_theme`
//! installs the light theme for upstream parity. `TestDisplay::create_inputs`
//! installs deterministic pointer, keypad and encoder devices.

#![warn(missing_docs)]

mod input;
mod screenshot;

pub use input::{SyntheticInput, TestInputs};

use std::io;
use std::path::{Path, PathBuf};

use ferrovgl_core::display::DisplayThemeInit;
use ferrovgl_core::display::FlushResult;
use ferrovgl_core::draw::draw_buf::{DRAW_BUF_ALIGN, DrawBuf, STRIDE_AUTO};
use ferrovgl_core::{Area, ColorFormat, DisplayDriver, DisplayId, FontRef, Obj, RenderMode, Ui, UiConfig};

pub use screenshot::{RgbImage, compare_screenshot};

/// Horizontal resolution of the upstream test display.
pub const DISPLAY_HOR_RES: i32 = 800;
/// Vertical resolution of the upstream test display.
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

struct HeadlessDriver;

impl DisplayDriver for HeadlessDriver {
    fn flush(&mut self, _disp: DisplayId, _area: &Area, _px_map: &mut [u8]) -> FlushResult {
        // The direct-mode buffer is the screenshot; nothing is copied.
        FlushResult::Done
    }
}

/// A real Ui, software draw unit and single-buffered headless display.
///
/// Construction, rendering and deletion use the public object/display paths.
/// Pixel format, size, DPI and font match the upstream 32-bit screenshot
/// harness. The theme is opt-in; synthetic input devices are not installed.
pub struct TestDisplay {
    /// The UI under test. Tests build scenes using its public methods.
    pub ui: Ui,
    display: DisplayId,
}

impl TestDisplay {
    /// Create the upstream 800×480, 130 DPI display with Montserrat 14.
    pub fn new() -> io::Result<Self> {
        Self::with_resolution(DISPLAY_HOR_RES, DISPLAY_VER_RES)
    }

    /// Create the upstream screenshot configuration with the light default
    /// theme's screen/base-object/button styles and Montserrat 14.
    pub fn with_default_theme() -> io::Result<Self> {
        Self::create(
            DISPLAY_HOR_RES,
            DISPLAY_VER_RES,
            Some(ferrovgl_widgets::theme::default::init_display),
        )
    }

    /// Create a smaller display for integration tests, preserving pixel format,
    /// DPI, font and rendering configuration.
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
        let buffer = DrawBuf::create(width as u32, height as u32, ColorFormat::Xrgb8888, STRIDE_AUTO)
            .ok_or_else(|| io::Error::other("could not allocate the test display buffer"))?;
        let mut ui = Ui::new(UiConfig {
            dpi_def: DISPLAY_DPI,
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        });
        ferrovgl_draw_sw::init(&mut ui);
        let display = ui
            .display_create_ex(width, height, theme_init)
            .ok_or_else(|| io::Error::other("could not create the test display"))?;
        ui.display_set_color_format(display, ColorFormat::Xrgb8888);
        ui.display_set_draw_buffers(display, Some(buffer), None);
        ui.display_set_render_mode(display, RenderMode::Direct);
        ui.display_set_driver(display, Some(Box::new(HeadlessDriver)));
        Ok(Self { ui, display })
    }

    /// Install synthetic pointer, keypad and encoder devices for this display.
    /// Call once per set of devices; each call creates three new UI-owned devices.
    pub fn create_inputs(&mut self) -> TestInputs {
        TestInputs::new(&mut self.ui, self.display)
    }

    /// The headless display handle.
    pub fn display(&self) -> DisplayId {
        self.display
    }

    /// The active screen, if the caller has not deleted it or the display.
    pub fn screen(&self) -> Option<Obj> {
        self.ui.display_get_screen_active(Some(self.display))
    }

    /// Invalidate the active screen and refresh synchronously, like upstream's
    /// screenshot assertion. This does not advance animation time.
    pub fn refresh(&mut self) {
        if let Some(screen) = self.screen() {
            self.ui.obj_invalidate(screen);
        }
        self.ui.refr_now(Some(self.display));
    }

    /// Advance the deterministic clock and run timers once per millisecond,
    /// then refresh pending invalidations even when `milliseconds` is zero.
    pub fn wait(&mut self, milliseconds: u32) {
        for _ in 0..milliseconds {
            self.ui.tick_inc(1);
            self.ui.timer_handler();
        }
        self.ui.refr_now(Some(self.display));
    }

    /// Refresh and copy visible RGB bytes from the XRGB8888 buffer, excluding
    /// row padding and the unused X byte. Rejects a changed display format,
    /// render mode, missing screen, or buffers that no longer cover the display.
    pub fn snapshot(&mut self) -> io::Result<RgbImage> {
        self.snapshot_buffer()?;
        self.refresh();
        // A callback may delete the display or change its configuration.
        let buffer = self.snapshot_buffer()?;
        RgbImage::from_xrgb8888(
            buffer.header.w.into(),
            buffer.header.h.into(),
            buffer.header.stride.into(),
            buffer.data(),
        )
    }

    fn snapshot_buffer(&self) -> io::Result<&DrawBuf> {
        let display = self
            .ui
            .display(self.display)
            .ok_or_else(|| io::Error::other("the test display was deleted"))?;
        let active = display.act_scr.map(Obj);
        if active.is_none_or(|obj| !self.ui.obj_is_valid(obj)) {
            return Err(io::Error::other("the test display has no active screen"));
        }
        if display.render_mode != RenderMode::Direct
            || display.buf_act != 1
            || display.buf_2.is_some()
            || display.buf_3.is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "screenshots require single-buffered direct rendering",
            ));
        }
        let buffer = display
            .buf_1
            .as_ref()
            .ok_or_else(|| io::Error::other("the test display has no active draw buffer"))?;
        if display.color_format != ColorFormat::Xrgb8888 || buffer.header.cf != ColorFormat::Xrgb8888 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the test display must use XRGB8888",
            ));
        }
        if i32::from(buffer.header.w) != display.hor_res || i32::from(buffer.header.h) != display.ver_res {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the draw buffer dimensions do not match the display",
            ));
        }
        screenshot::validate_xrgb8888(
            buffer.header.w.into(),
            buffer.header.h.into(),
            buffer.header.stride.into(),
            buffer.data().len(),
        )?;
        Ok(buffer)
    }

    /// Compare a freshly rendered frame with an upstream relative PNG path.
    /// Mismatches write actual, expected and diff PNGs under `target/parity`.
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

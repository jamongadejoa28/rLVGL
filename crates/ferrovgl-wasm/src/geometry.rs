//! Canvas sizing, pointer coordinate mapping and wheel steps.
//!
//! The canvas backing store is exactly `hor_res` x `ver_res`: one LVGL
//! pixel is one canvas pixel. Its CSS size is chosen so that each LVGL
//! pixel covers a whole number of device pixels (`pixel_scale`), which
//! keeps the image crisp on HiDPI screens:
//! `css = hor_res * pixel_scale / devicePixelRatio`.
//!
//! Pointer events report CSS pixels relative to the viewport; they are
//! mapped back with `getBoundingClientRect()`, which already contains any
//! page zoom or CSS transform, so the mapping works for every size.

/// A `DOMRect` from `getBoundingClientRect()`, in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CssRect {
    /// `left`
    pub left: f64,
    /// `top`
    pub top: f64,
    /// `width`
    pub width: f64,
    /// `height`
    pub height: f64,
}

/// Default number of device pixels per LVGL pixel for a
/// `devicePixelRatio`: the ratio rounded to the nearest integer, at least 1.
#[must_use]
pub fn default_pixel_scale(device_pixel_ratio: f64) -> f64 {
    if device_pixel_ratio.is_finite() && device_pixel_ratio > 0.0 {
        device_pixel_ratio.round().max(1.0)
    } else {
        1.0
    }
}

/// CSS length (in `px`) of `display_px` LVGL pixels when every LVGL pixel
/// covers `pixel_scale` device pixels at `device_pixel_ratio`.
#[must_use]
pub fn css_length(display_px: u32, pixel_scale: f64, device_pixel_ratio: f64) -> f64 {
    let dpr = if device_pixel_ratio.is_finite() && device_pixel_ratio > 0.0 {
        device_pixel_ratio
    } else {
        1.0
    };
    let scale = if pixel_scale.is_finite() && pixel_scale > 0.0 {
        pixel_scale
    } else {
        default_pixel_scale(dpr)
    };
    f64::from(display_px) * scale / dpr
}

/// Maps a viewport position (`clientX`, `clientY`) to display pixels.
///
/// Like `lv_sdl_mouse.c` (`(int16_t)((float)x / zoom)`), the position is
/// divided by the zoom (CSS pixels per display pixel) and truncated to an
/// `int16_t`. Positions outside the canvas are not clamped here; the
/// pointer indev clamps them to the display (`indev_pointer_proc()`).
#[must_use]
pub fn client_to_display(
    client_x: f64,
    client_y: f64,
    rect: CssRect,
    hor_res: i32,
    ver_res: i32,
) -> (i32, i32) {
    fn axis(client: f64, start: f64, size: f64, res: i32) -> i32 {
        if !(size.is_finite() && size > 0.0 && client.is_finite() && start.is_finite()) || res <= 0 {
            return 0;
        }
        let zoom = size / f64::from(res);
        ((client - start) / zoom) as f32 as i16 as i32
    }
    (
        axis(client_x, rect.left, rect.width, hor_res),
        axis(client_y, rect.top, rect.height, ver_res),
    )
}

/// Encoder steps for one DOM `wheel` event: upstream's `__EMSCRIPTEN__`
/// branch of `lv_sdl_mousewheel.c` counts one step per event in the
/// scroll direction (`if(wheel.y < 0) diff++; if(wheel.y > 0) diff--;`).
/// SDL's `y` points away from the user while DOM `deltaY` points towards
/// the content below, so a positive `deltaY` is `+1`.
#[must_use]
pub fn wheel_steps(delta_y: f64) -> i16 {
    if delta_y > 0.0 {
        1
    } else if delta_y < 0.0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_scale_and_css_size() {
        assert_eq!(default_pixel_scale(1.0), 1.0);
        assert_eq!(default_pixel_scale(2.0), 2.0);
        assert_eq!(default_pixel_scale(1.25), 1.0);
        assert_eq!(default_pixel_scale(1.5), 2.0);
        assert_eq!(default_pixel_scale(f64::NAN), 1.0);
        assert_eq!(css_length(800, 1.0, 1.0), 800.0);
        assert_eq!(css_length(800, 2.0, 2.0), 800.0);
        assert_eq!(css_length(800, 1.0, 2.0), 400.0);
        assert_eq!(css_length(800, 2.0, 1.0), 1600.0);
        // Invalid scale falls back to the rounded ratio.
        assert_eq!(css_length(300, 0.0, 1.5), 400.0);
        assert_eq!(css_length(300, 1.0, 0.0), 300.0);
    }

    #[test]
    fn identity_and_scaled_mapping() {
        let rect = CssRect {
            left: 10.0,
            top: 20.0,
            width: 800.0,
            height: 480.0,
        };
        assert_eq!(client_to_display(10.0, 20.0, rect, 800, 480), (0, 0));
        assert_eq!(client_to_display(809.0, 499.0, rect, 800, 480), (799, 479));
        assert_eq!(client_to_display(110.5, 70.9, rect, 800, 480), (100, 50));
        // A 2x CSS size (pixel_scale 2 at dpr 1) halves the coordinates.
        let big = CssRect {
            width: 1600.0,
            height: 960.0,
            ..rect
        };
        assert_eq!(
            client_to_display(10.0 + 401.0, 20.0 + 3.0, big, 800, 480),
            (200, 1)
        );
        // dpr 2 with pixel_scale 1 shows the display at half CSS size.
        let small = CssRect {
            width: 400.0,
            height: 240.0,
            ..rect
        };
        assert_eq!(
            client_to_display(10.0 + 100.25, 20.0 + 0.5, small, 800, 480),
            (200, 1)
        );
    }

    #[test]
    fn outside_positions_truncate_toward_zero_and_saturate() {
        let rect = CssRect {
            left: 0.0,
            top: 0.0,
            width: 100.0,
            height: 100.0,
        };
        assert_eq!(client_to_display(-0.5, -3.5, rect, 100, 100), (0, -3));
        assert_eq!(
            client_to_display(1e9, 150.0, rect, 100, 100),
            (i16::MAX as i32, 150)
        );
        let empty = CssRect { width: 0.0, ..rect };
        assert_eq!(client_to_display(5.0, 5.0, empty, 100, 100), (0, 5));
        assert_eq!(client_to_display(f64::NAN, 5.0, rect, 100, 100), (0, 5));
    }

    #[test]
    fn wheel_counts_events() {
        assert_eq!(wheel_steps(100.0), 1);
        assert_eq!(wheel_steps(0.01), 1);
        assert_eq!(wheel_steps(-3.0), -1);
        assert_eq!(wheel_steps(0.0), 0);
        assert_eq!(wheel_steps(f64::NAN), 0);
    }
}

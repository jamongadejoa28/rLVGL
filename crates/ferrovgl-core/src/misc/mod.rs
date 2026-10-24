//! Utilities mirrored from LVGL's `src/misc`: geometry, colours, math,
//! containers, styles, events, timers, animations.

pub mod anim;
pub mod area;
pub mod arena;
pub mod color;
pub mod event;
pub mod math;
pub mod palette;
pub mod style;
pub mod style_gen;
pub mod text;
pub mod tick;
pub mod timer;
pub mod types;

/// Report a failed argument check (`LV_CHECK_ARG`).
///
/// With the `check-arg-panic` feature this panics, which is the upstream
/// `LV_CHECK_ARG_ASSERT_ON_FAIL` behaviour used by the test suite. Otherwise
/// the message is logged (when the `log` feature is on) and the caller
/// returns a neutral value, like upstream's default "log and return".
#[cold]
#[inline(never)]
#[track_caller]
pub fn check_arg_failed(msg: &str) {
    #[cfg(feature = "check-arg-panic")]
    {
        panic!("argument check failed: {msg}");
    }
    #[cfg(all(not(feature = "check-arg-panic"), feature = "log"))]
    {
        log::warn!("argument check failed: {msg}");
    }
    #[cfg(all(not(feature = "check-arg-panic"), not(feature = "log")))]
    {
        let _ = msg;
    }
}

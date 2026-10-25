//! Frame-time bookkeeping for `lv_tick_inc()` from `performance.now()`.

/// Converts the fractional millisecond timestamps of
/// `requestAnimationFrame` / `performance.now()` into whole-millisecond
/// `lv_tick_inc()` periods without losing the fractions.
///
/// Only whole milliseconds are consumed; the remainder carries over to
/// the next frame, so the LVGL tick never drifts from wall-clock time
/// (the SDL backend does the same with `Instant`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameClock {
    last_ms: Option<f64>,
    total_ms: u64,
}

impl FrameClock {
    /// A clock that starts at the first [`Self::advance`] call.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_ms: None,
            total_ms: 0,
        }
    }

    /// Advances to `now_ms` and returns the whole milliseconds elapsed since
    /// the previous call (0 on the first call).
    ///
    /// Time going backwards or a non-finite timestamp yields 0 and restarts
    /// the measurement from `now_ms` (if finite). Gaps longer than
    /// `u32::MAX` ms are clamped.
    pub fn advance(&mut self, now_ms: f64) -> u32 {
        if !now_ms.is_finite() {
            return 0;
        }
        let Some(last) = self.last_ms else {
            self.last_ms = Some(now_ms);
            return 0;
        };
        let elapsed = now_ms - last;
        if elapsed < 0.0 {
            self.last_ms = Some(now_ms);
            return 0;
        }
        let whole = elapsed.floor().min(f64::from(u32::MAX));
        self.last_ms = Some(last + whole);
        self.total_ms = self.total_ms.saturating_add(whole as u64);
        whole as u32
    }

    /// Total milliseconds handed out by [`Self::advance`].
    #[must_use]
    pub fn total_ms(&self) -> u64 {
        self.total_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_call_starts_the_clock() {
        let mut c = FrameClock::new();
        assert_eq!(c.advance(1234.5), 0);
        assert_eq!(c.advance(1250.0), 15);
        assert_eq!(c.total_ms(), 15);
    }

    #[test]
    fn fractions_carry_over() {
        let mut c = FrameClock::new();
        c.advance(0.0);
        // 60 Hz frames are 16.666... ms apart: 16, 17, 17 in total 50.
        let steps: Vec<u32> = (1..=3).map(|i| c.advance(f64::from(i) * 50.0 / 3.0)).collect();
        assert_eq!(steps, [16, 17, 17]);
        assert_eq!(c.total_ms(), 50);
        let mut c = FrameClock::new();
        c.advance(0.0);
        let sum: u32 = (1..=600).map(|i| c.advance(f64::from(i) * 0.1)).sum();
        assert_eq!(sum, 60);
    }

    #[test]
    fn backwards_and_invalid_time() {
        let mut c = FrameClock::new();
        c.advance(100.0);
        assert_eq!(c.advance(50.0), 0);
        assert_eq!(c.advance(60.0), 10);
        assert_eq!(c.advance(f64::NAN), 0);
        assert_eq!(c.advance(f64::INFINITY), 0);
        assert_eq!(c.advance(70.0), 10);
        assert_eq!(c.advance(70.0 + 1e12), u32::MAX);
    }
}

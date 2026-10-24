//! Basic scalar types and coordinate encoding.
//!
//! Ported from lvgl v9.6.0: `include/lvgl/lv_types.h`, `include/lvgl/core/lv_area.h`
//! (the `LV_COORD_*` macros).

/// Precise value type (`lv_value_precise_t`).
///
/// `f32` with the `float` feature (the upstream test configuration), `i32`
/// otherwise.
#[cfg(feature = "float")]
pub type ValuePrecise = f32;
/// Precise value type (`lv_value_precise_t`).
#[cfg(not(feature = "float"))]
pub type ValuePrecise = i32;

/// Coordinate encoding helpers (`LV_COORD_*`, `LV_PCT`, `LV_SIZE_CONTENT`).
///
/// LVGL stores pixel coordinates and special values (percentages, "size to
/// content") in the same `i32`, using the two top bits below the sign as a
/// type tag. These functions reproduce that encoding exactly so that style
/// values stay compatible with upstream semantics.
pub mod coord {
    /// `LV_COORD_TYPE_SHIFT`
    pub const TYPE_SHIFT: u32 = 29;
    /// `LV_COORD_TYPE_MASK`
    pub const TYPE_MASK: i32 = 3 << TYPE_SHIFT;
    /// `LV_COORD_TYPE_PX`
    pub const TYPE_PX: i32 = 0 << TYPE_SHIFT;
    /// `LV_COORD_TYPE_SPEC`
    pub const TYPE_SPEC: i32 = 1 << TYPE_SHIFT;
    /// `LV_COORD_TYPE_PX_NEG`
    pub const TYPE_PX_NEG: i32 = 3 << TYPE_SHIFT;
    /// `LV_COORD_MAX`
    pub const MAX: i32 = (1 << TYPE_SHIFT) - 1;
    /// `LV_COORD_MIN`
    pub const MIN: i32 = -MAX;
    /// `LV_SIZE_CONTENT`: size the object to its content.
    pub const SIZE_CONTENT: i32 = set_spec(MAX);
    /// `LV_PCT_STORED_MAX`
    pub const PCT_STORED_MAX: i32 = MAX - 1;
    /// `LV_PCT_POS_MAX`
    pub const PCT_POS_MAX: i32 = PCT_STORED_MAX / 2;

    /// `LV_COORD_TYPE(x)`: extract the type tag.
    #[inline]
    pub const fn type_of(x: i32) -> i32 {
        x & TYPE_MASK
    }
    /// `LV_COORD_PLAIN(x)`: remove the type tag.
    #[inline]
    pub const fn plain(x: i32) -> i32 {
        x & !TYPE_MASK
    }
    /// `LV_COORD_IS_PX(x)`
    #[inline]
    pub const fn is_px(x: i32) -> bool {
        type_of(x) == TYPE_PX || type_of(x) == TYPE_PX_NEG
    }
    /// `LV_COORD_IS_SPEC(x)`
    #[inline]
    pub const fn is_spec(x: i32) -> bool {
        type_of(x) == TYPE_SPEC
    }
    /// `LV_COORD_SET_SPEC(x)`
    #[inline]
    pub const fn set_spec(x: i32) -> i32 {
        x | TYPE_SPEC
    }
    /// `LV_PCT(x)` / `lv_pct()`: encode a percentage.
    #[inline]
    pub const fn pct(x: i32) -> i32 {
        let v = if x < 0 {
            PCT_POS_MAX - (if x > -PCT_POS_MAX { x } else { -PCT_POS_MAX })
        } else if x < PCT_POS_MAX {
            x
        } else {
            PCT_POS_MAX
        };
        set_spec(v)
    }
    /// `LV_COORD_IS_PCT(x)`
    #[inline]
    pub const fn is_pct(x: i32) -> bool {
        is_spec(x) && plain(x) <= PCT_STORED_MAX
    }
    /// `LV_COORD_GET_PCT(x)`: decode a percentage.
    #[inline]
    pub const fn get_pct(x: i32) -> i32 {
        if plain(x) > PCT_POS_MAX {
            PCT_POS_MAX - plain(x)
        } else {
            plain(x)
        }
    }
    /// `lv_pct_to_px()`: resolve a percentage against `base`, pass pixels through.
    #[inline]
    pub const fn pct_to_px(v: i32, base: i32) -> i32 {
        if is_pct(v) { (get_pct(v) * base) / 100 } else { v }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Port of upstream `test_area.c: test_pct`.
        #[test]
        fn pct_roundtrip_and_clamping() {
            const PCT_MAX_VALUE: i32 = 268_435_455;
            for v in [
                0,
                1,
                100,
                111_111_111,
                PCT_MAX_VALUE,
                -1,
                -100,
                -111_111_111,
                -PCT_MAX_VALUE,
            ] {
                let c = pct(v);
                assert!(is_pct(c));
                assert!(!is_px(c));
                assert!(is_spec(c));
                assert_eq!(get_pct(c), v);
            }
            for (v, expected) in [
                (PCT_MAX_VALUE + 1, PCT_MAX_VALUE),
                (PCT_MAX_VALUE + 100, PCT_MAX_VALUE),
                (-PCT_MAX_VALUE - 1, -PCT_MAX_VALUE),
                (-PCT_MAX_VALUE - 100, -PCT_MAX_VALUE),
            ] {
                let c = pct(v);
                assert!(is_pct(c) && !is_px(c) && is_spec(c));
                assert_eq!(get_pct(c), expected);
            }
        }

        #[test]
        fn size_content_is_spec_not_pct() {
            assert!(is_spec(SIZE_CONTENT));
            assert!(!is_pct(SIZE_CONTENT));
            assert!(!is_px(SIZE_CONTENT));
            assert!(is_px(100));
            assert!(is_px(-100));
            assert_eq!(pct_to_px(pct(50), 200), 100);
            assert_eq!(pct_to_px(-50, 200), -50);
        }
    }
}

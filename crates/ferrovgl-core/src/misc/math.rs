//! Integer math helpers.
//!
//! Ported from lvgl v9.6.0: `src/misc/lv_math.c`, `include/lvgl/misc/lv_math.h`.
//! The operation order is kept so results match upstream bit for bit.

/// `LV_TRIGO_SIN_MAX`
pub const TRIGO_SIN_MAX: i32 = 32768;
/// `LV_TRIGO_SHIFT`: shift right by this to normalise [`trigo_sin`] results.
pub const TRIGO_SHIFT: u32 = 15;
/// `LV_BEZIER_VAL_SHIFT`
pub const BEZIER_VAL_SHIFT: u32 = 10;
/// `LV_BEZIER_VAL_MAX`: the "1.0" of the fixed-point Bézier functions.
pub const BEZIER_VAL_MAX: i32 = 1 << BEZIER_VAL_SHIFT;

/// `LV_BEZIER_VAL_FLOAT(f)`: convert a float control value to fixed point.
#[inline]
#[must_use]
pub const fn bezier_val_float(f: f32) -> i32 {
    (f * BEZIER_VAL_MAX as f32) as i32
}

const CUBIC_NEWTON_ITERATIONS: i32 = 8;
const CUBIC_PRECISION_BITS: u32 = 10;

static SIN0_90_TABLE: [u16; 91] = [
    0, 572, 1144, 1715, 2286, 2856, 3425, 3993, 4560, 5126, 5690, 6252, 6813, 7371, 7927, 8481, 9032, 9580,
    10126, 10668, 11207, 11743, 12275, 12803, 13328, 13848, 14365, 14876, 15384, 15886, 16384, 16877, 17364,
    17847, 18324, 18795, 19261, 19720, 20174, 20622, 21063, 21498, 21926, 22348, 22763, 23170, 23571, 23965,
    24351, 24730, 25102, 25466, 25822, 26170, 26510, 26842, 27166, 27482, 27789, 28088, 28378, 28660, 28932,
    29197, 29452, 29698, 29935, 30163, 30382, 30592, 30792, 30983, 31164, 31336, 31499, 31651, 31795, 31928,
    32052, 32166, 32270, 32365, 32449, 32524, 32588, 32643, 32688, 32723, 32748, 32763, 32768,
];

/// `lv_trigo_sin()`: sine of `angle` in degrees, scaled to [`TRIGO_SIN_MAX`].
#[must_use]
pub fn trigo_sin(angle: i16) -> i32 {
    let mut angle = angle as i32;
    while angle < 0 {
        angle += 360;
    }
    while angle >= 360 {
        angle -= 360;
    }
    let ret: i32 = if angle < 90 {
        SIN0_90_TABLE[angle as usize] as i32
    } else if angle < 180 {
        SIN0_90_TABLE[(180 - angle) as usize] as i32
    } else if angle < 270 {
        -(SIN0_90_TABLE[(angle - 180) as usize] as i32)
    } else {
        -(SIN0_90_TABLE[(360 - angle) as usize] as i32)
    };
    if ret == 32767 {
        32768
    } else if ret == -32767 {
        -32768
    } else {
        ret
    }
}

/// `lv_trigo_cos()`
#[inline]
#[must_use]
pub fn trigo_cos(angle: i16) -> i32 {
    trigo_sin(angle.wrapping_add(90))
}

fn do_cubic_bezier(t: i32, a: i32, b: i32, c: i32) -> i32 {
    // a * t^3 + b * t^2 + c * t, with wrapping arithmetic to match C's i32 behaviour.
    let mut ret = a;
    ret = ret.wrapping_mul(t) >> CUBIC_PRECISION_BITS;
    ret = ret.wrapping_add(b).wrapping_mul(t) >> CUBIC_PRECISION_BITS;
    ret = ret.wrapping_add(c).wrapping_mul(t) >> CUBIC_PRECISION_BITS;
    ret
}

/// `lv_cubic_bezier()`: cubic Bézier easing with control points `(x1, y1)`,
/// `(x2, y2)` in [`BEZIER_VAL_MAX`] fixed point. Returns `y` for the given `x`.
#[must_use]
pub fn cubic_bezier(x: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    if !(0..=BEZIER_VAL_MAX).contains(&x1) || !(0..=BEZIER_VAL_MAX).contains(&x2) {
        crate::misc::check_arg_failed("cubic_bezier: control x out of range");
        return 0;
    }
    if x == 0 || x == BEZIER_VAL_MAX {
        return x;
    }
    // CUBIC_PRECISION_BITS == BEZIER_VAL_SHIFT, so no rescaling is needed.
    let cx = 3 * x1;
    let bx = 3 * (x2 - x1) - cx;
    let ax = (1 << CUBIC_PRECISION_BITS) - cx - bx;
    let cy = 3 * y1;
    let by = 3 * (y2 - y1) - cy;
    let ay = (1 << CUBIC_PRECISION_BITS) - cy - by;

    // Newton's method first.
    let mut t = x;
    let mut found = false;
    for _ in 0..CUBIC_NEWTON_ITERATIONS {
        let xs = do_cubic_bezier(t, ax, bx, cx) - x;
        if xs.abs() <= 1 {
            found = true;
            break;
        }
        let mut d: i32 = ax;
        d = (3i32.wrapping_mul(d).wrapping_mul(t)) >> CUBIC_PRECISION_BITS;
        d = (d.wrapping_add(2 * bx)).wrapping_mul(t) >> CUBIC_PRECISION_BITS;
        d = d.wrapping_add(cx);
        if d.abs() <= 1 {
            break;
        }
        let d = ((xs as i64) * (1i64 << CUBIC_PRECISION_BITS)) / d as i64;
        if d == 0 {
            break;
        }
        t = t.wrapping_sub(d as i32);
    }
    if !found {
        // Fallback to bisection for reliability.
        let mut tl = 0;
        let mut tr = 1 << CUBIC_PRECISION_BITS;
        t = x;
        if t < tl {
            t = tl;
        } else if t > tr {
            t = tr;
        } else {
            while tl < tr {
                let xs = do_cubic_bezier(t, ax, bx, cx);
                if (xs - x).abs() <= 1 {
                    break;
                }
                if x > xs {
                    tl = t;
                } else {
                    tr = t;
                }
                t = (tr - tl) / 2 + tl;
                if t == tl {
                    break;
                }
            }
        }
    }
    do_cubic_bezier(t, ay, by, cy)
}

/// `lv_bezier3()`: legacy wrapper, equivalent to `cubic_bezier(t, 341, u1, 683, u2)`.
#[inline]
#[must_use]
pub fn bezier3(t: i32, _u0: i32, u1: i32, u2: i32, _u3: i32) -> i32 {
    cubic_bezier(t, 341, u1, 683, u2)
}

/// Result of [`sqrt`]: integer and fractional (`f / 256`) parts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SqrtRes {
    /// Integer part.
    pub i: u16,
    /// Fractional part, in 1/256 units (only the top 4 bits are significant).
    pub f: u16,
}

/// `lv_sqrt()`: square root with 4 fractional bits.
///
/// `mask` is the starting bit for the search, e.g. `0x8000` for a full 32-bit
/// input or `0x800` when the input is known to be below `0x10000`.
#[must_use]
pub fn sqrt(x: u32, mask: u32) -> SqrtRes {
    let x = x << 8; // 4 bit precision: sqrt(256) = 16 = 4 bit
    let mut root: u32 = 0;
    let mut mask = mask;
    loop {
        let trial = root + mask;
        if trial.wrapping_mul(trial) <= x {
            root = trial;
        }
        mask >>= 1;
        if mask == 0 {
            break;
        }
    }
    SqrtRes {
        i: (root >> 4) as u16,
        f: ((root & 0xf) << 4) as u16,
    }
}

static SQQ_TABLE: [u8; 256] = [
    0, 16, 22, 27, 32, 35, 39, 42, 45, 48, 50, 53, 55, 57, 59, 61, 64, 65, 67, 69, 71, 73, 75, 76, 78, 80,
    81, 83, 84, 86, 87, 89, 90, 91, 93, 94, 96, 97, 98, 99, 101, 102, 103, 104, 106, 107, 108, 109, 110, 112,
    113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 128, 128, 129, 130, 131, 132, 133,
    134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 144, 145, 146, 147, 148, 149, 150, 150, 151, 152,
    153, 154, 155, 155, 156, 157, 158, 159, 160, 160, 161, 162, 163, 163, 164, 165, 166, 167, 167, 168, 169,
    170, 170, 171, 172, 173, 173, 174, 175, 176, 176, 177, 178, 178, 179, 180, 181, 181, 182, 183, 183, 184,
    185, 185, 186, 187, 187, 188, 189, 189, 190, 191, 192, 192, 193, 193, 194, 195, 195, 196, 197, 197, 198,
    199, 199, 200, 201, 201, 202, 203, 203, 204, 204, 205, 206, 206, 207, 208, 208, 209, 209, 210, 211, 211,
    212, 212, 213, 214, 214, 215, 215, 216, 217, 217, 218, 218, 219, 219, 220, 221, 221, 222, 222, 223, 224,
    224, 225, 225, 226, 226, 227, 227, 228, 229, 229, 230, 230, 231, 231, 232, 232, 233, 234, 234, 235, 235,
    236, 236, 237, 237, 238, 238, 239, 240, 240, 241, 241, 242, 242, 243, 243, 244, 244, 245, 245, 246, 246,
    247, 247, 248, 248, 249, 249, 250, 250, 251, 251, 252, 252, 253, 253, 254, 254, 255,
];

/// `lv_sqrt32()`: integer square root.
#[must_use]
pub fn sqrt32(x: u32) -> i32 {
    let t = |i: u32| SQQ_TABLE[i as usize] as i32;
    let mut xn: i32;
    // Mirrors the nested if/goto structure of upstream.
    enum Next {
        Nr2,
        Nr1,
        Adj,
    }
    let next;
    if x >= 0x10000 {
        if x >= 0x1000000 {
            if x >= 0x10000000 {
                if x >= 0x40000000 {
                    if x >= 65535u32 * 65535u32 {
                        return 65535;
                    }
                    xn = t(x >> 24) << 8;
                } else {
                    xn = t(x >> 22) << 7;
                }
            } else if x >= 0x4000000 {
                xn = t(x >> 20) << 6;
            } else {
                xn = t(x >> 18) << 5;
            }
            next = Next::Nr2;
        } else {
            if x >= 0x100000 {
                if x >= 0x400000 {
                    xn = t(x >> 16) << 4;
                } else {
                    xn = t(x >> 14) << 3;
                }
            } else if x >= 0x40000 {
                xn = t(x >> 12) << 2;
            } else {
                xn = t(x >> 10) << 1;
            }
            next = Next::Nr1;
        }
    } else if x >= 0x100 {
        if x >= 0x1000 {
            if x >= 0x4000 {
                xn = t(x >> 8) + 1;
            } else {
                xn = (t(x >> 6) >> 1) + 1;
            }
        } else if x >= 0x400 {
            xn = (t(x >> 4) >> 2) + 1;
        } else {
            xn = (t(x >> 2) >> 3) + 1;
        }
        next = Next::Adj;
    } else {
        return t(x) >> 4;
    }
    let xi = x as i32; // x < 0x80000000 here except in the early-return branch above
    if matches!(next, Next::Nr2) {
        xn = (xn + 1 + (x / xn as u32) as i32) / 2;
    }
    if matches!(next, Next::Nr2 | Next::Nr1) {
        xn = (xn + 1 + (x / xn as u32) as i32) / 2;
    }
    if xn.wrapping_mul(xn) > xi {
        xn -= 1;
    }
    xn
}

/// `lv_atan2()`: fast integer arctangent in degrees, 0..=360.
///
/// Inputs should be within -1456..=1456 so the 16-bit multiply does not
/// overflow (same constraint as upstream).
#[must_use]
pub fn atan2(x: i32, y: i32) -> u16 {
    let mut negflag: u8 = 0;
    let ux: u32 = if x < 0 {
        negflag += 0x01;
        (-x) as u32
    } else {
        x as u32
    };
    let uy: u32 = if y < 0 {
        negflag += 0x02;
        (-y) as u32
    } else {
        y as u32
    };
    let mut degree: u32 = if ux > uy {
        negflag += 0x10;
        (uy * 45) / ux
    } else {
        (ux * 45) / uy
    };
    let mut comp: u8 = 0;
    let tempdegree = degree as u8;
    if tempdegree > 22 {
        if tempdegree <= 44 {
            comp += 1;
        }
        if tempdegree <= 41 {
            comp += 1;
        }
        if tempdegree <= 37 {
            comp += 1;
        }
        if tempdegree <= 32 {
            comp += 1;
        }
    } else {
        if tempdegree >= 2 {
            comp += 1;
        }
        if tempdegree >= 6 {
            comp += 1;
        }
        if tempdegree >= 10 {
            comp += 1;
        }
        if tempdegree >= 15 {
            comp += 1;
        }
    }
    degree += comp as u32;
    if negflag & 0x10 != 0 {
        degree = 90 - degree;
    }
    if negflag & 0x02 != 0 {
        if negflag & 0x01 != 0 {
            degree += 180;
        } else {
            degree = 180 - degree;
        }
    } else if negflag & 0x01 != 0 {
        degree = 360 - degree;
    }
    degree as u16
}

/// `lv_pow()`: integer power with wrapping multiplication.
#[must_use]
pub fn pow(base: i64, exp: i8) -> i64 {
    if exp < 0 {
        return match base {
            0 => 0,
            1 => 1,
            -1 => {
                if exp & 1 != 0 {
                    -1
                } else {
                    1
                }
            }
            _ => 0,
        };
    }
    let mut result: i64 = 1;
    let mut base = base;
    let mut exp = exp;
    while exp != 0 {
        if exp & 1 != 0 {
            result = result.wrapping_mul(base);
        }
        exp >>= 1;
        base = base.wrapping_mul(base);
    }
    result
}

/// `lv_map()`: linearly map `x` from `[min_in, max_in]` to `[min_out, max_out]`, clamped.
#[must_use]
pub fn map(x: i32, min_in: i32, max_in: i32, min_out: i32, max_out: i32) -> i32 {
    if max_in == min_in {
        return min_out;
    }
    if max_in >= min_in && x >= max_in {
        return max_out;
    }
    if max_in >= min_in && x <= min_in {
        return min_out;
    }
    if max_in <= min_in && x <= max_in {
        return max_out;
    }
    if max_in <= min_in && x >= min_in {
        return min_out;
    }
    let delta_in = max_in - min_in;
    let delta_out = max_out - min_out;
    ((x - min_in).wrapping_mul(delta_out)) / delta_in + min_out
}

/// Xorshift random number generator (`lv_rand`, `lv_rand_set_seed`).
#[derive(Clone, Debug)]
pub struct Rand {
    seed: u32,
}

impl Default for Rand {
    fn default() -> Self {
        // Same default seed as `lv_global_t::math_rand_seed`.
        Self { seed: 0x1234_ABCD }
    }
}

impl Rand {
    /// Create a generator with the given seed (`lv_rand_set_seed`).
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }
    /// `lv_rand_set_seed()`
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }
    /// `lv_rand()`: a pseudo-random number in `min..=max`.
    #[must_use]
    pub fn next(&mut self, min: u32, max: u32) -> u32 {
        if min > max {
            crate::misc::check_arg_failed("rand: min > max");
            return min;
        }
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x % (max - min + 1)) + min
    }
}

/// `LV_UDIV255(x)`: fast division by 255 for values up to 0xFFFF.
#[inline]
#[must_use]
pub const fn udiv255(x: u32) -> u32 {
    (x * 0x8081) >> 0x17
}

/// `LV_ALIGN_UP(x, align)`
#[inline]
#[must_use]
pub const fn align_up(x: u32, align: u32) -> u32 {
    (x + (align - 1)) & !(align - 1)
}

/// `LV_ROUND_UP(x, round)`
#[inline]
#[allow(clippy::manual_div_ceil)] // keep the upstream expression
#[must_use]
pub const fn round_up(x: u32, round: u32) -> u32 {
    ((x + (round - 1)) / round) * round
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Port of upstream `test_math.c: test_math_trigo_sin_*`.
    #[test]
    fn trigo_sin_quadrants_and_wrapping() {
        assert_eq!(trigo_sin(0), 0);
        assert_eq!(trigo_sin(90), 32768);
        assert_eq!(trigo_sin(180), 0);
        assert_eq!(trigo_sin(270), -32768);
        assert_eq!(trigo_sin(360), 0);
        assert_eq!(trigo_sin(30), 16384);
        assert_eq!(trigo_sin(120), 28378);
        assert_eq!(trigo_sin(210), -16384);
        assert_eq!(trigo_sin(300), -28378);
        assert_eq!(trigo_sin(-360), 0);
        assert_eq!(trigo_sin(-90), -32768);
        assert_eq!(trigo_sin(-180), 0);
        assert_eq!(trigo_sin(-270), 32768);
        assert_eq!(trigo_sin(720), 0);
        assert_eq!(trigo_sin(450), 32768);
        assert_eq!(trigo_sin(630), -32768);
    }

    #[test]
    fn trigo_cos_basic() {
        assert_eq!(trigo_cos(0), 32768);
        assert_eq!(trigo_cos(90), 0);
        assert_eq!(trigo_cos(180), -32768);
        assert_eq!(trigo_cos(270), 0);
        assert_eq!(trigo_cos(360), 32768);
    }

    fn cubic_bezier_f(x: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        fn do_cubic(t: f32, a: f32, b: f32, c: f32) -> f32 {
            ((a * t + b) * t + c) * t
        }
        if x == 0.0 || x == 1.0 {
            return x;
        }
        let cx = 3.0 * x1;
        let bx = 3.0 * (x2 - x1) - cx;
        let ax = 1.0 - cx - bx;
        let cy = 3.0 * y1;
        let by = 3.0 * (y2 - y1) - cy;
        let ay = 1.0 - cy - by;
        let mut t = x;
        let mut found = false;
        for _ in 0..8 {
            let xs = do_cubic(t, ax, bx, cx) - x;
            if xs.abs() < 1e-6 {
                found = true;
                break;
            }
            let d = (3.0 * ax * t + 2.0 * bx) * t + cx;
            if d.abs() < 1e-6 {
                break;
            }
            t -= xs / d;
        }
        if !found {
            let (mut tl, mut tr) = (0.0f32, 1.0f32);
            t = x.clamp(0.0, 1.0);
            while tl < tr {
                let xs = do_cubic(t, ax, bx, cx);
                if (xs - x).abs() < 1e-6 {
                    break;
                }
                if x > xs {
                    tl = t;
                } else {
                    tr = t;
                }
                t = (tr - tl) * 0.5 + tl;
            }
        }
        do_cubic(t, ay, by, cy)
    }

    /// Port of upstream `test_math.c: test_cubic_bezier_ease_functions` (error < 5/1024).
    #[test]
    fn cubic_bezier_matches_float_reference() {
        for (fx1, fy1, fx2, fy2) in [
            (0.25, 0.1, 0.25, 1.0), // ease
            (0.42, 0.0, 1.0, 1.0),  // ease-in
            (0.0, 0.0, 0.58, 1.0),  // ease-out
            (0.42, 0.0, 0.58, 1.0), // ease-in-out
            (0.0, 0.0, 1.0, 1.0),   // linear
        ] {
            let (x1, y1, x2, y2) = (
                bezier_val_float(fx1),
                bezier_val_float(fy1),
                bezier_val_float(fx2),
                bezier_val_float(fy2),
            );
            let mut t = 0.0f32;
            while t <= 1.0 {
                let fy = cubic_bezier_f(t, fx1, fy1, fx2, fy2);
                let y = cubic_bezier(bezier_val_float(t), x1, y1, x2, y2);
                assert!((bezier_val_float(fy) - y).abs() < 5, "t={t} fy={fy} y={y}");
                t += 0.001;
            }
        }
    }

    #[test]
    fn bezier3_endpoints() {
        assert_eq!(bezier3(0, 0, 0, 1024, 1024), 0);
        assert_eq!(bezier3(1024, 0, 0, 1024, 1024), 1024);
    }

    #[test]
    fn sqrt_variants() {
        let r = sqrt(16, 0x8000);
        assert_eq!((r.i, r.f), (4, 0));
        let r = sqrt(2, 0x8000);
        assert_eq!(r.i, 1);
        assert!(r.f > 0, "fractional part of sqrt(2)");
        for v in [
            0u32,
            1,
            2,
            3,
            4,
            15,
            16,
            17,
            255,
            256,
            1000,
            65535,
            65536,
            1 << 20,
            1 << 30,
            u32::MAX,
        ] {
            let expected = libm_isqrt(v);
            assert_eq!(sqrt32(v), expected as i32, "sqrt32({v})");
        }
    }

    fn libm_isqrt(v: u32) -> u32 {
        // floor(sqrt(v)), capped at 65535 like upstream
        let mut r = (v as f64).sqrt() as u32;
        while (r as u64) * (r as u64) > v as u64 {
            r -= 1;
        }
        while ((r + 1) as u64) * ((r + 1) as u64) <= v as u64 {
            r += 1;
        }
        r.min(65535)
    }

    #[test]
    fn atan2_cardinal_directions() {
        // Upstream convention: the angle is measured from the +Y axis towards +X,
        // i.e. atan2(0, 1) == 0 and atan2(1, 0) == 90.
        assert_eq!(atan2(0, 100), 0);
        assert_eq!(atan2(100, 0), 90);
        assert_eq!(atan2(0, -100), 180);
        assert_eq!(atan2(-100, 0), 270);
        assert_eq!(atan2(100, 100), 45);
        assert_eq!(atan2(-100, 100), 315);
        let a = atan2(100, -100);
        assert!((134..=136).contains(&a), "{a}");
    }

    #[test]
    fn map_and_pow() {
        assert_eq!(map(5, 0, 10, 0, 100), 50);
        assert_eq!(map(-5, 0, 10, 0, 100), 0);
        assert_eq!(map(15, 0, 10, 0, 100), 100);
        assert_eq!(map(5, 10, 0, 0, 100), 50);
        assert_eq!(map(3, 3, 3, 7, 9), 7);
        assert_eq!(pow(2, 10), 1024);
        assert_eq!(pow(-1, -3), -1);
        assert_eq!(pow(5, -1), 0);
    }

    #[test]
    fn rand_is_deterministic_and_bounded() {
        let mut a = Rand::new(42);
        let mut b = Rand::new(42);
        for _ in 0..100 {
            let x = a.next(10, 20);
            assert_eq!(x, b.next(10, 20));
            assert!((10..=20).contains(&x));
        }
    }

    #[test]
    fn udiv255_exact_for_u16_range() {
        for x in (0..=0xFFFFu32).step_by(7) {
            assert_eq!(udiv255(x), x / 255, "udiv255({x})");
        }
        assert_eq!(align_up(5, 4), 8);
        assert_eq!(round_up(5, 4), 8);
        assert_eq!(round_up(8, 4), 8);
    }
}

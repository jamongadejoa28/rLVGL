//! Ported from lvgl v9.6.0: `src/misc/lv_grad.c`, `include/lvgl/draw/lv_grad.h`.
//!
//! Gradient descriptors ([`GradDsc`]) and their init helpers. The colour map
//! computation lives in the software renderer (`ferrovgl-draw-sw::grad`).

use crate::misc::area::Point;
use crate::misc::check_arg_failed;
use crate::misc::color::{Color, OPA_COVER, Opa};

/// `LV_GRADIENT_MAX_STOPS` (lv_conf default; the upstream tests use 2 too).
pub const GRADIENT_MAX_STOPS: usize = 2;

/// `lv_grad_dir_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GradDir {
    /// `LV_GRAD_DIR_NONE`: no gradient (the `grad_color` property is ignored).
    #[default]
    None = 0,
    /// `LV_GRAD_DIR_VER`: simple vertical (top to bottom) gradient.
    Ver = 1,
    /// `LV_GRAD_DIR_HOR`: simple horizontal (left to right) gradient.
    Hor = 2,
    /// `LV_GRAD_DIR_LINEAR`: linear gradient defined by start and end points.
    Linear = 3,
    /// `LV_GRAD_DIR_RADIAL`: radial gradient defined by start and end circles.
    Radial = 4,
    /// `LV_GRAD_DIR_CONICAL`: conical gradient defined by center point, start and end angles.
    Conical = 5,
}

impl GradDir {
    /// Convert a raw style number back to a direction (`None` if unknown).
    pub const fn from_u8(v: u8) -> GradDir {
        match v {
            1 => GradDir::Ver,
            2 => GradDir::Hor,
            3 => GradDir::Linear,
            4 => GradDir::Radial,
            5 => GradDir::Conical,
            _ => GradDir::None,
        }
    }
}

/// `lv_grad_extend_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GradExtend {
    /// `LV_GRAD_EXTEND_PAD`: repeat the same color.
    #[default]
    Pad = 0,
    /// `LV_GRAD_EXTEND_REPEAT`: repeat the pattern.
    Repeat = 1,
    /// `LV_GRAD_EXTEND_REFLECT`: repeat the pattern mirrored.
    Reflect = 2,
}

/// `lv_grad_stop_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GradStop {
    /// `color`
    pub color: Color,
    /// `opa`
    pub opa: Opa,
    /// `frac`: location in `0..=255`.
    pub frac: u8,
}

/// `params` union of `lv_grad_dsc_t`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GradParams {
    /// No parameters (simple `Ver` / `Hor`).
    None,
    /// `params.linear`
    Linear {
        /// `start`
        start: Point,
        /// `end`
        end: Point,
    },
    /// `params.radial`
    Radial {
        /// `focal`: center of the focal (starting) circle.
        focal: Point,
        /// `focal_extent`: a point on the focal circle.
        focal_extent: Point,
        /// `end`: center of the end circle.
        end: Point,
        /// `end_extent`: a point on the end circle.
        end_extent: Point,
    },
    /// `params.conical`
    Conical {
        /// `center`
        center: Point,
        /// `start_angle`
        start_angle: i16,
        /// `end_angle`
        end_angle: i16,
    },
}

/// `lv_grad_dsc_t`. The renderer-private `state` pointer is not stored here;
/// the software renderer computes it per draw call.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GradDsc {
    /// `stops`
    pub stops: [GradStop; GRADIENT_MAX_STOPS],
    /// `stops_count`
    pub stops_count: u8,
    /// `dir`
    pub dir: GradDir,
    /// `extend`
    pub extend: GradExtend,
    /// `params`
    pub params: GradParams,
}

impl GradDsc {
    /// A zeroed descriptor (`lv_memzero`).
    pub const fn new() -> Self {
        Self {
            stops: [GradStop {
                color: Color::BLACK,
                opa: 0,
                frac: 0,
            }; GRADIENT_MAX_STOPS],
            stops_count: 0,
            dir: GradDir::None,
            extend: GradExtend::Pad,
            params: GradParams::None,
        }
    }

    /// `lv_grad_init_stops()`: `opa` / `fracs` may be `None` (cover / evenly spaced).
    ///
    /// Like the upstream `LV_CHECK_ARG`s, `num_stops` outside `2..=GRADIENT_MAX_STOPS`
    /// leaves the descriptor untouched. A slice shorter than `num_stops` is
    /// treated the same way (upstream would read out of bounds).
    pub fn init_stops(
        &mut self,
        colors: &[Color],
        opa: Option<&[Opa]>,
        fracs: Option<&[u8]>,
        num_stops: usize,
    ) {
        if num_stops > GRADIENT_MAX_STOPS {
            check_arg_failed("init_stops: num_stops <= LV_GRADIENT_MAX_STOPS");
            return;
        }
        if num_stops <= 1 {
            check_arg_failed("init_stops: num_stops > 1");
            return;
        }
        if colors.len() < num_stops {
            check_arg_failed("init_stops: colors != NULL");
            return;
        }
        if opa.is_some_and(|o| o.len() < num_stops) {
            check_arg_failed("init_stops: opa too short");
            return;
        }
        if fracs.is_some_and(|f| f.len() < num_stops) {
            check_arg_failed("init_stops: fracs too short");
            return;
        }

        self.stops_count = num_stops as u8;
        for i in 0..num_stops {
            self.stops[i].color = colors[i];
            self.stops[i].opa = match opa {
                Some(o) => o[i],
                None => OPA_COVER,
            };
            self.stops[i].frac = match fracs {
                Some(f) => f[i],
                // `255 * i / (num_stops - 1)` in int, truncated to uint8_t.
                None => (255 * i as i32 / (num_stops as i32 - 1)) as u8,
            };
        }
    }

    /// `lv_grad_horizontal_init()`
    pub fn horizontal_init(&mut self) {
        self.dir = GradDir::Hor;
    }

    /// `lv_grad_vertical_init()`
    pub fn vertical_init(&mut self) {
        self.dir = GradDir::Ver;
    }

    /// `lv_grad_linear_init()`
    pub fn linear_init(&mut self, from_x: i32, from_y: i32, to_x: i32, to_y: i32, extend: GradExtend) {
        self.dir = GradDir::Linear;
        self.params = GradParams::Linear {
            start: Point::new(from_x, from_y),
            end: Point::new(to_x, to_y),
        };
        self.extend = extend;
    }

    /// `lv_grad_radial_init()`
    pub fn radial_init(&mut self, center_x: i32, center_y: i32, to_x: i32, to_y: i32, extend: GradExtend) {
        self.dir = GradDir::Radial;
        self.params = GradParams::Radial {
            focal: Point::new(center_x, center_y),
            focal_extent: Point::new(center_x, center_y),
            end: Point::new(center_x, center_y),
            end_extent: Point::new(to_x, to_y),
        };
        self.extend = extend;
    }

    /// `lv_grad_conical_init()`: the angles are stored as `int16_t` upstream,
    /// so they are truncated the same way here.
    pub fn conical_init(
        &mut self,
        center_x: i32,
        center_y: i32,
        start_angle: i32,
        end_angle: i32,
        extend: GradExtend,
    ) {
        self.dir = GradDir::Conical;
        self.params = GradParams::Conical {
            center: Point::new(center_x, center_y),
            start_angle: start_angle as i16,
            end_angle: end_angle as i16,
        };
        self.extend = extend;
    }

    /// `lv_grad_radial_set_focal()`
    ///
    /// Upstream writes into the `radial` member of the `params` union without
    /// touching `end` / `end_extent`; if the descriptor does not hold radial
    /// parameters yet those two are zeroed.
    pub fn radial_set_focal(&mut self, center_x: i32, center_y: i32, radius: i32) {
        let focal = Point::new(center_x, center_y);
        let focal_extent = Point::new(center_x.wrapping_add(radius), center_y);
        match &mut self.params {
            GradParams::Radial {
                focal: f,
                focal_extent: fe,
                ..
            } => {
                *f = focal;
                *fe = focal_extent;
            }
            params => {
                *params = GradParams::Radial {
                    focal,
                    focal_extent,
                    end: Point::default(),
                    end_extent: Point::default(),
                };
            }
        }
    }
}

impl Default for GradDsc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::make(255, 0, 0);
    const BLUE: Color = Color::make(0, 0, 255);

    #[test]
    fn init_stops_defaults_to_cover_and_even_fracs() {
        let mut g = GradDsc::new();
        g.init_stops(&[RED, BLUE], None, None, 2);
        assert_eq!(g.stops_count, 2);
        assert_eq!(g.stops[0].color, RED);
        assert_eq!(g.stops[1].color, BLUE);
        assert_eq!(g.stops[0].opa, OPA_COVER);
        assert_eq!(g.stops[1].opa, OPA_COVER);
        assert_eq!(g.stops[0].frac, 0);
        assert_eq!(g.stops[1].frac, 255);
        // dir / extend are not touched by lv_grad_init_stops().
        assert_eq!(g.dir, GradDir::None);
        assert_eq!(g.extend, GradExtend::Pad);
    }

    #[test]
    fn init_stops_uses_explicit_opa_and_fracs() {
        let mut g = GradDsc::new();
        g.init_stops(&[RED, BLUE], Some(&[10, 200]), Some(&[20, 100]), 2);
        assert_eq!(g.stops[0].opa, 10);
        assert_eq!(g.stops[1].opa, 200);
        assert_eq!(g.stops[0].frac, 20);
        assert_eq!(g.stops[1].frac, 100);
    }

    #[test]
    #[cfg(not(feature = "check-arg-panic"))]
    fn init_stops_rejects_bad_counts() {
        let mut g = GradDsc::new();
        g.init_stops(&[RED, BLUE], None, None, 1);
        assert_eq!(g, GradDsc::new());
        g.init_stops(&[RED, BLUE], None, None, GRADIENT_MAX_STOPS + 1);
        assert_eq!(g, GradDsc::new());
        g.init_stops(&[RED], None, None, 2);
        assert_eq!(g, GradDsc::new());
    }

    #[test]
    fn simple_inits_set_dir_only() {
        let mut g = GradDsc::new();
        g.horizontal_init();
        assert_eq!(g.dir, GradDir::Hor);
        g.vertical_init();
        assert_eq!(g.dir, GradDir::Ver);
        assert_eq!(g.params, GradParams::None);
    }

    #[test]
    fn linear_init_stores_points_and_extend() {
        let mut g = GradDsc::new();
        g.linear_init(1, 2, 3, 4, GradExtend::Repeat);
        assert_eq!(g.dir, GradDir::Linear);
        assert_eq!(g.extend, GradExtend::Repeat);
        assert_eq!(
            g.params,
            GradParams::Linear {
                start: Point::new(1, 2),
                end: Point::new(3, 4),
            }
        );
    }

    #[test]
    fn radial_init_and_set_focal() {
        let mut g = GradDsc::new();
        g.radial_init(50, 60, 90, 60, GradExtend::Reflect);
        assert_eq!(g.dir, GradDir::Radial);
        assert_eq!(g.extend, GradExtend::Reflect);
        assert_eq!(
            g.params,
            GradParams::Radial {
                focal: Point::new(50, 60),
                focal_extent: Point::new(50, 60),
                end: Point::new(50, 60),
                end_extent: Point::new(90, 60),
            }
        );
        g.radial_set_focal(40, 45, 5);
        assert_eq!(
            g.params,
            GradParams::Radial {
                focal: Point::new(40, 45),
                focal_extent: Point::new(45, 45),
                end: Point::new(50, 60),
                end_extent: Point::new(90, 60),
            }
        );
        // On a non-radial descriptor the rest of the union reads as zero.
        let mut g = GradDsc::new();
        g.radial_set_focal(1, 2, 3);
        assert_eq!(
            g.params,
            GradParams::Radial {
                focal: Point::new(1, 2),
                focal_extent: Point::new(4, 2),
                end: Point::default(),
                end_extent: Point::default(),
            }
        );
    }

    #[test]
    fn conical_init_truncates_angles_to_i16() {
        let mut g = GradDsc::new();
        g.conical_init(10, 20, 45, 405, GradExtend::Pad);
        assert_eq!(g.dir, GradDir::Conical);
        assert_eq!(
            g.params,
            GradParams::Conical {
                center: Point::new(10, 20),
                start_angle: 45,
                end_angle: 405,
            }
        );
        g.conical_init(0, 0, 0x1_0000 + 7, -1, GradExtend::Pad);
        assert_eq!(
            g.params,
            GradParams::Conical {
                center: Point::new(0, 0),
                start_angle: 7,
                end_angle: -1,
            }
        );
    }

    #[test]
    fn enum_values_match_upstream() {
        assert_eq!(GradDir::None as u8, 0);
        assert_eq!(GradDir::Ver as u8, 1);
        assert_eq!(GradDir::Hor as u8, 2);
        assert_eq!(GradDir::Linear as u8, 3);
        assert_eq!(GradDir::Radial as u8, 4);
        assert_eq!(GradDir::Conical as u8, 5);
        assert_eq!(GradExtend::Pad as u8, 0);
        assert_eq!(GradExtend::Repeat as u8, 1);
        assert_eq!(GradExtend::Reflect as u8, 2);
        for v in 0..=6u8 {
            let d = GradDir::from_u8(v);
            if v <= 5 {
                assert_eq!(d as u8, v);
            } else {
                assert_eq!(d, GradDir::None);
            }
        }
    }
}

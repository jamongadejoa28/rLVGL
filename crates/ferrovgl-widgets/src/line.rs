//! Ported from LVGL v9.6.0: `src/widgets/line/lv_line.c`.
//!
//! Also covers `include/lvgl/widgets/lv_line.h` and
//! `src/widgets/line/lv_line_private.h`. The C `point_array` union becomes
//! [`LinePoints`]: a constant array (`'static` or reference counted, compared
//! by address) or a mutable one shared with the application.
//! `LV_USE_OBJ_PROPERTY` is disabled in the parity configuration, so the
//! property table is not provided.
//!
//! C reads `point_array[i]` for every `i < point_num` without a bounds check,
//! so a `point_num` larger than the array is undefined behaviour there. Here a
//! missing point ends the loop that reads it instead of panicking.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::draw::LineDsc;
use ferrovgl_core::misc::area::PointPrecise;
use ferrovgl_core::misc::style::SharedRef;
use ferrovgl_core::misc::types::{ValuePrecise, coord};
use ferrovgl_core::{Event, EventCode, LvResult, Obj, ObjClass, Part, Ui};

/// A mutable point array (`lv_point_precise_t *`) shared with the
/// application, which may change it and then invalidate the line.
pub type LinePointsMut = Rc<RefCell<Vec<PointPrecise>>>;

/// The `point_array` union of `lv_line_t`; `point_array_is_mutable` selects
/// the member.
#[derive(Clone)]
pub enum LinePoints {
    /// `constant`: only the address is saved, the array must stay alive.
    Const(SharedRef<[PointPrecise]>),
    /// `mut`: a mutable array.
    Mut(LinePointsMut),
}

impl LinePoints {
    /// `point_array.constant[i]`, `None` past the end of the array. The
    /// mutable array is borrowed only for the read.
    fn get(&self, i: usize) -> Option<PointPrecise> {
        match self {
            LinePoints::Const(p) => p.get(i).copied(),
            LinePoints::Mut(p) => p.borrow().get(i).copied(),
        }
    }
}

impl core::fmt::Debug for LinePoints {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LinePoints::Const(p) => f.debug_tuple("Const").field(&p.addr()).finish(),
            LinePoints::Mut(p) => f.debug_tuple("Mut").field(&Rc::as_ptr(p)).finish(),
        }
    }
}

/// `lv_line_t` without its `lv_obj_t` header. Zeroed like
/// `lv_malloc_zeroed`; the constructor sets the initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct LineState {
    /// `point_array` (`NULL` = `None`): the points of the line.
    pub(crate) point_array: Option<LinePoints>,
    /// `point_num`: number of points in `point_array`.
    pub(crate) point_num: u32,
    /// `y_inv : 1`: y == 0 will be on the bottom.
    pub(crate) y_inv: bool,
    /// `point_array_is_mutable : 1`
    pub(crate) point_array_is_mutable: bool,
}

ferrovgl_core::impl_widget_state!(LineState);

/// `lv_line_class`.
pub static LINE_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_line",
    new_state: || Box::new(LineState::default()),
    constructor: Some(constructor),
    destructor: None,
    event_cb: Some(line_event),
    width_def: coord::SIZE_CONTENT,
    height_def: coord::SIZE_CONTENT,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::Inherit,
    theme_inheritable: false,
};

/// A copyable line handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Line(Obj);

impl Line {
    /// `lv_line_create()`. `None` creates a line screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&LINE_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    #[must_use]
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the line class.
    #[must_use]
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &LINE_CLASS)).then_some(Self(obj))
    }

    /// `lv_line_set_points()`: connect the first `point_num` points of
    /// `points`. `None` (`NULL`) removes the points.
    pub fn set_points(self, ui: &mut Ui, points: Option<SharedRef<[PointPrecise]>>, point_num: u32) {
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        line_set_points(ui, self.0, points.map(LinePoints::Const), point_num, false);
    }

    /// `lv_line_set_points_mutable()`: like [`Line::set_points`] with an
    /// array that [`Line::points_mutable`] gives back.
    pub fn set_points_mutable(self, ui: &mut Ui, points: Option<LinePointsMut>, point_num: u32) {
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        line_set_points(ui, self.0, points.map(LinePoints::Mut), point_num, true);
    }

    /// `lv_line_set_y_invert()`: y = 0 is at the bottom when enabled.
    pub fn set_y_invert(self, ui: &mut Ui, en: bool) {
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        let Some(line) = state_mut(ui, self.0) else {
            return;
        };
        if line.y_inv == en {
            return;
        }

        line.y_inv = en;

        ui.obj_invalidate(self.0);
    }

    /// `lv_line_get_points()`: the point array, constant or mutable.
    #[must_use]
    pub fn points(self, ui: &Ui) -> Option<LinePoints> {
        // LV_CHECK_OBJ(obj, MY_CLASS, return NULL);
        state(ui, self.0)?.point_array.clone()
    }

    /// `lv_line_get_point_count()`
    #[must_use]
    pub fn point_count(self, ui: &Ui) -> u32 {
        // LV_CHECK_OBJ(obj, MY_CLASS, return 0);
        state(ui, self.0).map_or(0, |line| line.point_num)
    }

    /// `lv_line_is_point_array_mutable()`
    #[must_use]
    pub fn is_point_array_mutable(self, ui: &Ui) -> bool {
        // LV_CHECK_OBJ(obj, MY_CLASS, return false);
        state(ui, self.0).is_some_and(|line| line.point_array_is_mutable)
    }

    /// `lv_line_get_points_mutable()`: `None` if the array is not mutable.
    #[must_use]
    pub fn points_mutable(self, ui: &Ui) -> Option<LinePointsMut> {
        // LV_CHECK_OBJ(obj, MY_CLASS, return NULL);
        let line = state(ui, self.0)?;
        if !line.point_array_is_mutable {
            // LV_LOG_WARN("the line point array is not mutable");
            return None;
        }
        match &line.point_array {
            Some(LinePoints::Mut(p)) => Some(p.clone()),
            // `line_set_points()` stores a mutable array whenever
            // `point_array_is_mutable` is set, so this is only `NULL`.
            _ => None,
        }
    }

    /// `lv_line_get_y_invert()`
    #[must_use]
    pub fn y_invert(self, ui: &Ui) -> bool {
        // LV_CHECK_OBJ(obj, MY_CLASS, return false);
        state(ui, self.0).is_some_and(|line| line.y_inv)
    }
}

impl From<Line> for Obj {
    fn from(line: Line) -> Self {
        line.obj()
    }
}

fn state(ui: &Ui, obj: Obj) -> Option<&LineState> {
    ui.obj(obj)?.widget.get::<LineState>()
}

fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut LineState> {
    ui.obj_mut(obj)?.widget.get_mut::<LineState>()
}

/// `lv_line_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    if let Some(line) = state_mut(ui, obj) {
        line.point_num = 0;
        line.point_array = None;
        line.y_inv = false;
        line.point_array_is_mutable = false;
    }

    ui.obj_set_clickable(obj, false);
}

/// `line_set_points()`
fn line_set_points(ui: &mut Ui, obj: Obj, points: Option<LinePoints>, point_num: u32, is_mut: bool) {
    // LV_CHECK_OBJ(obj, MY_CLASS, return);
    let Some(line) = state_mut(ui, obj) else {
        return;
    };
    line.point_array = points;
    line.point_num = point_num;
    line.point_array_is_mutable = is_mut;

    ui.obj_refresh_self_size(obj);

    ui.obj_invalidate(obj);
}

/// `resolve_point_coord()`
#[allow(clippy::unnecessary_cast)] // `ValuePrecise` is `i32` without the `float` feature
fn resolve_point_coord(coord: ValuePrecise, max: i32) -> ValuePrecise {
    if coord::is_pct(coord as i32) {
        // `LV_CLAMP(0, max * LV_COORD_GET_PCT((int32_t)coord) / 100, max)`
        let v = max.wrapping_mul(coord::get_pct(coord as i32)) / 100;
        0.max(v.min(max)) as ValuePrecise
    } else {
        coord
    }
}

/// `+` and `-` of `lv_value_precise_t` as C evaluates them in
/// `lv_line_event()`. With an `int32_t` `lv_value_precise_t` the C
/// arithmetic wraps in practice (formally undefined behaviour), so the `i32`
/// implementation wraps instead of panicking on overflow like
/// [`resolve_point_coord`]; the `f32` one is plain float arithmetic.
trait PreciseArith: Copy {
    /// `a + b`
    fn add(self, rhs: Self) -> Self;
    /// `a - b`
    fn sub(self, rhs: Self) -> Self;
}

impl PreciseArith for i32 {
    fn add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }
    fn sub(self, rhs: Self) -> Self {
        self.wrapping_sub(rhs)
    }
}

impl PreciseArith for f32 {
    fn add(self, rhs: Self) -> Self {
        self + rhs
    }
    fn sub(self, rhs: Self) -> Self {
        self - rhs
    }
}

/// `LV_MAX(v, i)` of an `lv_value_precise_t` and an `int32_t`, cast back to
/// `int32_t`: the comparison and the result use `lv_value_precise_t`.
#[allow(clippy::unnecessary_cast)] // `ValuePrecise` is `i32` without the `float` feature
fn max_to_i32(v: ValuePrecise, i: i32) -> i32 {
    (if v > i as ValuePrecise {
        v
    } else {
        i as ValuePrecise
    }) as i32
}

/// `lv_line_event()`
#[allow(clippy::unnecessary_cast)] // `ValuePrecise` is `i32` without the `float` feature
fn line_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    /*Call the ancestor's event handler*/
    if ui.obj_event_base(&LINE_CLASS, event) != LvResult::Ok {
        return;
    }

    let code = event.code();
    let Some(obj) = event.current_target_obj() else {
        return;
    };

    if code == EventCode::REFR_EXT_DRAW_SIZE {
        /*The corner of the skew lines is out of the intended area*/
        let line_width = ui.obj_get_style_line_width(obj, Part::MAIN);
        // `if(*s < line_width) *s = line_width;`
        event.set_ext_draw_size(line_width);
    } else if code == EventCode::GET_SELF_SIZE {
        let Some(line) = state(ui, obj) else {
            return;
        };

        if line.point_num == 0 {
            return;
        }
        let Some(points) = line.point_array.as_ref() else {
            return;
        };

        let mut w: i32 = 0;
        let mut h: i32 = 0;

        for i in 0..line.point_num as usize {
            let Some(p) = points.get(i) else {
                break;
            };
            if !coord::is_pct(p.x as i32) {
                w = max_to_i32(p.x, w);
            }

            if !coord::is_pct(p.y as i32) {
                h = max_to_i32(p.y, h);
            }
        }

        if let Some(p) = event.self_size_info() {
            p.x = w;
            p.y = h;
        }
    } else if code == EventCode::DRAW_MAIN {
        draw_main(ui, obj, event);
    }
}

/// The `LV_EVENT_DRAW_MAIN` branch of `lv_line_event()`.
#[allow(clippy::unnecessary_cast)] // `ValuePrecise` is `i32` without the `float` feature
fn draw_main(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(line) = state(ui, obj) else {
        return;
    };
    let layer = event.layer();

    if line.point_num < 2 || line.point_array.is_none() {
        return;
    }
    let Some(layer) = layer else {
        return;
    };

    let area = ui.obj_get_coords(obj);
    let x_ofs = area.x1 - ui.obj_get_scroll_x(obj);
    let y_ofs = area.y1 - ui.obj_get_scroll_y(obj);

    let mut line_dsc = LineDsc::new();
    line_dsc.base.layer = Some(layer);
    ui.obj_init_draw_line_dsc(obj, Part::MAIN, &mut line_dsc);

    /*Read all points and draw the lines*/
    let mut i: u32 = 0;
    loop {
        // C re-reads `line->point_num` and `line->point_array` on every
        // iteration. `LV_EVENT_DRAW_TASK_ADDED` callbacks run inside
        // `lv_draw_line()` and may change them or delete the line.
        let Some(line) = state(ui, obj) else {
            return;
        };
        if i >= line.point_num.wrapping_sub(1) {
            break;
        }
        let Some(points) = line.point_array.as_ref() else {
            return;
        };
        let (Some(a), Some(b)) = (points.get(i as usize), points.get(i as usize + 1)) else {
            return;
        };
        let y_inv = line.y_inv;

        let w = ui.obj_get_width(obj);
        let h = ui.obj_get_height(obj);

        line_dsc.p1.x = resolve_point_coord(a.x, w).add(x_ofs as ValuePrecise);
        line_dsc.p1.y = resolve_point_coord(a.y, h);

        line_dsc.p2.x = resolve_point_coord(b.x, w).add(x_ofs as ValuePrecise);
        line_dsc.p2.y = resolve_point_coord(b.y, h);

        if !y_inv {
            line_dsc.p1.y = line_dsc.p1.y.add(y_ofs as ValuePrecise);
            line_dsc.p2.y = line_dsc.p2.y.add(y_ofs as ValuePrecise);
        } else {
            line_dsc.p1.y = (h as ValuePrecise).sub(line_dsc.p1.y).add(y_ofs as ValuePrecise);
            line_dsc.p2.y = (h as ValuePrecise).sub(line_dsc.p2.y).add(y_ofs as ValuePrecise);
        }

        ui.draw_line(layer, &line_dsc);
        line_dsc.round_start = false; /*Draw the rounding only on the end points after the first line*/
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::UiConfig;

    fn setup() -> (Ui, Obj) {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        (ui, screen)
    }

    #[test]
    fn constructor_sets_defaults() {
        let (mut ui, screen) = setup();
        let line = Line::create(&mut ui, screen);
        assert!(ui.obj_check_type(line.obj(), &LINE_CLASS));
        assert!(!ui.obj_is_clickable(line.obj()));
        assert_eq!(line.point_count(&ui), 0);
        assert!(line.points(&ui).is_none());
        assert!(!line.y_invert(&ui));
        assert!(!line.is_point_array_mutable(&ui));
        assert_eq!(Line::from_obj(&ui, line.obj()), Some(line));
        assert_eq!(Line::from_obj(&ui, screen), None);
    }

    #[test]
    fn self_size_skips_percentages() {
        let (mut ui, screen) = setup();
        let line = Line::create(&mut ui, screen);
        let pct = coord::pct(50) as ValuePrecise;
        let points: Rc<[PointPrecise]> = Rc::from(alloc::vec![
            PointPrecise::new(3 as ValuePrecise, pct),
            PointPrecise::new(pct, 7 as ValuePrecise),
        ]);
        line.set_points(&mut ui, Some(SharedRef::Shared(points)), 2);
        assert_eq!(ui.obj_get_self_width(line.obj()), 3);
        assert_eq!(ui.obj_get_self_height(line.obj()), 7);
    }

    #[test]
    fn point_count_past_the_array_does_not_panic() {
        let (mut ui, screen) = setup();
        let line = Line::create(&mut ui, screen);
        let points: Rc<[PointPrecise]> = Rc::from(alloc::vec![PointPrecise::new(
            9 as ValuePrecise,
            4 as ValuePrecise
        )]);
        line.set_points(&mut ui, Some(SharedRef::Shared(points)), 5);
        assert_eq!(ui.obj_get_self_width(line.obj()), 9);
        assert_eq!(ui.obj_get_self_height(line.obj()), 4);
        ui.refr_now(None);
    }

    /// C evaluates the offsets of `LV_EVENT_DRAW_MAIN` in `int32_t` when
    /// `lv_value_precise_t` is an integer, which wraps instead of trapping.
    #[test]
    fn precise_arith_wraps_integers() {
        assert_eq!(PreciseArith::add(i32::MAX, 10), i32::MIN + 9);
        assert_eq!(PreciseArith::sub(5, i32::MIN), i32::MIN + 5);
        assert_eq!(PreciseArith::add(1.5f32, 2.0), 3.5);
        assert_eq!(PreciseArith::sub(1.5f32, 2.0), -0.5);
    }

    /// A non-percentage point at the edge of the `int32_t` range, drawn by a
    /// moved line, must not overflow while the offsets are added.
    #[test]
    #[allow(clippy::unnecessary_cast)] // `ValuePrecise` is `i32` without the `float` feature
    fn draw_main_with_extreme_points_does_not_panic() {
        struct Headless;
        impl ferrovgl_core::DisplayDriver for Headless {
            fn flush(
                &mut self,
                _disp: ferrovgl_core::DisplayId,
                _area: &ferrovgl_core::Area,
                _px_map: &mut [u8],
            ) -> ferrovgl_core::display::FlushResult {
                ferrovgl_core::display::FlushResult::Done
            }
        }
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(64, 64);
        let buf = ferrovgl_core::draw::draw_buf::DrawBuf::create(
            64,
            64,
            ferrovgl_core::ColorFormat::Xrgb8888,
            ferrovgl_core::draw::draw_buf::STRIDE_AUTO,
        )
        .unwrap();
        ui.display_set_color_format(display, ferrovgl_core::ColorFormat::Xrgb8888);
        ui.display_set_draw_buffers(display, Some(buf), None);
        ui.display_set_render_mode(display, ferrovgl_core::RenderMode::Direct);
        ui.display_set_driver(display, Some(Box::new(Headless)));
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        for y_inv in [false, true] {
            let line = Line::create(&mut ui, screen);
            ui.obj_set_pos(line.obj(), 10, 10);
            /* A fixed size keeps the self size of the points out of the layout */
            ui.obj_set_size(line.obj(), 50, 50);
            line.set_y_invert(&mut ui, y_inv);
            /* (int32_t)0x7FFFFFFF has the type bits 3 << 29: not a percentage */
            let points: Rc<[PointPrecise]> = Rc::from(alloc::vec![
                PointPrecise::new(i32::MAX as ValuePrecise, i32::MIN as ValuePrecise),
                PointPrecise::new(0 as ValuePrecise, i32::MAX as ValuePrecise),
            ]);
            line.set_points(&mut ui, Some(SharedRef::Shared(points)), 2);
            ui.refr_now(None);
        }
    }

    /// The percentages are multiples of 64 so that they survive a `float`
    /// `lv_value_precise_t` (24 significant bits) unchanged.
    #[test]
    #[allow(clippy::unnecessary_cast)] // `ValuePrecise` is `i32` without the `float` feature
    fn resolve_point_coord_clamps_percentages() {
        let resolve = |v: i32, max: i32| resolve_point_coord(v as ValuePrecise, max) as i32;
        assert_eq!(resolve(coord::pct(64), 200), 128);
        assert_eq!(resolve(coord::pct(128), 200), 200);
        assert_eq!(resolve(coord::pct(0), 200), 0);
        /* LV_CLAMP(0, -6, -10) == LV_MAX(0, LV_MIN(-6, -10)) */
        assert_eq!(resolve(coord::pct(64), -10), 0);
        assert_eq!(resolve(17, 200), 17);
    }
}

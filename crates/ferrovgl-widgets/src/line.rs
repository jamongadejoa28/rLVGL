//! Ported from LVGL v9.6.0: `src/widgets/line/lv_line.c`.
//!
//! Also covers `include/lvgl/widgets/lv_line.h` and
//! `src/widgets/line/lv_line_private.h`. The C `point_array` union becomes
//! [`LinePoints`]: a constant array (`'static` or reference counted, compared
//! by address) or a mutable one shared with the application.
//! `LV_USE_OBJ_PROPERTY` is disabled in the parity configuration, so the
//! property table is not provided.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::misc::area::PointPrecise;
use ferrovgl_core::misc::style::SharedRef;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Event, Obj, ObjClass, Ui};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the line class.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &LINE_CLASS)).then_some(Self(obj))
    }

    /// `lv_line_set_points()`: connect the first `point_num` points of
    /// `points`. `None` (`NULL`) removes the points.
    pub fn set_points(self, ui: &mut Ui, points: Option<SharedRef<[PointPrecise]>>, point_num: u32) {
        todo!()
    }

    /// `lv_line_set_points_mutable()`: like [`Line::set_points`] with an
    /// array that [`Line::points_mutable`] gives back.
    pub fn set_points_mutable(self, ui: &mut Ui, points: Option<LinePointsMut>, point_num: u32) {
        todo!()
    }

    /// `lv_line_set_y_invert()`: y = 0 is at the bottom when enabled.
    pub fn set_y_invert(self, ui: &mut Ui, en: bool) {
        todo!()
    }

    /// `lv_line_get_points()`: the point array, constant or mutable.
    pub fn points(self, ui: &Ui) -> Option<LinePoints> {
        todo!()
    }

    /// `lv_line_get_point_count()`
    pub fn point_count(self, ui: &Ui) -> u32 {
        todo!()
    }

    /// `lv_line_is_point_array_mutable()`
    pub fn is_point_array_mutable(self, ui: &Ui) -> bool {
        todo!()
    }

    /// `lv_line_get_points_mutable()`: `None` if the array is not mutable.
    pub fn points_mutable(self, ui: &Ui) -> Option<LinePointsMut> {
        todo!()
    }

    /// `lv_line_get_y_invert()`
    pub fn y_invert(self, ui: &Ui) -> bool {
        todo!()
    }
}

impl From<Line> for Obj {
    fn from(line: Line) -> Self {
        line.obj()
    }
}

/// `lv_line_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_line_event()`
fn line_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    todo!()
}

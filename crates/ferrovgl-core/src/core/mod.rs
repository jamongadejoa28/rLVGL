//! The object tree and its services: `Ui` context, objects, classes,
//! positioning, styles, events, drawing, scrolling and refresh.
//!
//! Mirrors LVGL v9.6.0 `src/core` (minus `lv_global.c`, which became [`Ui`]).

pub mod obj;
pub mod obj_class;
pub mod obj_draw;
pub mod obj_event;
pub mod obj_pos;
pub mod obj_scroll;
pub mod obj_style;
pub mod obj_tree;
#[cfg(feature = "observer")]
#[cfg_attr(docsrs, doc(cfg(feature = "observer")))]
pub mod observer;
pub mod refr;
pub mod ui;

pub use obj::{Obj, ObjClass, ObjData, ObjFlags, ObjId, WidgetState};
pub use ui::{AsyncJob, Ui, UiConfig};

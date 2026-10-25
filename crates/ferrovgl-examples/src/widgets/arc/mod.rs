//! `examples/widgets/arc` (`lv_example_arc.h`).
//!
//! Not ported: `lv_example_arc_bind_value` and `lv_example_arc_value_range`
//! (need `lv_arc_bind_value`, still a `todo!()` in `ferrovgl-widgets`) and
//! `lv_example_arc_img_indicator` (1.2 MB `img_arc_*` assets, `lv_arc_bind_value`).

mod lv_example_arc_change_rate;
mod lv_example_arc_event;
mod lv_example_arc_knob_widget;
mod lv_example_arc_mode;
mod lv_example_arc_pie_chart;
mod lv_example_arc_rotation;
mod lv_example_arc_set_angles;
mod lv_example_arc_styling;

pub use lv_example_arc_change_rate::lv_example_arc_change_rate;
pub use lv_example_arc_event::lv_example_arc_event;
pub use lv_example_arc_knob_widget::lv_example_arc_knob_widget;
pub use lv_example_arc_mode::lv_example_arc_mode;
pub use lv_example_arc_pie_chart::lv_example_arc_pie_chart;
pub use lv_example_arc_rotation::lv_example_arc_rotation;
pub use lv_example_arc_set_angles::lv_example_arc_set_angles;
pub use lv_example_arc_styling::lv_example_arc_styling;

//! `examples/widgets/slider` (`lv_example_slider.h`).
//!
//! Not ported: `lv_example_slider_img_indicator` (needs the 200 KiB
//! `img_bar_*` assets).

mod lv_example_slider_bind_state;
mod lv_example_slider_bind_value;
mod lv_example_slider_event;
mod lv_example_slider_modes;
mod lv_example_slider_reversed_direction;
mod lv_example_slider_styling;
mod lv_example_slider_value_range_orientation;

pub use lv_example_slider_bind_state::lv_example_slider_bind_state;
pub use lv_example_slider_bind_value::lv_example_slider_bind_value;
pub use lv_example_slider_event::lv_example_slider_event;
pub use lv_example_slider_modes::lv_example_slider_modes;
pub use lv_example_slider_reversed_direction::lv_example_slider_reversed_direction;
pub use lv_example_slider_styling::lv_example_slider_styling;
pub use lv_example_slider_value_range_orientation::lv_example_slider_value_range_orientation;

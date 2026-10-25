//! `examples/get_started` (`lv_example_get_started.h`). Every example is ported.

mod lv_example_get_started_button;
#[path = "get_started_hello_world/lv_example_get_started_hello_world.rs"]
mod lv_example_get_started_hello_world;
#[path = "get_started_slider/lv_example_get_started_slider.rs"]
mod lv_example_get_started_slider;
#[path = "get_started_styles/lv_example_get_started_styles.rs"]
mod lv_example_get_started_styles;

pub use lv_example_get_started_button::lv_example_get_started_button;
pub use lv_example_get_started_hello_world::lv_example_get_started_hello_world;
pub use lv_example_get_started_slider::lv_example_get_started_slider;
pub use lv_example_get_started_styles::lv_example_get_started_styles;

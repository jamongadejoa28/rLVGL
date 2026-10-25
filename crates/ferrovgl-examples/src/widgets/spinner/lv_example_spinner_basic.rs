//! Port of `examples/widgets/spinner/lv_example_spinner_basic.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{StyleSelector, Ui};
use ferrovgl_widgets::Spinner;

use crate::util::screen_active;

/// `lv_example_spinner_basic()`: a long medium-speed and a short fast spinner.
pub fn lv_example_spinner_basic(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    /* Long arc, medium speed; then short arc, faster */
    for (duration, sweep) in [(1500, 270), (600, 180)] {
        let spinner = Spinner::create(ui, screen);
        ui.obj_set_size(spinner.obj(), 80, 80);
        spinner.set_anim_duration(ui, duration);
        spinner.set_arc_sweep(ui, sweep);
    }
}

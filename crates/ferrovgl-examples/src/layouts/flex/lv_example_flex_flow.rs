//! Port of `examples/layouts/flex/lv_example_flex_flow.c`.

use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `lv_example_flex_flow()`: a wrapping row of buttons.
pub fn lv_example_flex_flow(ui: &mut Ui) {
    let screen = screen_active(ui);

    /* Demo container with row wrapping */
    let example_flex_flow = ui.obj_create(Some(screen));
    ui.obj_set_align(example_flex_flow, Align::Center);
    ui.obj_set_flex_flow(example_flex_flow, FlexFlow::RowWrap);
    ui.obj_set_size(example_flex_flow, coord::pct(90), coord::pct(50));

    /* Item sequence to visualize wrapping */
    for text in [
        "First", "Second", "Third", "Forth", "Fifth", "Sixth", "Seventh", "Eighth", "Ninth", "Tenth",
    ] {
        let button = Button::create(ui, example_flex_flow).obj();
        let label = Label::create(ui, button);
        label.set_text(ui, text);
    }
}

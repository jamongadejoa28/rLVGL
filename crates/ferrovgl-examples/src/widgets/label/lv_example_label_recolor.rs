//! Port of `examples/widgets/label/lv_example_label_recolor.c`.

use ferrovgl_core::{Align, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `lv_example_label_recolor()`: inline `#RRGGBB text#` color commands.
pub fn lv_example_label_recolor(ui: &mut Ui) {
    let screen = screen_active(ui);

    /* Recolor-enabled label using inline #RRGGBB color commands */
    let label = Label::create(ui, screen);
    ui.obj_set_align(label.obj(), Align::Center);
    label.set_recolor(ui, true);
    label.set_text(
        ui,
        "Write a #ff0000 red# word, a #00a000 green# word, and a #0000ff blue# word.",
    );
}

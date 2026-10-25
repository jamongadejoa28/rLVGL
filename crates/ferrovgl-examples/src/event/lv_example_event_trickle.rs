//! Port of `examples/event/lv_example_event_trickle.c`.

use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::{Color, State, Style, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_reinit};

/// `static lv_style_t style_black;`
struct StyleBlack;

/// `lv_example_event_trickle()`: event trickle-down to the children.
pub fn lv_example_event_trickle(ui: &mut Ui) {
    let cont = ui.obj_create(Some(screen_active(ui)));
    ui.obj_set_size(cont, 290, 200);
    ui.obj_center(cont);
    ui.obj_set_flex_flow(cont, FlexFlow::RowWrap);

    let [style_black] = styles_reinit::<StyleBlack, 1>(ui, || {
        let mut style_black = Style::new();
        style_black.set_text_color(Color::white());
        style_black.set_bg_color(Color::black());
        [style_black]
    });

    /*Enable event trickle-down on the container*/
    ui.obj_set_event_trickle(cont, true);

    ui.obj_add_style(cont, style_black, State::PRESSED.into());

    for i in 0u32..9 {
        let subcont = ui.obj_create(Some(cont));
        ui.obj_set_size(subcont, 70, 50);

        let label = Label::create(ui, subcont);
        label.set_text_fmt(ui, format_args!("{i}"));

        /*Add style to the label when clicked*/
        ui.obj_add_style(subcont, style_black, State::FOCUSED.into());
    }
}

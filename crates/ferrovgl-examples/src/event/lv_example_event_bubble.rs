//! Port of `examples/event/lv_example_event_bubble.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::{Event, EventCode, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    /*The original target of the event. Can be the buttons or the container*/
    let Some(target) = e.target_obj() else { return };

    /*The current target is always the container as the event is added to it*/
    let cont = e.current_target_obj();

    /*If container was clicked do nothing*/
    if Some(target) == cont {
        return;
    }

    /*Make the clicked buttons red*/
    ui.obj_set_style_bg_color(target, palette::main(Palette::Red), StyleSelector::DEFAULT);
}

/// `lv_example_event_bubble()`: event bubbling.
pub fn lv_example_event_bubble(ui: &mut Ui) {
    let cont = ui.obj_create(Some(screen_active(ui)));
    ui.obj_set_size(cont, 290, 200);
    ui.obj_center(cont);
    ui.obj_set_flex_flow(cont, FlexFlow::RowWrap);

    for i in 0u32..30 {
        let btn = Button::create(ui, cont).obj();
        ui.obj_set_size(btn, 70, 50);
        ui.obj_set_event_bubble(btn, true);

        let label = Label::create(ui, btn);
        label.set_text_fmt(ui, format_args!("{i}"));
        ui.obj_center(label.obj());
    }

    ui.obj_add_event_cb(cont, Rc::new(event_cb), EventCode::CLICKED, None);
}

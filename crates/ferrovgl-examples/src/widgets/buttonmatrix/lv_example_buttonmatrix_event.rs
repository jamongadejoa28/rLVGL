//! Port of `examples/widgets/buttonmatrix/lv_example_buttonmatrix_event.c`.

use alloc::rc::Rc;
use alloc::string::String;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Event, EventCode, StyleSelector, Ui};
use ferrovgl_widgets::{Buttonmatrix, Label};

use crate::util::{btnm_map, screen_active};

/// `static lv_obj_t * status_label;`
#[derive(Default)]
struct StatusLabel(Option<Label>);

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(bm) = e.target_obj().and_then(|o| Buttonmatrix::from_obj(ui, o)) else {
        return;
    };
    let id = bm.selected_button(ui);
    let text = String::from(bm.button_text(ui, id).unwrap_or(""));
    // LV_LOG_USER("buttonmatrix: pressed %u (\"%s\")", ...): not printed, the crate has no logger.
    if let Some(status_label) = ui.widget_static_mut::<StatusLabel>().0 {
        status_label.set_text_fmt(ui, format_args!("Pressed: {text}"));
    }
}

/// `lv_example_buttonmatrix_event()`: show the pressed button of a keypad.
pub fn lv_example_buttonmatrix_event(ui: &mut Ui) {
    let map = btnm_map(&["1", "2", "3", "\n", "4", "5", "6", "\n", "7", "8", "9"]);

    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(scr, 12, StyleSelector::DEFAULT);

    let bm = Buttonmatrix::create(ui, scr);
    ui.obj_set_size(bm.obj(), 200, 180);
    bm.set_map(ui, map);
    ui.obj_add_event_cb(bm.obj(), Rc::new(event_cb), EventCode::VALUE_CHANGED, None);

    let status_label = Label::create(ui, scr);
    ui.widget_static_mut::<StatusLabel>().0 = Some(status_label);
    status_label.set_text(ui, "Pressed: \u{2014}");
}

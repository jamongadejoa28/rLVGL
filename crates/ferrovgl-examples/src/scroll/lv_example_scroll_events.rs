//! Port of `examples/scroll/lv_example_scroll_events.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Event, EventCode, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{obj_data, screen_active, user_obj};

/// `static void scroll_event_cb(lv_event_t * e)`
fn scroll_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(panel) = e.target_obj() else { return };
    let Some(label) = user_obj(e).and_then(|o| Label::from_obj(ui, o)) else {
        return;
    };

    /* Any scroll coordinate can be queried while the panel is moving. */
    let y = ui.obj_get_scroll_y(panel);
    let top = ui.obj_get_scroll_top(panel);
    let bottom = ui.obj_get_scroll_bottom(panel);
    label.set_text_fmt(ui, format_args!("y: {y}   top: {top}   bottom: {bottom}"));
}

/// `lv_example_scroll_events()`: read the scroll position in `LV_EVENT_SCROLL`.
pub fn lv_example_scroll_events(ui: &mut Ui) {
    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(scr, 12, StyleSelector::DEFAULT);

    let label = Label::create(ui, scr);
    label.set_text(ui, "Scroll the panel");
    ui.obj_align(label.obj(), Align::TopMid, 0, 10);

    let panel = ui.obj_create(Some(scr));
    ui.obj_set_size(panel, 240, 170);
    ui.obj_center(panel);
    ui.obj_set_flex_flow(panel, FlexFlow::Column);
    ui.obj_add_event_cb(
        panel,
        Rc::new(scroll_event_cb),
        EventCode::SCROLL,
        obj_data(label.obj()),
    );

    for i in 1u32..=8 {
        let btn = Button::create(ui, panel).obj();
        ui.obj_set_width(btn, coord::pct(100));
        let btn_label = Label::create(ui, btn);
        btn_label.set_text_fmt(ui, format_args!("Item {i}"));
        ui.obj_center(btn_label.obj());
    }
}

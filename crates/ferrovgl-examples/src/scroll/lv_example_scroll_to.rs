//! Port of `examples/scroll/lv_example_scroll_to.c`.

use core::any::Any;

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::anim::ANIM_ON;
use ferrovgl_core::misc::event::EventCallback;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Event, EventCode, Obj, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `static lv_obj_t * panel; static int32_t saved_scroll_y;`
#[derive(Default)]
struct Statics {
    panel: Option<Obj>,
    saved_scroll_y: i32,
}

/// `static void jump_cb(lv_event_t * e)`
fn jump_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(panel) = ui.widget_static_mut::<Statics>().panel else {
        return;
    };
    /* user_data is the target scroll-y, or -1 for "scroll to the bottom". */
    let mut target = e
        .user_data()
        .and_then(|d| d.downcast_ref::<i32>())
        .copied()
        .unwrap_or(0);
    if target < 0 {
        target = ui.obj_get_scroll_bottom(panel) + ui.obj_get_scroll_y(panel);
    }
    ui.obj_scroll_to_y(panel, target, ANIM_ON);
}

/// `static void save_cb(lv_event_t * e)`
fn save_cb(ui: &mut Ui, _e: &mut Event<'_>) {
    let Some(panel) = ui.widget_static_mut::<Statics>().panel else {
        return;
    };
    let y = ui.obj_get_scroll_y(panel);
    ui.widget_static_mut::<Statics>().saved_scroll_y = y;
}

/// `static void restore_cb(lv_event_t * e)`
fn restore_cb(ui: &mut Ui, _e: &mut Event<'_>) {
    let s = ui.widget_static_mut::<Statics>();
    let (Some(panel), saved_scroll_y) = (s.panel, s.saved_scroll_y) else {
        return;
    };
    ui.obj_scroll_to_y(panel, saved_scroll_y, ANIM_ON);
}

/// `static lv_obj_t * make_button(lv_obj_t * parent, const char * txt, lv_event_cb_t cb, void * data)`
fn make_button(ui: &mut Ui, parent: Obj, txt: &str, cb: EventCallback, data: Option<Rc<dyn Any>>) -> Obj {
    let btn = Button::create(ui, parent).obj();
    ui.obj_add_event_cb(btn, cb, EventCode::CLICKED, data);
    let label = Label::create(ui, btn);
    label.set_text(ui, txt);
    ui.obj_center(label.obj());
    btn
}

/// `lv_example_scroll_to()`: scroll a panel programmatically, save and restore the position.
pub fn lv_example_scroll_to(ui: &mut Ui) {
    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(scr, 10, StyleSelector::DEFAULT);

    let bar = ui.obj_create(Some(scr));
    ui.obj_set_size(bar, coord::pct(100), coord::SIZE_CONTENT);
    ui.obj_set_flex_flow(bar, FlexFlow::Row);
    ui.obj_set_flex_align(bar, FlexAlign::Center, FlexAlign::Center, FlexAlign::Center);

    make_button(ui, bar, "Top", Rc::new(jump_cb), Some(Rc::new(0i32)));
    make_button(ui, bar, "Bottom", Rc::new(jump_cb), Some(Rc::new(-1i32)));
    make_button(ui, bar, "Save", Rc::new(save_cb), None);
    make_button(ui, bar, "Restore", Rc::new(restore_cb), None);

    let panel = ui.obj_create(Some(scr));
    ui.widget_static_mut::<Statics>().panel = Some(panel);
    ui.obj_set_size(panel, 260, 150);
    ui.obj_set_flex_flow(panel, FlexFlow::Column);
    for i in 1u32..=12 {
        let btn = Button::create(ui, panel).obj();
        ui.obj_set_width(btn, coord::pct(100));
        let label = Label::create(ui, btn);
        label.set_text_fmt(ui, format_args!("Row {i}"));
        ui.obj_center(label.obj());
    }
}

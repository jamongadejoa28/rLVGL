//! Port of `examples/scroll/lv_example_scroll_infinite.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Event, EventCode, Obj, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// The file-scope statics of the C file.
#[derive(Default)]
struct Statics {
    /// `static lv_obj_t * high_label;`
    high_label: Option<Label>,
    /// `static lv_obj_t * low_label;`
    low_label: Option<Label>,
    /// `static int32_t top_num;`
    top_num: i32,
    /// `static int32_t bottom_num;`
    bottom_num: i32,
    /// `static bool update_running = false;`
    update_running: bool,
}

/// `static lv_obj_t * load_item(lv_obj_t * parent, int32_t num)`
fn load_item(ui: &mut Ui, parent: Obj, num: i32) -> Obj {
    let obj = ui.obj_create(Some(parent));
    ui.obj_set_size(obj, coord::pct(100), coord::SIZE_CONTENT);
    let label = Label::create(ui, obj);
    label.set_text_fmt(ui, format_args!("{num}"));
    obj
}

/// `static void update_scroll(lv_obj_t * obj)`
fn update_scroll(ui: &mut Ui, obj: Obj) {
    /* Re-entrancy guard: lv_obj_scroll_by below re-triggers LV_EVENT_SCROLL. */
    if ui.widget_static_mut::<Statics>().update_running {
        return;
    }
    ui.widget_static_mut::<Statics>().update_running = true;

    let top_before = ui.widget_static_mut::<Statics>().top_num;
    let bottom_before_num = ui.widget_static_mut::<Statics>().bottom_num;

    /* Append items as the bottom edge gets close. */
    while ui.widget_static_mut::<Statics>().bottom_num > -30 && ui.obj_get_scroll_bottom(obj) < 200 {
        ui.widget_static_mut::<Statics>().bottom_num -= 1;
        let bottom_num = ui.widget_static_mut::<Statics>().bottom_num;
        load_item(ui, obj, bottom_num);
        ui.obj_update_layout(obj);
    }

    /* Prepend items as the top edge gets close, compensating the scroll. */
    while ui.widget_static_mut::<Statics>().top_num < 30 && ui.obj_get_scroll_top(obj) < 200 {
        ui.widget_static_mut::<Statics>().top_num += 1;
        let top_num = ui.widget_static_mut::<Statics>().top_num;
        let bottom_before = ui.obj_get_scroll_bottom(obj);
        let item = load_item(ui, obj, top_num);
        ui.obj_move_to_index(item, 0);
        ui.obj_update_layout(obj);
        let dy = bottom_before - ui.obj_get_scroll_bottom(obj);
        ui.obj_scroll_by(obj, 0, dy, ANIM_OFF);
    }

    /* Drop items that scrolled far out of view. */
    while ui.obj_get_scroll_bottom(obj) > 600 {
        ui.widget_static_mut::<Statics>().bottom_num += 1;
        if let Some(child) = ui.obj_get_child(obj, -1) {
            ui.obj_delete(child);
        }
        ui.obj_update_layout(obj);
    }
    while ui.obj_get_scroll_top(obj) > 600 {
        ui.widget_static_mut::<Statics>().top_num -= 1;
        let bottom_before = ui.obj_get_scroll_bottom(obj);
        if let Some(child) = ui.obj_get_child(obj, 0) {
            ui.obj_delete(child);
        }
        ui.obj_update_layout(obj);
        let dy = bottom_before - ui.obj_get_scroll_bottom(obj);
        ui.obj_scroll_by(obj, 0, dy, ANIM_OFF);
    }

    let s = ui.widget_static_mut::<Statics>();
    let (top_num, bottom_num, high_label, low_label) = (s.top_num, s.bottom_num, s.high_label, s.low_label);
    if top_num != top_before
        && let Some(high_label) = high_label
    {
        high_label.set_text_fmt(ui, format_args!("largest:\n{top_num}"));
    }
    if bottom_num != bottom_before_num
        && let Some(low_label) = low_label
    {
        low_label.set_text_fmt(ui, format_args!("smallest:\n{bottom_num}"));
    }

    ui.widget_static_mut::<Statics>().update_running = false;
}

/// `static void scroll_cb(lv_event_t * e)`
fn scroll_cb(ui: &mut Ui, e: &mut Event<'_>) {
    if let Some(obj) = e.target_obj() {
        update_scroll(ui, obj);
    }
}

/// `lv_example_scroll_infinite()`: load and drop items while scrolling.
pub fn lv_example_scroll_infinite(ui: &mut Ui) {
    let scr = screen_active(ui);

    let high_label = Label::create(ui, scr);
    ui.widget_static_mut::<Statics>().high_label = Some(high_label);
    high_label.set_text(ui, "largest:");
    ui.obj_align(high_label.obj(), Align::TopLeft, 10, 10);

    let low_label = Label::create(ui, scr);
    ui.widget_static_mut::<Statics>().low_label = Some(low_label);
    low_label.set_text(ui, "smallest:");
    ui.obj_align(low_label.obj(), Align::BottomLeft, 10, -10);

    let obj = ui.obj_create(Some(scr));
    ui.obj_set_size(obj, 160, 220);
    ui.obj_align(obj, Align::RightMid, -10, 0);
    ui.obj_set_flex_flow(obj, FlexFlow::Column);

    load_item(ui, obj, 3);
    ui.widget_static_mut::<Statics>().top_num = 3;
    ui.widget_static_mut::<Statics>().bottom_num = 3;
    ui.obj_update_layout(obj);
    update_scroll(ui, obj);

    ui.obj_add_event_cb(obj, Rc::new(scroll_cb), EventCode::SCROLL, None);
}

//! Port of `examples/scroll/lv_example_scroll_properties.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Event, EventCode, Obj, State, Ui};
use ferrovgl_widgets::{Button, Label, Switch};

use crate::util::screen_active;

/// `static lv_obj_t * list;`
#[derive(Default)]
struct List(Option<Obj>);

/// `typedef void (*scroll_flag_setter_t)(lv_obj_t *, bool);`
type ScrollFlagSetter = fn(&mut Ui, Obj, bool);

/// `static const struct { const char * title; scroll_flag_setter_t setter; } scroll_flags[]`
const SCROLL_FLAGS: [(&str, ScrollFlagSetter); 4] = [
    ("Scrollable", Ui::obj_set_scrollable),
    ("Scroll chain", Ui::obj_set_scroll_chain),
    ("Elastic scroll", Ui::obj_set_scroll_elastic),
    ("Scroll momentum", Ui::obj_set_scroll_momentum),
];

/// `static void flag_switch_cb(lv_event_t * e)`
fn flag_switch_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(sw) = e.target_obj() else { return };
    let Some(idx) = e.user_data().and_then(|d| d.downcast_ref::<usize>()).copied() else {
        return;
    };
    let Some(list) = ui.widget_static_mut::<List>().0 else {
        return;
    };
    let checked = ui.obj_has_state(sw, State::CHECKED);
    (SCROLL_FLAGS[idx].1)(ui, list, checked);
}

/// `static void add_flag_row(lv_obj_t * parent, uint32_t idx)`
fn add_flag_row(ui: &mut Ui, parent: Obj, idx: usize) {
    let row = ui.obj_create(Some(parent));
    ui.obj_remove_style_all(row);
    ui.obj_set_size(row, coord::pct(100), coord::SIZE_CONTENT);
    ui.obj_set_flex_flow(row, FlexFlow::Row);
    ui.obj_set_flex_align(row, FlexAlign::Start, FlexAlign::Center, FlexAlign::Center);

    let label = Label::create(ui, row);
    label.set_text(ui, SCROLL_FLAGS[idx].0);
    ui.obj_set_flex_grow(label.obj(), 1);

    let sw = Switch::create(ui, row).obj();
    ui.obj_add_state(sw, State::CHECKED); /* flags start enabled */
    if let Some(list) = ui.widget_static_mut::<List>().0 {
        (SCROLL_FLAGS[idx].1)(ui, list, true);
    }
    ui.obj_add_event_cb(
        sw,
        Rc::new(flag_switch_cb),
        EventCode::VALUE_CHANGED,
        Some(Rc::new(idx)),
    );
}

/// `lv_example_scroll_properties()`: toggle the scroll flags of a list with switches.
pub fn lv_example_scroll_properties(ui: &mut Ui) {
    let panel = ui.obj_create(Some(screen_active(ui)));
    ui.obj_set_size(panel, 300, 220);
    ui.obj_center(panel);
    ui.obj_set_flex_flow(panel, FlexFlow::Column);

    let list = ui.obj_create(Some(panel));
    ui.widget_static_mut::<List>().0 = Some(list);
    ui.obj_set_size(list, coord::pct(100), 110);
    ui.obj_set_flex_flow(list, FlexFlow::Column);
    for i in 1u32..=8 {
        let btn = Button::create(ui, list).obj();
        ui.obj_set_width(btn, coord::pct(100));
        let label = Label::create(ui, btn);
        label.set_text_fmt(ui, format_args!("Item {i}"));
        ui.obj_center(label.obj());
    }

    for i in 0..SCROLL_FLAGS.len() {
        add_flag_row(ui, panel, i);
    }
}

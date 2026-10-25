//! Port of `examples/scroll/lv_example_scroll_circular.c`.

use alloc::rc::Rc;

use ferrovgl_core::core::obj::ScrollbarMode;
use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Event, EventCode, Obj, Part, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/* Applies to item width (row) and item height (column). */
/// `ITEM_SIZE`
const ITEM_SIZE: i32 = 80;

/// `static bool adjusting = false;` of `wrap_scroll_cb`.
#[derive(Default)]
struct Adjusting(bool);

/// `static int32_t content_size(lv_obj_t * cont, bool horizontal)`
fn content_size(ui: &Ui, cont: Obj, horizontal: bool) -> i32 {
    let total = ui.obj_get_child_count(cont) as i32;
    let gap = if horizontal {
        ui.obj_get_style_pad_column(cont, Part::MAIN)
    } else {
        ui.obj_get_style_pad_row(cont, Part::MAIN)
    };
    let mut s = 0;
    for i in 0..total {
        let Some(child) = ui.obj_get_child(cont, i) else {
            continue;
        };
        s += if horizontal {
            ui.obj_get_width(child)
        } else {
            ui.obj_get_height(child)
        };
        if i < total - 1 {
            s += gap;
        }
    }

    if horizontal {
        return s
            + ui.obj_get_style_pad_left(cont, Part::MAIN)
            + ui.obj_get_style_pad_right(cont, Part::MAIN);
    }
    s + ui.obj_get_style_pad_top(cont, Part::MAIN) + ui.obj_get_style_pad_bottom(cont, Part::MAIN)
}

/// `static void wrap_scroll_cb(lv_event_t * e)`
fn wrap_scroll_cb(ui: &mut Ui, e: &mut Event<'_>) {
    if ui.widget_static_mut::<Adjusting>().0 {
        return;
    }

    let Some(cont) = e.current_target_obj() else {
        return;
    };
    let horizontal = ui.obj_get_style_flex_flow(cont, Part::MAIN) == FlexFlow::Row;
    let scroll = if horizontal {
        ui.obj_get_scroll_x(cont)
    } else {
        ui.obj_get_scroll_y(cont)
    };
    let view = if horizontal {
        ui.obj_get_width(cont)
    } else {
        ui.obj_get_height(cont)
    };
    let content = content_size(ui, cont, horizontal);
    let last = ui.obj_get_child_count(cont) as i32 - 1;

    ui.widget_static_mut::<Adjusting>().0 = true;
    if scroll <= 0 {
        /* Reached the start: move the last child to the front. */
        if let Some(child) = ui.obj_get_child(cont, last) {
            ui.obj_move_to_index(child, 0);
        }
        if horizontal {
            ui.obj_scroll_to_x(cont, scroll + ITEM_SIZE, ANIM_OFF);
        } else {
            ui.obj_scroll_to_y(cont, scroll + ITEM_SIZE, ANIM_OFF);
        }
    } else if scroll > content - view {
        /* Reached the end: move the first child to the back. */
        if let Some(child) = ui.obj_get_child(cont, 0) {
            ui.obj_move_to_index(child, last);
        }
        if horizontal {
            ui.obj_scroll_to_x(cont, scroll - ITEM_SIZE, ANIM_OFF);
        } else {
            ui.obj_scroll_to_y(cont, scroll - ITEM_SIZE, ANIM_OFF);
        }
    }
    ui.widget_static_mut::<Adjusting>().0 = false;
}

/// `static lv_obj_t * make_strip(lv_obj_t * parent, lv_flex_flow_t flow, int32_t w, int32_t h)`
fn make_strip(ui: &mut Ui, parent: Obj, flow: FlexFlow, w: i32, h: i32) -> Obj {
    let cont = ui.obj_create(Some(parent));
    ui.obj_set_size(cont, w, h);
    ui.obj_set_flex_flow(cont, flow);
    ui.obj_set_scrollbar_mode(cont, ScrollbarMode::Off);
    ui.obj_add_event_cb(cont, Rc::new(wrap_scroll_cb), EventCode::SCROLL, None);

    let horizontal = flow == FlexFlow::Row;
    for i in 1u32..=10 {
        let btn = Button::create(ui, cont).obj();
        ui.obj_set_size(
            btn,
            if horizontal { ITEM_SIZE } else { coord::pct(100) },
            if horizontal { coord::pct(100) } else { ITEM_SIZE },
        );
        let label = Label::create(ui, btn);
        label.set_text_fmt(ui, format_args!("Item {i}"));
        ui.obj_center(label.obj());
    }
    cont
}

/// `lv_example_scroll_circular()`: endless circular scrolling in both directions.
pub fn lv_example_scroll_circular(ui: &mut Ui) {
    let scr = screen_active(ui);

    let row = make_strip(ui, scr, FlexFlow::Row, 300, 75);
    ui.obj_align(row, Align::TopMid, 0, 5);

    let col = make_strip(ui, scr, FlexFlow::Column, 200, 150);
    ui.obj_align_to(col, row, Align::OutBottomMid, 0, 5);
}

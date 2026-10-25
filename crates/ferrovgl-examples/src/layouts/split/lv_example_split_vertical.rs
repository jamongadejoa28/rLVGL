//! Port of `examples/layouts/split/lv_example_split_vertical.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Event, EventCode, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `SPLIT_DIVIDER_H`
const SPLIT_DIVIDER_H: i32 = 12;
/// `SPLIT_MIN_PANE`
const SPLIT_MIN_PANE: i32 = 60;

/// `static void splitter_pressing_v_cb(lv_event_t * e)`: drag the divider to
/// resize the top pane; the bottom pane grows to fill the rest.
fn splitter_pressing_v_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(divider) = e.target_obj() else { return };
    let Some(cont) = ui.obj_get_parent(divider) else {
        return;
    };
    let Some(pane_top) = ui.obj_get_child(cont, 0) else {
        return;
    };

    let Some(indev) = ui.indev_active() else { return };

    let p = ui.indev_get_point(indev);

    let content = ui.obj_get_content_coords(cont);
    let avail = content.height() - SPLIT_DIVIDER_H;
    if avail <= 2 * SPLIT_MIN_PANE {
        return;
    }

    let mut top_h = (p.y - content.y1) - SPLIT_DIVIDER_H / 2;
    if top_h < SPLIT_MIN_PANE {
        top_h = SPLIT_MIN_PANE;
    }
    if top_h > avail - SPLIT_MIN_PANE {
        top_h = avail - SPLIT_MIN_PANE;
    }

    ui.obj_set_height(pane_top, top_h);
}

/// `lv_example_split_vertical()`: two stacked panes separated by a draggable divider.
pub fn lv_example_split_vertical(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let cont = ui.obj_create(Some(screen_active(ui)));
    ui.obj_set_size(cont, coord::pct(100), coord::pct(100));
    ui.obj_set_scrollable(cont, false);
    ui.obj_set_style_pad_all(cont, 0, sel);
    ui.obj_set_style_border_width(cont, 0, sel);
    ui.obj_set_flex_flow(cont, FlexFlow::Column);

    /* Top pane: fixed height (changes while dragging) */
    let pane_top = ui.obj_create(Some(cont));
    ui.obj_set_width(pane_top, coord::pct(100));
    ui.obj_set_height(pane_top, coord::pct(50));
    ui.obj_set_style_bg_color(pane_top, Color::hex(0xe8f0fe), sel);
    let label_top = Label::create(ui, pane_top);
    label_top.set_text(ui, "Top\n(drag the divider)");
    ui.obj_center(label_top.obj());

    /* Divider: the draggable handle */
    let divider = ui.obj_create(Some(cont));
    ui.obj_set_size(divider, coord::pct(100), SPLIT_DIVIDER_H);
    ui.obj_set_scrollable(divider, false);
    ui.obj_set_style_radius(divider, 0, sel);
    ui.obj_set_style_border_width(divider, 0, sel);
    ui.obj_set_style_bg_color(divider, palette::main(Palette::Grey), sel);
    ui.obj_add_event_cb(
        divider,
        Rc::new(splitter_pressing_v_cb),
        EventCode::PRESSING,
        None,
    );

    /* Bottom pane: grows to fill the remaining space */
    let pane_bottom = ui.obj_create(Some(cont));
    ui.obj_set_width(pane_bottom, coord::pct(100));
    ui.obj_set_flex_grow(pane_bottom, 1);
    ui.obj_set_style_bg_color(pane_bottom, Color::hex(0xe6f4ea), sel);
    let label_bottom = Label::create(ui, pane_bottom);
    label_bottom.set_text(ui, "Bottom");
    ui.obj_center(label_bottom.obj());
}

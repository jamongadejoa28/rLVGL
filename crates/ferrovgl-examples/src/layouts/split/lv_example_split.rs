//! Port of `examples/layouts/split/lv_example_split.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Event, EventCode, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `SPLIT_DIVIDER_W`
const SPLIT_DIVIDER_W: i32 = 12;
/// `SPLIT_MIN_PANE`
const SPLIT_MIN_PANE: i32 = 60;

/// `static void splitter_pressing_cb(lv_event_t * e)`: drag the divider to
/// resize the left pane; the right pane grows to fill the rest.
fn splitter_pressing_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(divider) = e.target_obj() else { return };
    let Some(cont) = ui.obj_get_parent(divider) else {
        return;
    };
    let Some(pane_left) = ui.obj_get_child(cont, 0) else {
        return;
    };

    let Some(indev) = ui.indev_active() else { return };

    let p = ui.indev_get_point(indev);

    let content = ui.obj_get_content_coords(cont);
    let avail = content.width() - SPLIT_DIVIDER_W;
    if avail <= 2 * SPLIT_MIN_PANE {
        return;
    }

    let mut left_w = (p.x - content.x1) - SPLIT_DIVIDER_W / 2;
    if left_w < SPLIT_MIN_PANE {
        left_w = SPLIT_MIN_PANE;
    }
    if left_w > avail - SPLIT_MIN_PANE {
        left_w = avail - SPLIT_MIN_PANE;
    }

    ui.obj_set_width(pane_left, left_w);
}

/// `lv_example_split()`: two panes separated by a draggable divider.
pub fn lv_example_split(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let cont = ui.obj_create(Some(screen_active(ui)));
    ui.obj_set_size(cont, coord::pct(100), coord::pct(100));
    ui.obj_set_scrollable(cont, false);
    ui.obj_set_style_pad_all(cont, 0, sel);
    ui.obj_set_style_border_width(cont, 0, sel);
    ui.obj_set_flex_flow(cont, FlexFlow::Row);

    /* Left pane: fixed width (changes while dragging) */
    let pane_left = ui.obj_create(Some(cont));
    ui.obj_set_height(pane_left, coord::pct(100));
    ui.obj_set_width(pane_left, coord::pct(50));
    ui.obj_set_style_bg_color(pane_left, Color::hex(0xe8f0fe), sel);
    let label_left = Label::create(ui, pane_left);
    label_left.set_text(ui, "Left\n(drag the divider)");
    ui.obj_center(label_left.obj());

    /* Divider: the draggable handle */
    let divider = ui.obj_create(Some(cont));
    ui.obj_set_size(divider, SPLIT_DIVIDER_W, coord::pct(100));
    ui.obj_set_scrollable(divider, false);
    ui.obj_set_style_radius(divider, 0, sel);
    ui.obj_set_style_border_width(divider, 0, sel);
    ui.obj_set_style_bg_color(divider, palette::main(Palette::Grey), sel);
    ui.obj_add_event_cb(divider, Rc::new(splitter_pressing_cb), EventCode::PRESSING, None);

    /* Right pane: grows to fill the remaining space */
    let pane_right = ui.obj_create(Some(cont));
    ui.obj_set_height(pane_right, coord::pct(100));
    ui.obj_set_flex_grow(pane_right, 1);
    ui.obj_set_style_bg_color(pane_right, Color::hex(0xe6f4ea), sel);
    let label_right = Label::create(ui, pane_right);
    label_right.set_text(ui, "Right");
    ui.obj_center(label_right.obj());
}

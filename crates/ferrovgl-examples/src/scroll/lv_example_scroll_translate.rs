//! Port of `examples/scroll/lv_example_scroll_translate.c`.

use alloc::rc::Rc;

use ferrovgl_core::core::obj::{ScrollSnap, ScrollbarMode};
use ferrovgl_core::draw::rect::RADIUS_CIRCLE;
use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::color::{OPA_COVER, OPA_TRANSP, Opa};
use ferrovgl_core::misc::math;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Dir, Event, EventCode, EventParam, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `static void scroll_event_cb(lv_event_t * e)`
fn scroll_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(cont) = e.target_obj() else { return };

    let cont_a = ui.obj_get_coords(cont);
    let cont_y_center = cont_a.y1 + cont_a.height() / 2;

    let r = ui.obj_get_height(cont) * 7 / 10;
    let child_cnt = ui.obj_get_child_count(cont) as i32;
    for i in 0..child_cnt {
        let Some(child) = ui.obj_get_child(cont, i) else {
            continue;
        };
        let child_a = ui.obj_get_coords(child);

        let child_y_center = child_a.y1 + child_a.height() / 2;

        let diff_y = (child_y_center - cont_y_center).abs();

        /* Map the vertical distance from the centre onto a circle of radius r. */
        let x = if diff_y >= r {
            r
        } else {
            let x_sqr = (r * r - diff_y * diff_y) as u32;
            let res = math::sqrt(x_sqr, 0x8000);
            r - i32::from(res.i)
        };

        ui.obj_set_style_translate_x(child, x, StyleSelector::DEFAULT);

        /* Fade items further from the centre. */
        let opa = math::map(x, 0, r, i32::from(OPA_TRANSP), i32::from(OPA_COVER)) as Opa;
        ui.obj_set_style_opa(child, OPA_COVER - opa, StyleSelector::DEFAULT);
    }
}

/// `lv_example_scroll_translate()`: translate and fade the items on a circle while scrolling.
pub fn lv_example_scroll_translate(ui: &mut Ui) {
    let cont = ui.obj_create(Some(screen_active(ui)));
    ui.obj_set_size(cont, 200, 200);
    ui.obj_center(cont);
    ui.obj_set_flex_flow(cont, FlexFlow::Column);
    ui.obj_add_event_cb(cont, Rc::new(scroll_event_cb), EventCode::SCROLL, None);
    ui.obj_set_style_radius(cont, RADIUS_CIRCLE, StyleSelector::DEFAULT);
    ui.obj_set_style_clip_corner(cont, true, StyleSelector::DEFAULT);
    ui.obj_set_scroll_dir(cont, Dir::VER);
    ui.obj_set_scroll_snap_y(cont, ScrollSnap::Center);
    ui.obj_set_scrollbar_mode(cont, ScrollbarMode::Off);

    for i in 0u32..20 {
        let btn = Button::create(ui, cont).obj();
        ui.obj_set_width(btn, coord::pct(100));

        let label = Label::create(ui, btn);
        label.set_text_fmt(ui, format_args!("Button {i}"));
    }

    /* Position the items once before the first user interaction. */
    ui.obj_send_event(cont, EventCode::SCROLL, EventParam::None);
    if let Some(child) = ui.obj_get_child(cont, 0) {
        ui.obj_scroll_to_view(child, ANIM_OFF);
    }
}

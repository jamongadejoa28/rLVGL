//! Port of `examples/event/lv_example_event_draw.c`.

use alloc::rc::Rc;

use ferrovgl_core::draw::RectDsc;
use ferrovgl_core::draw::rect::RADIUS_CIRCLE;
use ferrovgl_core::{Align, Area, Color, Event, EventCode, Obj, Part, TimerId, Ui};

use crate::util::{obj_data, screen_active};

/// `static int32_t size = 0; static bool size_dec = false;`
#[derive(Default)]
struct Statics {
    size: i32,
    size_dec: bool,
}

/// `static void timer_cb(lv_timer_t * timer)`
fn timer_cb(ui: &mut Ui, timer: TimerId) {
    let obj = ui
        .timer_get_user_data(timer)
        .and_then(|d| d.downcast_ref::<Obj>().copied());
    if let Some(obj) = obj {
        ui.obj_invalidate(obj);
    }
    let s = ui.widget_static_mut::<Statics>();
    if s.size_dec {
        s.size -= 1;
    } else {
        s.size += 1;
    }

    if s.size == 50 {
        s.size_dec = true;
    } else if s.size == 0 {
        s.size_dec = false;
    }
}

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(obj) = e.target_obj() else { return };
    let Some(draw_task) = e.draw_task() else { return };
    let Some(base_dsc) = ui.draw.task_get_draw_dsc(draw_task).and_then(|d| d.base()) else {
        return;
    };
    let (part, layer) = (base_dsc.part, base_dsc.layer);
    if part == Part::MAIN {
        let mut draw_dsc = RectDsc::new();
        draw_dsc.bg_color = Color::hex(0xffaaaa);
        draw_dsc.radius = RADIUS_CIRCLE;
        draw_dsc.border_color = Color::hex(0xff5555);
        draw_dsc.border_width = 2;
        draw_dsc.outline_color = Color::hex(0xff0000);
        draw_dsc.outline_pad = 3;
        draw_dsc.outline_width = 2;

        let size = ui.widget_static_mut::<Statics>().size;
        let mut a = Area::new(0, 0, size, size);
        let obj_coords = ui.obj_get_coords(obj);
        Area::align(&obj_coords, &mut a, Align::Center, 0, 0);

        if let Some(layer) = layer {
            ui.draw_rect(layer, &draw_dsc, &a);
        }
    }
}

/// `lv_example_event_draw()`: demonstrate the usage of draw events.
pub fn lv_example_event_draw(ui: &mut Ui) {
    let cont = ui.obj_create(Some(screen_active(ui)));
    ui.obj_set_size(cont, 200, 200);
    ui.obj_center(cont);
    ui.obj_add_event_cb(cont, Rc::new(event_cb), EventCode::DRAW_TASK_ADDED, None);
    ui.obj_set_send_draw_task_events(cont, true);
    ui.timer_create(Rc::new(timer_cb), 30, obj_data(cont));
}

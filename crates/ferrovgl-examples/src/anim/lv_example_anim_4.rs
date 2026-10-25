//! Port of `examples/anim/lv_example_anim_4.c`.

use alloc::rc::Rc;

use ferrovgl_core::misc::anim::{AnimExec, AnimTarget};
use ferrovgl_core::{Anim, AnimId, AnimPath, Event, EventCode, State, TimerId, Ui};
use ferrovgl_widgets::{Label, Switch};

use crate::util::{obj_data, screen_active, user_obj};

/// Identity of this file's `anim_x_cb`.
const ANIM_X_CB: u32 = 0x4558_0401;

/// `static void anim_x_cb(void * var, int32_t v)`
fn anim_x_cb() -> AnimExec {
    AnimExec::Custom {
        tag: ANIM_X_CB,
        f: Rc::new(|ui: &mut Ui, var, v| {
            if let AnimTarget::Obj(obj) = var {
                ui.obj_set_x(obj, v);
            }
        }),
    }
}

/// `static void timer_cb(lv_timer_t * timer)`
fn timer_cb(ui: &mut Ui, timer: TimerId) {
    let anim = ui
        .timer_get_user_data(timer)
        .and_then(|d| d.downcast_ref::<AnimId>().copied());
    if let Some(anim) = anim {
        ui.anim_pause_for(anim, 1000);
    }
    ui.timer_delete(timer);
}

/// `lv_timer_create(timer_cb, 200, lv_anim_start(&a))`
fn start_and_pause_later(ui: &mut Ui, a: &Anim) {
    let anim = ui.anim_start(a);
    ui.timer_create(Rc::new(timer_cb), 200, Some(Rc::new(anim)));
}

/// `static void sw_event_cb(lv_event_t * e)`
fn sw_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(sw) = e.target_obj() else { return };
    let Some(label) = user_obj(e) else { return };

    if ui.obj_has_state(sw, State::CHECKED) {
        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(label));
        a.set_values(ui.obj_get_x(label), 100);
        a.set_duration(500);
        a.set_exec_cb(anim_x_cb());
        a.set_path_cb(AnimPath::Overshoot);
        start_and_pause_later(ui, &a);
    } else {
        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(label));
        a.set_values(ui.obj_get_x(label), -ui.obj_get_width(label));
        a.set_duration(500);
        a.set_exec_cb(anim_x_cb());
        a.set_path_cb(AnimPath::EaseIn);
        start_and_pause_later(ui, &a);
    }
}

/// `lv_example_anim_4()`: pause an animation for a while.
pub fn lv_example_anim_4(ui: &mut Ui) {
    let label = Label::create(ui, screen_active(ui));
    label.set_text(ui, "Hello animations!");
    ui.obj_set_pos(label.obj(), 100, 10);

    let sw = Switch::create(ui, screen_active(ui));
    ui.obj_center(sw.obj());
    ui.obj_add_state(sw.obj(), State::CHECKED);
    ui.obj_add_event_cb(
        sw.obj(),
        Rc::new(sw_event_cb),
        EventCode::VALUE_CHANGED,
        obj_data(label.obj()),
    );
}

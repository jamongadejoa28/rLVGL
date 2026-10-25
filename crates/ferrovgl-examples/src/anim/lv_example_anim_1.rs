//! Port of `examples/anim/lv_example_anim_1.c`.

use alloc::rc::Rc;

use ferrovgl_core::misc::anim::{AnimExec, AnimTarget};
use ferrovgl_core::{Anim, AnimPath, Event, EventCode, State, Ui};
use ferrovgl_widgets::{Label, Switch};

use crate::util::{obj_data, screen_active, user_obj};

/// Identity of this file's `anim_x_cb` (compared like a C function pointer).
const ANIM_X_CB: u32 = 0x4558_0101;

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
        ui.anim_start(&a);
    } else {
        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(label));
        a.set_values(ui.obj_get_x(label), -ui.obj_get_width(label));
        a.set_duration(500);
        a.set_exec_cb(anim_x_cb());
        a.set_path_cb(AnimPath::EaseIn);
        ui.anim_start(&a);
    }
}

/// `lv_example_anim_1()`: start animation on an event.
pub fn lv_example_anim_1(ui: &mut Ui) {
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

//! Port of `examples/anim/lv_example_anim_2.c`.

use alloc::rc::Rc;

use ferrovgl_core::draw::rect::RADIUS_CIRCLE;
use ferrovgl_core::misc::anim::{ANIM_REPEAT_INFINITE, AnimExec, AnimTarget};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::{Align, Anim, AnimPath, StyleSelector, Ui};

use crate::util::screen_active;

/// Identity of this file's `anim_x_cb`.
const ANIM_X_CB: u32 = 0x4558_0201;
/// Identity of this file's `anim_size_cb`.
const ANIM_SIZE_CB: u32 = 0x4558_0202;

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

/// `static void anim_size_cb(void * var, int32_t v)`
fn anim_size_cb() -> AnimExec {
    AnimExec::Custom {
        tag: ANIM_SIZE_CB,
        f: Rc::new(|ui: &mut Ui, var, v| {
            if let AnimTarget::Obj(obj) = var {
                ui.obj_set_size(obj, v, v);
            }
        }),
    }
}

/// `lv_example_anim_2()`: create a playback animation.
pub fn lv_example_anim_2(ui: &mut Ui) {
    let obj = ui.obj_create(Some(screen_active(ui)));
    ui.obj_set_scrollable(obj, false);
    ui.obj_set_style_bg_color(obj, palette::main(Palette::Red), StyleSelector::DEFAULT);
    ui.obj_set_style_radius(obj, RADIUS_CIRCLE, StyleSelector::DEFAULT);

    ui.obj_align(obj, Align::LeftMid, 10, 0);

    let mut a = Anim::new();
    a.set_var(AnimTarget::Obj(obj));
    a.set_values(10, 50);
    a.set_duration(1000);
    a.set_reverse_delay(100);
    a.set_reverse_duration(300);
    a.set_repeat_delay(500);
    a.set_repeat_count(ANIM_REPEAT_INFINITE);
    a.set_path_cb(AnimPath::EaseInOut);

    a.set_exec_cb(anim_size_cb());
    ui.anim_start(&a);
    a.set_exec_cb(anim_x_cb());
    a.set_values(10, 240);
    ui.anim_start(&a);
}

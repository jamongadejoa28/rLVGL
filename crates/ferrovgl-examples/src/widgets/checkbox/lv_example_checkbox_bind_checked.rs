//! Port of `examples/widgets/checkbox/lv_example_checkbox_bind_checked.c`.

use alloc::rc::Rc;

use ferrovgl_core::core::observer::{SubjectId, SubjectType};
use ferrovgl_core::{Align, Obj, Ui};
use ferrovgl_widgets::{Checkbox, Label};

use crate::util::screen_active;

/// `static lv_subject_t * subject_flag; static bool inited = false;`
#[derive(Default)]
struct Statics {
    subject_flag: Option<SubjectId>,
    inited: bool,
}

/// `static void show_label(lv_obj_t * obj, bool value)`
fn show_label(ui: &mut Ui, obj: Obj, value: bool) {
    ui.obj_set_hidden(obj, !value);
}

/// `lv_example_checkbox_bind_checked()`: a checkbox bound to a subject that
/// shows or hides a label.
pub fn lv_example_checkbox_bind_checked(ui: &mut Ui) {
    if !ui.widget_static_mut::<Statics>().inited {
        let subject_flag = ui.subject_create(SubjectType::Int);
        if let Some(subject_flag) = subject_flag {
            ui.subject_set_int(subject_flag, 0);
        }
        let s = ui.widget_static_mut::<Statics>();
        s.subject_flag = subject_flag;
        s.inited = true;
    }
    let Some(subject_flag) = ui.widget_static_mut::<Statics>().subject_flag else {
        return;
    };

    let screen = screen_active(ui);

    let checkbox = Checkbox::create(ui, screen);
    checkbox.set_text(ui, "Show extra details");
    ui.obj_set_align(checkbox.obj(), Align::Center);
    ui.obj_bind_checked(checkbox.obj(), subject_flag);

    let label = Label::create(ui, screen);
    label.set_text(ui, "Extra details only visible while the box is ticked.");
    ui.obj_set_align(label.obj(), Align::Center);
    ui.obj_set_y(label.obj(), 30);
    ui.obj_bind_bool(label.obj(), subject_flag, Rc::new(show_label));
}

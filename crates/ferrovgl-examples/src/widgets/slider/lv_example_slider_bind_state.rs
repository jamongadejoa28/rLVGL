//! Port of `examples/widgets/slider/lv_example_slider_bind_state.c`.

use alloc::rc::Rc;

use ferrovgl_core::core::observer::{SubjectId, SubjectType};
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Align, Obj, StyleSelector, Ui, coord};
use ferrovgl_widgets::{Button, Label, Slider};

use crate::util::screen_active;

/// `static lv_subject_t * subject_value; static bool inited = false;`
#[derive(Default)]
struct Statics {
    subject_value: Option<SubjectId>,
    inited: bool,
}

/// `static void disable_if_gt_80(lv_obj_t * obj, int32_t value)`
fn disable_if_gt_80(ui: &mut Ui, obj: Obj, value: i32) {
    ui.obj_set_disabled(obj, value > 80);
}

/// `lv_example_slider_bind_state()`: a button disabled while the slider's
/// subject is above 80.
pub fn lv_example_slider_bind_state(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    if !ui.widget_static_mut::<Statics>().inited {
        let subject_value = ui.subject_create(SubjectType::Int);
        if let Some(subject_value) = subject_value {
            ui.subject_set_min_value_int(subject_value, 0);
            ui.subject_set_max_value_int(subject_value, 100);
            ui.subject_set_int(subject_value, 50);
        }
        let s = ui.widget_static_mut::<Statics>();
        s.subject_value = subject_value;
        s.inited = true;
    }
    let Some(subject_value) = ui.widget_static_mut::<Statics>().subject_value else {
        return;
    };

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* 💡 Drag past 80: the button becomes disabled; drop below 80 to re-enable it. */
    let slider = Slider::create(ui, screen);
    ui.obj_set_width(slider.obj(), coord::pct(90));
    slider.bind_value(ui, subject_value);

    let label_1 = Label::create(ui, screen);
    label_1.bind_text(ui, subject_value, Some("Current: %d (disabled above 80)"));

    let button = Button::create(ui, screen).obj();
    let label_2 = Label::create(ui, button);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Submit");
    ui.obj_bind_int(button, subject_value, Rc::new(disable_if_gt_80));
}

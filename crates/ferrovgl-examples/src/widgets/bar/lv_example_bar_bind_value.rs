//! Port of `examples/widgets/bar/lv_example_bar_bind_value.c`.

use ferrovgl_core::core::observer::{SubjectId, SubjectType};
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Align, EventCode, StyleSelector, Ui, coord};
use ferrovgl_widgets::{Bar, Button, Label};

use crate::util::screen_active;

/// `static lv_subject_t * subject_value; static bool inited = false;`
#[derive(Default)]
struct Statics {
    subject_value: Option<SubjectId>,
    inited: bool,
}

/// `lv_example_bar_bind_value()`: a bar bound to an integer subject that
/// +/- buttons step (with rollover) on click and long-press repeat.
pub fn lv_example_bar_bind_value(ui: &mut Ui) {
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

    /* 💡 Tap +/- to step; hold to repeat. Reach the limit to see rollover. */
    let bar = Bar::create(ui, screen);
    ui.obj_set_size(bar.obj(), coord::pct(90), 20);
    bar.bind_value(ui, subject_value);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_flex_flow(container, FlexFlow::Row);
    ui.obj_set_style_flex_cross_place(container, FlexAlign::Center, sel);
    ui.obj_set_style_flex_main_place(container, FlexAlign::SpaceBetween, sel);
    ui.obj_set_style_pad_column(container, 12, sel);
    ui.obj_set_style_bg_opa(container, 0, sel);
    ui.obj_set_style_border_width(container, 0, sel);
    ui.obj_set_size(container, 180, coord::SIZE_CONTENT);
    let button_1 = Button::create(ui, container).obj();
    let label_2 = Label::create(ui, button_1);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "-");

    for trigger in [EventCode::CLICKED, EventCode::LONG_PRESSED_REPEAT] {
        if let Some(dsc) = ui.obj_add_subject_increment_event(button_1, subject_value, trigger, -5) {
            ui.obj_set_subject_increment_event_min_value(button_1, &dsc, 0);
            ui.obj_set_subject_increment_event_max_value(button_1, &dsc, 100);
            ui.obj_set_subject_increment_event_rollover(button_1, &dsc, true);
        }
    }

    let label_1 = Label::create(ui, container);
    label_1.bind_text(ui, subject_value, Some("%d%%"));

    let button_2 = Button::create(ui, container).obj();
    let label_3 = Label::create(ui, button_2);
    ui.obj_set_align(label_3.obj(), Align::Center);
    label_3.set_text(ui, "+");

    for trigger in [EventCode::CLICKED, EventCode::LONG_PRESSED_REPEAT] {
        if let Some(dsc) = ui.obj_add_subject_increment_event(button_2, subject_value, trigger, 5) {
            ui.obj_set_subject_increment_event_min_value(button_2, &dsc, 0);
            ui.obj_set_subject_increment_event_max_value(button_2, &dsc, 100);
            ui.obj_set_subject_increment_event_rollover(button_2, &dsc, true);
        }
    }
}

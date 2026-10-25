//! Port of `examples/widgets/slider/lv_example_slider_bind_value.c`.

use ferrovgl_core::core::observer::{SubjectId, SubjectType};
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{StyleSelector, Ui, coord};
use ferrovgl_widgets::{Label, Slider};

use crate::util::screen_active;

/// `static lv_subject_t * subject_value; static bool inited = false;`
#[derive(Default)]
struct Statics {
    subject_value: Option<SubjectId>,
    inited: bool,
}

/// `lv_example_slider_bind_value()`: a slider two-way bound to an integer
/// subject that a label mirrors.
pub fn lv_example_slider_bind_value(ui: &mut Ui) {
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

    /* 💡 Drag the slider; the label re-renders because both widgets share `subject_value`. */
    let slider = Slider::create(ui, screen);
    ui.obj_set_width(slider.obj(), coord::pct(90));
    slider.bind_value(ui, subject_value);

    let label = Label::create(ui, screen);
    label.bind_text(ui, subject_value, Some("Value: %d/100"));
}

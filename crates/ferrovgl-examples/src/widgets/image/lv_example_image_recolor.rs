//! Port of `examples/widgets/image/lv_example_image_recolor.c`.

use ferrovgl_core::core::observer::{SubjectId, SubjectType};
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::{Color, Part, StyleProp, StyleSelector, Ui};
use ferrovgl_widgets::{Image, Label, Slider};

use crate::assets::IMG_EXAMPLE_LVGL_LOGO;
use crate::util::screen_active;

/// `static lv_subject_t * subject_opa; static bool inited = false;`
#[derive(Default)]
struct Statics {
    subject_opa: Option<SubjectId>,
    inited: bool,
}

/// `lv_example_image_recolor()`: a slider drives the image's recolor
/// intensity through `lv_obj_bind_style_prop()`.
pub fn lv_example_image_recolor(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    if !ui.widget_static_mut::<Statics>().inited {
        let subject_opa = ui.subject_create(SubjectType::Int);
        if let Some(subject_opa) = subject_opa {
            ui.subject_set_min_value_int(subject_opa, 0);
            ui.subject_set_max_value_int(subject_opa, 255);
            ui.subject_set_int(subject_opa, 128);
        }
        let s = ui.widget_static_mut::<Statics>();
        s.subject_opa = subject_opa;
        s.inited = true;
    }
    let Some(subject_opa) = ui.widget_static_mut::<Statics>().subject_opa else {
        return;
    };

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* 💡 Drag the slider; the logo's tint fades in and out as `subject_opa` (0..255) moves. */
    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::Static(&IMG_EXAMPLE_LVGL_LOGO));
    ui.obj_set_style_image_recolor(image.obj(), Color::hex(0x6366f1), Part::MAIN.into());
    ui.obj_bind_style_prop(
        image.obj(),
        StyleProp::IMAGE_RECOLOR_OPA,
        Part::MAIN.into(),
        subject_opa,
    );

    let slider = Slider::create(ui, screen);
    ui.obj_set_width(slider.obj(), 240);
    slider.set_min_value(ui, 0);
    slider.set_max_value(ui, 255);
    slider.bind_value(ui, subject_opa);

    let label = Label::create(ui, screen);
    label.bind_text(ui, subject_opa, Some("recolor_opa = %d / 255"));
}

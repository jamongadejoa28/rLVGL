//! Port of `examples/widgets/obj/lv_example_obj_bind_style.c`.

use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::core::observer::{SubjectId, SubjectType};
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Align, Color, EventCode, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `static lv_style_t style_dark; static lv_subject_t * subject_flag; static bool inited = false;`
#[derive(Default)]
struct Statics {
    style_dark: Option<StyleId>,
    subject_flag: Option<SubjectId>,
    inited: bool,
}

/// `lv_example_obj_bind_style()`: a style applied while a subject equals 1.
pub fn lv_example_obj_bind_style(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    if !ui.widget_static_mut::<Statics>().inited {
        let mut style_dark = Style::new();
        style_dark.set_bg_color(Color::hex(0x1f2937));
        style_dark.set_text_color(Color::hex(0xf9fafb));
        style_dark.set_border_color(Color::hex(0x4b5563));
        let style_dark = ui.style_create(style_dark);

        let subject_flag = ui.subject_create(SubjectType::Int);
        if let Some(subject_flag) = subject_flag {
            ui.subject_set_int(subject_flag, 0);
        }
        *ui.widget_static_mut::<Statics>() = Statics {
            style_dark: Some(style_dark),
            subject_flag,
            inited: true,
        };
    }
    let s = ui.widget_static_mut::<Statics>();
    let (Some(style_dark), Some(subject_flag)) = (s.style_dark, s.subject_flag) else {
        return;
    };

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 240, 120);
    ui.obj_set_style_pad_all(container, 16, sel);
    ui.obj_set_style_radius(container, 8, sel);
    ui.obj_set_style_border_width(container, 1, sel);
    ui.obj_bind_style(container, style_dark, sel, subject_flag, 1);
    let label_1 = Label::create(ui, container);
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Panel content");

    let button = Button::create(ui, screen).obj();
    let label_2 = Label::create(ui, button);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Toggle theme");
    ui.obj_add_subject_toggle_event(button, subject_flag, EventCode::CLICKED);
}

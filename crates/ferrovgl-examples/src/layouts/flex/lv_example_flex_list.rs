//! Port of `examples/layouts/flex/lv_example_flex_list.c`.

use alloc::rc::Rc;

use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::color::{OPA_20, OPA_COVER};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::{BorderSide, ImageSrc, SharedRef};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Event, EventCode, Obj, State, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Image, Label, LabelLongMode};

use crate::util::{screen_active, styles_once, symbol};

/// `static lv_obj_t * flex_list;`
#[derive(Default)]
struct FlexList(Option<Obj>);

/// `style_list`, `style_header`, `style_button`, `style_button_pressed`.
struct Styles;

/// `static void styles_init(void)`
fn styles_init(ui: &mut Ui) -> [StyleId; 4] {
    styles_once::<Styles, 4>(ui, || {
        /*List background: flatten the base object's card into a tight, edge-clipped column.*/
        let mut style_list = Style::new();
        style_list.set_pad_ver(0);
        style_list.set_pad_gap(0);
        style_list.set_clip_corner(true);

        /*Section header: a full-width grey bar with dark text. The transform makes the
         *grey reach the list edges despite the list's horizontal padding.*/
        let mut style_header = Style::new();
        style_header.set_bg_opa(OPA_COVER);
        style_header.set_bg_color(palette::lighten(Palette::Grey, 2));
        style_header.set_text_color(palette::darken(Palette::Grey, 4));
        style_header.set_transform_width(16);

        /*List button: a flat white row with a grey bottom divider, overriding the
         *default blue button look.*/
        let mut style_button = Style::new();
        style_button.set_radius(0);
        style_button.set_shadow_width(0);
        style_button.set_bg_opa(OPA_COVER);
        style_button.set_bg_color(Color::white());
        style_button.set_text_color(palette::darken(Palette::Grey, 4));
        style_button.set_border_width(1);
        style_button.set_border_color(palette::lighten(Palette::Grey, 2));
        style_button.set_border_side(BorderSide::BOTTOM);
        style_button.set_pad_all(8);
        style_button.set_pad_column(8);

        /*Pressed feedback: a subtle dark recolor.*/
        let mut style_button_pressed = Style::new();
        style_button_pressed.set_recolor(Color::black());
        style_button_pressed.set_recolor_opa(OPA_20);
        [style_list, style_header, style_button, style_button_pressed]
    })
}

/// `static lv_obj_t * flex_list_create(lv_obj_t * parent)`
fn flex_list_create(ui: &mut Ui, parent: Obj, styles: &[StyleId; 4]) -> Obj {
    let list = ui.obj_create(Some(parent));
    ui.obj_set_flex_flow(list, FlexFlow::Column);
    ui.obj_add_style(list, styles[0], StyleSelector::DEFAULT);
    list
}

/// `static lv_obj_t * flex_list_add_text(lv_obj_t * list, const char * txt)`
fn flex_list_add_text(ui: &mut Ui, list: Obj, txt: &str, styles: &[StyleId; 4]) -> Obj {
    let label = Label::create(ui, list);
    ui.obj_set_width(label.obj(), coord::pct(100));
    ui.obj_add_style(label.obj(), styles[1], StyleSelector::DEFAULT);
    label.set_text(ui, txt);
    label.obj()
}

/// `static lv_obj_t * flex_list_add_button(lv_obj_t * list, const void * icon, const char * txt)`
fn flex_list_add_button(
    ui: &mut Ui,
    list: Obj,
    icon: Option<&'static str>,
    txt: Option<&str>,
    styles: &[StyleId; 4],
) -> Obj {
    let btn = Button::create(ui, list).obj();
    ui.obj_set_size(btn, coord::pct(100), coord::SIZE_CONTENT);
    ui.obj_set_flex_flow(btn, FlexFlow::Row);
    ui.obj_add_style(btn, styles[2], StyleSelector::DEFAULT);
    ui.obj_add_style(btn, styles[3], State::PRESSED.into());

    /* LV_USE_IMAGE == 1 */
    if let Some(icon) = icon {
        let img = Image::create(ui, btn);
        img.set_src(ui, ImageSrc::Symbol(SharedRef::Static(icon)));
    }

    if let Some(txt) = txt {
        let label = Label::create(ui, btn);
        label.set_text(ui, txt);
        label.set_long_mode(ui, LabelLongMode::ScrollCircular);
        ui.obj_set_flex_grow(label.obj(), 1);
    }

    btn
}

/// `static void event_handler(lv_event_t * e)` with `LV_USE_LOG 0`.
fn event_handler(_ui: &mut Ui, _e: &mut Event<'_>) {
    /* LV_UNUSED(e); */
}

/// `lv_example_flex_list()`: a list built from a flex column.
pub fn lv_example_flex_list(ui: &mut Ui) {
    let styles = styles_init(ui);

    /*Create a list from a flex column*/
    let flex_list = flex_list_create(ui, screen_active(ui), &styles);
    ui.widget_static_mut::<FlexList>().0 = Some(flex_list);
    ui.obj_set_size(flex_list, 180, 220);
    ui.obj_center(flex_list);

    /*Add buttons to the list*/
    let rows: [(Option<&'static str>, &str); 14] = [
        (None, "File"),
        (Some(symbol::FILE), "New"),
        (Some(symbol::DIRECTORY), "Open"),
        (Some(symbol::SAVE), "Save"),
        (Some(symbol::CLOSE), "Delete"),
        (Some(symbol::EDIT), "Edit"),
        (None, "Connectivity"),
        (Some(symbol::BLUETOOTH), "Bluetooth"),
        (Some(symbol::GPS), "Navigation"),
        (Some(symbol::USB), "USB"),
        (Some(symbol::BATTERY_FULL), "Battery"),
        (None, "Exit"),
        (Some(symbol::OK), "Apply"),
        (Some(symbol::CLOSE), "Close"),
    ];
    for (icon, txt) in rows {
        match icon {
            None => {
                flex_list_add_text(ui, flex_list, txt, &styles);
            }
            Some(icon) => {
                let btn = flex_list_add_button(ui, flex_list, Some(icon), Some(txt), &styles);
                ui.obj_add_event_cb(btn, Rc::new(event_handler), EventCode::CLICKED, None);
            }
        }
    }
}

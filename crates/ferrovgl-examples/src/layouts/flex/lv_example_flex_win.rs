//! Port of `examples/layouts/flex/lv_example_flex_win.c`.

use alloc::rc::Rc;

use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::color::OPA_COVER;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::{ImageSrc, SharedRef};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Event, EventCode, Obj, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Image, Label, LabelLongMode};

use crate::util::{screen_active, styles_once, symbol};

/// `style_win`, `style_header`, `style_content`.
struct Styles;

/// `static void styles_init(void)`
fn styles_init(ui: &mut Ui) -> [StyleId; 3] {
    styles_once::<Styles, 3>(ui, || {
        /*Window background: a flat, edge-to-edge column with the base object's card
         *flattened away (no padding, border or radius).*/
        let mut style_win = Style::new();
        style_win.set_pad_all(0);
        style_win.set_pad_gap(0);
        style_win.set_border_width(0);
        style_win.set_radius(0);

        /*Title bar: a grey header with tight padding.*/
        let mut style_header = Style::new();
        style_header.set_bg_opa(OPA_COVER);
        style_header.set_bg_color(palette::lighten(Palette::Grey, 2));
        style_header.set_text_color(palette::darken(Palette::Grey, 4));
        style_header.set_pad_all(2);
        style_header.set_pad_gap(2);
        style_header.set_border_width(0);
        style_header.set_radius(0);

        /*Content area: a screen-coloured body with normal padding.*/
        let mut style_content = Style::new();
        style_content.set_bg_opa(OPA_COVER);
        style_content.set_bg_color(palette::lighten(Palette::Grey, 4));
        style_content.set_pad_all(16);
        style_content.set_border_width(0);
        style_content.set_radius(0);
        [style_win, style_header, style_content]
    })
}

/// `static lv_obj_t * win_create(lv_obj_t * parent)`
fn win_create(ui: &mut Ui, parent: Obj, styles: &[StyleId; 3]) -> Obj {
    let win = ui.obj_create(Some(parent));
    let (w, h) = (ui.obj_get_width(parent), ui.obj_get_height(parent));
    ui.obj_set_size(win, w, h);
    ui.obj_set_flex_flow(win, FlexFlow::Column);
    ui.obj_add_style(win, styles[0], StyleSelector::DEFAULT);

    let header = ui.obj_create(Some(win));
    let dpi = ui.display_get_dpi(ui.obj_get_display(win));
    ui.obj_set_size(header, coord::pct(100), dpi / 2);
    ui.obj_set_flex_flow(header, FlexFlow::Row);
    ui.obj_set_flex_align(header, FlexAlign::Start, FlexAlign::Center, FlexAlign::Center);
    ui.obj_add_style(header, styles[1], StyleSelector::DEFAULT);

    let content = ui.obj_create(Some(win));
    ui.obj_set_width(content, coord::pct(100));
    ui.obj_set_flex_grow(content, 1);
    ui.obj_add_style(content, styles[2], StyleSelector::DEFAULT);

    win
}

/// `static lv_obj_t * win_get_header(lv_obj_t * win)`
fn win_get_header(ui: &Ui, win: Obj) -> Option<Obj> {
    ui.obj_get_child(win, 0)
}

/// `static lv_obj_t * win_get_content(lv_obj_t * win)`
fn win_get_content(ui: &Ui, win: Obj) -> Option<Obj> {
    ui.obj_get_child(win, 1)
}

/// `static lv_obj_t * win_add_title(lv_obj_t * win, const char * txt)`
fn win_add_title(ui: &mut Ui, win: Obj, txt: &str) -> Option<Obj> {
    let header = win_get_header(ui, win)?;
    let title = Label::create(ui, header);
    title.set_long_mode(ui, LabelLongMode::Dots);
    title.set_text(ui, txt);
    ui.obj_set_flex_grow(title.obj(), 1);
    Some(title.obj())
}

/// `static lv_obj_t * win_add_button(lv_obj_t * win, const void * icon, int32_t btn_w)`
fn win_add_button(ui: &mut Ui, win: Obj, icon: Option<&'static str>, btn_w: i32) -> Option<Obj> {
    let header = win_get_header(ui, win)?;
    let btn = Button::create(ui, header).obj();
    ui.obj_set_size(btn, btn_w, coord::pct(100));

    /* LV_USE_IMAGE == 1 */
    if let Some(icon) = icon {
        let img = Image::create(ui, btn);
        img.set_src(ui, ImageSrc::Symbol(SharedRef::Static(icon)));
        ui.obj_align(img.obj(), Align::Center, 0, 0);
    }

    Some(btn)
}

/// `static void event_handler(lv_event_t * e)` with `LV_USE_LOG 0`.
fn event_handler(_ui: &mut Ui, _e: &mut Event<'_>) {
    /* LV_UNUSED(e); */
}

/// `lv_example_flex_win()`: a window built from a flex column.
pub fn lv_example_flex_win(ui: &mut Ui) {
    let styles = styles_init(ui);

    let win = win_create(ui, screen_active(ui), &styles);

    for (icon, w, title) in [
        (symbol::LEFT, 40, Some("A title")),
        (symbol::RIGHT, 40, None),
        (symbol::CLOSE, 60, None),
    ] {
        if let Some(btn) = win_add_button(ui, win, Some(icon), w) {
            ui.obj_add_event_cb(btn, Rc::new(event_handler), EventCode::CLICKED, None);
        }
        if let Some(title) = title {
            win_add_title(ui, win, title);
        }
    }

    let Some(content) = win_get_content(ui, win) else {
        return;
    }; /*Content can be added here*/
    let label = Label::create(ui, content);
    label.set_text(
        ui,
        "This is\n\
         a pretty\n\
         long text\n\
         to see how\n\
         the window\n\
         becomes\n\
         scrollable.\n\
         \n\
         \n\
         Some more\n\
         text to be\n\
         sure it\n\
         overflows. :)",
    );
}

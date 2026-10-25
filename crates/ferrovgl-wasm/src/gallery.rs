//! The demo scenes: a gallery of all 13 v0.1 widgets on the default theme
//! and the list of runnable scenes (gallery plus the ported
//! `lv_example_*` functions of `ferrovgl-examples`).

use std::rc::Rc;

use ferrovgl_core::draw::DrawBuf;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::{ImageSrc, SharedRef, SharedStr};
use ferrovgl_core::{
    ColorFormat, EventCode, Obj, ObjFlags, PointPrecise, State, StyleSelector, Ui, ValuePrecise, coord,
};
use ferrovgl_widgets::{
    Arc, Bar, Button, Buttonmatrix, ButtonmatrixMap, Checkbox, Image, Label, LabelLongMode, Led, Line,
    Slider, Spinner, Switch, Textarea,
};

/// Name of the gallery scene in [`scene_names`].
pub const GALLERY: &str = "gallery";

/// Names of the widgets shown by [`widget_gallery`], in order.
pub const WIDGETS: [&str; 13] = [
    "Label",
    "Button",
    "Image",
    "Bar",
    "Slider",
    "Switch",
    "Checkbox",
    "Arc",
    "Line",
    "LED",
    "Spinner",
    "Button matrix",
    "Text area",
];

/// Every scene the demo can show: [`GALLERY`] first, then each entry of
/// [`ferrovgl_examples::EXAMPLES`].
pub fn scene_names() -> Vec<&'static str> {
    std::iter::once(GALLERY)
        .chain(ferrovgl_examples::EXAMPLES.iter().map(|e| e.name))
        .collect()
}

/// Looks up a scene by name: [`GALLERY`] or anything
/// [`ferrovgl_examples::find`] accepts.
pub fn find_scene(name: &str) -> Option<fn(&mut Ui)> {
    if name == GALLERY || name.is_empty() {
        return Some(widget_gallery);
    }
    ferrovgl_examples::find(name).map(|e| e.run)
}

static LINE_POINTS: [PointPrecise; 5] = [
    PointPrecise::new(0 as ValuePrecise, 30 as ValuePrecise),
    PointPrecise::new(30 as ValuePrecise, 0 as ValuePrecise),
    PointPrecise::new(60 as ValuePrecise, 40 as ValuePrecise),
    PointPrecise::new(90 as ValuePrecise, 10 as ValuePrecise),
    PointPrecise::new(120 as ValuePrecise, 30 as ValuePrecise),
];

/// `{"1", "2", "3", "\n", "4", "5", "6", ""}`
fn button_map() -> ButtonmatrixMap {
    SharedRef::Shared(
        ["1", "2", "3", "\n", "4", "5", "6", ""]
            .into_iter()
            .map(SharedRef::Static)
            .collect::<Rc<[SharedStr]>>(),
    )
}

/// A 40x40 ARGB8888 badge drawn at runtime: a blue-to-teal disc with a
/// light ring, so the image card needs no asset file.
fn badge() -> Option<ImageSrc> {
    const SIZE: usize = 40;
    let mut buf = DrawBuf::create(SIZE as u32, SIZE as u32, ColorFormat::Argb8888, 0)?;
    let stride = usize::from(buf.header.stride);
    let pixels = buf.data_mut()?;
    let c = (SIZE as i32 - 1) * 2; // centre at 19.5, doubled to stay integral
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as i32 * 4 - c;
            let dy = y as i32 * 4 - c;
            let d2 = dx * dx + dy * dy; // (4 * distance)^2
            let r = SIZE as i32 * 2; // 4 * radius
            let px = if d2 > r * r {
                [0, 0, 0, 0]
            } else if d2 > (r - 12) * (r - 12) {
                [0xF0, 0xF0, 0xF0, 0xFF]
            } else {
                let t = (x + y) as u32 * 255 / (2 * SIZE as u32 - 2);
                // B, G, R, A in memory.
                [
                    0xF3 - (t * 0x50 / 255) as u8,
                    0x96 + (t * 0x40 / 255) as u8,
                    0x21,
                    0xFF,
                ]
            };
            let o = y * stride + x * 4;
            pixels[o..o + 4].copy_from_slice(&px);
        }
    }
    Some(ImageSrc::DrawBuf(Rc::new(buf)))
}

fn card(ui: &mut Ui, parent: Obj, title: &str) -> Obj {
    let card = ui.obj_create(Some(parent));
    ui.obj_set_size(card, 150, 140);
    ui.obj_set_style_pad_all(card, 10, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_gap(card, 8, StyleSelector::DEFAULT);
    ui.obj_set_flex_flow(card, FlexFlow::Column);
    ui.obj_set_flex_align(card, FlexAlign::Start, FlexAlign::Center, FlexAlign::Center);
    ui.obj_remove_flag(card, ObjFlags::SCROLLABLE);
    let caption = Label::create(ui, card);
    caption.set_text(ui, title);
    ui.obj_set_style_text_color(
        caption.obj(),
        palette::main(Palette::Grey),
        StyleSelector::DEFAULT,
    );
    card
}

/// Builds the widget gallery on the default display's active screen, like
/// an `lv_example_*` function: one card per widget in a wrapping flex row.
/// Focusable widgets join the default group (if one is set), so Tab and
/// the wheel walk through them.
pub fn widget_gallery(ui: &mut Ui) {
    let Some(screen) = ui.screen_active() else {
        return;
    };
    let sel = StyleSelector::DEFAULT;
    ui.obj_set_flex_flow(screen, FlexFlow::RowWrap);
    ui.obj_set_flex_align(screen, FlexAlign::SpaceEvenly, FlexAlign::Start, FlexAlign::Start);
    ui.obj_set_style_pad_all(screen, 8, sel);
    ui.obj_set_style_pad_gap(screen, 8, sel);

    let full = coord::pct(100);

    // Label
    let c = card(ui, screen, WIDGETS[0]);
    let label = Label::create(ui, c);
    label.set_long_mode(ui, LabelLongMode::Wrap);
    ui.obj_set_width(label.obj(), full);
    label.set_text(ui, "ferrovgl renders LVGL v9.6 in pure Rust.");

    // Button
    let c = card(ui, screen, WIDGETS[1]);
    let button = Button::create(ui, c);
    let text = Label::create(ui, button.obj());
    text.set_text(ui, "Click me");
    let status = Label::create(ui, c);
    status.set_text(ui, "Clicked 0x");
    let clicks = Rc::new(std::cell::Cell::new(0u32));
    ui.obj_add_event_cb(
        button.obj(),
        Rc::new(move |ui, _| {
            clicks.set(clicks.get().wrapping_add(1));
            status.set_text_fmt(ui, format_args!("Clicked {}x", clicks.get()));
        }),
        EventCode::CLICKED,
        None,
    );

    // Image
    let c = card(ui, screen, WIDGETS[2]);
    let image = Image::create(ui, c);
    if let Some(src) = badge() {
        image.set_src(ui, src);
    }
    let scaled = Image::create(ui, c);
    if let Some(src) = badge() {
        scaled.set_src(ui, src);
        scaled.set_scale(ui, 192);
    }

    // Bar
    let c = card(ui, screen, WIDGETS[3]);
    let bar = Bar::create(ui, c);
    ui.obj_set_width(bar.obj(), full);
    bar.set_value(ui, 70, false);
    let bar2 = Bar::create(ui, c);
    ui.obj_set_width(bar2.obj(), full);
    bar2.set_value(ui, 30, false);

    // Slider: drives a value label.
    let c = card(ui, screen, WIDGETS[4]);
    let slider = Slider::create(ui, c);
    ui.obj_set_width(slider.obj(), coord::pct(90));
    slider.set_value(ui, 40, false);
    let value = Label::create(ui, c);
    value.set_text(ui, "40");
    ui.obj_add_event_cb(
        slider.obj(),
        Rc::new(move |ui, _| {
            let v = slider.value(ui);
            value.set_text_fmt(ui, format_args!("{v}"));
        }),
        EventCode::VALUE_CHANGED,
        None,
    );

    // Switch
    let c = card(ui, screen, WIDGETS[5]);
    Switch::create(ui, c);
    let on = Switch::create(ui, c);
    ui.obj_add_state(on.obj(), State::CHECKED);

    // Checkbox
    let c = card(ui, screen, WIDGETS[6]);
    let apples = Checkbox::create(ui, c);
    apples.set_text(ui, "Apples");
    ui.obj_add_state(apples.obj(), State::CHECKED);
    let pears = Checkbox::create(ui, c);
    pears.set_text(ui, "Pears");

    // Arc
    let c = card(ui, screen, WIDGETS[7]);
    let arc = Arc::create(ui, c);
    ui.obj_set_size(arc.obj(), 80, 80);
    arc.set_value(ui, 60);

    // Line
    let c = card(ui, screen, WIDGETS[8]);
    let line = Line::create(ui, c);
    line.set_points(
        ui,
        Some(SharedRef::Static(&LINE_POINTS)),
        LINE_POINTS.len() as u32,
    );
    ui.obj_set_style_line_width(line.obj(), 4, sel);
    ui.obj_set_style_line_color(line.obj(), palette::main(Palette::Blue), sel);
    ui.obj_set_style_line_rounded(line.obj(), true, sel);

    // LED: click to toggle.
    let c = card(ui, screen, WIDGETS[9]);
    ui.obj_set_flex_flow(c, FlexFlow::Column);
    let led = Led::create(ui, c);
    led.set_color(ui, palette::main(Palette::Green));
    led.on(ui);
    ui.obj_add_flag(led.obj(), ObjFlags::CLICKABLE);
    ui.obj_add_event_cb(
        led.obj(),
        Rc::new(move |ui, _| led.toggle(ui)),
        EventCode::CLICKED,
        None,
    );
    let dim = Led::create(ui, c);
    dim.set_color(ui, palette::main(Palette::Orange));
    dim.set_brightness(ui, 120);

    // Spinner
    let c = card(ui, screen, WIDGETS[10]);
    let spinner = Spinner::create(ui, c);
    ui.obj_set_size(spinner.obj(), 70, 70);

    // Button matrix
    let c = card(ui, screen, WIDGETS[11]);
    let btnm = Buttonmatrix::create(ui, c);
    btnm.set_map(ui, button_map());
    ui.obj_set_size(btnm.obj(), full, 84);
    ui.obj_set_style_pad_all(btnm.obj(), 4, sel);
    ui.obj_set_style_pad_gap(btnm.obj(), 4, sel);

    // Text area
    let c = card(ui, screen, WIDGETS[12]);
    let ta = Textarea::create(ui, c);
    ta.set_one_line(ui, true);
    ta.set_placeholder_text(ui, Some("Type here"));
    ui.obj_set_width(ta.obj(), full);
}

//! Port of `examples/widgets/textarea/lv_example_textarea_cursor.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::style::BorderSide;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Part, State, Style, StyleSelector, Ui};
use ferrovgl_widgets::Textarea;

use crate::util::{screen_active, styles_once};

/// `style_block_cursor`, `style_bar_cursor`.
struct Styles;

/// `lv_example_textarea_cursor()`: block and bar cursor styles.
pub fn lv_example_textarea_cursor(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_block_cursor, style_bar_cursor] = styles_once::<Styles, 2>(ui, || {
        let mut style_block_cursor = Style::new();
        style_block_cursor.set_bg_opa((255 * 100 / 100) as u8);
        style_block_cursor.set_bg_color(Color::hex(0x2563eb));
        style_block_cursor.set_text_color(Color::hex(0xffffff));
        style_block_cursor.set_border_side(BorderSide::NONE);

        let mut style_bar_cursor = Style::new();
        style_bar_cursor.set_bg_opa((255 * 0 / 100) as u8);
        style_bar_cursor.set_border_side(BorderSide::LEFT);
        style_bar_cursor.set_border_width(2);
        style_bar_cursor.set_border_color(Color::hex(0xdc2626));
        [style_block_cursor, style_bar_cursor]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Block cursor, then bar cursor */
    for (text, style) in [("Block", style_block_cursor), ("Bar", style_bar_cursor)] {
        let textarea = Textarea::create(ui, screen);
        ui.obj_set_width(textarea.obj(), coord::pct(60));
        textarea.set_one_line(ui, true);
        textarea.set_text(ui, text);
        textarea.set_cursor_pos(ui, 2);
        ui.obj_set_state(textarea.obj(), State::FOCUSED, true);
        ui.obj_add_style(textarea.obj(), style, Part::CURSOR | State::FOCUSED);
    }
}

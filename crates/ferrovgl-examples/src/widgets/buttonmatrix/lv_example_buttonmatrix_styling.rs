//! Port of `examples/widgets/buttonmatrix/lv_example_buttonmatrix_styling.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Part, State, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Buttonmatrix, ButtonmatrixCtrl};

use crate::util::{btnm_map, screen_active, styles_once};

/// `style_bm_item`, `style_bm_item_checked`.
struct Styles;

/// `lv_example_buttonmatrix_styling()`: styled items with a checked state.
pub fn lv_example_buttonmatrix_styling(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_bm_item, style_bm_item_checked] = styles_once::<Styles, 2>(ui, || {
        let mut style_bm_item = Style::new();
        style_bm_item.set_bg_opa((255 * 100 / 100) as u8);
        style_bm_item.set_bg_color(Color::hex(0xf3f4f6));
        style_bm_item.set_radius(8);
        style_bm_item.set_border_color(Color::hex(0xd1d5db));
        style_bm_item.set_border_width(1);
        style_bm_item.set_text_color(Color::hex(0x111827));

        let mut style_bm_item_checked = Style::new();
        style_bm_item_checked.set_bg_color(Color::hex(0x6366f1));
        style_bm_item_checked.set_text_color(Color::hex(0xffffff));
        style_bm_item_checked.set_outline_color(Color::hex(0x6366f1));
        style_bm_item_checked.set_outline_width(3);
        style_bm_item_checked.set_outline_pad(2);
        style_bm_item_checked.set_outline_opa(80);
        [style_bm_item, style_bm_item_checked]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Weekday matrix with one-checked semantics + styled items */
    let buttonmatrix = Buttonmatrix::create(ui, screen);
    let obj = buttonmatrix.obj();
    ui.obj_set_size(obj, coord::pct(90), 80);
    buttonmatrix.set_map(ui, btnm_map(&["M", "T", "W", "T", "F", "S", "S"]));
    buttonmatrix.set_one_checked(ui, true);
    let buttonmatrix_ctrl_map_1 = [
        ButtonmatrixCtrl::CHECKABLE,
        ButtonmatrixCtrl::CHECKABLE,
        ButtonmatrixCtrl::CHECKABLE | ButtonmatrixCtrl::CHECKED,
        ButtonmatrixCtrl::CHECKABLE,
        ButtonmatrixCtrl::CHECKABLE,
        ButtonmatrixCtrl::CHECKABLE,
        ButtonmatrixCtrl::CHECKABLE,
    ];
    buttonmatrix.set_ctrl_map(ui, &buttonmatrix_ctrl_map_1);
    ui.obj_set_style_bg_opa(obj, (255 * 100 / 100) as u8, sel);
    ui.obj_set_style_bg_color(obj, Color::hex(0xffffff), sel);
    ui.obj_set_style_border_color(obj, Color::hex(0xe5e7eb), sel);
    ui.obj_set_style_border_width(obj, 1, sel);
    ui.obj_set_style_radius(obj, 10, sel);
    ui.obj_set_style_pad_all(obj, 6, sel);
    ui.obj_set_style_pad_row(obj, 6, sel);
    ui.obj_set_style_pad_column(obj, 6, sel);
    ui.obj_add_style(obj, style_bm_item, Part::ITEMS.into());
    ui.obj_add_style(obj, style_bm_item_checked, Part::ITEMS | State::CHECKED);
}

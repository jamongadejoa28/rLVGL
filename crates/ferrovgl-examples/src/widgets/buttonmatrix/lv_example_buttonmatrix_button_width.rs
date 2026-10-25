//! Port of `examples/widgets/buttonmatrix/lv_example_buttonmatrix_button_width.c`.

use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Ui};
use ferrovgl_widgets::{Buttonmatrix, ButtonmatrixCtrl};

use crate::util::{btnm_map, screen_active};

/// `lv_example_buttonmatrix_button_width()`: equal widths and a 1:2:3 split.
pub fn lv_example_buttonmatrix_button_width(ui: &mut Ui) {
    let screen = screen_active(ui);

    /* First row: equal widths. Second row: 1:2:3 split via width_1/2/3 tokens. */
    let buttonmatrix = Buttonmatrix::create(ui, screen);
    ui.obj_set_align(buttonmatrix.obj(), Align::Center);
    ui.obj_set_size(buttonmatrix.obj(), coord::pct(90), 140);
    buttonmatrix.set_map(ui, btnm_map(&["A", "B", "C", "\n", "x1", "x2", "x3"]));
    let buttonmatrix_ctrl_map_1 = [
        ButtonmatrixCtrl::NONE,
        ButtonmatrixCtrl::NONE,
        ButtonmatrixCtrl::NONE,
        ButtonmatrixCtrl::WIDTH_1,
        ButtonmatrixCtrl::WIDTH_2,
        ButtonmatrixCtrl::WIDTH_3,
    ];
    buttonmatrix.set_ctrl_map(ui, &buttonmatrix_ctrl_map_1);
}

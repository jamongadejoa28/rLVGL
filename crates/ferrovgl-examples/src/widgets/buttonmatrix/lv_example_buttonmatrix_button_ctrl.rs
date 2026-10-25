//! Port of `examples/widgets/buttonmatrix/lv_example_buttonmatrix_button_ctrl.c`.

use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Ui};
use ferrovgl_widgets::{Buttonmatrix, ButtonmatrixCtrl};

use crate::util::{btnm_map, screen_active};

/// `lv_example_buttonmatrix_button_ctrl()`: one row of differently flagged buttons.
pub fn lv_example_buttonmatrix_button_ctrl(ui: &mut Ui) {
    let screen = screen_active(ui);

    /* One row with four differently-flagged buttons */
    let buttonmatrix = Buttonmatrix::create(ui, screen);
    ui.obj_set_align(buttonmatrix.obj(), Align::Center);
    ui.obj_set_size(buttonmatrix.obj(), coord::pct(90), 80);
    buttonmatrix.set_map(ui, btnm_map(&["Normal", "Checked", "Disabled", "Hidden"]));
    let buttonmatrix_ctrl_map_1 = [
        ButtonmatrixCtrl::NONE,
        ButtonmatrixCtrl::CHECKABLE | ButtonmatrixCtrl::CHECKED,
        ButtonmatrixCtrl::DISABLED,
        ButtonmatrixCtrl::HIDDEN,
    ];
    buttonmatrix.set_ctrl_map(ui, &buttonmatrix_ctrl_map_1);
}

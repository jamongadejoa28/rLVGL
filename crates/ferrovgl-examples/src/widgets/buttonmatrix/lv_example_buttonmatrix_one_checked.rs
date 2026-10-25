//! Port of `examples/widgets/buttonmatrix/lv_example_buttonmatrix_one_checked.c`.

use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Ui};
use ferrovgl_widgets::{Buttonmatrix, ButtonmatrixCtrl};

use crate::util::{btnm_map, screen_active};

/// `lv_example_buttonmatrix_one_checked()`: a weekday selector with one checked button.
pub fn lv_example_buttonmatrix_one_checked(ui: &mut Ui) {
    let screen = screen_active(ui);

    /* Weekday selector with single-checked semantics */
    let buttonmatrix = Buttonmatrix::create(ui, screen);
    ui.obj_set_align(buttonmatrix.obj(), Align::Center);
    ui.obj_set_size(buttonmatrix.obj(), coord::pct(90), 80);
    buttonmatrix.set_one_checked(ui, true);
    buttonmatrix.set_map(ui, btnm_map(&["Mon", "Tue", "Wed", "Thu", "Fri"]));
    let buttonmatrix_ctrl_map_1 = [
        ButtonmatrixCtrl::CHECKABLE,
        ButtonmatrixCtrl::CHECKABLE | ButtonmatrixCtrl::CHECKED,
        ButtonmatrixCtrl::CHECKABLE,
        ButtonmatrixCtrl::CHECKABLE,
        ButtonmatrixCtrl::CHECKABLE,
    ];
    buttonmatrix.set_ctrl_map(ui, &buttonmatrix_ctrl_map_1);
}

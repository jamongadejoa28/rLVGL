//! Port of `examples/widgets/buttonmatrix/lv_example_buttonmatrix_popover.c`.

use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Ui};
use ferrovgl_widgets::{Buttonmatrix, ButtonmatrixCtrl};

use crate::util::{btnm_map, screen_active};

/// `lv_example_buttonmatrix_popover()`: a keyboard-style row with popovers.
pub fn lv_example_buttonmatrix_popover(ui: &mut Ui) {
    let screen = screen_active(ui);

    /* Keyboard-style row with popover enabled on every button */
    let buttonmatrix = Buttonmatrix::create(ui, screen);
    ui.obj_set_align(buttonmatrix.obj(), Align::Center);
    ui.obj_set_size(buttonmatrix.obj(), coord::pct(90), 80);
    buttonmatrix.set_map(ui, btnm_map(&["Q", "W", "E", "R", "T", "Y"]));
    let buttonmatrix_ctrl_map_1 = [ButtonmatrixCtrl::POPOVER; 6];
    buttonmatrix.set_ctrl_map(ui, &buttonmatrix_ctrl_map_1);
}

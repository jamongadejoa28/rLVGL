//! Port of `examples/widgets/buttonmatrix/lv_example_buttonmatrix_recolor.c`.

use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Ui};
use ferrovgl_widgets::{Buttonmatrix, ButtonmatrixCtrl};

use crate::util::{btnm_map, screen_active};

/// `lv_example_buttonmatrix_recolor()`: recolored words on the buttons.
pub fn lv_example_buttonmatrix_recolor(ui: &mut Ui) {
    let screen = screen_active(ui);

    /* Three buttons, each highlighting a different word with recolor */
    let buttonmatrix = Buttonmatrix::create(ui, screen);
    ui.obj_set_align(buttonmatrix.obj(), Align::Center);
    ui.obj_set_size(buttonmatrix.obj(), coord::pct(90), 80);
    buttonmatrix.set_map(ui, btnm_map(&["#ff0000 Stop#", "#e08800 Warn#", "#00a000 Go#"]));
    let buttonmatrix_ctrl_map_1 = [ButtonmatrixCtrl::RECOLOR; 3];
    buttonmatrix.set_ctrl_map(ui, &buttonmatrix_ctrl_map_1);
}

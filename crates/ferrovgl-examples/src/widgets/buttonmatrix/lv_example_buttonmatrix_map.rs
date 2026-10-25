//! Port of `examples/widgets/buttonmatrix/lv_example_buttonmatrix_map.c`.

use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Ui};
use ferrovgl_widgets::Buttonmatrix;

use crate::util::{btnm_map, screen_active};

/// `lv_example_buttonmatrix_map()`: a 3x3 numeric keypad.
pub fn lv_example_buttonmatrix_map(ui: &mut Ui) {
    let screen = screen_active(ui);

    /* 3x3 numeric keypad: rows split by '\n' inside the map */
    let buttonmatrix = Buttonmatrix::create(ui, screen);
    ui.obj_set_align(buttonmatrix.obj(), Align::Center);
    ui.obj_set_size(buttonmatrix.obj(), coord::pct(90), 200);
    buttonmatrix.set_map(
        ui,
        btnm_map(&["1", "2", "3", "\n", "4", "5", "6", "\n", "7", "8", "9"]),
    );
}

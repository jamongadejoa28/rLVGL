//! Port of `examples/widgets/textarea/lv_example_textarea_buttonmatrix.c`.

use alloc::rc::Rc;
use alloc::string::String;

use ferrovgl_core::{Align, Event, EventCode, EventParam, State, Ui};
use ferrovgl_widgets::{Buttonmatrix, Textarea};

use crate::util::{btnm_map, obj_data, screen_active, symbol, user_obj};

/// `static void textarea_event_handler(lv_event_t * e)`
fn textarea_event_handler(_ui: &mut Ui, e: &mut Event<'_>) {
    let _ta = e.target_obj();
    // LV_LOG_USER("Enter was pressed. The current text is: %s", ...): not
    // printed, the crate has no logger.
}

/// `static void btnm_event_handler(lv_event_t * e)`
fn btnm_event_handler(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(obj) = e.target_obj().and_then(|o| Buttonmatrix::from_obj(ui, o)) else {
        return;
    };
    let Some(ta) = user_obj(e) else { return };
    let Some(ta_widget) = Textarea::from_obj(ui, ta) else {
        return;
    };
    let selected = obj.selected_button(ui);
    let Some(txt) = obj.button_text(ui, selected).map(String::from) else {
        return;
    };

    if txt == symbol::BACKSPACE {
        ta_widget.delete_char(ui);
    } else if txt == symbol::NEW_LINE {
        ui.obj_send_event(ta, EventCode::READY, EventParam::None);
    } else {
        ta_widget.add_text(ui, &txt);
    }
}

/// `lv_example_textarea_buttonmatrix()`: a one-line text area fed by a keypad.
pub fn lv_example_textarea_buttonmatrix(ui: &mut Ui) {
    let ta = Textarea::create(ui, screen_active(ui));
    ta.set_one_line(ui, true);
    ui.obj_align(ta.obj(), Align::TopMid, 0, 10);
    ui.obj_add_event_cb(
        ta.obj(),
        Rc::new(textarea_event_handler),
        EventCode::READY,
        obj_data(ta.obj()),
    );
    ui.obj_add_state(ta.obj(), State::FOCUSED); /*To be sure the cursor is visible*/

    let btnm_map = btnm_map(&[
        "1",
        "2",
        "3",
        "\n",
        "4",
        "5",
        "6",
        "\n",
        "7",
        "8",
        "9",
        "\n",
        symbol::BACKSPACE,
        "0",
        symbol::NEW_LINE,
        "",
    ]);

    let btnm = Buttonmatrix::create(ui, screen_active(ui));
    ui.obj_set_size(btnm.obj(), 200, 150);
    ui.obj_align(btnm.obj(), Align::BottomMid, 0, -10);
    ui.obj_add_event_cb(
        btnm.obj(),
        Rc::new(btnm_event_handler),
        EventCode::VALUE_CHANGED,
        obj_data(ta.obj()),
    );
    ui.obj_set_click_focusable(btnm.obj(), false); /*To keep the text area focused on button clicks*/
    btnm.set_map(ui, btnm_map);
}

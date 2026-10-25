//! Deterministic input state ported from upstream `lv_test_indev.c`.

use std::cell::RefCell;
use std::rc::Rc;

use ferrovgl_core::group::GroupId;
use ferrovgl_core::indev::{IndevDriver, IndevId, IndevReadData, IndevState, IndevType};
use ferrovgl_core::{DisplayId, Obj, Ui};

use crate::TestDisplay;

/// One device's share of `lv_test_state_t` (`x_act`/`y_act`/`mouse_pressed`,
/// `key_act`/`key_pressed` or `diff_act`/`enc_pressed`).
#[derive(Default)]
struct State {
    /// `x_act`
    x_act: i32,
    /// `y_act`
    y_act: i32,
    /// `mouse_pressed`, `key_pressed` or `enc_pressed`.
    pressed: bool,
    /// `key_act`
    key_act: u32,
    /// `diff_act`
    diff_act: i32,
    /// The hardware button index of a [`IndevType::Button`] device, which
    /// upstream's test helpers do not provide.
    btn_id: u32,
}

impl State {
    fn indev_state(&self) -> IndevState {
        if self.pressed {
            IndevState::Pressed
        } else {
            IndevState::Released
        }
    }
}

struct Driver(Rc<RefCell<State>>, IndevType);

impl IndevDriver for Driver {
    fn read(&mut self, _: IndevId, data: &mut IndevReadData) {
        let mut state = self.0.borrow_mut();
        match self.1 {
            // `lv_test_mouse_read_cb()`
            IndevType::Pointer => {
                data.point.x = state.x_act;
                data.point.y = state.y_act;
                data.state = state.indev_state();
            }
            // `lv_test_keypad_read_cb()`
            IndevType::Keypad => {
                data.key = state.key_act;
                data.state = state.indev_state();
            }
            // `lv_test_encoder_read_cb()`: `enc_diff` is an `int16_t`, so the
            // `int32_t` `diff_act` is truncated by the implicit conversion.
            IndevType::Encoder => {
                data.enc_diff = state.diff_act as i16;
                data.state = state.indev_state();
                state.diff_act = 0;
            }
            // No upstream helper: a hardware button reports its index.
            IndevType::Button => {
                data.btn_id = state.btn_id;
                data.state = state.indev_state();
            }
            IndevType::None => {}
        }
    }
}

/// A driver state that is read by the real UI input timer.
///
/// Setters change the next sample, like upstream's test helpers. Call
/// [`TestDisplay::wait`] to advance timer/long-press/animation processing, or
/// [`Self::read`] to process one sample synchronously. Clones share driver
/// state; the device is owned and deleted by `Ui`, not by this handle.
#[derive(Clone)]
pub struct SyntheticInput {
    id: IndevId,
    state: Rc<RefCell<State>>,
}

impl SyntheticInput {
    /// Install a pointer, keypad, encoder or button input on a display.
    ///
    /// [`TestDisplay`] already installs upstream's three test devices; this
    /// is for additional devices, such as hardware buttons, that upstream's
    /// test helpers do not provide.
    pub fn new(ui: &mut Ui, display: DisplayId, kind: IndevType) -> Self {
        let input = Self::create(ui, kind);
        ui.indev_set_display(input.id, Some(display));
        input
    }

    /// `lv_indev_create()`, `lv_indev_set_type()` and
    /// `lv_indev_set_read_cb()` as in `lv_test_indev_create_all()`: the
    /// device stays on the default display.
    pub(crate) fn create(ui: &mut Ui, kind: IndevType) -> Self {
        let state = Rc::new(RefCell::new(State::default()));
        let id = ui.indev_create();
        ui.indev_set_type(id, kind);
        ui.indev_set_driver(id, Some(Box::new(Driver(state.clone(), kind))));
        Self { id, state }
    }

    /// Handle for configuring the underlying device through `Ui`.
    pub fn id(&self) -> IndevId {
        self.id
    }

    /// `lv_indev_read()`: read the current state immediately through the core
    /// event processor.
    pub fn read(&self, ui: &mut Ui) {
        ui.indev_read(self.id);
    }

    /// `lv_test_mouse_move_to()`: set the next pointer coordinates without
    /// advancing time.
    pub fn move_to(&self, x: i32, y: i32) {
        let mut state = self.state.borrow_mut();
        state.x_act = x;
        state.y_act = y;
    }

    /// `lv_test_mouse_move_by()`: move relative to the last pointer
    /// coordinates.
    pub fn move_by(&self, x: i32, y: i32) {
        let mut state = self.state.borrow_mut();
        state.x_act = state.x_act.wrapping_add(x);
        state.y_act = state.y_act.wrapping_add(y);
    }

    /// `lv_test_mouse_move_to_obj()`: move to the center of the object's
    /// current coordinates. Like upstream, the layout is not updated first,
    /// and an invalid object (upstream's `LV_CHECK_OBJ` `NULL` check) is
    /// ignored.
    pub fn move_to_obj(&self, ui: &Ui, obj: Obj) {
        if !ui.obj_is_valid(obj) {
            return;
        }
        let coords = ui.obj_get_coords(obj);
        let x = coords.x1 + ui.obj_get_width(obj) / 2;
        let y = coords.y1 + ui.obj_get_height(obj) / 2;
        self.move_to(x, y);
    }

    /// `lv_test_mouse_press()`, `lv_test_encoder_press()`: press the pointer,
    /// encoder switch, current key or selected button.
    pub fn press(&self) {
        self.state.borrow_mut().pressed = true;
    }

    /// `lv_test_mouse_release()`, `lv_test_key_release()`,
    /// `lv_test_encoder_release()`: release the pointer, encoder switch, key
    /// or button. A keypad keeps its last key.
    pub fn release(&self) {
        self.state.borrow_mut().pressed = false;
    }

    /// `lv_test_key_press()`: press a keypad key (control key or LVGL encoded
    /// character).
    pub fn key_press(&self, key: u32) {
        let mut state = self.state.borrow_mut();
        state.key_act = key;
        state.pressed = true;
    }

    /// `lv_test_encoder_add_diff()`: accumulate encoder steps. The next read
    /// reports them as one `int16_t` difference and clears them.
    pub fn add_diff(&self, diff: i32) {
        let mut state = self.state.borrow_mut();
        state.diff_act = state.diff_act.wrapping_add(diff);
    }

    /// Select a hardware button index from `Ui::indev_set_button_points`
    /// (no upstream test helper).
    pub fn set_button(&self, button: u32) {
        self.state.borrow_mut().btn_id = button;
    }
}

/// Upstream's pointer, keypad and encoder test devices
/// (`lv_test_indev_create_all()`).
///
/// [`TestDisplay`] creates them once, right after the display, like
/// `lv_test_init()`. The click/hit helpers use the same release/wait-50/
/// press/wait-50/release/wait-50 sequence as `lv_test_indev.c`.
#[derive(Clone)]
pub struct TestInputs {
    /// Synthetic mouse pointer.
    pub mouse: SyntheticInput,
    /// Synthetic keypad.
    pub keypad: SyntheticInput,
    /// Synthetic encoder.
    pub encoder: SyntheticInput,
}

impl TestInputs {
    /// `lv_test_indev_create_all()`: mouse, keypad and encoder, in that order.
    pub(crate) fn create_all(ui: &mut Ui) -> Self {
        let mouse = SyntheticInput::create(ui, IndevType::Pointer);
        let keypad = SyntheticInput::create(ui, IndevType::Keypad);
        let encoder = SyntheticInput::create(ui, IndevType::Encoder);
        Self {
            mouse,
            keypad,
            encoder,
        }
    }

    /// `lv_test_indev_get_indev()`: the device of an input type, if it is one
    /// of the three test devices.
    pub fn get_indev(&self, kind: IndevType) -> Option<IndevId> {
        match kind {
            IndevType::Pointer => Some(self.mouse.id()),
            IndevType::Keypad => Some(self.keypad.id()),
            IndevType::Encoder => Some(self.encoder.id()),
            _ => None,
        }
    }

    /// Attach the keypad and encoder to a focus group.
    pub fn set_group(&self, ui: &mut Ui, group: Option<GroupId>) {
        ui.indev_set_group(self.keypad.id(), group);
        ui.indev_set_group(self.encoder.id(), group);
    }

    /// `lv_test_mouse_click_at()`
    pub fn mouse_click_at(&self, display: &mut TestDisplay, x: i32, y: i32) {
        self.mouse.release();
        display.wait(50);
        self.mouse.move_to(x, y);
        self.mouse.press();
        display.wait(50);
        self.mouse.release();
        display.wait(50);
    }

    /// `lv_test_key_hit()`
    pub fn key_hit(&self, display: &mut TestDisplay, key: u32) {
        self.keypad.release();
        display.wait(50);
        self.keypad.key_press(key);
        display.wait(50);
        self.keypad.release();
        display.wait(50);
    }

    /// `lv_test_encoder_turn()`: add the steps and wait 50 milliseconds.
    pub fn encoder_turn(&self, display: &mut TestDisplay, steps: i32) {
        self.encoder.add_diff(steps);
        display.wait(50);
    }

    /// `lv_test_encoder_click()`
    pub fn encoder_click(&self, display: &mut TestDisplay) {
        self.encoder.release();
        display.wait(50);
        self.encoder.press();
        display.wait(50);
        self.encoder.release();
        display.wait(50);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(state: &Rc<RefCell<State>>, kind: IndevType, data: &mut IndevReadData) {
        let id = Ui::new(Default::default()).indev_create();
        Driver(state.clone(), kind).read(id, data);
    }

    #[test]
    fn read_callbacks_only_write_their_upstream_fields() {
        let state = Rc::new(RefCell::new(State {
            x_act: 3,
            y_act: 4,
            pressed: true,
            key_act: 65,
            diff_act: 70_000,
            btn_id: 2,
        }));
        let prefilled = IndevReadData {
            key: 9,
            ..IndevReadData::default()
        };

        let mut data = prefilled.clone();
        read(&state, IndevType::Pointer, &mut data);
        assert_eq!(
            (data.point.x, data.point.y, data.state),
            (3, 4, IndevState::Pressed)
        );
        assert_eq!((data.key, data.enc_diff, data.btn_id), (9, 0, 0));

        let mut data = prefilled.clone();
        read(&state, IndevType::Keypad, &mut data);
        assert_eq!((data.key, data.point.x, data.enc_diff), (65, 0, 0));

        // `data->enc_diff = _state.diff_act` truncates to `int16_t` and the
        // whole difference is cleared, with no `continue_reading`.
        let mut data = prefilled.clone();
        read(&state, IndevType::Encoder, &mut data);
        assert_eq!(data.enc_diff, 70_000_i32 as i16);
        assert_eq!((data.key, data.continue_reading), (9, false));
        assert_eq!(state.borrow().diff_act, 0);
        let mut data = prefilled;
        read(&state, IndevType::Encoder, &mut data);
        assert_eq!(data.enc_diff, 0);
    }
}

//! Deterministic input state modeled after upstream `lv_test_indev.c`.

use std::cell::RefCell;
use std::rc::Rc;

use ferrovgl_core::group::GroupId;
use ferrovgl_core::indev::{IndevDriver, IndevId, IndevReadData, IndevState, IndevType};
use ferrovgl_core::{DisplayId, Obj, Point, Ui};

use crate::TestDisplay;

#[derive(Default)]
struct State {
    sample: IndevReadData,
    encoder_diff: i32,
}

struct Driver(Rc<RefCell<State>>, IndevType);

impl IndevDriver for Driver {
    fn read(&mut self, _: IndevId, data: &mut IndevReadData) {
        let mut state = self.0.borrow_mut();
        data.state = state.sample.state;
        data.point = state.sample.point;
        if self.1 == IndevType::Keypad {
            data.key = state.sample.key;
        }
        data.btn_id = state.sample.btn_id;
        let diff = state.encoder_diff.clamp(i16::MIN.into(), i16::MAX.into()) as i16;
        data.enc_diff = diff;
        state.encoder_diff -= i32::from(diff);
        data.continue_reading = state.encoder_diff != 0;
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
    pub fn new(ui: &mut Ui, display: DisplayId, kind: IndevType) -> Self {
        let state = Rc::new(RefCell::new(State::default()));
        let id = ui.indev_create();
        ui.indev_set_type(id, kind);
        ui.indev_set_display(id, Some(display));
        ui.indev_set_driver(id, Some(Box::new(Driver(state.clone(), kind))));
        Self { id, state }
    }

    /// Handle for configuring the underlying device through `Ui`.
    pub fn id(&self) -> IndevId {
        self.id
    }

    /// Read the current state immediately through the core event processor.
    pub fn read(&self, ui: &mut Ui) {
        ui.indev_read(self.id);
    }

    /// Set the next pointer coordinates without advancing time.
    pub fn move_to(&self, x: i32, y: i32) {
        self.state.borrow_mut().sample.point = Point::new(x, y);
    }

    /// Move relative to the last pointer coordinates.
    pub fn move_by(&self, x: i32, y: i32) {
        let mut state = self.state.borrow_mut();
        state.sample.point.x += x;
        state.sample.point.y += y;
    }

    /// Move to the center of the object's current layout rectangle.
    pub fn move_to_obj(&self, ui: &mut Ui, obj: Obj) {
        ui.obj_update_layout(obj);
        if let Some(object) = ui.obj(obj) {
            self.move_to(
                object.coords.x1 + object.coords.width() / 2,
                object.coords.y1 + object.coords.height() / 2,
            );
        }
    }

    /// Press the pointer, encoder switch or currently selected button/key.
    pub fn press(&self) {
        self.state.borrow_mut().sample.state = IndevState::Pressed;
    }

    /// Release the pointer, encoder switch or currently selected button/key.
    pub fn release(&self) {
        self.state.borrow_mut().sample.state = IndevState::Released;
    }

    /// Press a keypad key (control key or LVGL encoded character).
    pub fn key_press(&self, key: u32) {
        let mut state = self.state.borrow_mut();
        state.sample.key = key;
        state.sample.state = IndevState::Pressed;
    }

    /// Accumulate encoder steps; each step is consumed exactly once.
    pub fn add_diff(&self, diff: i32) {
        let mut state = self.state.borrow_mut();
        state.encoder_diff = state.encoder_diff.saturating_add(diff);
    }

    /// Select a hardware button index from `Ui::indev_set_button_points`.
    pub fn set_button(&self, button: u32) {
        self.state.borrow_mut().sample.btn_id = button;
    }
}

/// Upstream's pointer, keypad and encoder test devices.
///
/// The click/hit helpers use the same release/wait-50/press/wait-50/release/
/// wait-50 sequence as `lv_test_indev.c`.
pub struct TestInputs {
    /// Synthetic mouse pointer.
    pub mouse: SyntheticInput,
    /// Synthetic keypad.
    pub keypad: SyntheticInput,
    /// Synthetic encoder.
    pub encoder: SyntheticInput,
}

impl TestInputs {
    pub(crate) fn new(ui: &mut Ui, display: DisplayId) -> Self {
        Self {
            mouse: SyntheticInput::new(ui, display, IndevType::Pointer),
            keypad: SyntheticInput::new(ui, display, IndevType::Keypad),
            encoder: SyntheticInput::new(ui, display, IndevType::Encoder),
        }
    }

    /// Attach the keypad and encoder to a focus group.
    pub fn set_group(&self, ui: &mut Ui, group: Option<GroupId>) {
        ui.indev_set_group(self.keypad.id(), group);
        ui.indev_set_group(self.encoder.id(), group);
    }

    /// Click at a coordinate using the upstream timing sequence.
    pub fn mouse_click_at(&self, display: &mut TestDisplay, x: i32, y: i32) {
        self.mouse.release();
        display.wait(50);
        self.mouse.move_to(x, y);
        self.mouse.press();
        display.wait(50);
        self.mouse.release();
        display.wait(50);
    }

    /// Hit a key using the upstream timing sequence.
    pub fn key_hit(&self, display: &mut TestDisplay, key: u32) {
        self.keypad.release();
        display.wait(50);
        self.keypad.key_press(key);
        display.wait(50);
        self.keypad.release();
        display.wait(50);
    }

    /// Turn the encoder and allow input processing for 50 milliseconds.
    pub fn encoder_turn(&self, display: &mut TestDisplay, steps: i32) {
        self.encoder.add_diff(steps);
        display.wait(50);
    }

    /// Click the encoder using the upstream timing sequence.
    pub fn encoder_click(&self, display: &mut TestDisplay) {
        self.encoder.release();
        display.wait(50);
        self.encoder.press();
        display.wait(50);
        self.encoder.release();
        display.wait(50);
    }
}

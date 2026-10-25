//! Pointer, keypad and encoder input devices fed by browser events.
//!
//! The devices run in `LV_INDEV_MODE_EVENT` like the upstream SDL drivers
//! (`lv_sdl_mouse.c`, `lv_sdl_keyboard.c`, `lv_sdl_mousewheel.c`): every
//! browser event updates the device sample and calls `lv_indev_read()`.

use std::cell::RefCell;
use std::rc::Rc;

use ferrovgl_core::indev::{IndevDriver, IndevId, IndevMode, IndevReadData, IndevState, IndevType};
use ferrovgl_core::{DisplayId, Point, Ui};

/// One browser input event, already mapped to display coordinates and
/// LVGL keys (see [`crate::geometry`] and [`crate::keys`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    /// Primary button / touch / pen down at a display position
    /// (`SDL_MOUSEBUTTONDOWN` with `SDL_BUTTON_LEFT`).
    PointerDown(Point),
    /// Pointer motion (`SDL_MOUSEMOTION`).
    PointerMove(Point),
    /// Primary button released; keeps the last position (`SDL_MOUSEBUTTONUP`).
    PointerUp,
    /// The pointer left the canvas or was cancelled
    /// (`SDL_WINDOWEVENT_LEAVE`: release without moving).
    PointerLeave,
    /// One key press with its synthetic release (packed UTF-8 or `LV_KEY_*`).
    Key(u32),
    /// Encoder rotation in steps (`SDL_MOUSEWHEEL`).
    Wheel(i16),
    /// Middle button pressed (`true`) or released (`false`): the encoder
    /// button (`SDL_BUTTON_MIDDLE`).
    EncoderButton(bool),
    /// The canvas lost keyboard focus: release pointer and encoder so no
    /// press stays stuck.
    Blur,
}

#[derive(Default)]
struct Sample {
    point: Point,
    state: IndevState,
    key: u32,
    diff: i16,
}

struct Driver(Rc<RefCell<Sample>>, IndevType);

impl IndevDriver for Driver {
    fn read(&mut self, _: IndevId, data: &mut IndevReadData) {
        let mut sample = self.0.borrow_mut();
        data.point = sample.point;
        data.state = sample.state;
        if self.1 == IndevType::Keypad {
            data.key = sample.key;
        }
        data.enc_diff = std::mem::take(&mut sample.diff);
    }
}

/// The three input devices of a browser display.
pub struct Inputs {
    /// Pointer device (mouse, touch, pen).
    pub pointer: IndevId,
    /// Keyboard device.
    pub keypad: IndevId,
    /// Wheel / middle button encoder.
    pub encoder: IndevId,
    mouse: Rc<RefCell<Sample>>,
    keyboard: Rc<RefCell<Sample>>,
    wheel: Rc<RefCell<Sample>>,
}

impl Inputs {
    /// Creates the event-mode pointer, keypad and encoder devices on `display`.
    pub fn new(ui: &mut Ui, display: DisplayId) -> Self {
        fn create(ui: &mut Ui, display: DisplayId, ty: IndevType) -> (IndevId, Rc<RefCell<Sample>>) {
            let sample = Rc::new(RefCell::new(Sample::default()));
            let id = ui.indev_create();
            ui.indev_set_type(id, ty);
            ui.indev_set_display(id, Some(display));
            ui.indev_set_driver(id, Some(Box::new(Driver(sample.clone(), ty))));
            ui.indev_set_mode(id, IndevMode::Event);
            (id, sample)
        }
        let (pointer, mouse) = create(ui, display, IndevType::Pointer);
        let (keypad, keyboard) = create(ui, display, IndevType::Keypad);
        let (encoder, wheel) = create(ui, display, IndevType::Encoder);
        Self {
            pointer,
            keypad,
            encoder,
            mouse,
            keyboard,
            wheel,
        }
    }

    /// Applies one event and reads the affected device.
    pub fn handle(&self, ui: &mut Ui, event: InputEvent) {
        match event {
            InputEvent::PointerDown(point) => {
                {
                    let mut s = self.mouse.borrow_mut();
                    s.state = IndevState::Pressed;
                    s.point = point;
                }
                self.read(ui, self.pointer);
            }
            InputEvent::PointerMove(point) => {
                self.mouse.borrow_mut().point = point;
                self.read(ui, self.pointer);
            }
            InputEvent::PointerUp | InputEvent::PointerLeave => {
                self.mouse.borrow_mut().state = IndevState::Released;
                self.read(ui, self.pointer);
            }
            InputEvent::Key(key) => {
                // `sdl_keyboard_read()` reports the press and then a dummy
                // release; event mode delivers both right away.
                {
                    let mut s = self.keyboard.borrow_mut();
                    s.key = key;
                    s.state = IndevState::Pressed;
                }
                self.read(ui, self.keypad);
                self.keyboard.borrow_mut().state = IndevState::Released;
                self.read(ui, self.keypad);
            }
            InputEvent::Wheel(diff) => {
                {
                    let mut s = self.wheel.borrow_mut();
                    s.diff = s.diff.saturating_add(diff);
                }
                self.read(ui, self.encoder);
            }
            InputEvent::EncoderButton(pressed) => {
                self.wheel.borrow_mut().state = if pressed {
                    IndevState::Pressed
                } else {
                    IndevState::Released
                };
                self.read(ui, self.encoder);
            }
            InputEvent::Blur => {
                self.mouse.borrow_mut().state = IndevState::Released;
                self.wheel.borrow_mut().state = IndevState::Released;
                self.read(ui, self.pointer);
                self.read(ui, self.encoder);
            }
        }
    }

    fn read(&self, ui: &mut Ui, id: IndevId) {
        if ui.indev(id).is_some() {
            ui.indev_read(id);
        }
    }
}

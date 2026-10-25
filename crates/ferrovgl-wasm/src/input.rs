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

/// `MouseEvent.buttons` bit of the primary (left) button, touch contact or
/// pen tip.
pub const BUTTONS_LEFT: u16 = 1;
/// `MouseEvent.buttons` bit of the auxiliary (middle) button.
pub const BUTTONS_MIDDLE: u16 = 4;

/// The events for a change of the pressed buttons from `prev` to `now`
/// (`MouseEvent.buttons` bits; only [`BUTTONS_LEFT`] and [`BUTTONS_MIDDLE`]
/// matter), left first, then middle.
///
/// Browsers fire `pointerdown` only for the first pressed button and
/// `pointerup` only for the last released one; other changes arrive as a
/// `pointermove`. SDL reports every left or middle change as its own
/// `SDL_MOUSEBUTTONDOWN`/`UP` (`lv_sdl_mouse.c`, `lv_sdl_mousewheel.c`), so
/// all three handlers compare the `buttons` bits with the last known ones.
/// `at` is the position of a left press.
pub fn button_changes(prev: u16, now: u16, at: Point) -> impl Iterator<Item = InputEvent> {
    let changed = |bit: u16| (prev & bit != now & bit).then_some(now & bit != 0);
    let left = changed(BUTTONS_LEFT).map(|down| {
        if down {
            InputEvent::PointerDown(at)
        } else {
            InputEvent::PointerUp
        }
    });
    let middle = changed(BUTTONS_MIDDLE).map(InputEvent::EncoderButton);
    left.into_iter().chain(middle)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn changes(prev: u16, now: u16) -> Vec<InputEvent> {
        button_changes(prev, now, Point::new(3, 4)).collect()
    }

    #[test]
    fn chorded_buttons_report_every_change() {
        let p = Point::new(3, 4);
        assert_eq!(changes(0, 0), []);
        assert_eq!(changes(0, BUTTONS_LEFT), [InputEvent::PointerDown(p)]);
        // Left held, middle pressed (a `pointermove` in the browser).
        assert_eq!(
            changes(BUTTONS_LEFT, BUTTONS_LEFT | BUTTONS_MIDDLE),
            [InputEvent::EncoderButton(true)]
        );
        // Left released while middle stays down: still a pointer release.
        assert_eq!(
            changes(BUTTONS_LEFT | BUTTONS_MIDDLE, BUTTONS_MIDDLE),
            [InputEvent::PointerUp]
        );
        assert_eq!(changes(BUTTONS_MIDDLE, 0), [InputEvent::EncoderButton(false)]);
        assert_eq!(
            changes(0, BUTTONS_LEFT | BUTTONS_MIDDLE),
            [InputEvent::PointerDown(p), InputEvent::EncoderButton(true)]
        );
        // The secondary button (2) is ignored.
        assert_eq!(changes(0, 2), []);
        assert_eq!(changes(2 | BUTTONS_LEFT, 2), [InputEvent::PointerUp]);
    }
}

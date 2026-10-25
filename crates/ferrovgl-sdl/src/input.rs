//! SDL mouse, keyboard and wheel adapters, following LVGL's SDL event drivers.

use std::cell::RefCell;
use std::rc::Rc;

use ferrovgl_core::group::*;
use ferrovgl_core::indev::{IndevDriver, IndevId, IndevMode, IndevReadData, IndevState, IndevType};
use ferrovgl_core::{DisplayId, Point, Ui};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::MouseButton;

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

pub(super) struct Inputs {
    pub pointer: IndevId,
    pub keypad: IndevId,
    pub encoder: IndevId,
    mouse: Rc<RefCell<Sample>>,
    keyboard: Rc<RefCell<Sample>>,
    wheel: Rc<RefCell<Sample>>,
}

impl Inputs {
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

    pub fn handle(&self, ui: &mut Ui, event: &Event, window_id: u32) {
        match *event {
            Event::MouseMotion {
                window_id: id, x, y, ..
            } if id == window_id => {
                self.mouse.borrow_mut().point = Point::new(x, y);
                self.read(ui, self.pointer);
            }
            Event::MouseButtonDown {
                window_id: id,
                mouse_btn,
                x,
                y,
                ..
            }
            | Event::MouseButtonUp {
                window_id: id,
                mouse_btn,
                x,
                y,
                ..
            } if id == window_id => {
                let state = if matches!(event, Event::MouseButtonDown { .. }) {
                    IndevState::Pressed
                } else {
                    IndevState::Released
                };
                if mouse_btn == MouseButton::Left {
                    {
                        let mut sample = self.mouse.borrow_mut();
                        // LVGL keeps the last motion/press position on release.
                        if state == IndevState::Pressed {
                            sample.point = Point::new(x, y);
                        }
                        sample.state = state;
                    }
                    self.read(ui, self.pointer);
                } else if mouse_btn == MouseButton::Middle {
                    self.wheel.borrow_mut().state = state;
                    self.read(ui, self.encoder);
                }
            }
            Event::MouseWheel { window_id: id, y, .. } if id == window_id => {
                // Match LVGL's encoder mode: one wheel unit is -y.
                let diff = -i64::from(y);
                self.wheel.borrow_mut().diff = diff.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16;
                self.read(ui, self.encoder);
            }
            Event::KeyDown {
                window_id: id,
                keycode: Some(key),
                keymod,
                ..
            } if id == window_id => {
                if let Some(key) = control_key(key, keymod) {
                    self.key(ui, key);
                }
            }
            Event::TextInput {
                window_id: id,
                ref text,
                ..
            } if id == window_id => {
                for character in text.chars() {
                    // LVGL SDL carries the UTF-8 bytes in the native u32, not
                    // a Unicode scalar. ASCII and control keys coincide.
                    let mut bytes = [0; 4];
                    character.encode_utf8(&mut bytes);
                    self.key(ui, u32::from_ne_bytes(bytes));
                }
            }
            Event::Window {
                window_id: id,
                win_event: WindowEvent::Leave | WindowEvent::FocusLost,
                ..
            } if id == window_id => {
                // Avoid a stuck press when release happens outside the window.
                self.mouse.borrow_mut().state = IndevState::Released;
                self.wheel.borrow_mut().state = IndevState::Released;
                self.read(ui, self.pointer);
                self.read(ui, self.encoder);
            }
            _ => {}
        }
    }

    fn key(&self, ui: &mut Ui, key: u32) {
        {
            let mut sample = self.keyboard.borrow_mut();
            sample.key = key;
            sample.state = IndevState::Pressed;
        }
        self.read(ui, self.keypad);
        self.keyboard.borrow_mut().state = IndevState::Released;
        self.read(ui, self.keypad);
    }

    fn read(&self, ui: &mut Ui, id: IndevId) {
        if ui.indev(id).is_some() {
            ui.indev_read(id);
        }
    }
}

fn control_key(key: Keycode, modifiers: Mod) -> Option<u32> {
    Some(match key {
        Keycode::Right | Keycode::KpPlus => KEY_RIGHT,
        Keycode::Left | Keycode::KpMinus => KEY_LEFT,
        Keycode::Up => KEY_UP,
        Keycode::Down => KEY_DOWN,
        Keycode::Escape => KEY_ESC,
        Keycode::Backspace => KEY_BACKSPACE,
        Keycode::Delete => KEY_DEL,
        Keycode::Return | Keycode::KpEnter => KEY_ENTER,
        Keycode::Tab if modifiers.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) => KEY_PREV,
        Keycode::Tab | Keycode::PageDown => KEY_NEXT,
        Keycode::PageUp => KEY_PREV,
        Keycode::Home => KEY_HOME,
        Keycode::End => KEY_END,
        _ => return None,
    })
}

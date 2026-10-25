//! Ported from LVGL v9.6.0: `src/widgets/led/lv_led.c`.
//!
//! Also covers `include/lvgl/widgets/lv_led.h` and
//! `src/widgets/led/lv_led_private.h`. `LV_USE_OBJ_PROPERTY` is disabled in
//! the parity configuration, so the property table is not provided.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::{Color, Event, Obj, ObjClass, Ui};

/// `LV_LED_BRIGHT_MIN`: brightness when the LED is off.
pub const LED_BRIGHT_MIN: u8 = 80;
/// `LV_LED_BRIGHT_MAX`: brightness when the LED is on.
pub const LED_BRIGHT_MAX: u8 = 255;

/// `lv_led_t` without its `lv_obj_t` header. Zeroed like `lv_malloc_zeroed`;
/// the constructor sets the initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct LedState {
    /// `color`
    pub(crate) color: Color,
    /// `bright`: current brightness of the LED (0..255).
    pub(crate) bright: u8,
}

ferrovgl_core::impl_widget_state!(LedState);

/// `lv_led_class`.
pub static LED_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_led",
    new_state: || Box::new(LedState::default()),
    constructor: Some(constructor),
    destructor: None,
    event_cb: Some(led_event),
    width_def: DPI_DEF / 5,
    height_def: DPI_DEF / 5,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::Inherit,
    theme_inheritable: false,
};

/// A copyable LED handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Led(Obj);

impl Led {
    /// `lv_led_create()`. `None` creates a LED screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&LED_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the LED class.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &LED_CLASS)).then_some(Self(obj))
    }

    /// `lv_led_set_color()`
    pub fn set_color(self, ui: &mut Ui, color: Color) {
        todo!()
    }

    /// `lv_led_set_brightness()`: [`LED_BRIGHT_MIN`] (max. dark) ..
    /// [`LED_BRIGHT_MAX`] (max. light).
    pub fn set_brightness(self, ui: &mut Ui, bright: u8) {
        todo!()
    }

    /// `lv_led_on()`: light on the LED.
    pub fn on(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_led_off()`: light off the LED.
    pub fn off(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_led_toggle()`: toggle the state of the LED.
    pub fn toggle(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_led_get_brightness()`: 0 (max. dark) .. 255 (max. light).
    pub fn brightness(self, ui: &Ui) -> u8 {
        todo!()
    }

    /// `lv_led_get_color()`
    pub fn color(self, ui: &Ui) -> Color {
        todo!()
    }
}

impl From<Led> for Obj {
    fn from(led: Led) -> Self {
        led.obj()
    }
}

/// `lv_led_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_led_event()`
fn led_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    todo!()
}

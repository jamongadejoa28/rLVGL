//! Ported from LVGL v9.6.0: `src/widgets/led/lv_led.c`.
//!
//! Also covers `include/lvgl/widgets/lv_led.h` and
//! `src/widgets/led/lv_led_private.h`. `LV_USE_OBJ_PROPERTY` is disabled in
//! the parity configuration, so the property table is not provided.

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::draw::RectDsc;
use ferrovgl_core::{Color, Event, EventCode, LvResult, Obj, ObjClass, Part, Ui};

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
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        let Some(led) = state_mut(ui, self.0) else {
            return;
        };
        led.color = color;
        ui.obj_invalidate(self.0);
    }

    /// `lv_led_set_brightness()`: [`LED_BRIGHT_MIN`] (max. dark) ..
    /// [`LED_BRIGHT_MAX`] (max. light).
    pub fn set_brightness(self, ui: &mut Ui, bright: u8) {
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        let Some(led) = state_mut(ui, self.0) else {
            return;
        };
        if led.bright == bright {
            return;
        }

        // `LV_CLAMP(LV_LED_BRIGHT_MIN, bright, LV_LED_BRIGHT_MAX)`; the
        // `LV_LED_BRIGHT_MAX` bound is the full `uint8_t` range.
        led.bright = bright.clamp(LED_BRIGHT_MIN, LED_BRIGHT_MAX);

        /*Invalidate the object there fore it will be redrawn*/
        ui.obj_invalidate(self.0);
    }

    /// `lv_led_on()`: light on the LED.
    pub fn on(self, ui: &mut Ui) {
        // LV_CHECK_OBJ(led, MY_CLASS, return);
        if state(ui, self.0).is_none() {
            return;
        }

        self.set_brightness(ui, LED_BRIGHT_MAX);
    }

    /// `lv_led_off()`: light off the LED.
    pub fn off(self, ui: &mut Ui) {
        // LV_CHECK_OBJ(led, MY_CLASS, return);
        if state(ui, self.0).is_none() {
            return;
        }

        self.set_brightness(ui, LED_BRIGHT_MIN);
    }

    /// `lv_led_toggle()`: toggle the state of the LED.
    pub fn toggle(self, ui: &mut Ui) {
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        if state(ui, self.0).is_none() {
            return;
        }

        let bright = self.brightness(ui);
        if i32::from(bright) > (i32::from(LED_BRIGHT_MIN) + i32::from(LED_BRIGHT_MAX)) >> 1 {
            self.off(ui);
        } else {
            self.on(ui);
        }
    }

    /// `lv_led_get_brightness()`: 0 (max. dark) .. 255 (max. light).
    pub fn brightness(self, ui: &Ui) -> u8 {
        // LV_CHECK_OBJ(obj, MY_CLASS, return 0);
        state(ui, self.0).map_or(0, |led| led.bright)
    }

    /// `lv_led_get_color()`
    pub fn color(self, ui: &Ui) -> Color {
        // LV_CHECK_OBJ(obj, MY_CLASS, return lv_color_black());
        state(ui, self.0).map_or(Color::BLACK, |led| led.color)
    }
}

impl From<Led> for Obj {
    fn from(led: Led) -> Self {
        led.obj()
    }
}

fn state(ui: &Ui, obj: Obj) -> Option<&LedState> {
    ui.obj(obj)?.widget.get::<LedState>()
}

fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut LedState> {
    ui.obj_mut(obj)?.widget.get_mut::<LedState>()
}

/// `lv_led_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    let color = ui.theme_get_color_primary(Some(obj));
    if let Some(led) = state_mut(ui, obj) {
        led.color = color;
        led.bright = LED_BRIGHT_MAX;
    }
}

/// `lv_led_event()`
fn led_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    /* Call the ancestor's event handler */
    let code = event.code();
    if code != EventCode::DRAW_MAIN
        && code != EventCode::DRAW_MAIN_END
        && ui.obj_event_base(&LED_CLASS, event) != LvResult::Ok
    {
        return;
    }

    let Some(obj) = event.current_target_obj() else {
        return;
    };
    if code == EventCode::DRAW_MAIN {
        draw_main(ui, obj, event);
    }
}

/// The `LV_EVENT_DRAW_MAIN` branch of `lv_led_event()`.
fn draw_main(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    /*Make darker colors in a temporary style according to the brightness*/
    let Some(led) = state(ui, obj) else {
        return;
    };
    let (led_color, led_bright) = (led.color, led.bright);
    let Some(layer) = event.layer() else {
        return;
    };

    let mut rect_dsc = RectDsc::new();
    rect_dsc.base.layer = Some(layer);
    ui.obj_init_draw_rect_dsc(obj, Part::MAIN, &mut rect_dsc);

    /*Use the original colors brightness to modify color->led*/
    let black = Color::BLACK;
    rect_dsc.bg_color = Color::mix(led_color, black, rect_dsc.bg_color.brightness());
    rect_dsc.bg_grad.stops[0].color =
        Color::mix(led_color, black, rect_dsc.bg_grad.stops[0].color.brightness());
    rect_dsc.bg_grad.stops[1].color =
        Color::mix(led_color, black, rect_dsc.bg_grad.stops[1].color.brightness());
    rect_dsc.shadow_color = Color::mix(led_color, black, rect_dsc.shadow_color.brightness());
    rect_dsc.border_color = Color::mix(led_color, black, rect_dsc.border_color.brightness());
    rect_dsc.outline_color = Color::mix(led_color, black, rect_dsc.outline_color.brightness());

    /*Mix. the color with black proportionally with brightness*/
    rect_dsc.bg_color = Color::mix(rect_dsc.bg_color, black, led_bright);
    rect_dsc.bg_grad.stops[0].color = Color::mix(rect_dsc.bg_grad.stops[0].color, black, led_bright);
    rect_dsc.bg_grad.stops[1].color = Color::mix(rect_dsc.bg_grad.stops[1].color, black, led_bright);
    rect_dsc.border_color = Color::mix(rect_dsc.border_color, black, led_bright);
    rect_dsc.shadow_color = Color::mix(rect_dsc.shadow_color, black, led_bright);
    rect_dsc.outline_color = Color::mix(rect_dsc.outline_color, black, led_bright);

    /*Set the current shadow width according to brightness proportionally between LV_LED_BRIGHT_OFF
     * and LV_LED_BRIGHT_ON*/
    // Integer promotion: `uint8_t - int` and the `int32_t` products are `int`.
    let bright_ofs = i32::from(led_bright) - i32::from(LED_BRIGHT_MIN);
    let bright_range = i32::from(LED_BRIGHT_MAX) - i32::from(LED_BRIGHT_MIN);
    rect_dsc.shadow_width = (bright_ofs * rect_dsc.shadow_width) / bright_range;
    rect_dsc.shadow_spread = (bright_ofs * rect_dsc.shadow_spread) / bright_range;

    let coords = ui.obj_get_coords(obj);
    ui.draw_rect(layer, &rect_dsc, &coords);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::UiConfig;

    fn setup() -> (Ui, Obj) {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        (ui, screen)
    }

    #[test]
    fn constructor_sets_defaults() {
        let (mut ui, screen) = setup();
        let led = Led::create(&mut ui, screen);
        assert!(ui.obj_check_type(led.obj(), &LED_CLASS));
        assert_eq!(led.brightness(&ui), LED_BRIGHT_MAX);
        assert_eq!(led.color(&ui), ui.theme_get_color_primary(Some(led.obj())));
        assert_eq!(Led::from_obj(&ui, led.obj()), Some(led));
        assert_eq!(Led::from_obj(&ui, screen), None);
    }

    #[test]
    fn brightness_is_clamped_and_toggles() {
        let (mut ui, screen) = setup();
        let led = Led::create(&mut ui, screen);
        led.set_brightness(&mut ui, 0);
        assert_eq!(led.brightness(&ui), LED_BRIGHT_MIN);
        led.toggle(&mut ui);
        assert_eq!(led.brightness(&ui), LED_BRIGHT_MAX);
        led.set_brightness(&mut ui, 168);
        led.toggle(&mut ui);
        assert_eq!(led.brightness(&ui), LED_BRIGHT_MIN);
        led.set_brightness(&mut ui, 167);
        led.toggle(&mut ui);
        assert_eq!(led.brightness(&ui), LED_BRIGHT_MAX);
    }

    #[test]
    fn non_led_objects_are_ignored() {
        let (mut ui, screen) = setup();
        let not_led = Led(screen);
        not_led.set_brightness(&mut ui, 100);
        not_led.toggle(&mut ui);
        assert_eq!(not_led.brightness(&ui), 0);
        assert_eq!(not_led.color(&ui), Color::BLACK);
    }
}

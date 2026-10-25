//! Real Button/default-theme scenarios from the pinned LVGL v9.6.0 tests.
//! Screenshots use unmodified upstream references and the public object/input
//! pipeline. The separate simple-theme assertions cover only implemented classes.

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::group::{KEY_ENTER, KEY_NEXT};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, EventCode, Part, State, StyleSelector};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Button, Label};

const DEFAULT: StyleSelector = StyleSelector::DEFAULT;

/// `test_indev.c:246..295`, including the original 50/1000ms timing and both
/// press-lock settings. These scenes require actual inherited button behavior.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn button_scroll_between_press_locked_and_unlocked_buttons() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let inputs = display.create_inputs();
    let screen = display.screen().unwrap();
    let button1 = Button::create(&mut display.ui, screen);
    display.ui.obj_set_press_lock(button1.obj(), false);
    display.ui.obj_set_size(button1.obj(), 120, 100);
    display.ui.obj_set_pos(button1.obj(), 300, 200);
    let button2 = Button::create(&mut display.ui, screen);
    display
        .ui
        .obj_set_style_bg_color(button2.obj(), palette::main(Palette::Red), DEFAULT);
    display.ui.obj_set_size(button2.obj(), 120, 100);
    display.ui.obj_set_pos(button2.obj(), 900, 200);
    let counts: [Rc<Cell<u32>>; 4] = core::array::from_fn(|_| Rc::new(Cell::new(0)));
    for ((button, code), counter) in [
        (button1, EventCode::PRESSED),
        (button1, EventCode::PRESS_LOST),
        (button2, EventCode::PRESSED),
        (button2, EventCode::PRESS_LOST),
    ]
    .into_iter()
    .zip(&counts)
    {
        let counter = counter.clone();
        display.ui.obj_add_event_cb(
            button.obj(),
            Rc::new(move |_, _| counter.set(counter.get() + 1)),
            code,
            None,
        );
    }
    display.screenshot_compare("scroll_initial.png").unwrap();
    inputs.mouse.move_to(350, 250);
    inputs.mouse.press();
    display.wait(50);
    assert_eq!(counts[0].get(), 1);
    inputs.mouse.move_by(-300, 0);
    display.wait(50);
    inputs.mouse.release();
    display.wait(1000);
    assert_eq!(counts[1].get(), 1);
    display.screenshot_compare("scroll_after.png").unwrap();
    inputs.mouse.move_to(750, 250);
    inputs.mouse.press();
    display.wait(50);
    assert_eq!(counts[2].get(), 1);
    inputs.mouse.move_by(300, 0);
    display.wait(50);
    inputs.mouse.release();
    display.wait(1000);
    assert_eq!(counts[3].get(), 0);
    display.screenshot_compare("scroll_initial.png").unwrap();
}

/// `widgets/test_obj_pos.c:53..83`: the parent's content size, the button's
/// percentage size and its label's size interact with minimum style values.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn button_minimum_size_fixed_and_content() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let parent = display.ui.obj_create(display.screen());
    display
        .ui
        .obj_set_size(parent, coord::SIZE_CONTENT, coord::SIZE_CONTENT);
    let button = Button::create(&mut display.ui, parent);
    let label = Label::create(&mut display.ui, button.obj());
    label.set_text(&mut display.ui, "Button");
    display
        .ui
        .obj_set_size(button.obj(), coord::pct(100), coord::pct(100));
    display
        .ui
        .obj_set_size(label.obj(), coord::SIZE_CONTENT, coord::SIZE_CONTENT);
    display
        .screenshot_compare("widgets/obj_pos_no_min_size.png")
        .unwrap();
    assert!(display.ui.obj_is_width_min(button.obj()));
    assert!(display.ui.obj_is_height_min(button.obj()));
    display.ui.obj_set_style_min_width(button.obj(), 200, DEFAULT);
    display.ui.obj_set_style_min_height(button.obj(), 300, DEFAULT);
    display
        .screenshot_compare("widgets/obj_pos_fixed_min_size.png")
        .unwrap();
    assert_eq!(display.ui.obj_get_width(button.obj()), 200);
    assert_eq!(display.ui.obj_get_height(button.obj()), 300);
    display
        .ui
        .obj_set_style_min_width(button.obj(), coord::SIZE_CONTENT, DEFAULT);
    display
        .ui
        .obj_set_style_min_height(button.obj(), coord::SIZE_CONTENT, DEFAULT);
    display
        .screenshot_compare("widgets/obj_pos_content_min_size.png")
        .unwrap();
}

/// Behavioral regression, not an upstream reference image: exercise group
/// focus, keyboard activation, checked/disabled selectors and theme animation
/// through the input timer instead of writing object state directly.
#[test]
fn button_keyboard_focus_activation_and_default_theme_transitions() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let inputs = display.create_inputs();
    let group = display.ui.group_create();
    display.ui.group_set_default(Some(group));
    inputs.set_group(&mut display.ui, Some(group));
    let screen = display.screen().unwrap();
    let first = Button::create(&mut display.ui, screen);
    display.ui.obj_set_size(first.obj(), 100, 50);
    let second = Button::create(&mut display.ui, screen);
    display.ui.obj_set_size(second.obj(), 100, 50);
    display.ui.obj_set_pos(second.obj(), 120, 0);
    display.ui.obj_set_checkable(second.obj(), true);
    let label = Label::create(&mut display.ui, second.obj());
    label.set_text(&mut display.ui, "Toggle");
    let clicked = Rc::new(Cell::new(0));
    let copy = clicked.clone();
    display.ui.obj_add_event_cb(
        second.obj(),
        Rc::new(move |_, _| copy.set(copy.get() + 1)),
        EventCode::CLICKED,
        None,
    );
    display.snapshot().unwrap();
    inputs.key_hit(&mut display, KEY_NEXT);
    assert_eq!(display.ui.group_get_focused(group), Some(second.obj()));
    assert!(display.ui.obj_has_state(second.obj(), State::FOCUS_KEY));
    assert!(display.ui.obj_get_style_outline_width(second.obj(), Part::MAIN) > 0);
    inputs.keypad.key_press(KEY_ENTER);
    display.wait(150);
    assert!(display.ui.obj_has_state(second.obj(), State::PRESSED));
    assert_eq!(display.ui.obj_get_style_recolor_opa(second.obj(), Part::MAIN), 35);
    let growth = display.ui.obj_get_style_transform_width(second.obj(), Part::MAIN);
    assert!(growth > 0);
    inputs.keypad.release();
    display.wait(50);
    assert_eq!(clicked.get(), 1);
    assert!(display.ui.obj_has_state(second.obj(), State::CHECKED));
    // Delayed transition back to normal lasts 70 + 80ms.
    assert_eq!(
        display.ui.obj_get_style_transform_width(second.obj(), Part::MAIN),
        growth
    );
    display.wait(150);
    assert_eq!(
        display.ui.obj_get_style_transform_width(second.obj(), Part::MAIN),
        0
    );
    assert_eq!(display.ui.obj_get_style_recolor_opa(second.obj(), Part::MAIN), 0);
    assert_eq!(
        display.ui.obj_get_style_bg_color(second.obj(), Part::MAIN),
        palette::main(Palette::Red)
    );
    assert_eq!(
        display.ui.obj_get_style_text_color(label.obj(), Part::MAIN),
        Color::WHITE
    );
    display.ui.obj_add_state(second.obj(), State::DISABLED);
    display.wait(200);
    assert_eq!(
        display.ui.obj_get_style_recolor_opa(second.obj(), Part::MAIN),
        127
    );
    assert_eq!(
        display.ui.obj_get_style_recolor(second.obj(), Part::MAIN),
        palette::lighten(Palette::Grey, 2)
    );
    inputs.key_hit(&mut display, KEY_ENTER);
    assert_eq!(clicked.get(), 1);
}

#[test]
fn simple_button_renders_fixed_palette_without_default_theme_effects() {
    let mut display = TestDisplay::new().unwrap();
    let id = display.display();
    let theme = ferrovgl_widgets::theme::simple::init(&mut display.ui, id).unwrap();
    display.ui.display_set_theme(id, Some(theme));
    let screen = display.screen().unwrap();
    // Like C's lv_theme_apply: explicitly style this existing active screen.
    // lv_display_set_theme only reapplies screens[0] (the bottom layer here).
    display.ui.theme_apply(screen);
    let button = Button::create(&mut display.ui, screen);
    display.ui.obj_set_size(button.obj(), 80, 40);
    display.ui.obj_set_pos(button.obj(), 20, 20);
    let label = Label::create(&mut display.ui, button.obj());
    label.set_text(&mut display.ui, "Simple");
    let normal = display.snapshot().unwrap();
    assert_eq!(normal.pixel(0, 0), Some([245; 3]));
    assert_eq!(normal.pixel(20, 20), Some([158; 3]));
    assert_eq!(normal.pixel(90, 50), Some([158; 3]));
    assert_eq!(
        display.ui.obj_get_style_text_color(label.obj(), Part::MAIN),
        Color::hex(0x616161)
    );
    display.ui.obj_add_state(
        button.obj(),
        State::PRESSED | State::CHECKED | State::DISABLED | State::FOCUS_KEY,
    );
    display.wait(200);
    assert_eq!(display.snapshot().unwrap(), normal);
}

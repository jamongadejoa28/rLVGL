//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_textarea.c`.
//!
//! Every upstream test function, in upstream order, with its values and
//! screenshot names. `setUp` creates a text area on the active screen of a
//! fresh default-theme display with upstream's mouse, keypad and encoder test
//! devices; `tearDown`'s `lv_obj_clean` is the display drop. `lv_test_wait`,
//! `lv_test_mouse_click_at` and `lv_test_mouse_move_to_obj` are the harness
//! ports ([`TestDisplay::wait`] and friends).
//!
//! Upstream runs these tests one after another on one screen and only cleans
//! its children. No test in this file changes the screen itself, a focus
//! group or a global style, and every screenshot test waits for its scroll
//! animations to finish, so a fresh display reproduces the inherited state.
//! The only global the tests share, `insert_replace_text`, is used by one
//! test alone.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ferrovgl_core::font::GlyphDsc;
use ferrovgl_core::group::{KEY_BACKSPACE, KEY_DEL, KEY_DOWN, KEY_END, KEY_HOME};
use ferrovgl_core::misc::event::EventCallback;
use ferrovgl_core::misc::style::SharedStr;
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::{Align, EventCode, EventParam, Font, FontRef, Obj, Part, State, StyleSelector, coord};
use ferrovgl_test::{TestDisplay, TestInputs};
use ferrovgl_widgets::{Label, Textarea};

const MAIN: StyleSelector = StyleSelector::DEFAULT;
const ANIM_OFF: bool = false;
/// `LV_DPI_DEF` of the test configuration.
const DPI_DEF: i32 = ferrovgl_test::DISPLAY_DPI;

/// `static const char * textarea_default_text = "";`
const TEXTAREA_DEFAULT_TEXT: &str = "";

/// The globals of `test_textarea.c` set by `setUp()`.
struct Fixture {
    display: TestDisplay,
    inputs: TestInputs,
    active_screen: Obj,
    textarea: Textarea,
}

/// `setUp()`
fn set_up() -> Fixture {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let inputs = display.create_inputs();
    let active_screen = display.screen().unwrap();
    let textarea = Textarea::create(&mut display.ui, active_screen);
    Fixture {
        display,
        inputs,
        active_screen,
        textarea,
    }
}

/// `lv_test_wait()`
fn test_wait(f: &mut Fixture, ms: u32) {
    f.display.wait(ms);
}

/// `lv_test_mouse_click_at()`
fn mouse_click_at(f: &mut Fixture, x: i32, y: i32) {
    f.display.mouse_click_at(x, y);
}

/// `lv_test_mouse_move_to_obj()`: no layout update, like upstream.
fn mouse_move_to_obj(f: &mut Fixture, obj: Obj) {
    f.display.mouse_move_to_obj(obj);
}

/// `lv_obj_send_event(obj, LV_EVENT_KEY, &key)`
fn send_key(f: &mut Fixture, obj: Textarea, key: u32) {
    let _ = f
        .display
        .ui
        .obj_send_event(obj.obj(), EventCode::KEY, EventParam::Key(key));
}

/// `lv_label_get_text(lv_textarea_get_label(textarea))`
fn label_text(f: &Fixture, ta: Textarea) -> String {
    let ui = &f.display.ui;
    let label = Label::from_obj(ui, ta.label(ui).unwrap()).unwrap();
    label.text(ui).to_string()
}

/// `event_counter_cb()` counting into `event_count`.
fn event_counter_cb(event_count: &Rc<Cell<u32>>) -> EventCallback {
    let event_count = event_count.clone();
    Rc::new(move |_ui, _e| event_count.set(event_count.get() + 1))
}

/// `test_font_no_bullet`: a font with only printable ASCII glyphs.
struct TestFontNoBullet;

impl Font for TestFontNoBullet {
    fn line_height(&self) -> i32 {
        14
    }
    fn base_line(&self) -> i32 {
        12
    }
    fn underline_position(&self) -> i8 {
        0
    }
    fn underline_thickness(&self) -> i8 {
        0
    }
    /// `test_font_get_glyph_dsc()`
    fn glyph_dsc(&self, out: &mut GlyphDsc, letter: u32, _letter_next: u32) -> bool {
        if (0x20..=0x7E).contains(&letter) {
            out.adv_w = 10;
            out.box_w = 8;
            out.box_h = 12;
            return true;
        }
        false
    }
    /// `test_font_get_glyph_bitmap()`: `NULL`.
    fn glyph_bitmap_a8(&self, _g: &GlyphDsc, _out: &mut [u8], _stride: usize) -> bool {
        false
    }
}

static TEST_FONT_NO_BULLET: TestFontNoBullet = TestFontNoBullet;

#[test]
fn textarea_should_have_valid_documented_default_values() {
    let f = set_up();
    let ui = &f.display.ui;
    let textarea = f.textarea;
    assert!(textarea.cursor_click_pos(ui));
    assert!(!textarea.one_line(ui));
    /* No placeholder text should be set on widget creation */
    assert_eq!(TEXTAREA_DEFAULT_TEXT, textarea.placeholder_text(ui));
    assert_eq!(TEXTAREA_DEFAULT_TEXT, textarea.text(ui));
}

/* When in password mode the lv_textarea_get_text function returns
 * the actual text, not the bullet characters. */
#[test]
fn textarea_should_return_actual_text_when_password_mode_is_enabled() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    let text = "Hello LVGL!";

    textarea.add_text(ui, text);
    textarea.set_password_mode(ui, true);

    assert!(textarea.password_mode(ui));
    assert_eq!(text, textarea.text(ui));

    textarea.set_password_mode(ui, false);
}

#[test]
fn textarea_should_update_label_style_with_one_line_enabled() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    let label = textarea.label(ui).unwrap();

    textarea.add_text(ui, "Hi");
    textarea.set_one_line(ui, true);

    let left_padding = ui.obj_get_style_pad_left(label, Part::MAIN);
    let right_padding = ui.obj_get_style_pad_right(label, Part::MAIN);
    let line_width = ui.obj_get_width(label);
    let expected_size = left_padding + right_padding + line_width;

    assert!(textarea.one_line(ui));
    /*TEST_ASSERT_EQUAL_UINT16 compares the low 16 bits*/
    assert_eq!(expected_size as u16, ui.obj_get_width(label) as u16);
    assert_eq!(
        coord::pct(100) as u16,
        ui.obj_get_style_min_width(label, Part::MAIN) as u16
    );
}

#[test]
fn textarea_cursor_click_pos_field_update() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    f.textarea.set_cursor_click_pos(ui, false);

    assert!(!f.textarea.cursor_click_pos(ui));
}

#[test]
fn textarea_should_scroll_to_the_end() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    textarea.set_one_line(ui, true);
    textarea.add_text(
        ui,
        "Hi this is a long text to test if the textarea scrolls to the end",
    );
    ui.obj_set_width(textarea.obj(), DPI_DEF * 3);

    let cur_pos = textarea.cursor_pos(ui) as i32;
    let font = ui.obj_get_style_text_font(textarea.obj(), Part::MAIN).unwrap();
    let font_h = ferrovgl_core::font::get_line_height(font.as_font());
    let w = ui.obj_get_content_width(textarea.obj());
    if cur_pos + font_h - ui.obj_get_scroll_left(textarea.obj()) > w {
        assert_eq!(ui.obj_get_scroll_x(textarea.obj()) + 10, cur_pos - w + font_h);
    }

    assert!(textarea.one_line(ui));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn textarea_should_not_scroll_if_text_is_fully_visible() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    textarea.set_text(ui, "Type here...");
    ui.obj_set_width(textarea.obj(), 100);
    ui.obj_center(textarea.obj());
    f.display.screenshot_compare("widgets/textarea_1.png").unwrap();
}

#[test]
fn textarea_should_update_placeholder_text() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    let new_placeholder = "LVGL Rocks!!!!!";
    let text = "Hello LVGL!";

    /* Allocating memory for placeholder text */
    textarea.set_placeholder_text(ui, Some(text));
    assert_eq!(text, textarea.placeholder_text(ui));

    /* Reallocating memory for the new placeholder text */
    textarea.set_placeholder_text(ui, Some(new_placeholder));
    assert_eq!(new_placeholder, textarea.placeholder_text(ui));

    /* Freeing allocated memory for placeholder text */
    textarea.set_placeholder_text(ui, Some(""));
    assert_eq!("", textarea.placeholder_text(ui));
}

#[test]
fn textarea_should_keep_only_accepted_chars() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    let accepted_list = "abcd";

    textarea.set_accepted_chars(ui, Some(accepted_list));
    textarea.set_text(ui, "abcde");

    assert_eq!(accepted_list, textarea.text(ui));
}

#[test]
fn textarea_in_one_line_mode_should_ignore_line_break_characters() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    textarea.set_one_line(ui, true);

    textarea.add_char(ui, u32::from(b'\n'));
    assert_eq!(TEXTAREA_DEFAULT_TEXT, textarea.text(ui));

    textarea.add_char(ui, u32::from(b'\r'));
    assert_eq!(TEXTAREA_DEFAULT_TEXT, textarea.text(ui));
}

#[test]
fn textarea_should_hide_password_characters() {
    let mut f = set_up();
    let textarea = f.textarea;
    textarea.set_password_mode(&mut f.display.ui, true);
    textarea.set_text(&mut f.display.ui, "12345");

    /* setting bullet hides characters */
    textarea.set_password_bullet(&mut f.display.ui, Some("O"));
    assert_eq!("OOOOO", label_text(&f, textarea));

    /* setting text hides characters */
    textarea.set_text(&mut f.display.ui, "A");
    assert_eq!("O", label_text(&f, textarea));

    textarea.add_char(&mut f.display.ui, u32::from(b'B'));
    assert_eq!("OB", label_text(&f, textarea));

    /* setting show time hides characters */
    /* current behavior is to hide the characters upon setting the show time regardless of the value */
    let time = textarea.password_show_time(&f.display.ui);
    textarea.set_password_show_time(&mut f.display.ui, time);
    assert_eq!("OO", label_text(&f, textarea));

    textarea.set_password_mode(&mut f.display.ui, false);
    assert_eq!("AB", label_text(&f, textarea));

    /* enabling password mode hides characters */
    textarea.set_password_mode(&mut f.display.ui, true);
    assert_eq!("OO", label_text(&f, textarea));
}

/// `LV_USE_OBJ_PROPERTY` is disabled in the parity configuration, so the
/// upstream body is compiled out and only `setUp`/`tearDown` run.
#[test]
fn textarea_properties() {
    let _f = set_up();
}

#[test]
fn textarea_set_text_should_emit_value_changed_event_only_once() {
    let mut f = set_up();
    let accepted_list = "abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ!,";
    let text = "Hello, World!";
    let text_len: u32 = 13; /* strlen("Hello, World!") */
    let event_count = Rc::new(Cell::new(0u32));

    /* Test 1: with accepted_chars set */
    event_count.set(0);
    let ui = &mut f.display.ui;
    let mut textarea = f.textarea;
    textarea.set_accepted_chars(ui, Some(accepted_list));
    ui.obj_add_event_cb(
        textarea.obj(),
        event_counter_cb(&event_count),
        EventCode::VALUE_CHANGED,
        None,
    );

    textarea.set_text(ui, text);

    assert_eq!(text, textarea.text(ui));
    assert_eq!(1, event_count.get());

    /* Test 2: with max_length set to exactly the text length — if set_text
     * doesn't clear before re-adding chars, char_is_accepted sees the buffer
     * as already full and rejects every character, leaving the textarea empty */
    ui.obj_clean(f.active_screen);
    textarea = Textarea::create(ui, f.active_screen);

    event_count.set(0);
    textarea.set_max_length(ui, text_len);
    ui.obj_add_event_cb(
        textarea.obj(),
        event_counter_cb(&event_count),
        EventCode::VALUE_CHANGED,
        None,
    );

    textarea.set_text(ui, text);

    assert_eq!(text, textarea.text(ui));
    assert_eq!(1, event_count.get());

    /* Test 3: with both accepted_chars and max_length set */
    ui.obj_clean(f.active_screen);
    textarea = Textarea::create(ui, f.active_screen);

    event_count.set(0);
    textarea.set_accepted_chars(ui, Some(accepted_list));
    textarea.set_max_length(ui, text_len);
    ui.obj_add_event_cb(
        textarea.obj(),
        event_counter_cb(&event_count),
        EventCode::VALUE_CHANGED,
        None,
    );

    textarea.set_text(ui, text);

    assert_eq!(text, textarea.text(ui));
    assert_eq!(1, event_count.get());

    /* Test 4: empty string — no characters to add, no event */
    ui.obj_clean(f.active_screen);
    textarea = Textarea::create(ui, f.active_screen);

    event_count.set(0);
    textarea.set_accepted_chars(ui, Some(accepted_list));
    textarea.set_max_length(ui, text_len);
    ui.obj_add_event_cb(
        textarea.obj(),
        event_counter_cb(&event_count),
        EventCode::VALUE_CHANGED,
        None,
    );

    textarea.set_text(ui, "");

    assert_eq!("", textarea.text(ui));
    assert_eq!(0, event_count.get());

    /* Test 5: all characters rejected by accepted_chars — no characters added, no event */
    ui.obj_clean(f.active_screen);
    textarea = Textarea::create(ui, f.active_screen);

    event_count.set(0);
    textarea.set_accepted_chars(ui, Some(accepted_list));
    ui.obj_add_event_cb(
        textarea.obj(),
        event_counter_cb(&event_count),
        EventCode::VALUE_CHANGED,
        None,
    );

    textarea.set_text(ui, "123"); /* digits not in accepted_list */

    assert_eq!("", textarea.text(ui));
    assert_eq!(0, event_count.get());
}

#[test]
fn textarea_set_max_length() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    f.textarea.set_max_length(ui, 8);
    f.textarea.add_text(ui, "1234567890");
    assert_eq!("12345678", f.textarea.text(ui));
}

#[test]
fn textarea_set_insert_replace() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    /* static char insert_replace_text[10] = "123"; */
    let insert_replace_text = Rc::new(RefCell::new(String::from("123")));

    textarea.set_text(ui, "1234567890");
    /* event_handler() */
    let replace = insert_replace_text.clone();
    let event_handler: EventCallback = Rc::new(move |ui, e| {
        let code = e.code();
        if code == EventCode::INSERT {
            let first = match e.param() {
                EventParam::Text(txt) => txt.as_bytes().first().copied(),
                _ => None,
            };
            if first.is_some_and(|c| c.is_ascii_digit()) {
                /* insert */
            } else if let Some(obj) = e.target_obj() {
                let text: Rc<str> = Rc::from(replace.borrow().as_str());
                Textarea::from_obj(ui, obj)
                    .unwrap()
                    .set_insert_replace(ui, Some(SharedStr::Shared(text)));
            }
        }
    });
    ui.obj_add_event_cb(textarea.obj(), event_handler, EventCode::INSERT, None);

    textarea.add_text(ui, "abc");
    assert_eq!("1234567890123", textarea.text(ui));

    /* lv_memset(insert_replace_text, 0, sizeof(insert_replace_text)); */
    insert_replace_text.borrow_mut().clear();
    textarea.add_text(ui, "abc");
    assert_eq!("1234567890123", textarea.text(ui));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn textarea_placeholder_text_show_one_line() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    f.textarea.set_one_line(ui, true);
    f.textarea.set_placeholder_text(ui, Some("1234567890"));

    f.display
        .screenshot_compare("textarea_placeholder_show.png")
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn textarea_password_mode() {
    let mut f = set_up();
    let textarea = f.textarea;
    let ui = &mut f.display.ui;
    textarea.set_one_line(ui, false);

    textarea.set_text(ui, "123456");
    textarea.set_password_mode(ui, true);
    f.display
        .screenshot_compare("textarea_password_mode.png")
        .unwrap();

    let ui = &mut f.display.ui;
    textarea.set_password_mode(ui, false);

    textarea.set_text(ui, "123456789");
    textarea.set_password_mode(ui, true);
    f.display
        .screenshot_compare("textarea_password_mode_update.png")
        .unwrap();

    let ui = &mut f.display.ui;
    textarea.set_password_mode(ui, false);

    textarea.add_text(ui, "abc");
    textarea.set_password_mode(ui, true);
    f.display
        .screenshot_compare("textarea_password_mode_add_text.png")
        .unwrap();

    let ui = &mut f.display.ui;
    textarea.set_password_mode(ui, false);

    textarea.add_char(ui, u32::from(b'a'));
    textarea.set_password_mode(ui, true);
    f.display
        .screenshot_compare("textarea_password_mode_add_char.png")
        .unwrap();

    let ui = &mut f.display.ui;
    textarea.set_password_mode(ui, false);

    textarea.delete_char(ui);
    textarea.set_password_mode(ui, true);
    f.display
        .screenshot_compare("textarea_password_mode_delete_char.png")
        .unwrap();

    let ui = &mut f.display.ui;
    textarea.set_password_mode(ui, false);

    textarea.set_text(ui, "1234567890");
    f.display.screenshot_compare("textarea_normal_mode.png").unwrap();
}

/* In password mode, adding text must go through the realloc path of
 * lv_textarea_add_text() so that pwd_tmp stays valid. */
#[test]
fn textarea_add_text_in_password_mode() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    textarea.set_password_mode(ui, true);
    textarea.add_text(ui, "Hello");
    assert_eq!("Hello", textarea.text(ui));

    /*Adding more text should keep pwd_tmp in sync*/
    textarea.add_text(ui, " World");
    assert_eq!("Hello World", textarea.text(ui));

    textarea.set_password_mode(ui, false);
    assert_eq!("Hello World", textarea.text(ui));
}

/* In password mode, deleting a char must go through the realloc path of
 * lv_textarea_delete_char() so that pwd_tmp stays valid. */
#[test]
fn textarea_delete_char_in_password_mode() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    textarea.set_text(ui, "Hello World");
    textarea.set_password_mode(ui, true);
    textarea.set_cursor_pos(ui, 11);

    textarea.delete_char(ui);
    assert_eq!("Hello Worl", textarea.text(ui));

    textarea.delete_char(ui);
    assert_eq!("Hello Wor", textarea.text(ui));

    textarea.set_password_mode(ui, false);
    assert_eq!("Hello Wor", textarea.text(ui));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn textarea_password_mode_hide_char() {
    let mut f = set_up();
    let textarea = f.textarea;
    let ui = &mut f.display.ui;
    textarea.set_one_line(ui, false);
    textarea.set_password_mode(ui, true);

    textarea.set_password_show_time(ui, 0);
    textarea.add_char(ui, u32::from(b'a'));

    test_wait(&mut f, 50);
    f.display
        .screenshot_compare("textarea_password_mode_hide_char_one.png")
        .unwrap();

    let ui = &mut f.display.ui;
    textarea.set_password_show_time(ui, 500);
    textarea.add_char(ui, u32::from(b'b'));

    test_wait(&mut f, 550);
    f.display
        .screenshot_compare("textarea_password_mode_hide_char_two.png")
        .unwrap();

    let ui = &mut f.display.ui;
    textarea.add_char(ui, u32::from(b'c'));
    textarea.set_password_mode(ui, false);
    f.display
        .screenshot_compare("textarea_password_mode_to_normal_mode.png")
        .unwrap();

    textarea.set_text(&mut f.display.ui, "");
}

#[test]
fn textarea_set_password_bullet() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    textarea.set_one_line(ui, false);
    textarea.set_text(ui, "1234567890");
    textarea.set_password_mode(ui, true);

    ui.obj_set_style_text_font(textarea.obj(), FontRef::Static(&TEST_FONT_NO_BULLET), MAIN);
    assert_eq!("*", textarea.password_bullet(ui));

    textarea.set_password_bullet(ui, Some("*"));
    assert_eq!("*", textarea.password_bullet(ui));

    textarea.set_password_bullet(ui, None);
    assert_eq!("*", textarea.password_bullet(ui));

    textarea.set_password_mode(ui, false);
    textarea.set_text(ui, "");
}

#[test]
fn textarea_delete_char() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    f.textarea.set_one_line(ui, false);
    f.textarea.set_text(ui, "1234567890");
    f.textarea.delete_char(ui);
}

#[test]
fn textarea_delete_char_forward() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    f.textarea.set_one_line(ui, false);
    f.textarea.set_text(ui, "1234567890");
    f.textarea.delete_char_forward(ui);
}

#[test]
fn textarea_set_text_selection() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    textarea.set_one_line(ui, false);
    textarea.set_text(ui, "1234567890");

    textarea.set_text_selection(ui, true);
    assert!(textarea.text_selection(ui));

    let label = Label::from_obj(ui, textarea.label(ui).unwrap()).unwrap();
    label.set_text_selection_start(ui, 0);
    label.set_text_selection_end(ui, 10);
    assert!(textarea.text_is_selected(ui));

    textarea.clear_selection(ui);
    assert!(!textarea.text_is_selected(ui));
}

#[test]
fn textarea_set_password_show_time() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    f.textarea.set_password_show_time(ui, 1000);

    assert_eq!(1000, f.textarea.password_show_time(ui));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
#[allow(deprecated)] // LV_DEPRECATIONS_IGNORE_BEGIN
fn textarea_set_align() {
    let mut f = set_up();
    let textarea = f.textarea;
    let ui = &mut f.display.ui;
    textarea.set_align(ui, TextAlign::Center);
    textarea.set_text(ui, "1234567890");
    f.display.screenshot_compare("textarea_align_center.png").unwrap();

    let ui = &mut f.display.ui;
    textarea.set_align(ui, TextAlign::Left);
    textarea.set_text(ui, "1234567890");
    f.display.screenshot_compare("textarea_align_left.png").unwrap();

    let ui = &mut f.display.ui;
    textarea.set_align(ui, TextAlign::Right);
    textarea.set_text(ui, "1234567890");
    f.display.screenshot_compare("textarea_align_right.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn textarea_text_align() {
    let mut f = set_up();
    let textarea = f.textarea;
    let ui = &mut f.display.ui;
    ui.obj_set_style_text_align(textarea.obj(), TextAlign::Center, Part::MAIN.into());
    textarea.set_text(ui, "1234567890");
    f.display.screenshot_compare("textarea_align_center.png").unwrap();

    let ui = &mut f.display.ui;
    ui.obj_set_style_text_align(textarea.obj(), TextAlign::Left, Part::MAIN.into());
    textarea.set_text(ui, "1234567890");
    f.display.screenshot_compare("textarea_align_left.png").unwrap();

    let ui = &mut f.display.ui;
    ui.obj_set_style_text_align(textarea.obj(), TextAlign::Right, Part::MAIN.into());
    textarea.set_text(ui, "1234567890");
    f.display.screenshot_compare("textarea_align_right.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn textarea_cursor_show() {
    let mut f = set_up();
    let textarea = f.textarea;
    let ui = &mut f.display.ui;
    textarea.set_password_show_time(ui, 1000);
    let _ = ui.obj_send_event(textarea.obj(), EventCode::FOCUSED, EventParam::None);

    test_wait(&mut f, 1000);
    f.display.screenshot_compare("textarea_cursor_show.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn textarea_set_cursor_pos() {
    let mut f = set_up();
    let textarea = f.textarea;
    let text = "123456789012345678901234567890123456789012345678901";
    let ui = &mut f.display.ui;
    ui.obj_set_size(textarea.obj(), 100, 60);

    textarea.set_one_line(ui, false);
    textarea.set_text(ui, text);

    textarea.set_cursor_pos(ui, 0);
    f.display
        .screenshot_compare("textarea_cursor_pos_top.png")
        .unwrap();

    textarea.set_cursor_pos(&mut f.display.ui, 50);
    f.display
        .screenshot_compare("textarea_cursor_pos_bottom.png")
        .unwrap();

    let ui = &mut f.display.ui;
    textarea.set_one_line(ui, true);
    textarea.set_text(ui, text);

    textarea.set_cursor_pos(ui, 0);
    f.display
        .screenshot_compare("textarea_cursor_pos_left.png")
        .unwrap();

    textarea.set_cursor_pos(&mut f.display.ui, 50);
    f.display
        .screenshot_compare("textarea_cursor_pos_right.png")
        .unwrap();

    let ui = &mut f.display.ui;
    textarea.set_one_line(ui, false);
    textarea.set_text(ui, text);
    let label = textarea.label(ui).unwrap();
    ui.obj_set_size(label, 100, 200);

    ui.obj_scroll_to_y(textarea.obj(), 300, ANIM_OFF);
    textarea.set_cursor_pos(ui, 4);
    test_wait(&mut f, 500);
    assert_eq!(0, f.display.ui.obj_get_scroll_y(textarea.obj()));

    let ui = &mut f.display.ui;
    textarea.set_one_line(ui, true);
    textarea.set_cursor_pos(ui, 50);
    test_wait(&mut f, 100);
    textarea.set_cursor_pos(&mut f.display.ui, 10);
    test_wait(&mut f, 100);
    assert_eq!(10, textarea.cursor_pos(&f.display.ui));
}

#[test]
fn textarea_get_current_char() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    textarea.set_one_line(ui, false);
    textarea.set_text(ui, "1234567890");

    textarea.set_cursor_pos(ui, 2);
    assert_eq!(u32::from(b'2'), textarea.current_char(ui));

    textarea.set_cursor_pos(ui, 0);
    assert_eq!(0, textarea.current_char(ui));
}

#[test]
fn textarea_cursor_position_on_click() {
    let mut f = set_up();
    let textarea = f.textarea;
    let ui = &mut f.display.ui;
    textarea.set_one_line(ui, true);
    textarea.set_text(ui, "Hello World");
    ui.obj_set_size(textarea.obj(), 200, 100);

    textarea.set_cursor_click_pos(ui, true);

    mouse_click_at(&mut f, 5, 15);
    assert_eq!(0, textarea.cursor_pos(&f.display.ui));

    mouse_click_at(&mut f, 180, 15);
    assert_eq!(11, textarea.cursor_pos(&f.display.ui));

    let label_width = f.display.ui.obj_get_width(textarea.label(&f.display.ui).unwrap());
    mouse_click_at(&mut f, label_width + 20, 15);
    assert_eq!(11, textarea.cursor_pos(&f.display.ui));

    textarea.set_text_selection(&mut f.display.ui, true);
    mouse_click_at(&mut f, 40, 15);
    f.inputs.mouse.press();
    test_wait(&mut f, 100);
    f.inputs.mouse.move_to(100, 15);
    test_wait(&mut f, 100);
    f.inputs.mouse.release();
    assert_eq!(11, textarea.cursor_pos(&f.display.ui));

    mouse_click_at(&mut f, 40, 15);
    f.inputs.mouse.press();
    test_wait(&mut f, 100);
    f.inputs.mouse.move_to(5, 15);
    test_wait(&mut f, 100);
    f.inputs.mouse.release();
    assert_eq!(0, textarea.cursor_pos(&f.display.ui));

    textarea.set_text_selection(&mut f.display.ui, false);
    mouse_click_at(&mut f, 5, 15);
    assert_eq!(0, textarea.cursor_pos(&f.display.ui));

    textarea.set_cursor_click_pos(&mut f.display.ui, false);
    let pos_before = textarea.cursor_pos(&f.display.ui);
    mouse_click_at(&mut f, 50, 15);
    assert_eq!(pos_before, textarea.cursor_pos(&f.display.ui));
}

#[test]
fn textarea_key_event() {
    let mut f = set_up();
    let textarea = f.textarea;
    textarea.set_text(&mut f.display.ui, "Hello World");
    textarea.set_cursor_pos(&mut f.display.ui, 11);

    let mut key = KEY_BACKSPACE;
    send_key(&mut f, textarea, key);
    assert_eq!("Hello Worl", textarea.text(&f.display.ui));

    textarea.set_text(&mut f.display.ui, "Hello World");
    textarea.set_cursor_pos(&mut f.display.ui, 0);

    key = KEY_DEL;
    send_key(&mut f, textarea, key);
    assert_eq!("ello World", textarea.text(&f.display.ui));

    key = KEY_HOME;
    textarea.set_cursor_pos(&mut f.display.ui, 10);
    send_key(&mut f, textarea, key);
    assert_eq!(0, textarea.cursor_pos(&f.display.ui));

    key = KEY_END;
    textarea.set_cursor_pos(&mut f.display.ui, 0);
    send_key(&mut f, textarea, key);
    assert_eq!(10, textarea.cursor_pos(&f.display.ui));

    let ui = &mut f.display.ui;
    ui.obj_set_size(textarea.obj(), 100, 40);
    textarea.set_one_line(ui, false);
    textarea.set_text(ui, "Hello World, this is a test for the key event");
    textarea.set_cursor_pos(ui, 0);

    key = KEY_DOWN;
    send_key(&mut f, textarea, key);
    test_wait(&mut f, 100);
    assert_eq!(6, textarea.cursor_pos(&f.display.ui));

    textarea.set_text(&mut f.display.ui, "Hello World");
    textarea.set_cursor_pos(&mut f.display.ui, 11);

    key = 49;
    send_key(&mut f, textarea, key);
    assert_eq!("Hello World1", textarea.text(&f.display.ui));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn textarea_check_placeholder_text_position() {
    let mut f = set_up();
    let textarea = f.textarea;
    let ui = &mut f.display.ui;
    textarea.set_placeholder_text(ui, Some("Placeholder"));
    textarea.set_one_line(ui, true);
    f.display
        .screenshot_compare("textarea_placeholder_center.png")
        .unwrap();

    textarea.set_one_line(&mut f.display.ui, false);
    f.display
        .screenshot_compare("textarea_placeholder_top.png")
        .unwrap();

    let ui = &mut f.display.ui;
    let label = textarea.label(ui).unwrap();
    ui.obj_set_style_align(label, Align::LeftMid, Part::MAIN.into());
    f.display
        .screenshot_compare("textarea_placeholder_left_mid.png")
        .unwrap();

    let ui = &mut f.display.ui;
    ui.obj_set_style_align(label, Align::TopLeft, Part::MAIN.into());
    ui.obj_set_style_pad_top(label, 50, ferrovgl_widgets::PART_TEXTAREA_PLACEHOLDER.into());
    ui.obj_set_style_pad_left(label, 50, ferrovgl_widgets::PART_TEXTAREA_PLACEHOLDER.into());
    f.display
        .screenshot_compare("textarea_placeholder_pad_left_top_50.png")
        .unwrap();
}

/// `static const char * scroll_test_text`
const SCROLL_TEST_TEXT: &str = "Lorem ipsum dolor sit amet";

/// `one_line_textarea_create()`
fn one_line_textarea_create(f: &mut Fixture, w: i32) -> Textarea {
    let ui = &mut f.display.ui;
    let obj = Textarea::create(ui, f.active_screen);
    obj.set_one_line(ui, true);
    ui.obj_set_pos(obj.obj(), 20, 20);
    ui.obj_set_size(obj.obj(), w, 40);

    ui.obj_set_scroll_momentum(obj.obj(), false);

    for _ in 0..3 {
        obj.add_text(ui, SCROLL_TEST_TEXT);
    }
    test_wait(f, 500);

    obj
}

/* Scrolling a one_line textarea must not be undone by an unrelated style change. See #9929. */
#[test]
fn textarea_one_line_should_keep_scroll_position_on_style_change() {
    let mut f = set_up();
    f.display.ui.obj_clean(f.active_screen);
    let textarea = one_line_textarea_create(&mut f, 100);

    /* The cursor is at the end of the text, so the textarea is scrolled to the end */
    let scroll_x_end = f.display.ui.obj_get_scroll_x(textarea.obj());
    assert!(scroll_x_end > 0);

    /* Drag towards the beginning of the text */
    mouse_move_to_obj(&mut f, textarea.obj());
    f.inputs.mouse.press();
    test_wait(&mut f, 50);
    for _ in 0..10 {
        f.inputs.mouse.move_by(5, 0);
        test_wait(&mut f, 20);
    }
    let scroll_x = f.display.ui.obj_get_scroll_x(textarea.obj());
    assert!(scroll_x < scroll_x_end);

    f.inputs.mouse.release();
    test_wait(&mut f, 500);
    assert_eq!(scroll_x, f.display.ui.obj_get_scroll_x(textarea.obj()));

    /* Any other style change must not move the scroll position either */
    f.display.ui.obj_add_state(textarea.obj(), State::DISABLED);
    test_wait(&mut f, 500);
    assert_eq!(scroll_x, f.display.ui.obj_get_scroll_x(textarea.obj()));
}

#[test]
#[allow(deprecated)] // LV_DEPRECATIONS_IGNORE_BEGIN
fn textarea_one_line_scroll_should_not_depend_on_text_align() {
    let mut f = set_up();
    let aligns = [TextAlign::Left, TextAlign::Center, TextAlign::Right];
    let mut reference = [0i32; 2];

    for (a, &align) in aligns.iter().enumerate() {
        f.display.ui.obj_clean(f.active_screen);
        let textarea = one_line_textarea_create(&mut f, 100);
        textarea.set_align(&mut f.display.ui, align);
        test_wait(&mut f, 500);

        /* The text overflows, so the textarea has to be scrolled to show the cursor */
        let scrolled_to_end = f.display.ui.obj_get_scroll_x(textarea.obj());
        assert!(scrolled_to_end > 0);

        /* Shrink the text, but keep it overflowing */
        for _ in 0..20 {
            textarea.delete_char(&mut f.display.ui);
        }
        test_wait(&mut f, 500);
        let scrolled_after_delete = f.display.ui.obj_get_scroll_x(textarea.obj());
        assert!(scrolled_after_delete > 0);

        if a == 0 {
            reference[0] = scrolled_to_end;
            reference[1] = scrolled_after_delete;
        } else {
            assert_eq!(reference[0], scrolled_to_end);
            assert_eq!(reference[1], scrolled_after_delete);
        }
    }
}

/* The scroll position may only be moved if the cursor would end up outside of the
 * visible area. Moving the cursor within the visible area must not scroll. */
#[test]
fn textarea_one_line_should_not_scroll_when_cursor_stays_visible() {
    let mut f = set_up();
    f.display.ui.obj_clean(f.active_screen);
    let textarea = one_line_textarea_create(&mut f, 200);

    let scroll_x = f.display.ui.obj_get_scroll_x(textarea.obj());
    assert!(scroll_x > 0);

    /* Ten characters back from the end of the text is still within the visible
     * area of a 200 px wide textarea */
    let pos = textarea.cursor_pos(&f.display.ui).wrapping_sub(10);
    textarea.set_cursor_pos(&mut f.display.ui, pos as i32);
    test_wait(&mut f, 500);

    assert_eq!(scroll_x, f.display.ui.obj_get_scroll_x(textarea.obj()));
}

/* Resizing the textarea must keep the cursor in view. The label has LV_SIZE_CONTENT
 * width in one_line mode, so no label size change reports the new geometry. */
#[test]
fn textarea_one_line_should_keep_cursor_in_view_when_resized() {
    let mut f = set_up();
    f.display.ui.obj_clean(f.active_screen);
    let textarea = one_line_textarea_create(&mut f, 200);

    /* The cursor is at the end of the text, so the view is scrolled to the end */
    assert_eq!(0, f.display.ui.obj_get_scroll_right(textarea.obj()));

    f.display.ui.obj_set_width(textarea.obj(), 100);
    test_wait(&mut f, 500);
    assert_eq!(0, f.display.ui.obj_get_scroll_right(textarea.obj()));
}

#[test]
fn textarea_should_keep_cursor_in_view_when_resized() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    ui.obj_clean(f.active_screen);
    let textarea = Textarea::create(ui, f.active_screen);
    ui.obj_set_pos(textarea.obj(), 20, 20);
    ui.obj_set_size(textarea.obj(), 200, 60);
    textarea.set_text(ui, "aaa\nbbb\nccc\nddd\neee\nfff");
    test_wait(&mut f, 500);

    /* The cursor is on the last line, so the view is scrolled to the bottom */
    assert_eq!(0, f.display.ui.obj_get_scroll_bottom(textarea.obj()));

    f.display.ui.obj_set_height(textarea.obj(), 40);
    test_wait(&mut f, 500);
    assert_eq!(0, f.display.ui.obj_get_scroll_bottom(textarea.obj()));
}

/// `scroll_screenshot_row()`
fn scroll_screenshot_row(f: &mut Fixture, y: i32, caption: &str) -> Textarea {
    /* static const char * ruler_text */
    let ruler_text = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    let ui = &mut f.display.ui;
    let label = Label::create(ui, f.active_screen);
    label.set_text(ui, caption);
    ui.obj_set_pos(label.obj(), 20, y);

    let obj = Textarea::create(ui, f.active_screen);
    obj.set_one_line(ui, true);
    ui.obj_set_pos(obj.obj(), 20, y + 20);
    ui.obj_set_size(obj.obj(), 200, 40);
    obj.add_text(ui, ruler_text);

    /* Draw the cursor, without blinking, so that it is visible where the view ends up */
    ui.obj_set_style_anim_duration(obj.obj(), 0, Part::CURSOR | State::FOCUSED);
    ui.obj_add_state(obj.obj(), State::FOCUSED);
    let _ = ui.obj_send_event(obj.obj(), EventCode::FOCUSED, EventParam::None);
    test_wait(f, 500);

    obj
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn textarea_one_line_scroll_screenshot() {
    let mut f = set_up();
    f.display.ui.obj_clean(f.active_screen);

    /* Moving the cursor within the visible area must not scroll */
    let ta = scroll_screenshot_row(&mut f, 20, "cursor moved back 10 characters");
    let pos = ta.cursor_pos(&f.display.ui).wrapping_sub(10);
    ta.set_cursor_pos(&mut f.display.ui, pos as i32);
    test_wait(&mut f, 500);

    /* Resizing has to bring the cursor back into view */
    let ta = scroll_screenshot_row(&mut f, 100, "resized from 200 px to 120 px");
    f.display.ui.obj_set_width(ta.obj(), 120);
    test_wait(&mut f, 500);

    /* A style change must not move the scroll position */
    let ta = scroll_screenshot_row(&mut f, 180, "scrolled to the start, then disabled");
    f.display.ui.obj_scroll_to_x(ta.obj(), 0, ANIM_OFF);
    test_wait(&mut f, 1000);
    f.display.ui.obj_add_state(ta.obj(), State::DISABLED);
    test_wait(&mut f, 1000);

    f.display
        .screenshot_compare("widgets/textarea_one_line_scroll.png")
        .unwrap();
}

/* A text area narrowed from its default width must not be left scrolled away from
 * centred text. The label is resized after the text area, so a scroll started for
 * the old label width has to be dropped when the label catches up. See #10689. */
fn centred_textarea_should_not_scroll(f: &mut Fixture, one_line: bool) {
    let ui = &mut f.display.ui;
    ui.obj_clean(f.active_screen);

    let textarea = Textarea::create(ui, f.active_screen);
    f.textarea = textarea;
    ui.obj_set_width(textarea.obj(), 100);
    ui.obj_center(textarea.obj());
    if one_line {
        textarea.set_one_line(ui, true);
    }
    ui.obj_set_style_text_align(textarea.obj(), TextAlign::Center, Part::MAIN.into());
    textarea.set_text(ui, "test");

    /*The bad scroll is animated, so a shorter wait passes without proving anything*/
    test_wait(f, 500);

    /*"test" is far narrower than the text area, so there is nothing to scroll*/
    assert_eq!(0, f.display.ui.obj_get_scroll_x(textarea.obj()));
}

#[test]
fn textarea_one_line_centred_text_should_not_be_scrolled_out_of_view() {
    let mut f = set_up();
    centred_textarea_should_not_scroll(&mut f, true);
}

#[test]
fn textarea_centred_text_should_not_be_scrolled_out_of_view() {
    let mut f = set_up();
    centred_textarea_should_not_scroll(&mut f, false);
}

#[test]
fn textarea_set_text_should_emit_value_changed_event_without_filters() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let event_count = Rc::new(Cell::new(0u32));
    ui.obj_add_event_cb(
        f.textarea.obj(),
        event_counter_cb(&event_count),
        EventCode::VALUE_CHANGED,
        None,
    );

    f.textarea.set_text(ui, "Hello");

    assert_eq!("Hello", f.textarea.text(ui));
    assert_eq!(1, event_count.get());
}

#[test]
fn textarea_add_text_should_emit_value_changed_event_only_once() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let textarea = f.textarea;
    let event_count = Rc::new(Cell::new(0u32));
    textarea.set_accepted_chars(ui, Some("abcdefghijklmnopqrstuvwxyz"));
    ui.obj_add_event_cb(
        textarea.obj(),
        event_counter_cb(&event_count),
        EventCode::VALUE_CHANGED,
        None,
    );

    textarea.add_text(ui, "abc");

    assert_eq!("abc", textarea.text(ui));
    assert_eq!(1, event_count.get());

    /*All characters rejected: nothing added, no event*/
    textarea.add_text(ui, "123");

    assert_eq!("abc", textarea.text(ui));
    assert_eq!(1, event_count.get());
}

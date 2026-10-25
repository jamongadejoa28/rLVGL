//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_label.c`.
//!
//! Every test function of the upstream file that no other parity file ports,
//! in upstream order, with its values and screenshot names. Ported elsewhere:
//! `test_label_rtl_dot_long_mode` (`parity_bidi.rs`),
//! `test_label_scroll_mid_update` (`parity_scroll.rs`) and
//! `test_label_with_recolor_cmd`, `test_label_long_mode_clip`,
//! `test_label_wrap_mode_clip`, `test_label_no_leading_space_after_line_wrap`,
//! `test_label_preserve_spaces_after_explicit_newline` (`parity_label.rs`).
//!
//! Not ported: `test_label_translation_tag` and
//! `test_label_setting_text_disables_translation` need `lv_translation_*`
//! and `lv_label_set_translation_tag()`, which ferrovgl does not implement.
//!
//! Each `#[test]` gets a fresh display and replays the file's `setUp()` (four
//! labels on the active screen). No test depends on screen state left by an
//! earlier test: `test_label_text_trim` is the only test that styles the
//! screen, and the two tests after it build their own container precisely so
//! that those styles cannot change their result.

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::color::{OPA_50, OPA_100};
use ferrovgl_core::misc::event::EventCallback;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::text::{LeadingTrim, TextAlign};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, EventCode, FontRef, Obj, Point, RenderMode, StyleSelector};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::label::LABEL_DEFAULT_TEXT;
use ferrovgl_widgets::{Label, LabelLongMode};

const MAIN: StyleSelector = StyleSelector::DEFAULT;

/// `long_text`
const LONG_TEXT: &str =
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Cras malesuada ultrices magna in rutrum.";
/// `long_text_multiline`
const LONG_TEXT_MULTILINE: &str =
    "Lorem ipsum dolor sit amet,\nconsectetur adipiscing elit.\nCras malesuada ultrices magna in rutrum.\n";
/// `empty_text`
const EMPTY_TEXT: &str = "";

/// The file-scope objects created by `setUp()`.
struct Fixture {
    display: TestDisplay,
    label: Label,
    long_label: Label,
    long_label_multiline: Label,
    empty_label: Label,
}

/// `setUp()`
fn set_up() -> Fixture {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let active_screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let label = Label::create(ui, active_screen);
    let long_label = Label::create(ui, active_screen);
    let long_label_multiline = Label::create(ui, active_screen);
    let empty_label = Label::create(ui, active_screen);

    long_label.set_text(ui, LONG_TEXT);
    long_label_multiline.set_text(ui, LONG_TEXT_MULTILINE);
    empty_label.set_text(ui, EMPTY_TEXT);
    Fixture {
        display,
        label,
        long_label,
        long_label_multiline,
        empty_label,
    }
}

/// `strlen(lv_label_get_text(obj)) - 1` as the `uint32_t` it is stored in
/// (wraps to `UINT32_MAX` for empty text).
fn last_idx(f: &Fixture, label: Label) -> u32 {
    (label.text(&f.display.ui).len() as u32).wrapping_sub(1)
}

/// `lv_label_get_letter_pos()` for the first and the given last index.
fn first_and_last(f: &Fixture, label: Label, last: u32) -> (Point, Point) {
    (
        label.letter_pos(&f.display.ui, 0),
        label.letter_pos(&f.display.ui, last),
    )
}

#[test]
fn label_creation() {
    let f = set_up();
    assert_eq!(f.label.text(&f.display.ui), LABEL_DEFAULT_TEXT);
    assert_eq!(f.label.long_mode(&f.display.ui), LabelLongMode::Wrap);
}

#[test]
fn label_set_text() {
    let mut f = set_up();
    let new_text = "Hello world";

    /* Refresh with the current text. */
    f.label.refresh_text(&mut f.display.ui);
    assert_eq!(f.label.text(&f.display.ui), LABEL_DEFAULT_TEXT);

    /* '\0' terminated character string to set new text. */
    f.label.set_text(&mut f.display.ui, new_text);
    assert_eq!(f.label.text(&f.display.ui), new_text);
}

#[test]
fn label_get_letter_pos_align_left() {
    let mut f = set_up();
    f.display
        .ui
        .obj_set_style_text_align(f.label.obj(), TextAlign::Left, MAIN);
    let last = last_idx(&f, f.long_label_multiline);
    let (first, last) = first_and_last(&f, f.label, last);
    assert_eq!(first, Point::new(0, 0));
    assert_eq!(last, Point::new(29, 0));
}

#[test]
fn label_get_letter_pos_align_left_on_empty_text() {
    let mut f = set_up();
    f.display
        .ui
        .obj_set_style_text_align(f.empty_label.obj(), TextAlign::Left, MAIN);
    let last = last_idx(&f, f.empty_label);
    let (first, last) = first_and_last(&f, f.empty_label, last);
    assert_eq!(first, Point::new(0, 0));
    assert_eq!(last, Point::new(0, 0));
}

/// The three `lv_label_get_letter_pos()` calls of the
/// `test_label_long_text_multiline_get_letter_pos_align_*` tests.
fn multiline_letter_pos(align: TextAlign) -> [Point; 3] {
    let mut f = set_up();
    let obj = f.long_label_multiline.obj();
    f.display.ui.obj_set_style_text_align(obj, align, MAIN);
    let last_letter_idx = last_idx(&f, f.long_label_multiline);
    let last_letter_after_new_line = last_letter_idx.wrapping_add(1);
    let (first, last) = first_and_last(&f, f.long_label_multiline, last_letter_idx);
    let after = f
        .long_label_multiline
        .letter_pos(&f.display.ui, last_letter_after_new_line);
    [first, last, after]
}

#[test]
fn label_long_text_multiline_get_letter_pos_align_left() {
    assert_eq!(
        multiline_letter_pos(TextAlign::Left),
        [Point::new(0, 0), Point::new(305, 32), Point::new(0, 48)]
    );
}

/// The `lv_label_get_letter_pos()` calls of the
/// `test_label_long_text_get_letter_pos_align_*` tests.
fn long_text_letter_pos(align: TextAlign) -> (Point, Point) {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let obj = f.long_label.obj();
    f.long_label.set_long_mode(ui, LabelLongMode::Wrap);
    ui.obj_set_width(obj, 150);
    ui.obj_set_height(obj, 500);
    ui.obj_set_style_text_align(obj, align, MAIN);
    let last = last_idx(&f, f.long_label);
    first_and_last(&f, f.long_label, last)
}

#[test]
fn label_long_text_get_letter_pos_align_left() {
    assert_eq!(
        long_text_letter_pos(TextAlign::Left),
        (Point::new(0, 0), Point::new(0, 1328))
    );
}

#[test]
fn label_get_letter_pos_align_right() {
    let mut f = set_up();
    f.display
        .ui
        .obj_set_style_text_align(f.label.obj(), TextAlign::Right, MAIN);
    let last = last_idx(&f, f.long_label_multiline);
    let (first, last) = first_and_last(&f, f.label, last);
    assert_eq!(first, Point::new(-29, 0));
    assert_eq!(last, Point::new(0, 0));
}

#[test]
fn label_get_letter_pos_align_right_on_empty_text() {
    let mut f = set_up();
    f.display
        .ui
        .obj_set_style_text_align(f.empty_label.obj(), TextAlign::Right, MAIN);
    let last = last_idx(&f, f.empty_label);
    let (first, last) = first_and_last(&f, f.empty_label, last);
    assert_eq!(first, Point::new(0, 0));
    assert_eq!(last, Point::new(0, 0));
}

#[test]
fn label_long_text_multiline_get_letter_pos_align_right() {
    assert_eq!(
        multiline_letter_pos(TextAlign::Right),
        [Point::new(-205, 0), Point::new(0, 32), Point::new(0, 48)]
    );
}

#[test]
fn label_long_text_get_letter_pos_align_right() {
    assert_eq!(
        long_text_letter_pos(TextAlign::Right),
        (Point::new(-8, 0), Point::new(-3, 1328))
    );
}

#[test]
fn label_get_letter_pos_align_center() {
    let mut f = set_up();
    f.display
        .ui
        .obj_set_style_text_align(f.label.obj(), TextAlign::Center, MAIN);
    let last = last_idx(&f, f.long_label_multiline);
    let (first, last) = first_and_last(&f, f.label, last);
    assert_eq!(first, Point::new(-14, 0));
    assert_eq!(last, Point::new(15, 0));
}

#[test]
fn label_get_letter_pos_align_center_on_empty_text() {
    let mut f = set_up();
    f.display
        .ui
        .obj_set_style_text_align(f.empty_label.obj(), TextAlign::Center, MAIN);
    let last = last_idx(&f, f.empty_label);
    let (first, last) = first_and_last(&f, f.empty_label, last);
    assert_eq!(first, Point::new(0, 0));
    assert_eq!(last, Point::new(0, 0));
}

#[test]
fn label_long_text_multiline_get_letter_pos_align_center() {
    assert_eq!(
        multiline_letter_pos(TextAlign::Center),
        [Point::new(-102, 0), Point::new(153, 32), Point::new(0, 48)]
    );
}

#[test]
fn label_long_text_get_letter_pos_align_center() {
    assert_eq!(
        long_text_letter_pos(TextAlign::Center),
        (Point::new(-4, 0), Point::new(-1, 1328))
    );
}

/// `test_label_is_char_under_pos_align_*`: the last letter is under its own
/// position, but not 50 px away (`dx` is `+50` or `-50`).
fn is_char_under_pos(align: TextAlign, dx: i32) {
    let mut f = set_up();
    f.display.ui.obj_set_style_text_align(f.label.obj(), align, MAIN);

    /* Getting the last letter point */
    let last_letter_idx = last_idx(&f, f.label);
    let mut last_letter_point = f.label.letter_pos(&f.display.ui, last_letter_idx);

    /* We should get true, since we know the last letter is in this coordinate */
    assert!(f.label.is_char_under_pos(&f.display.ui, last_letter_point));

    /* If we test a move away from the last letter then we should get false */
    last_letter_point.x += dx;
    assert!(!f.label.is_char_under_pos(&f.display.ui, last_letter_point));
}

#[test]
fn label_is_char_under_pos_align_left() {
    is_char_under_pos(TextAlign::Left, 50);
}

#[test]
fn label_is_char_under_pos_align_right() {
    is_char_under_pos(TextAlign::Right, -50);
}

#[test]
fn label_is_char_under_pos_align_center() {
    is_char_under_pos(TextAlign::Center, -50);
}

#[test]
fn label_cut_text() {
    let mut f = set_up();
    let expected_text = "Te";
    /* label has the default text which is "Text", let's remove "xt" from it */
    f.label.cut_text(&mut f.display.ui, 2, 2);
    assert_eq!(expected_text, f.label.text(&f.display.ui));
}

/// `test_label_get_letter_on_*`
fn get_letter_on(align: TextAlign) {
    let mut f = set_up();
    f.display.ui.obj_set_style_text_align(f.label.obj(), align, MAIN);
    let last_letter_idx = last_idx(&f, f.label);
    let last_letter_point = f.label.letter_pos(&f.display.ui, last_letter_idx);
    let letter_idx_result = f.label.letter_on(&f.display.ui, last_letter_point, true);
    assert_eq!(last_letter_idx, letter_idx_result);
}

#[test]
fn label_get_letter_on_left() {
    get_letter_on(TextAlign::Left);
}

#[test]
fn label_get_letter_on_center() {
    get_letter_on(TextAlign::Center);
}

#[test]
fn label_get_letter_on_right() {
    get_letter_on(TextAlign::Right);
}

#[test]
fn label_text_selection() {
    let mut f = set_up();
    let selection_start = 0;
    let selection_end = last_idx(&f, f.label);

    f.label
        .set_text_selection_start(&mut f.display.ui, selection_start);
    f.label.set_text_selection_end(&mut f.display.ui, selection_end);

    assert_eq!(selection_start, f.label.text_selection_start(&f.display.ui));
    assert_eq!(selection_end, f.label.text_selection_end(&f.display.ui));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_max_width() {
    let mut f = set_up();
    let screen = f.display.screen().unwrap();
    let ui = &mut f.display.ui;
    ui.obj_clean(screen);

    let test_label1 = Label::create(ui, screen);
    test_label1.set_text(ui, LONG_TEXT);
    let obj1 = test_label1.obj();
    ui.obj_set_width(obj1, 600);
    ui.obj_set_style_max_width(obj1, 200, MAIN);
    ui.obj_set_style_bg_color(obj1, palette::main(Palette::Grey), MAIN);
    ui.obj_set_style_bg_opa(obj1, OPA_100, MAIN);

    let test_label2 = Label::create(ui, screen);
    test_label2.set_text(ui, LONG_TEXT);
    let obj2 = test_label2.obj();
    ui.obj_set_width(obj2, 600);
    ui.obj_set_height(obj2, 50);
    ui.obj_set_x(obj2, 300);
    ui.obj_set_style_max_width(obj2, 200, MAIN);
    ui.obj_set_style_bg_color(obj2, palette::main(Palette::Grey), MAIN);
    ui.obj_set_style_bg_opa(obj2, OPA_100, MAIN);
    test_label2.set_long_mode(ui, LabelLongMode::Dots);

    f.display
        .screenshot_compare("widgets/label_max_width.png")
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_dots() {
    let mut f = set_up();
    let screen = f.display.screen().unwrap();
    let ui = &mut f.display.ui;
    ui.obj_clean(screen);

    let cont = ui.obj_create(Some(screen));
    ui.obj_set_size(cont, coord::pct(100), coord::pct(100));
    ui.obj_set_flex_flow(cont, FlexFlow::Column);

    let label1 = Label::create(ui, cont);
    let label2 = Label::create(ui, cont);
    let label3 = Label::create(ui, cont);
    let label4 = Label::create(ui, cont);

    for label in [label1, label2, label3, label4] {
        ui.obj_set_size(label.obj(), 150, 30);
    }

    let temp_text = "Some other text";

    label1.set_text(ui, LONG_TEXT);
    label2.set_text(ui, temp_text);

    /* `lv_label_set_text_fmt(label, "%.*s", (int)strlen(text), text)` */
    label3.set_text_fmt(ui, format_args!("{LONG_TEXT}"));
    label4.set_text_fmt(ui, format_args!("{temp_text}"));

    for label in [label1, label2, label3, label4] {
        label.set_long_mode(ui, LabelLongMode::Dots);
    }

    /* Setting the text with _fmt when long mode was dots would cause the incorrect text to be displayed */
    label2.set_text(ui, LONG_TEXT);
    label4.set_text_fmt(ui, format_args!("{LONG_TEXT}"));

    f.display.screenshot_compare("widgets/label_dots.png").unwrap();
}

/// `&lv_font_montserrat_22`
fn montserrat_22() -> FontRef {
    FontRef::Static(&ferrovgl_fonts::MONTSERRAT_22)
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_recolor_with_text_wrap() {
    let mut f = set_up();
    let screen = f.display.screen().unwrap();
    let ui = &mut f.display.ui;
    ui.obj_clean(screen);

    /* Create a label with recolor enabled and text wrapping */
    let label_recolor_wrap = Label::create(ui, screen);
    let obj = label_recolor_wrap.obj();

    /* Configure the label similar to the bug report */
    ui.obj_set_style_text_font(obj, montserrat_22(), MAIN);
    ui.obj_set_style_text_color(obj, Color::hex(0x8199f7), MAIN);
    ui.obj_set_width(obj, 300); /* Fixed width to force wrapping */
    ui.obj_set_style_text_align(obj, TextAlign::Center, MAIN);
    label_recolor_wrap.set_long_mode(ui, LabelLongMode::Wrap);
    label_recolor_wrap.set_recolor(ui, true);

    /* Test text with recolor commands that should wrap across multiple lines */
    let test_text_with_recolor = "Before color. #ff0000 This is a very long red colored text that should maintain its red color even when it wraps across multiple lines due to the label width being too narrow to contain the entire text on a single line# After color.";

    label_recolor_wrap.set_text(ui, test_text_with_recolor);
    ui.obj_align(obj, Align::TopMid, 0, 20);

    /* Create a reference label without recolor for comparison */
    let label_no_recolor = Label::create(ui, screen);
    let obj = label_no_recolor.obj();
    ui.obj_set_style_text_font(obj, montserrat_22(), MAIN);
    ui.obj_set_style_text_color(obj, Color::hex(0x8199f7), MAIN);
    ui.obj_set_width(obj, 300);
    ui.obj_set_style_text_align(obj, TextAlign::Center, MAIN);
    label_no_recolor.set_long_mode(ui, LabelLongMode::Wrap);
    label_no_recolor.set_recolor(ui, false); /* Disabled for comparison */

    /* Same text but without recolor commands */
    let test_text_without_recolor = "Before color. This is a very long red colored text that should maintain its red color even when it wraps across multiple lines due to the label width being too narrow to contain the entire text on a single line After color.";

    label_no_recolor.set_text(ui, test_text_without_recolor);
    ui.obj_align(obj, Align::BottomMid, 0, -20);

    f.display
        .screenshot_compare("widgets/label_recolor_2.png")
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_max_lines() {
    let mut f = set_up();
    let screen = f.display.screen().unwrap();
    let ui = &mut f.display.ui;
    ui.obj_clean(screen);
    let parent = ui.obj_create(Some(screen));
    ui.obj_set_size(parent, coord::pct(100), coord::pct(100));
    ui.obj_set_flex_flow(parent, FlexFlow::Row);
    ui.obj_set_flex_align(parent, FlexAlign::Center, FlexAlign::Center, FlexAlign::Center);
    ui.obj_set_style_text_line_space(parent, 10, MAIN);

    let texts = [
        "Fits in single line",
        "Length such that exactly three lines will be needed for this text",
        "Text\nwith line breaks\nwhere display will need more than three lines",
    ];

    for text in texts {
        let test_label = Label::create(ui, parent);
        let obj = test_label.obj();
        ui.obj_set_width(obj, 200);
        ui.obj_set_style_bg_color(obj, palette::main(Palette::Grey), MAIN);
        ui.obj_set_style_bg_opa(obj, OPA_100, MAIN);
        test_label.set_long_mode(ui, LabelLongMode::Dots);
        test_label.set_text(ui, text);
        test_label.set_max_lines(ui, 3);
    }

    f.display
        .screenshot_compare("widgets/label_max_lines.png")
        .unwrap();
}

#[test]
fn label_invalidate_area() {
    let mut f = set_up();
    let i = Rc::new(Cell::new(0_i32));
    let screen = f.display.screen().unwrap();
    let ui = &mut f.display.ui;
    let label = Label::create(ui, screen);

    /* In FULL render mode lv_inv_area() requests a whole-screen redraw via
     * LV_EVENT_REFR_REQUEST and never emits LV_EVENT_INVALIDATE_AREA, so listen
     * for whichever event signals "something got invalidated" for this display. */
    let disp = ui.display_get_default().unwrap();
    let inv_event = if ui.display_get_render_mode(disp) == RenderMode::Full {
        EventCode::REFR_REQUEST
    } else {
        EventCode::INVALIDATE_AREA
    };
    /* `display_invalidate_area_cb()` */
    let cb: EventCallback = Rc::new(|_, e| {
        let i = e
            .user_data()
            .and_then(|data| data.downcast_ref::<Cell<i32>>())
            .unwrap();
        i.set(i.get() + 1);
    });
    let user_data: Rc<dyn std::any::Any> = i.clone();
    ui.display_add_event_cb(disp, cb.clone(), inv_event, Some(user_data.clone()));

    i.set(0);
    label.set_text_static(ui, "Hello world");
    assert!(i.get() > 0);

    i.set(0);
    label.set_text(ui, "Hello world");
    assert!(i.get() > 0);

    i.set(0);
    label.set_text_fmt(ui, format_args!("{}", "Hello world"));
    assert!(i.get() > 0);

    i.set(0);
    label.set_long_mode(ui, LabelLongMode::Scroll);
    assert!(i.get() > 0);

    /* LV_LABEL_TEXT_SELECTION */
    i.set(0);
    label.set_text_selection_start(ui, 1);
    assert!(i.get() > 0);

    i.set(0);
    label.set_text_selection_end(ui, 1);
    assert!(i.get() > 0);

    i.set(0);
    label.set_recolor(ui, true);
    assert!(i.get() > 0);

    i.set(0);
    label.insert_text(ui, 5, " world");
    assert!(i.get() > 0);

    i.set(0);
    label.cut_text(ui, 5, 5);
    assert!(i.get() > 0);

    i.set(0);
    ui.obj_set_style_align(label.obj(), Align::Center, MAIN);
    assert!(i.get() > 0);

    ui.display_remove_event_cb_with_user_data(disp, &cb, &user_data);
}

/// `&lv_font_montserrat_40`
fn montserrat_40() -> FontRef {
    FontRef::Static(&ferrovgl_fonts::MONTSERRAT_40)
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_text_trim() {
    let mut f = set_up();
    let parent = f.display.screen().unwrap();
    let ui = &mut f.display.ui;
    ui.obj_clean(parent);
    ui.obj_set_flex_flow(parent, FlexFlow::Column);
    ui.obj_set_style_pad_row(parent, 32, MAIN);
    ui.obj_set_style_pad_all(parent, 32, MAIN);
    ui.obj_set_style_text_font(parent, montserrat_40(), MAIN);

    let rows = [
        ("Text Leading Trim None", None),
        ("Text Leading Trim Capital", Some(LeadingTrim::Capital)),
        ("Text Leading Trim Lower", Some(LeadingTrim::Lower)),
        (
            "Text Leading Trim Capital Baseline",
            Some(LeadingTrim::CapitalBaseline),
        ),
        (
            "Text Leading Trim Lower Baseline",
            Some(LeadingTrim::LowerBaseline),
        ),
    ];
    for (text, trim) in rows {
        let label = Label::create(ui, parent);
        label.set_text(ui, text);
        let obj = label.obj();
        ui.obj_set_style_bg_color(obj, Color::hex(0xFFCCCC), MAIN);
        ui.obj_set_style_bg_opa(obj, OPA_50, MAIN);
        if let Some(trim) = trim {
            ui.obj_set_style_text_leading_trim(obj, trim, MAIN);
        }
    }

    f.display
        .screenshot_compare("widgets/label_text_trim.png")
        .unwrap();
}

/// `create_wrap_pair()`: two labels which show the same text, side by side
/// under identical styles, on their own container.
fn create_wrap_pair(f: &mut Fixture, text_a: &str, text_b: &str) -> (Obj, Obj) {
    let screen = f.display.screen().unwrap();
    let ui = &mut f.display.ui;
    ui.obj_clean(screen);

    let cont = ui.obj_create(Some(screen));
    ui.obj_remove_style_all(cont);
    ui.obj_set_size(cont, 400, 200);
    ui.obj_set_style_text_font(
        cont,
        /* `LV_FONT_DEFAULT` is Montserrat 14 in the upstream test config */
        FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14),
        MAIN,
    );

    let mut out = [cont; 2];
    for (slot, text) in out.iter_mut().zip([text_a, text_b]) {
        let wrap_label = Label::create(ui, cont);
        wrap_label.set_long_mode(ui, LabelLongMode::Wrap);
        /*Wide enough for "hello world" but not for another word after it*/
        ui.obj_set_style_max_width(wrap_label.obj(), 100, MAIN);
        ui.obj_set_width(wrap_label.obj(), coord::SIZE_CONTENT);
        wrap_label.set_text(ui, text);
        *slot = wrap_label.obj();
    }

    ui.obj_update_layout(cont);
    (out[0], out[1])
}

#[test]
fn label_wrap_at_space_measures_like_explicit_newline() {
    let mut f = set_up();
    let (wrapped, hard_break) =
        create_wrap_pair(&mut f, "hello world hello world", "hello world\nhello world");
    let ui = &f.display.ui;

    /*Both show the same two lines, so both must measure the same*/
    assert_eq!(ui.obj_get_width(hard_break), ui.obj_get_width(wrapped));
    assert_eq!(ui.obj_get_height(hard_break), ui.obj_get_height(wrapped));
}

#[test]
fn label_trailing_spaces_do_not_widen_the_label() {
    let mut f = set_up();
    let (plain, padded) = create_wrap_pair(&mut f, "hello world", "hello world              ");
    let ui = &f.display.ui;
    assert_eq!(ui.obj_get_width(plain), ui.obj_get_width(padded));
}

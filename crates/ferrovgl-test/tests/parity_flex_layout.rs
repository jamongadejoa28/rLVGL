//! Scene ports from LVGL v9.6.0 `tests/src/test_cases/test_align_flex.c`
//! (every test after `test_align`, which `parity_flex` already ports) and
//! `tests/src/test_cases/test_margin_flex.c`. `test_flex_grow.c` has a single
//! test, `test_grid_fr`, ported by `parity_flex` as `flex_grow.png`.
//!
//! Upstream runs a C file as one suite on one screen and `tearDown()` only
//! calls `lv_obj_clean(active_screen)`, so the screen keeps its local flex
//! styles from one test to the next. Each Rust test therefore starts with the
//! screen styles the previous upstream test left behind ([`inherit_screen_flex`])
//! before running its own body.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::color::OPA_30;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Obj, Part, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Button, Label};

const DEFAULT: StyleSelector = StyleSelector::DEFAULT;
const MAIN: Part = Part::MAIN;

/// `lv_obj_set_flex_flow()` + `lv_obj_set_flex_align()` on the active screen,
/// the screen state a previous upstream test leaves behind.
fn inherit_screen_flex(
    display: &mut TestDisplay,
    flow: FlexFlow,
    main: FlexAlign,
    cross: FlexAlign,
    track: FlexAlign,
) -> Obj {
    let screen = display.screen().unwrap();
    display.ui.obj_set_flex_flow(screen, flow);
    display.ui.obj_set_flex_align(screen, main, cross, track);
    screen
}

/// Body shared by the `test_wrap_grow_*` functions (`test_align_flex.c:81..222`):
/// `count` items, the item at `new_track` starts a new track, `size` applies the
/// test's own min/max width styles.
fn wrap_grow(display: &mut TestDisplay, count: i32, new_track: i32, mut size: impl FnMut(&mut Ui, Obj, i32)) {
    let screen = inherit_screen_flex(
        display,
        FlexFlow::RowWrap,
        FlexAlign::Center,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    for i in 0..count {
        let ui = &mut display.ui;
        let obj = ui.obj_create(Some(screen));
        size(ui, obj, i);
        let grow = (i % 3) as u8;
        ui.obj_set_flex_grow(obj, grow);
        if i == new_track {
            ui.obj_set_flex_in_new_track(obj, true);
        }
        let label = Label::create(ui, obj);
        let suffix = if i == new_track { "\nnew track" } else { "" };
        label.set_text_fmt(ui, format_args!("grow:{grow}{suffix}"));
        ui.obj_center(label.obj());
    }
}

/// `test_wrap_grow_min_width` (see LVGL issue 7035).
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_wrap_grow_min_width() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    wrap_grow(&mut display, 14, 9, |ui, obj, i| {
        ui.obj_set_style_min_width(obj, 100, DEFAULT);
        ui.obj_set_style_pad_all(obj, 0, DEFAULT);
        ui.obj_set_height(obj, 70 + (i % 3) * 20);
    });
    display
        .screenshot_compare("flex_wrap_grow_min_width.png")
        .unwrap();
}

/// `test_wrap_grow_min_width_content`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_wrap_grow_min_width_content() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    // The C loop interleaves its styles with the grow/new-track calls; none of
    // them reads layout, so only the order among styles matters.
    let screen = inherit_screen_flex(
        &mut display,
        FlexFlow::RowWrap,
        FlexAlign::Center,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    for i in 0..17 {
        let ui = &mut display.ui;
        let obj = ui.obj_create(Some(screen));
        ui.obj_set_style_min_width(obj, coord::SIZE_CONTENT, DEFAULT);
        ui.obj_set_style_pad_all(obj, 0, DEFAULT);
        ui.obj_set_height(obj, 70 + (i % 3) * 20);
        ui.obj_set_width(obj, 120);
        let grow = (i % 3) as u8;
        ui.obj_set_flex_grow(obj, grow);
        if i == 7 {
            ui.obj_set_style_min_width(obj, 0, DEFAULT);
        }
        if i == 12 {
            ui.obj_set_flex_in_new_track(obj, true);
        }
        let label = Label::create(ui, obj);
        let suffix = if i == 12 { "\nnew track" } else { "" };
        label.set_text_fmt(ui, format_args!("grow:{grow}{suffix}"));
        ui.obj_center(label.obj());
    }
    display
        .screenshot_compare("flex_wrap_grow_min_width_content.png")
        .unwrap();
}

/// `test_wrap_grow_min_width_pct`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_wrap_grow_min_width_pct() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    wrap_grow(&mut display, 14, 9, |ui, obj, i| {
        ui.obj_set_style_min_width(obj, coord::pct(20), DEFAULT);
        ui.obj_set_style_pad_all(obj, 0, DEFAULT);
        ui.obj_set_height(obj, 70 + (i % 3) * 20);
    });
    display
        .screenshot_compare("flex_wrap_grow_min_width_pct.png")
        .unwrap();
}

/// `test_wrap_grow_max_width_content`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_wrap_grow_max_width_content() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    wrap_grow(&mut display, 14, 9, |ui, obj, i| {
        ui.obj_set_style_max_width(obj, coord::SIZE_CONTENT, DEFAULT);
        ui.obj_set_style_pad_all(obj, 0, DEFAULT);
        ui.obj_set_height(obj, 70 + (i % 3) * 20);
    });
    display
        .screenshot_compare("flex_wrap_grow_max_width_content.png")
        .unwrap();
}

/// `test_wrap_grow_max_width_pct`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_wrap_grow_max_width_pct() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    wrap_grow(&mut display, 14, 9, |ui, obj, i| {
        ui.obj_set_style_max_width(obj, coord::pct(20), DEFAULT);
        ui.obj_set_style_pad_all(obj, 0, DEFAULT);
        ui.obj_set_height(obj, 70 + (i % 3) * 20);
    });
    display
        .screenshot_compare("flex_wrap_grow_max_width_pct.png")
        .unwrap();
}

/// `create_col_wrap()` (`test_align_flex.c:222`) when `column`, otherwise
/// `create_row_wrap()` (`test_align_flex.c:269`).
fn create_wrap(display: &mut TestDisplay, count: usize, column: bool) -> Obj {
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let cont = ui.obj_create(Some(screen));
    ui.obj_set_flex_flow(
        cont,
        if column {
            FlexFlow::ColumnWrap
        } else {
            FlexFlow::RowWrap
        },
    );
    ui.obj_set_flex_align(cont, FlexAlign::Center, FlexAlign::Start, FlexAlign::SpaceEvenly);
    let base = if column { 70 } else { 50 };
    for i in 0..count {
        let obj = ui.obj_create(Some(cont));
        ui.obj_set_style_pad_all(obj, 0, DEFAULT);
        ui.obj_set_height(obj, base + (i % 3) as i32 * 20);
        ui.obj_set_width(obj, 100);
        ui.obj_set_style_bg_color(obj, palette::main(Palette::Blue), DEFAULT);
        let label = Label::create(ui, obj);
        label.set_text_fmt(ui, format_args!("{i}"));
        ui.obj_center(label.obj());
    }
    cont
}

/// `test_col_wrap_grow_size_content`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_col_wrap_grow_size() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    inherit_screen_flex(
        &mut display,
        FlexFlow::Row,
        FlexAlign::Center,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    let obj = create_wrap(&mut display, 12, true);
    display.ui.obj_set_size(obj, coord::SIZE_CONTENT, coord::pct(100));
    let obj = create_wrap(&mut display, 12, true);
    display.ui.obj_set_size(obj, coord::SIZE_CONTENT, coord::pct(100));
    display.ui.obj_set_flex_grow(obj, 1);
    display.screenshot_compare("flex_col_wrap_grow_size.png").unwrap();
}

/// `test_col_wrap_grow_size_content_2`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_col_wrap_grow_size_2() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    inherit_screen_flex(
        &mut display,
        FlexFlow::Column,
        FlexAlign::Center,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    let obj = create_wrap(&mut display, 12, true);
    display.ui.obj_set_size(obj, coord::pct(100), coord::pct(50));
    let obj = create_wrap(&mut display, 12, true);
    display.ui.obj_set_size(obj, coord::pct(100), coord::SIZE_CONTENT);
    display.ui.obj_set_flex_grow(obj, 1);
    display
        .screenshot_compare("flex_col_wrap_grow_size_2.png")
        .unwrap();
}

/// `test_row_wrap_grow_size_content`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_row_wrap_grow_size() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    inherit_screen_flex(
        &mut display,
        FlexFlow::Row,
        FlexAlign::Center,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    let obj = create_wrap(&mut display, 12, false);
    display.ui.obj_set_size(obj, coord::pct(50), coord::pct(100));
    let obj = create_wrap(&mut display, 12, false);
    display.ui.obj_set_size(obj, coord::SIZE_CONTENT, coord::pct(100));
    display.ui.obj_set_flex_grow(obj, 1);
    display.screenshot_compare("flex_row_wrap_grow_size.png").unwrap();
}

/// `test_row_wrap_grow_size_content_2`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_row_wrap_grow_size_2() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    inherit_screen_flex(
        &mut display,
        FlexFlow::Column,
        FlexAlign::Center,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    let obj = create_wrap(&mut display, 12, false);
    display.ui.obj_set_size(obj, coord::pct(100), coord::SIZE_CONTENT);
    let obj = create_wrap(&mut display, 12, false);
    display.ui.obj_set_size(obj, coord::pct(100), coord::SIZE_CONTENT);
    display.ui.obj_set_flex_grow(obj, 1);
    display
        .screenshot_compare("flex_row_wrap_grow_size_2.png")
        .unwrap();
}

/// The screen state `test_row_wrap_grow_size_content_2` leaves for the three
/// assertion-only tests that follow it.
fn after_row_wrap_grow_size_2() -> (TestDisplay, Obj) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = inherit_screen_flex(
        &mut display,
        FlexFlow::Column,
        FlexAlign::Center,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    (display, screen)
}

/// `test_col_grow_size_content` (`test_align_flex.c:315..443`) and
/// `test_row_grow_size_content` (`test_align_flex.c:449..577`) are the same
/// test along the two axes; `row` selects the second one.
fn grow_size_content(row: bool) {
    let (mut display, screen) = after_row_wrap_grow_size_2();
    let ui = &mut display.ui;
    // Height/width helpers along the flex axis of `cont`.
    let main_size = |ui: &Ui, obj: Obj| {
        if row {
            ui.obj_get_width(obj)
        } else {
            ui.obj_get_height(obj)
        }
    };
    let set_min = |ui: &mut Ui, obj: Obj, v: i32| {
        if row {
            ui.obj_set_style_min_width(obj, v, MAIN.into());
        } else {
            ui.obj_set_style_min_height(obj, v, MAIN.into());
        }
    };
    let set_max = |ui: &mut Ui, obj: Obj, v: i32| {
        if row {
            ui.obj_set_style_max_width(obj, v, MAIN.into());
        } else {
            ui.obj_set_style_max_height(obj, v, MAIN.into());
        }
    };

    let cont = ui.obj_create(Some(screen));
    ui.obj_set_name(cont, Some("cont".into()));
    if row {
        ui.obj_set_size(cont, coord::SIZE_CONTENT, coord::pct(100));
        ui.obj_set_flex_flow(cont, FlexFlow::Row);
    } else {
        ui.obj_set_size(cont, coord::pct(100), coord::SIZE_CONTENT);
        ui.obj_set_flex_flow(cont, FlexFlow::Column);
    }
    ui.obj_set_style_bg_color(cont, Color::hex(0xff0000), DEFAULT);
    ui.obj_set_style_bg_opa(cont, 255, DEFAULT);

    let pad_all = if row {
        ui.obj_get_style_pad_left(cont, MAIN)
    } else {
        ui.obj_get_style_pad_top(cont, MAIN)
    };
    let pad_gap = ui.obj_get_style_pad_row(cont, MAIN);
    let (space_start, space_end) = if row {
        (
            ui.obj_get_style_space_left(cont, MAIN),
            ui.obj_get_style_space_right(cont, MAIN),
        )
    } else {
        (
            ui.obj_get_style_space_top(cont, MAIN),
            ui.obj_get_style_space_bottom(cont, MAIN),
        )
    };
    ui.obj_set_style_pad_all(cont, pad_all, DEFAULT);
    ui.obj_set_style_pad_gap(cont, pad_gap, DEFAULT);

    let first = Label::create(ui, cont);
    ui.obj_set_name(first.obj(), Some(if row { "left" } else { "header" }.into()));
    first.set_text(ui, if row { "left" } else { "header" });

    let item = ui.obj_create(Some(cont));
    ui.obj_set_name(item, Some("item".into()));
    if row {
        ui.obj_set_height(item, coord::pct(100));
    } else {
        ui.obj_set_width(item, coord::pct(100));
    }
    ui.obj_set_flex_grow(item, 1);
    ui.obj_set_style_bg_color(item, Color::hex(0x00ff00), DEFAULT);
    ui.obj_set_style_bg_opa(item, 255, DEFAULT);

    let last = Label::create(ui, cont);
    ui.obj_set_name(last.obj(), Some(if row { "right" } else { "footer" }.into()));
    last.set_text(ui, if row { "right" } else { "footer" });

    ui.obj_update_layout(cont);
    let first_size = main_size(ui, first.obj());
    let last_size = main_size(ui, last.obj());

    let fixed_size = space_start + first_size + 2 * pad_gap + last_size + space_end;
    assert!(fixed_size > 0);
    assert_eq!(fixed_size, main_size(ui, cont));
    assert_eq!(0, main_size(ui, item));

    let item_min = 200;
    set_min(ui, item, item_min);
    ui.obj_update_layout(cont);
    assert_eq!(item_min + fixed_size, main_size(ui, cont));
    assert_eq!(item_min, main_size(ui, item));

    let item_min = 0;
    let cont_min = 400;
    if row {
        set_min(ui, item, item_min);
        set_min(ui, cont, cont_min);
    } else {
        set_min(ui, cont, cont_min);
        set_min(ui, item, item_min);
    }
    ui.obj_update_layout(cont);
    assert_eq!(cont_min, main_size(ui, cont));
    assert_eq!(cont_min - fixed_size, main_size(ui, item));

    let item_max = 200;
    set_max(ui, item, item_max);
    ui.obj_update_layout(cont);
    assert!(item_max < cont_min - fixed_size);
    assert_eq!(cont_min, main_size(ui, cont));
    assert_eq!(item_max, main_size(ui, item));

    let cont_min = 0;
    let cont_max = 500;
    let item_max = 0;
    assert!(cont_max > fixed_size);
    set_min(ui, cont, cont_min);
    set_max(ui, item, item_max);
    set_max(ui, cont, cont_max);
    ui.obj_update_layout(cont);
    assert_eq!(0, main_size(ui, item));
    assert_eq!(fixed_size, main_size(ui, cont));

    let item_max = 0;
    let cont_max = 3 * fixed_size / 4;
    set_max(ui, item, item_max);
    set_max(ui, cont, cont_max);
    ui.obj_update_layout(cont);
    assert_eq!(0, main_size(ui, item));
    assert_eq!(cont_max, main_size(ui, cont));

    let item_min = 50;
    set_min(ui, item, item_min);
    ui.obj_update_layout(cont);
    assert_eq!(item_min, main_size(ui, item));
    assert_eq!(cont_max, main_size(ui, cont));

    let cont_max = coord::pct(70);
    assert!(cont_max > fixed_size + item_min);
    set_max(ui, cont, cont_max);
    ui.obj_update_layout(cont);
    assert_eq!(item_min, main_size(ui, item));
    assert_eq!(fixed_size + item_min, main_size(ui, cont));
}

/// `test_col_grow_size_content`
#[test]
fn flex_col_grow_size_content() {
    grow_size_content(false);
}

/// `test_row_grow_size_content`
#[test]
fn flex_row_grow_size_content() {
    grow_size_content(true);
}

/// `test_nested_flex_grow_size_content` (`test_align_flex.c:580..680`).
#[test]
fn flex_nested_grow_size_content() {
    let (mut display, screen) = after_row_wrap_grow_size_2();
    let ui = &mut display.ui;
    let cont = ui.obj_create(Some(screen));
    ui.obj_set_name(cont, Some("cont".into()));
    ui.obj_set_style_min_width(cont, coord::SIZE_CONTENT, MAIN.into());
    ui.obj_set_size(cont, coord::pct(100), coord::SIZE_CONTENT);
    ui.obj_set_flex_flow(cont, FlexFlow::Row);
    ui.obj_set_style_bg_color(cont, Color::hex(0xff0000), DEFAULT);
    ui.obj_set_style_bg_opa(cont, 255, DEFAULT);

    let pad_all = ui.obj_get_style_pad_left(cont, MAIN);
    let pad_gap = ui.obj_get_style_pad_row(cont, MAIN);
    let space_start = ui.obj_get_style_space_left(cont, MAIN);
    let space_end = ui.obj_get_style_space_right(cont, MAIN);
    ui.obj_set_style_pad_all(cont, pad_all, DEFAULT);
    ui.obj_set_style_pad_gap(cont, pad_gap, DEFAULT);

    let label = Label::create(ui, cont);
    ui.obj_set_name(label.obj(), Some("label".into()));
    label.set_text(ui, "label");

    let sub_cont = ui.obj_create(Some(cont));
    ui.obj_set_name(sub_cont, Some("sub_cont".into()));
    ui.obj_set_height(sub_cont, coord::SIZE_CONTENT);
    ui.obj_set_flex_grow(sub_cont, 1);
    ui.obj_set_flex_flow(sub_cont, FlexFlow::RowWrap);
    ui.obj_set_style_bg_color(sub_cont, Color::hex(0x00ff00), DEFAULT);
    ui.obj_set_style_bg_opa(sub_cont, 255, DEFAULT);
    ui.obj_set_style_pad_all(sub_cont, pad_all, DEFAULT);
    ui.obj_set_style_pad_gap(sub_cont, pad_gap, DEFAULT);

    ui.obj_update_layout(cont);
    let label_width = ui.obj_get_width(label.obj());
    let cont_width = ui.obj_get_width(cont);
    let fixed_size = space_start + label_width + pad_gap + space_end;

    assert_eq!(cont_width - fixed_size, ui.obj_get_width(sub_cont));
    assert_eq!(
        0,
        ui.obj_get_height(sub_cont)
            - (ui.obj_get_style_space_top(sub_cont, MAIN) + ui.obj_get_style_space_bottom(sub_cont, MAIN))
    );

    let items = 5;
    let mut total_item_width = 0;
    for i in 0..items {
        let item = Label::create(ui, sub_cont);
        ui.obj_set_name(item.obj(), Some("item_#".into()));
        item.set_text_fmt(ui, format_args!("item_{i}"));
        ui.obj_set_style_text_color(item.obj(), Color::black(), DEFAULT);
        ui.obj_update_layout(item.obj());
        total_item_width += ui.obj_get_width(item.obj());
    }

    ui.obj_set_style_max_width(sub_cont, coord::SIZE_CONTENT, MAIN.into());
    ui.obj_update_layout(cont);

    let content_size = ui.obj_get_style_space_left(sub_cont, MAIN)
        + total_item_width
        + (items - 1) * pad_gap
        + ui.obj_get_style_space_right(sub_cont, MAIN);
    assert!(content_size < cont_width - fixed_size);
    assert_eq!(content_size, ui.obj_get_width(sub_cont));

    let new_items = 20;
    for i in items..new_items {
        let item = Label::create(ui, sub_cont);
        ui.obj_set_name(item.obj(), Some("item_#".into()));
        item.set_text_fmt(ui, format_args!("item_{i}"));
        ui.obj_set_style_text_color(item.obj(), Color::black(), DEFAULT);
        ui.obj_update_layout(item.obj());
        total_item_width += ui.obj_get_width(item.obj());
    }

    ui.obj_update_layout(sub_cont);
    let new_content_size = ui.obj_get_style_space_left(sub_cont, MAIN)
        + total_item_width
        + (new_items - 1) * pad_gap
        + ui.obj_get_style_space_right(sub_cont, MAIN);
    assert!(new_content_size > cont_width - fixed_size);
    assert_eq!(cont_width - fixed_size, ui.obj_get_width(sub_cont));

    ui.obj_set_style_max_width(sub_cont, coord::MAX, MAIN.into());
    ui.obj_update_layout(sub_cont);
    ui.obj_set_style_max_width(sub_cont, coord::SIZE_CONTENT, MAIN.into());
    assert_eq!(cont_width - fixed_size, ui.obj_get_width(sub_cont));
}

/// `cont_row_5_create()` (`test_align_flex.c:682`).
fn cont_row_5_create(ui: &mut Ui, screen: Obj) -> Obj {
    let cont = ui.obj_create(Some(screen));
    ui.obj_set_size(cont, coord::SIZE_CONTENT, coord::SIZE_CONTENT);
    ui.obj_center(cont);
    ui.obj_set_flex_flow(cont, FlexFlow::Row);
    for i in 0..5 {
        let btn = Button::create(ui, cont).obj();
        ui.obj_set_size(btn, 40, 60);
        let label = Label::create(ui, btn);
        label.set_text_fmt(ui, format_args!("{i}"));
        ui.obj_center(label.obj());
    }
    cont
}

/// `hide()` (`test_align_flex.c:700`).
fn hide(ui: &mut Ui, cont: Obj, idx: i32) {
    let child = ui.obj_get_child(cont, idx).unwrap();
    ui.obj_set_hidden(child, true);
}

/// The screen styles `test_flex_hide_items` sets and the two following tests
/// inherit.
fn hide_items_screen(display: &mut TestDisplay) -> Obj {
    inherit_screen_flex(
        display,
        FlexFlow::ColumnWrap,
        FlexAlign::Center,
        FlexAlign::Start,
        FlexAlign::SpaceEvenly,
    )
}

/// `test_flex_hide_items`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_hide_items() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = hide_items_screen(&mut display);
    let ui = &mut display.ui;
    let hidden: [&[i32]; 8] = [&[], &[0], &[0, 1], &[4], &[3, 4], &[0, 1, 3, 4], &[1, 3], &[0, 4]];
    for indices in hidden {
        let cont = cont_row_5_create(ui, screen);
        for &idx in indices {
            hide(ui, cont, idx);
        }
    }
    display.screenshot_compare("flex_hide_items.png").unwrap();
}

/// `test_flex_hide_item_first_grow`, on the screen `test_flex_hide_items` left.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_hide_items_first_grow() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = hide_items_screen(&mut display);
    let ui = &mut display.ui;
    let cont = ui.obj_create(Some(screen));
    ui.obj_set_flex_flow(cont, FlexFlow::Row);
    ui.obj_set_style_pad_column(cont, 50, DEFAULT);
    ui.obj_set_size(cont, 400, 80);

    let label1 = Label::create(ui, cont);
    label1.set_text(ui, "Hidden");
    ui.obj_set_hidden(label1.obj(), true);
    ui.obj_set_style_bg_color(label1.obj(), Color::hex3(0xf00), DEFAULT);
    ui.obj_set_style_bg_opa(label1.obj(), OPA_30, DEFAULT);

    let label2 = Label::create(ui, cont);
    label2.set_text(ui, "Growing");
    ui.obj_set_flex_grow(label2.obj(), 1);
    ui.obj_set_style_bg_color(label2.obj(), Color::hex3(0x0f0), DEFAULT);
    ui.obj_set_style_bg_opa(label2.obj(), OPA_30, DEFAULT);

    let label3 = Label::create(ui, cont);
    label3.set_text(ui, "Fixed");
    ui.obj_set_width(label3.obj(), 60);
    ui.obj_set_style_bg_color(label3.obj(), Color::hex3(0x00f), DEFAULT);
    ui.obj_set_style_bg_opa(label3.obj(), OPA_30, DEFAULT);

    display
        .screenshot_compare("flex_hide_items_first_grow.png")
        .unwrap();
}

/// `test_flex_use_rtl`, on the screen `test_flex_hide_items` left.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_use_rtl() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = hide_items_screen(&mut display);
    let ui = &mut display.ui;
    let test = ui.obj_create(Some(screen));
    ui.obj_center(test);
    ui.obj_set_width(test, coord::pct(40));
    ui.obj_set_height(test, coord::SIZE_CONTENT);
    ui.obj_set_style_border_width(test, 2, DEFAULT);
    ui.obj_set_style_base_dir(test, BaseDir::Rtl, DEFAULT);
    ui.obj_set_flex_flow(test, FlexFlow::RowWrap);
    ui.obj_set_flex_align(test, FlexAlign::SpaceEvenly, FlexAlign::Start, FlexAlign::Center);
    for _ in 0..8 {
        let obj = ui.obj_create(Some(test));
        ui.obj_set_size(obj, 70, 70);
        ui.obj_set_style_border_width(obj, 2, DEFAULT);
    }
    display.screenshot_compare("flex_use_rtl.png").unwrap();
}

/// `test_flex_grow_on_screen_should_not_crash`
#[test]
fn flex_grow_on_screen_should_not_crash() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = hide_items_screen(&mut display);
    display.ui.obj_set_flex_grow(screen, 1);
    display.ui.refr_now(None);
}

/// `obj_create()` of `test_margin_flex.c:20`.
fn margin_obj_create(ui: &mut Ui, parent: Obj, color: Color) -> Obj {
    let obj = ui.obj_create(Some(parent));
    ui.obj_set_style_bg_color(obj, color, MAIN.into());
    ui.obj_set_width(obj, 150);
    ui.obj_set_height(obj, 150);
    obj
}

/// `obj_set_margin()` of `test_margin_flex.c:30`.
fn obj_set_margin(ui: &mut Ui, obj: Obj, left: i32, top: i32, right: i32, bottom: i32) {
    ui.obj_set_style_margin_left(obj, left, MAIN.into());
    ui.obj_set_style_margin_top(obj, top, MAIN.into());
    ui.obj_set_style_margin_right(obj, right, MAIN.into());
    ui.obj_set_style_margin_bottom(obj, bottom, MAIN.into());
}

/// `test_margin_flex.c::test_flex`: six screenshots of one screen whose flex
/// styles change between them. Every comparison runs; failures are reported
/// together.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn margin_flex() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let obj0 = display.screen().unwrap();
    let ui = &mut display.ui;
    ui.obj_set_flex_flow(obj0, FlexFlow::Row);
    ui.obj_set_style_pad_all(obj0, 0, MAIN.into());
    ui.obj_set_style_pad_gap(obj0, 0, MAIN.into());
    ui.obj_set_style_border_width(obj0, 0, MAIN.into());

    let objs = [
        Palette::Blue,
        Palette::Red,
        Palette::Purple,
        Palette::Green,
        Palette::Pink,
    ]
    .map(|color| margin_obj_create(ui, obj0, palette::main(color)));
    for &o in &objs {
        ui.obj_set_style_radius(o, 0, MAIN.into());
        ui.obj_set_scrollbar_mode(o, ferrovgl_core::core::obj::ScrollbarMode::Off);
        ui.obj_set_style_border_width(o, 0, MAIN.into());
    }
    ui.obj_set_width(objs[0], coord::pct(50));
    ui.obj_set_height(objs[0], coord::pct(50));

    obj_set_margin(ui, objs[0], 50, 3, 10, 50);
    obj_set_margin(ui, objs[1], 25, 6, 20, 50);
    obj_set_margin(ui, objs[2], 12, 12, 30, 50);
    obj_set_margin(ui, objs[3], 12, 25, 30, 50);
    obj_set_margin(ui, objs[4], 24, 50, 50, 50);
    ui.obj_update_layout(obj0);

    let mut failures = Vec::new();
    let mut check = |display: &mut TestDisplay, name: &str| {
        if let Err(err) = display.screenshot_compare(name) {
            failures.push(format!("{name}: {err}"));
        }
    };
    check(&mut display, "margin_flex_0.png");
    display.ui.obj_set_flex_flow(obj0, FlexFlow::RowWrap);
    check(&mut display, "margin_flex_1.png");
    display
        .ui
        .obj_set_style_flex_main_place(obj0, FlexAlign::Center, MAIN.into());
    check(&mut display, "margin_flex_2.png");
    display
        .ui
        .obj_set_style_flex_cross_place(obj0, FlexAlign::Center, MAIN.into());
    check(&mut display, "margin_flex_3.png");
    display
        .ui
        .obj_set_style_flex_track_place(obj0, FlexAlign::Center, MAIN.into());
    check(&mut display, "margin_flex_4.png");
    display.ui.obj_set_flex_flow(obj0, FlexFlow::ColumnWrap);
    check(&mut display, "margin_flex_5.png");
    assert!(failures.is_empty(), "{failures:#?}");
}

//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_bar.c`.
//!
//! Every upstream test function, in upstream order, with its values and
//! screenshot names. `setUp` creates `g_bar` on the active screen of a fresh
//! default-theme display; `tearDown`'s `lv_obj_clean` is the display drop.

use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::color::{OPA_30, OPA_70, OPA_100};
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Obj, Part, Style, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Bar, BarMode, BarOrientation, Label};

const MAIN: StyleSelector = StyleSelector::DEFAULT;
const ANIM_OFF: bool = false;
const ANIM_ON: bool = true;

fn indicator() -> StyleSelector {
    Part::INDICATOR.into()
}

/// `setUp()`: `g_active_screen = lv_screen_active(); g_bar = lv_bar_create(g_active_screen);`
fn set_up() -> (TestDisplay, Obj, Bar) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    let bar = Bar::create(&mut display.ui, screen);
    (display, screen, bar)
}

#[test]
fn bar_should_have_valid_default_attributes() {
    let (display, _, g_bar) = set_up();
    assert_eq!(0, g_bar.min_value(&display.ui));
    assert_eq!(100, g_bar.max_value(&display.ui));
    assert_eq!(BarMode::Normal, g_bar.mode(&display.ui));
}

/// Bar properties assumed: mode `LV_BAR_MODE_NORMAL`, min 0, max 100, LTR.
#[test]
fn bar_should_update_indicator_right_coordinate_based_on_bar_value() {
    let (mut display, _, g_bar) = set_up();
    let style_padding = 5;
    let bar_width = 200;
    let bar_height = 20;
    let bar_value = 10;

    let mut bar_style = Style::new();
    bar_style.set_pad_all(style_padding);
    let bar_style = display.ui.style_create(bar_style);

    /* Setup new style */
    display.ui.obj_remove_style_all(g_bar.obj());
    display.ui.obj_add_style(g_bar.obj(), bar_style, MAIN);

    /* Set properties */
    display.ui.obj_set_size(g_bar.obj(), bar_width, bar_height);
    g_bar.set_value(&mut display.ui, bar_value, ANIM_OFF);

    /* FIXME: Remove wait */
    display.wait(50);

    let actual_coord = g_bar.indic_area(&display.ui).width();

    /* Calculate bar indicator right coordinate, using rule of 3 */
    let bar_max_value = g_bar.max_value(&display.ui);
    let indicator_part_width = display.ui.obj_get_content_width(g_bar.obj());

    let expected_coord = (bar_value * indicator_part_width) / bar_max_value;

    assert_eq!(expected_coord, actual_coord);
}

/// Bar properties assumed: mode `LV_BAR_MODE_NORMAL`, min 0, max 100.
#[test]
fn bar_rtl_should_update_indicator_left_coordinate_based_on_bar_value() {
    let (mut display, _, g_bar) = set_up();
    let style_padding = 5;
    let bar_width = 200;
    let bar_height = 20;
    let bar_value = 10;

    let mut bar_style = Style::new();
    bar_style.set_pad_all(style_padding);
    let bar_style = display.ui.style_create(bar_style);

    /* Setup new style */
    display.ui.obj_remove_style_all(g_bar.obj());
    display.ui.obj_add_style(g_bar.obj(), bar_style, MAIN);

    /* Set properties */
    display.ui.obj_set_size(g_bar.obj(), bar_width, bar_height);
    g_bar.set_value(&mut display.ui, bar_value, ANIM_OFF);
    display.ui.obj_set_style_base_dir(g_bar.obj(), BaseDir::Rtl, MAIN);

    /* FIXME: Remove wait */
    display.wait(50);

    let actual_coord = g_bar.indic_area(&display.ui).x1;

    /* Calculate current indicator width */
    let bar_max_value = g_bar.max_value(&display.ui);
    let indicator_part_width = display.ui.obj_get_content_width(g_bar.obj());
    let right_padding = display.ui.obj_get_style_pad_right(g_bar.obj(), Part::MAIN);
    let indicator_width = (bar_value * indicator_part_width) / bar_max_value;

    let expected_coord = (bar_width - right_padding) - indicator_width;

    assert_eq!(expected_coord, actual_coord);
}

/// The repeated block of `test_bar_normal` / `test_bar_normal_empty_and_full`.
#[allow(clippy::too_many_arguments)]
fn normal_bar(
    ui: &mut Ui,
    screen: Obj,
    min: i32,
    max: i32,
    value: i32,
    w: i32,
    h: i32,
    x: i32,
    y: i32,
    rtl: bool,
) {
    let bg_color = Color::black();
    let indic_color = Color::hex(0x0000FF);
    let test_bar = Bar::create(ui, screen);
    let obj = test_bar.obj();
    ui.obj_set_style_radius(obj, 100, MAIN);
    ui.obj_set_style_radius(obj, 100, MAIN);
    ui.obj_set_style_bg_opa(obj, OPA_30, MAIN);
    ui.obj_set_style_bg_color(obj, bg_color, MAIN);
    ui.obj_set_style_bg_color(obj, indic_color, indicator());
    test_bar.set_range(ui, min, max);
    test_bar.set_value(ui, value, ANIM_OFF);
    ui.obj_set_size(obj, w, h);
    ui.obj_align(obj, Align::TopLeft, x, y);
    if rtl {
        ui.obj_set_style_base_dir(obj, BaseDir::Rtl, MAIN);
    }
}

/// The white full-screen container of `test_bar_normal*`.
fn white_screen(ui: &mut Ui) -> Obj {
    let active = ui.display_get_screen_active(None);
    let screen = ui.obj_create(active);
    ui.obj_remove_style_all(screen);
    ui.obj_set_size(screen, 800, 480);
    ui.obj_center(screen);
    ui.obj_set_style_bg_color(screen, Color::white(), MAIN);
    ui.obj_set_style_bg_opa(screen, OPA_100, MAIN);
    ui.obj_set_style_pad_all(screen, 0, MAIN);
    screen
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn bar_normal() {
    let (mut display, _, _) = set_up();
    let w = 300;
    let h = 40;
    let h_gap = 20;
    let w_gap = 20;
    let mut y = h_gap;
    let mut x = w_gap;

    let ui = &mut display.ui;
    let screen = white_screen(ui);

    normal_bar(ui, screen, 0, 100, 30, w, h, x, y, false);
    y += h + h_gap;

    normal_bar(ui, screen, 0, 100, 30, w, h, x, y, true);
    y += h + h_gap;

    x = 150;
    normal_bar(ui, screen, 0, 100, 30, h, w, x, y, false);
    x += h + w_gap;

    normal_bar(ui, screen, 100, 0, 30, h, w, x, y, false);

    display.screenshot_compare("widgets/bar_1.png").unwrap();
}

/// Same as `test_bar_normal`, but with min and max values set.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn bar_normal_empty_and_full() {
    let (mut display, _, _) = set_up();
    let w = 300;
    let h = 40;
    let h_gap = 20;
    let w_gap = 20;
    let mut y = h_gap;
    let mut x = w_gap;

    let ui = &mut display.ui;
    let screen = white_screen(ui);

    normal_bar(ui, screen, 0, 100, 0, w, h, x, y, false);
    y += h + h_gap;

    normal_bar(ui, screen, 0, 100, 0, w, h, x, y, true);
    y += h + h_gap;

    x = 150;
    normal_bar(ui, screen, 0, 100, 0, h, w, x, y, false);
    x += h + w_gap;

    normal_bar(ui, screen, 100, 0, 0, h, w, x, y, false);

    y = h_gap;
    x = w_gap + 400;

    normal_bar(ui, screen, 0, 100, 100, w, h, x, y, false);
    y += h + h_gap;

    normal_bar(ui, screen, 0, 100, 100, w, h, x, y, true);
    y += h + h_gap;

    x += 150;
    normal_bar(ui, screen, 0, 100, 100, h, w, x, y, false);
    x += h + w_gap;

    normal_bar(ui, screen, 100, 0, 100, h, w, x, y, false);

    display.screenshot_compare("widgets/bar_3.png").unwrap();
}

#[test]
fn bar_indicator_area_should_get_smaller_when_padding_is_increased() {
    let (mut display, _, g_bar) = set_up();
    let style_padding = 10;

    g_bar.set_value(&mut display.ui, 50, ANIM_OFF);
    display.wait(50);

    let original_width = g_bar.indic_area(&display.ui).width();
    let original_height = g_bar.indic_area(&display.ui).height();

    /* Setup new padding */
    let mut bar_style = Style::new();
    bar_style.set_pad_all(style_padding);
    let bar_style = display.ui.style_create(bar_style);
    display.ui.obj_set_size(g_bar.obj(), 100, 50);

    /* Apply new style  */
    display.ui.obj_remove_style_all(g_bar.obj());
    display.ui.obj_add_style(g_bar.obj(), bar_style, MAIN);

    /* Notify LVGL of style change */
    display.ui.obj_report_style_change(Some(bar_style));
    display.wait(50);

    let new_height = g_bar.indic_area(&display.ui).height();
    let new_width = g_bar.indic_area(&display.ui).width();

    /* TEST_ASSERT_LESS_THAN_INT32(threshold, actual): actual < threshold */
    assert!(new_height < original_height, "{new_height} < {original_height}");
    assert!(new_width < original_width, "{new_width} < {original_width}");
}

#[test]
fn bar_start_value_should_only_change_when_in_range_mode() {
    let (mut display, _, g_bar) = set_up();
    let new_start_value = 20;

    g_bar.set_value(&mut display.ui, 90, ANIM_OFF);
    g_bar.set_start_value(&mut display.ui, new_start_value, ANIM_OFF);

    /* Start value shouldn't be updated when not in RANGE mode */
    assert_eq!(0, g_bar.start_value(&display.ui));

    /* Set bar in RANGE mode so we can edit the start value */
    g_bar.set_mode(&mut display.ui, BarMode::Range);
    g_bar.set_start_value(&mut display.ui, new_start_value, ANIM_OFF);

    assert_eq!(new_start_value, g_bar.start_value(&display.ui));
}

#[test]
fn bar_start_value_should_be_smaller_than_current_value_in_range_mode() {
    let (mut display, _, g_bar) = set_up();
    /* Set bar in RANGE mode so we can edit the start value */
    g_bar.set_mode(&mut display.ui, BarMode::Range);
    g_bar.set_value(&mut display.ui, 50, ANIM_OFF);
    g_bar.set_start_value(&mut display.ui, 100, ANIM_OFF);

    assert_eq!(g_bar.value(&display.ui), g_bar.start_value(&display.ui));
}

#[test]
fn bar_current_value_should_be_truncated_to_max_value_when_exceeds_it() {
    let (mut display, _, g_bar) = set_up();
    let max_value = g_bar.max_value(&display.ui);
    let new_value = max_value + 1;

    g_bar.set_value(&mut display.ui, new_value, ANIM_OFF);
    assert_eq!(max_value, g_bar.value(&display.ui));
}

#[test]
fn bar_current_value_should_be_truncated_to_min_value_when_it_is_below_it() {
    let (mut display, _, g_bar) = set_up();
    let min_value = g_bar.min_value(&display.ui);
    /* `min_value - 1u` wraps to -1 when converted back to int32_t */
    let new_value = (min_value as u32).wrapping_sub(1) as i32;

    g_bar.set_value(&mut display.ui, new_value, ANIM_OFF);
    assert_eq!(min_value, g_bar.value(&display.ui));
}

/// When in symmetrical mode, the bar indicator has to be drawn towards the
/// min range value. Requires a negative min range value and a positive max
/// range value. Bar properties assumed: base direction LTR.
#[test]
fn bar_indicator_should_be_drawn_towards_the_min_range_side_after_setting_a_more_negative_value() {
    let (mut display, _, g_bar) = set_up();

    /* Setup bar properties */
    display.ui.obj_set_size(g_bar.obj(), 100, 50);
    g_bar.set_mode(&mut display.ui, BarMode::Symmetrical);
    g_bar.set_range(&mut display.ui, -100, 100);

    /* Set bar value to 1, so it gets drawn at the middle of the bar */
    g_bar.set_value(&mut display.ui, 1, ANIM_OFF);
    display.wait(50);

    let original_pos = g_bar.indic_area(&display.ui).x1;

    /* Set bar to a more negative value */
    g_bar.set_value(&mut display.ui, -50, ANIM_OFF);
    display.wait(50);

    let final_pos = g_bar.indic_area(&display.ui).x1;

    /* TEST_ASSERT_LESS_THAN(threshold, actual): actual < threshold */
    assert!(final_pos < original_pos, "{final_pos} < {original_pos}");
}

/// `styled_bar_create()`
#[allow(clippy::too_many_arguments)]
fn styled_bar_create(
    ui: &mut Ui,
    ver: bool,
    start_value: i32,
    end_value: i32,
    grad_dir: GradDir,
    bg_radius: i32,
    indic_radius: i32,
    bg_pad: i32,
) -> Bar {
    let active = ui.display_get_screen_active(None);
    let bar = Bar::create(ui, active);
    let obj = bar.obj();
    if ver {
        ui.obj_set_size(obj, 20, 100);
    } else {
        ui.obj_set_size(obj, 100, 20);
    }
    bar.set_range(ui, 0, 100);
    bar.set_mode(ui, BarMode::Range);
    bar.set_value(ui, end_value, ANIM_OFF);
    bar.set_start_value(ui, start_value, ANIM_OFF);

    ui.obj_set_style_bg_opa(obj, 255, MAIN);
    ui.obj_set_style_bg_color(obj, Color::hex3(0x0ff), MAIN);
    ui.obj_set_style_radius(obj, bg_radius, MAIN);
    ui.obj_set_style_pad_all(obj, bg_pad, MAIN);

    let ind = indicator();
    ui.obj_set_style_bg_opa(obj, OPA_70, ind);
    ui.obj_set_style_bg_color(obj, Color::hex3(0xf0f), ind);
    ui.obj_set_style_bg_grad_color(obj, Color::hex3(0x8f8), ind);
    ui.obj_set_style_bg_grad_dir(obj, grad_dir, ind);

    ui.obj_set_style_border_width(obj, 2, ind);
    ui.obj_set_style_border_color(obj, Color::hex3(0x0f0), ind);
    ui.obj_set_style_border_opa(obj, OPA_70, ind);
    ui.obj_set_style_outline_width(obj, 2, ind);
    ui.obj_set_style_outline_pad(obj, 4, ind);
    ui.obj_set_style_outline_color(obj, Color::hex3(0xff0), ind);
    ui.obj_set_style_outline_opa(obj, OPA_70, ind);
    ui.obj_set_style_shadow_width(obj, 20, ind);
    ui.obj_set_style_shadow_spread(obj, 5, ind);
    ui.obj_set_style_shadow_color(obj, Color::hex3(0xf00), ind);
    ui.obj_set_style_shadow_opa(obj, OPA_70, ind);
    ui.obj_set_style_radius(obj, indic_radius, ind);

    bar
}

/// `render_test_screen_create()`
fn render_test_screen_create(display: &mut TestDisplay, ver: bool, grad_dir: GradDir, ref_img_path: &str) {
    let ui = &mut display.ui;
    let active_screen = ui.display_get_screen_active(None).unwrap();
    ui.obj_clean(active_screen);

    ui.obj_set_flex_flow(
        active_screen,
        if ver {
            FlexFlow::RowWrap
        } else {
            FlexFlow::ColumnWrap
        },
    );
    ui.obj_set_flex_align(
        active_screen,
        FlexAlign::Start,
        FlexAlign::Start,
        FlexAlign::SpaceEvenly,
    );
    ui.obj_set_style_pad_row(active_screen, 17, MAIN);
    ui.obj_set_style_pad_column(active_screen, 12, MAIN);
    ui.obj_set_style_pad_all(active_screen, 15, MAIN);

    let bar_values: [[i32; 2]; 6] = [[0, 5], [0, 50], [0, 95], [5, 100], [95, 100], [20, 90]];
    let bg_radius = [0, 15];
    let indic_radius = [0, 15];
    let bg_pad = [-5, 0, 5];

    for pad in bg_pad {
        for indic_r in indic_radius {
            for bg_r in bg_radius {
                for v in bar_values {
                    styled_bar_create(ui, ver, v[0], v[1], grad_dir, bg_r, indic_r, pad);
                }
            }
        }
    }

    display.screenshot_compare(ref_img_path).unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn bar_render_corner() {
    let (mut display, _, _) = set_up();
    render_test_screen_create(&mut display, false, GradDir::None, "widgets/bar_corner_1.png");
    render_test_screen_create(&mut display, false, GradDir::Hor, "widgets/bar_corner_2.png");
    render_test_screen_create(&mut display, false, GradDir::Ver, "widgets/bar_corner_3.png");
    render_test_screen_create(&mut display, true, GradDir::None, "widgets/bar_corner_4.png");
    render_test_screen_create(&mut display, true, GradDir::Hor, "widgets/bar_corner_5.png");
    render_test_screen_create(&mut display, true, GradDir::Ver, "widgets/bar_corner_6.png");
}

/// `bar_create_orientation()`
fn bar_create_orientation(ui: &mut Ui, screen: Obj, orientation: BarOrientation, w: i32, h: i32) -> Bar {
    let bar = Bar::create(ui, screen);
    bar.set_orientation(ui, orientation);
    ui.obj_set_size(bar.obj(), w, h);
    bar.set_value(ui, 30, ANIM_OFF);

    bar
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn bar_orientation() {
    let (mut display, g_active_screen, _) = set_up();
    let ui = &mut display.ui;
    /* Upstream runs every test of the file on one screen and `tearDown` only
     * cleans its children, so this test inherits the local styles that the
     * last `render_test_screen_create(true, ..)` of `test_bar_render_corner`
     * left on the active screen; `widgets/bar_2.png` shows them. */
    ui.obj_set_flex_flow(g_active_screen, FlexFlow::RowWrap);
    ui.obj_set_flex_align(
        g_active_screen,
        FlexAlign::Start,
        FlexAlign::Start,
        FlexAlign::SpaceEvenly,
    );
    ui.obj_set_style_pad_row(g_active_screen, 17, MAIN);
    ui.obj_set_style_pad_column(g_active_screen, 12, MAIN);
    ui.obj_set_style_pad_all(g_active_screen, 15, MAIN);

    ui.obj_clean(g_active_screen);

    ui.obj_set_flex_flow(g_active_screen, FlexFlow::RowWrap);

    for (text, orientation) in [
        ("Auto", BarOrientation::Auto),
        ("Vertical", BarOrientation::Vertical),
        ("Horizontal", BarOrientation::Horizontal),
    ] {
        let label = Label::create(ui, g_active_screen);
        label.set_text(ui, text);
        ui.obj_set_width(label.obj(), coord::pct(100));

        bar_create_orientation(ui, g_active_screen, orientation, 100, 20);
        bar_create_orientation(ui, g_active_screen, orientation, 20, 100);
        bar_create_orientation(ui, g_active_screen, orientation, 100, 100);
    }

    display.screenshot_compare("widgets/bar_2.png").unwrap();
}

/// `test_bar_properties` is compiled only with `LV_USE_OBJ_PROPERTY`, which
/// the parity configuration (`full.defconfig`) leaves off, so upstream runs an
/// empty body. Property reflection is not implemented; the same sequence is
/// checked through the functions the `lv_bar_properties[]` table maps to
/// (`lv_bar_set_value_helper` = `lv_bar_set_value(.., LV_ANIM_OFF)`, ...).
#[test]
fn bar_properties() {
    let (mut display, screen, _) = set_up();
    let ui = &mut display.ui;
    let obj = Bar::create(ui, screen);

    /* Test VALUE property */
    obj.set_value(ui, 50, ANIM_OFF);
    assert_eq!(50, obj.value(ui));

    /* Test MIN_VALUE property */
    obj.set_min_value(ui, 10);
    assert_eq!(10, obj.min_value(ui));

    /* Test MAX_VALUE property */
    obj.set_max_value(ui, 200);
    assert_eq!(200, obj.max_value(ui));

    /* Test MODE property */
    obj.set_mode(ui, BarMode::Range);
    assert_eq!(BarMode::Range, obj.mode(ui));

    /* Test START_VALUE property (only works in RANGE mode) */
    obj.set_start_value(ui, 20, ANIM_OFF);
    assert_eq!(20, obj.start_value(ui));

    /* Test ORIENTATION property */
    obj.set_orientation(ui, BarOrientation::Vertical);
    assert_eq!(BarOrientation::Vertical, obj.orientation(ui));

    ui.obj_delete(obj.obj());
}

#[test]
fn bar_large_range_cur_value_no_overflow() {
    let (mut display, _, g_bar) = set_up();
    /* Prevent int32 overflow when indic_w * (cur-min) > INT32_MAX */
    display.ui.obj_set_size(g_bar.obj(), 600, 24);
    g_bar.set_range(&mut display.ui, 0, 20_000_000);
    g_bar.set_value(&mut display.ui, 16_000_000, ANIM_OFF);
    display.ui.obj_update_layout(g_bar.obj());
    display.ui.refr_now(None);

    let indic_w = g_bar.indic_area(&display.ui).width();
    assert!(indic_w > 300, "{indic_w}");
}

#[test]
fn bar_large_range_start_value_no_overflow() {
    let (mut display, _, g_bar) = set_up();
    /* Prevent int32 overflow for start_value pixel position */
    display.ui.obj_set_size(g_bar.obj(), 600, 24);
    g_bar.set_mode(&mut display.ui, BarMode::Range);
    g_bar.set_range(&mut display.ui, 0, 20_000_000);
    g_bar.set_value(&mut display.ui, 19_000_000, ANIM_OFF);
    g_bar.set_start_value(&mut display.ui, 12_000_000, ANIM_OFF);
    display.ui.obj_update_layout(g_bar.obj());
    display.ui.refr_now(None);

    let x1 = g_bar.indic_area(&display.ui).x1;
    assert!(x1 > 200, "{x1}");
}

#[test]
fn bar_safe_values_preserved() {
    let (mut display, _, g_bar) = set_up();
    display.ui.obj_set_size(g_bar.obj(), 600, 24);
    g_bar.set_range(&mut display.ui, 0, 20_000_000);
    g_bar.set_value(&mut display.ui, 2_000_000, ANIM_OFF);
    display.ui.obj_update_layout(g_bar.obj());
    display.ui.refr_now(None);

    let indic_w = g_bar.indic_area(&display.ui).width();
    assert!(indic_w > 30 && indic_w < 150, "{indic_w}");
}

#[test]
fn bar_large_range_cur_value_anim_no_overflow() {
    let (mut display, _, g_bar) = set_up();
    /* Prevent int32 overflow during cur_value animation */
    display.ui.obj_set_size(g_bar.obj(), 600, 24);
    g_bar.set_range(&mut display.ui, 0, 20_000_000);
    g_bar.set_value(&mut display.ui, 2_000_000, ANIM_OFF);
    display.ui.refr_now(None);

    g_bar.set_value(&mut display.ui, 14_000_000, ANIM_ON);
    display.ui.obj_update_layout(g_bar.obj());
    display.ui.refr_now(None);

    let indic_w = g_bar.indic_area(&display.ui).width();
    assert!(indic_w > 300, "{indic_w}");
}

#[test]
fn bar_large_range_start_value_anim_no_overflow() {
    let (mut display, _, g_bar) = set_up();
    /* Prevent int32 overflow during start_value animation */
    display.ui.obj_set_size(g_bar.obj(), 600, 24);
    g_bar.set_mode(&mut display.ui, BarMode::Range);
    g_bar.set_range(&mut display.ui, 0, 20_000_000);
    g_bar.set_value(&mut display.ui, 19_000_000, ANIM_OFF);
    g_bar.set_start_value(&mut display.ui, 2_000_000, ANIM_OFF);
    display.ui.refr_now(None);

    g_bar.set_start_value(&mut display.ui, 10_000_000, ANIM_ON);
    display.ui.obj_update_layout(g_bar.obj());
    display.ui.refr_now(None);

    let x1 = g_bar.indic_area(&display.ui).x1;
    assert!(x1 > 200, "{x1}");
}

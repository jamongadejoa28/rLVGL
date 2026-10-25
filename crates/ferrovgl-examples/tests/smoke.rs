//! Per-example smoke tests: every ported example is built on the
//! upstream-configured 800x480 test display with the default theme, then
//! [`common::FRAMES`] frames are rendered with `wait()`. Each test checks
//! that nothing panicked and compares the key properties of the scene with
//! the C example: the number of objects of each widget class under the
//! active screen and texts the example sets (labels, checkboxes `cb:`, text
//! areas `ta:`), after the frames ran.
//!
//! The expectations are read from the upstream C sources; a mismatch means
//! the port or a widget diverged from upstream.

mod common;

use common::Tree;

/// `name => [class: count, ...], [texts...];` -> one `#[test]` per example.
macro_rules! smoke {
    ($( $name:ident => [$($class:ident: $count:literal),* $(,)?], [$($text:literal),* $(,)?]; )*) => {
        $(
            #[test]
            fn $name() {
                check(stringify!($name), &[$((stringify!($class), $count)),*], &[$($text),*]);
            }
        )*

        /// Every example of `EXAMPLES` has a smoke test above.
        #[test]
        fn every_example_has_a_smoke_test() {
            let tested: &[&str] = &[$(stringify!($name)),*];
            for example in ferrovgl_examples::EXAMPLES {
                assert!(tested.contains(&example.name), "{} has no smoke test", example.name);
            }
            assert_eq!(tested.len(), ferrovgl_examples::EXAMPLES.len());
        }
    };
}

/// Build `name`, render the frames and compare the scene.
fn check(name: &str, classes: &[(&str, u32)], texts: &[&str]) {
    let example = ferrovgl_examples::find(name).expect("example in EXAMPLES");
    assert_eq!(example.name, name);
    let mut display = common::build(example);
    common::render_frames(&mut display);
    let screen = display.screen().expect("the example keeps an active screen");
    assert!(display.ui.obj_is_valid(screen));

    let tree = Tree::of_screen(&display);
    let actual: Vec<(&str, u32)> = tree.classes.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(actual, classes, "{name}: objects per class");
    for text in texts {
        assert!(tree.has_text(text), "{name}: no {text:?} in {:?}", tree.texts);
    }
    // The final frame can still be captured.
    let _ = common::frame_hash(&mut display);
}

smoke! {
    lv_example_anim_1 => [lv_label: 1, lv_switch: 1], ["Hello animations!"];
    lv_example_anim_2 => [lv_obj: 1], [];
    lv_example_anim_4 => [lv_label: 1, lv_switch: 1], ["Hello animations!"];
    lv_example_event_click => [lv_button: 1, lv_label: 1], ["Click me!"];
    lv_example_event_streak => [lv_button: 1, lv_label: 2], ["No events yet", "Click me!"];
    lv_example_event_button => [lv_button: 1, lv_label: 2], ["Click me!", "The last button event:\nNone"];
    lv_example_event_bubble => [lv_button: 30, lv_label: 30, lv_obj: 1], ["0", "1", "2", "3"];
    lv_example_event_trickle => [lv_label: 9, lv_obj: 10], ["0", "1", "2", "3"];
    lv_example_event_draw => [lv_obj: 1], [];
    lv_example_get_started_button => [lv_button: 1, lv_label: 1], ["Button"];
    lv_example_get_started_hello_world => [lv_label: 1], ["Hello world"];
    lv_example_get_started_slider => [lv_label: 1, lv_slider: 1], ["50"];
    lv_example_get_started_styles => [lv_button: 2, lv_label: 2], ["Button", "Button 2"];
    lv_example_flex_align => [lv_button: 1, lv_checkbox: 1, lv_label: 1, lv_switch: 1], ["64px", "cb:Check Me!"];
    lv_example_flex_flow => [lv_button: 10, lv_label: 10, lv_obj: 1], ["First", "Second", "Third", "Forth"];
    lv_example_flex_grow => [lv_button: 3, lv_label: 3, lv_obj: 1], ["Fix", "Grow x1", "Grow x2"];
    lv_example_flex_ignore_layout => [lv_button: 4, lv_label: 4, lv_obj: 1], ["Item A", "I'm Free!", "Item B", "Item C"];
    lv_example_flex_internal_padding => [lv_label: 9, lv_obj: 1], ["HTML", "CSS", "JavaScript", "Python"];
    lv_example_flex_list => [lv_button: 11, lv_image: 11, lv_label: 14, lv_obj: 1], ["File", "New", "Open", "Save"];
    lv_example_flex_new_track => [lv_label: 7, lv_obj: 1], ["Mon", "Tue", "Wed", "Thu"];
    lv_example_flex_rtl => [lv_label: 8, lv_obj: 2], ["LTR (default):", "First", "Second", "Third"];
    lv_example_flex_win => [lv_button: 3, lv_image: 3, lv_label: 2, lv_obj: 3], ["A title", "This is\na pretty\nlong text\nto see how\nthe window\nbecomes\nscrollable.\n\n\nSome more\ntext to be\nsure it\noverflows. :)"];
    lv_example_grid_align => [lv_label: 6, lv_obj: 1], ["0,0", "1,0", "2,0", "0,1"];
    lv_example_grid_cell_span => [lv_label: 5, lv_obj: 6], ["start/start", "center", "end/end", "span 2 columns"];
    lv_example_grid_descriptors => [lv_label: 6, lv_obj: 1], ["0,0", "1,0", "2,0", "0,1"];
    lv_example_grid_fr => [lv_label: 6, lv_obj: 4], ["80px", "1fr", "2fr"];
    lv_example_grid_ignore_layout => [lv_label: 5, lv_obj: 1], ["A", "B", "C", "D"];
    lv_example_grid_internal_padding => [lv_label: 6, lv_obj: 1], ["One", "Two", "Three", "Four"];
    lv_example_grid_rtl => [lv_label: 8, lv_obj: 2], ["LTR (default):", "0", "1", "2"];
    lv_example_grid_subgrid => [lv_label: 5, lv_obj: 2], ["P0,0", "S0,0", "S1,0", "S0,1"];
    lv_example_split => [lv_label: 2, lv_obj: 4], ["Left\n(drag the divider)", "Right"];
    lv_example_split_vertical => [lv_label: 2, lv_obj: 4], ["Top\n(drag the divider)", "Bottom"];
    lv_example_scroll_chain => [lv_button: 8, lv_label: 8, lv_obj: 2], ["Outer 1", "Inner 1", "Inner 2", "Inner 3"];
    lv_example_scroll_circular => [lv_button: 20, lv_label: 20, lv_obj: 2], ["Item 1", "Item 2", "Item 3", "Item 4"];
    lv_example_scroll_events => [lv_button: 8, lv_label: 9, lv_obj: 1], ["Scroll the panel", "Item 1", "Item 2", "Item 3"];
    lv_example_scroll_floating => [lv_button: 7, lv_label: 7, lv_obj: 1], ["Track 1", "Track 2", "Track 3", "Track 4"];
    lv_example_scroll_infinite => [lv_label: 11, lv_obj: 10], ["largest:\n6", "smallest:\n-2", "6", "5"];
    lv_example_scroll_one => [lv_button: 9, lv_label: 9, lv_obj: 1], ["Panel 1", "Panel 2", "Panel 3", "Panel 4"];
    lv_example_scroll_overview => [lv_button: 6, lv_label: 6, lv_obj: 1], ["Item 1", "Item 2", "Item 3", "Item 4"];
    lv_example_scroll_properties => [lv_button: 8, lv_label: 12, lv_obj: 6, lv_switch: 4], ["Item 1", "Item 2", "Item 3", "Item 4"];
    lv_example_scroll_rtl => [lv_button: 6, lv_label: 6, lv_obj: 1], ["Row 1", "Row 2", "Row 3", "Row 4"];
    lv_example_scroll_scrollable => [lv_button: 8, lv_label: 10, lv_obj: 3], ["scrollable", "A", "B", "C"];
    lv_example_scroll_scrollbar_mode => [lv_button: 8, lv_label: 10, lv_obj: 3], ["mode: off", "A", "B", "C"];
    lv_example_scroll_scrollbar_style => [lv_button: 6, lv_label: 6, lv_obj: 1], ["Row 1", "Row 2", "Row 3", "Row 4"];
    lv_example_scroll_snap => [lv_button: 6, lv_label: 6, lv_obj: 1], ["Panel 1", "Panel 2", "No snap", "Panel 4"];
    lv_example_scroll_to => [lv_button: 16, lv_label: 16, lv_obj: 2], ["Top", "Bottom", "Save", "Restore"];
    lv_example_scroll_translate => [lv_button: 20, lv_label: 20, lv_obj: 1], ["Button 0", "Button 1", "Button 2", "Button 3"];
    lv_example_style_arc => [lv_arc: 1], [];
    lv_example_style_background => [lv_label: 1, lv_obj: 1], ["Gradient"];
    lv_example_style_blur => [lv_button: 1, lv_label: 3, lv_obj: 1], ["Hey there, I'm a little text in the background which is blurred. You might seem that some of parts are sharp, but other under the glass cannot be read. ", "Click me", "Frosted glass"];
    lv_example_style_border => [lv_label: 1, lv_obj: 1], ["Bordered card"];
    lv_example_style_drop_shadow => [lv_arc: 1, lv_label: 1], ["Style: drop shadow"];
    lv_example_style_gradient_buttons => [lv_button: 4, lv_label: 4], ["Horizontal", "Vertical", "Linear", "Radial"];
    lv_example_style_gradient_conic => [lv_obj: 1], [];
    lv_example_style_gradient_radial => [lv_obj: 1], [];
    lv_example_style_image => [lv_image: 1], [];
    lv_example_style_line => [lv_label: 1, lv_line: 1], ["Style: line stroke"];
    lv_example_style_local => [lv_button: 3, lv_label: 3], ["Shared", "Override", "On press"];
    lv_example_style_margin => [lv_label: 3], ["A", "B", "C"];
    lv_example_style_modal => [lv_label: 2, lv_obj: 2], ["Base", "Warning"];
    lv_example_style_multiple => [lv_label: 2, lv_obj: 2], ["Base", "Warning"];
    lv_example_style_opacity_transform => [lv_button: 3, lv_label: 3], ["Normal", "Opa 50%", "Transformed"];
    lv_example_style_outline => [lv_label: 1, lv_obj: 1], ["Focused"];
    lv_example_style_padding => [lv_obj: 4], [];
    lv_example_style_parts_states => [lv_slider: 1], [];
    lv_example_style_shadow => [lv_label: 1, lv_obj: 1], ["Elevated"];
    lv_example_style_size_position => [lv_label: 2, lv_obj: 2], ["140 wide", "260 wide, roomy padding"];
    lv_example_style_text => [lv_label: 1, lv_obj: 1], ["Custom\nTypography"];
    lv_example_style_theme => [lv_button: 2, lv_label: 2], ["Original theme", "New theme"];
    lv_example_style_transition => [lv_obj: 1], [];
    lv_example_arc_change_rate => [lv_arc: 2, lv_label: 2], ["rate=40", "rate=360"];
    lv_example_arc_event => [lv_arc: 1, lv_label: 1], ["Value: -32768"];
    lv_example_arc_knob_widget => [lv_arc: 1, lv_label: 1], ["10%"];
    lv_example_arc_mode => [lv_arc: 3, lv_label: 3], ["normal", "rev.", "sym."];
    lv_example_arc_pie_chart => [lv_arc: 5], [];
    lv_example_arc_rotation => [lv_arc: 2, lv_label: 2], ["rot=45", "rot=135"];
    lv_example_arc_set_angles => [lv_arc: 2, lv_label: 2], ["value", "angles"];
    lv_example_arc_styling => [lv_arc: 1], [];
    lv_example_bar_bind_value => [lv_bar: 1, lv_button: 2, lv_label: 3, lv_obj: 1], ["-", "50%", "+"];
    lv_example_bar_modes => [lv_bar: 3, lv_label: 3], ["normal", "symmetrical", "range"];
    lv_example_bar_orientation => [lv_bar: 2, lv_label: 2], ["Horizontal", "V\ne\nr"];
    lv_example_bar_styling => [lv_bar: 2], [];
    lv_example_bar_value_range => [lv_bar: 2, lv_label: 2], ["0..100", "-20..80"];
    lv_example_button_basic => [lv_button: 2, lv_label: 2], ["Click me", "Wide button"];
    lv_example_button_event => [lv_button: 1, lv_label: 2], ["Click me", "Clicks: 0"];
    lv_example_button_states => [lv_button: 3, lv_label: 3], ["Normal", "Checked", "Disabled"];
    lv_example_button_styling => [lv_button: 3, lv_label: 3], ["Press me", "Toggle me", "Local props"];
    lv_example_buttonmatrix_button_ctrl => [lv_buttonmatrix: 1], [];
    lv_example_buttonmatrix_button_width => [lv_buttonmatrix: 1], [];
    lv_example_buttonmatrix_event => [lv_buttonmatrix: 1, lv_label: 1], ["Pressed: —"];
    lv_example_buttonmatrix_map => [lv_buttonmatrix: 1], [];
    lv_example_buttonmatrix_one_checked => [lv_buttonmatrix: 1], [];
    lv_example_buttonmatrix_popover => [lv_buttonmatrix: 1], [];
    lv_example_buttonmatrix_recolor => [lv_buttonmatrix: 1], [];
    lv_example_buttonmatrix_styling => [lv_buttonmatrix: 1], [];
    lv_example_checkbox_bind_checked => [lv_checkbox: 1, lv_label: 1], ["cb:Show extra details", "Extra details only visible while the box is ticked."];
    lv_example_checkbox_event => [lv_checkbox: 1, lv_label: 1], ["cb:Enable feature", "Feature: OFF"];
    lv_example_checkbox_states => [lv_checkbox: 4], ["cb:Unchecked", "cb:Checked", "cb:Disabled", "cb:Disabled + Checked"];
    lv_example_checkbox_styling => [lv_checkbox: 2], ["cb:Default look", "cb:Pre-checked"];
    lv_example_checkbox_text => [lv_checkbox: 3], ["cb:Enable", "cb:Send anonymous usage data", "cb:I agree to the terms\nand the privacy policy"];
    lv_example_image_inner_align => [lv_image: 3], [];
    lv_example_image_recolor => [lv_image: 1, lv_label: 1, lv_slider: 1], ["recolor_opa = 128 / 255"];
    lv_example_image_src => [lv_image: 1], [];
    lv_example_image_transformations => [lv_image: 3], [];
    lv_example_label_bind_text => [lv_button: 2, lv_label: 3], ["Hello", "Idle", "Running"];
    lv_example_label_long_mode => [lv_label: 5], ["Wrap keeps all text visible by splitting long lines into multiple rows.", "Dots shortens overflowing text and r...", "Scroll moves overflowing text back and forth inside the label area.", "Scroll circular continuously loops overflowing text forever."];
    lv_example_label_recolor => [lv_label: 1], ["Write a #ff0000 red# word, a #00a000 green# word, and a #0000ff blue# word."];
    lv_example_label_set_text_newline => [lv_label: 2], ["First line\nSecond line\n\nFourth line", "Just a normal single line text."];
    lv_example_label_styling => [lv_label: 2], ["NEW", "Heads up"];
    lv_example_label_text_align => [lv_label: 3], ["Left aligned\nSecond line", "Centered\nSecond line", "Right aligned\nSecond line"];
    lv_example_led_brightness => [lv_led: 3], [];
    lv_example_led_color => [lv_led: 3], [];
    lv_example_led_styling => [lv_led: 3], [];
    lv_example_line_points => [lv_line: 1], [];
    lv_example_obj_bind_style => [lv_button: 1, lv_label: 2, lv_obj: 1], ["Panel content", "Toggle theme"];
    lv_example_obj_draggable => [lv_label: 1, lv_obj: 1], ["Drag me"];
    lv_example_obj_transform => [lv_label: 1], ["LV_DRAW_TRANSFORM_USE_MATRIX is not enabled"];
    lv_example_slider_bind_state => [lv_button: 1, lv_label: 2, lv_slider: 1], ["Current: 50 (disabled above 80)", "Submit"];
    lv_example_slider_bind_value => [lv_label: 1, lv_slider: 1], ["Value: 50/100"];
    lv_example_slider_event => [lv_label: 1, lv_slider: 1], ["Drag the slider"];
    lv_example_slider_modes => [lv_label: 3, lv_slider: 3], ["Normal", "Symmetrical", "Range"];
    lv_example_slider_reversed_direction => [lv_label: 2, lv_slider: 2], ["0..100", "100..0"];
    lv_example_slider_styling => [lv_slider: 1], [];
    lv_example_slider_value_range_orientation => [lv_label: 2, lv_slider: 2], ["Horizontal", "V\ne\nr"];
    lv_example_spinner_basic => [lv_spinner: 2], [];
    lv_example_switch_event => [lv_label: 1, lv_switch: 1], ["State: OFF"];
    lv_example_switch_orientation => [lv_switch: 2], [];
    lv_example_switch_state => [lv_switch: 4], [];
    lv_example_switch_styling => [lv_switch: 2], [];
    lv_example_textarea_buttonmatrix => [lv_buttonmatrix: 1, lv_label: 1, lv_textarea: 1], ["ta:"];
    lv_example_textarea_cursor => [lv_label: 2, lv_textarea: 2], ["ta:Block", "Block", "ta:Bar", "Bar"];
    lv_example_textarea_placeholder => [lv_label: 2, lv_textarea: 2], ["ta:", "ta:Hello world!", "Hello world!"];
}

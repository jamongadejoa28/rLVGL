#![no_main]

use ferrovgl_core::{Color, Part, State, Style, StyleProp, StyleSelector, StyleValue, Ui, UiConfig};
use libfuzzer_sys::fuzz_target;

const PROPS: [StyleProp; 8] = [
    StyleProp::WIDTH,
    StyleProp::HEIGHT,
    StyleProp::PAD_LEFT,
    StyleProp::RADIUS,
    StyleProp::BG_OPA,
    StyleProp::TEXT_OPA,
    StyleProp::BG_COLOR,
    StyleProp::TEXT_COLOR,
];

fuzz_target!(|data: &[u8]| {
    // A separate flat map is the oracle for mutation/copy/merge and group-cache
    // lookups. Values stay within normal style ranges, avoiding C-undefined inputs.
    let mut styles = [Style::new(), Style::new()];
    let mut expected: [[Option<StyleValue>; 8]; 2] = Default::default();
    let mut ui = Ui::new(UiConfig::default());
    let display = ui.display_create(160, 120);
    let root = ui.display_get_screen_active(Some(display)).unwrap();
    let child = ui.obj_create(Some(root));
    let shared = ui.style_create(Style::new());
    ui.obj_add_style(child, shared, StyleSelector::DEFAULT);
    let mut inherited = Color::BLACK;
    let mut direct = None;
    let mut pressed = None;
    let mut state = false;
    for op in data.chunks_exact(4).take(256) {
        let index = (op[1] as usize) % PROPS.len();
        let which = usize::from(op[2] & 1);
        let other = 1 - which;
        let color = Color::make(op[1], op[2], op[3]);
        let value = if index >= 6 {
            StyleValue::Color(color)
        } else {
            StyleValue::Num(op[3].into())
        };
        match op[0] % 12 {
            0..=2 => {
                styles[which].set_prop(PROPS[index], value.clone());
                expected[which][index] = Some(value);
            }
            3 => {
                assert_eq!(
                    styles[which].remove_prop(PROPS[index]),
                    expected[which][index].take().is_some()
                );
            }
            4 => {
                styles[which].reset();
                expected[which] = Default::default();
            }
            5 => {
                let source = styles[other].clone();
                styles[which].copy_from(&source);
                expected[which] = expected[other].clone();
            }
            6 => {
                let source = styles[other].clone();
                styles[which].merge_from(&source);
                for i in 0..PROPS.len() {
                    if expected[other][i].is_some() {
                        expected[which][i] = expected[other][i].clone();
                    }
                }
            }
            7 => {
                inherited = color;
                ui.obj_set_style_text_color(root, color, StyleSelector::DEFAULT);
            }
            8 => {
                direct = Some(color);
                ui.style_mut(shared).unwrap().set_text_color(color);
                ui.obj_report_style_change(Some(shared));
            }
            9 => {
                direct = None;
                ui.style_mut(shared).unwrap().remove_prop(StyleProp::TEXT_COLOR);
                ui.obj_report_style_change(Some(shared));
            }
            10 => {
                pressed = if op[3] & 1 == 0 {
                    ui.obj_remove_local_style_prop(child, StyleProp::TEXT_COLOR, State::PRESSED | Part::MAIN);
                    None
                } else {
                    ui.obj_set_style_text_color(child, color, State::PRESSED | Part::MAIN);
                    Some(color)
                };
            }
            _ => {
                state = !state;
                ui.obj_set_state(child, State::PRESSED, state);
            }
        }
        for which in 0..2 {
            assert_eq!(styles[which].len(), expected[which].iter().flatten().count());
            for (i, prop) in PROPS.into_iter().enumerate() {
                assert_eq!(styles[which].get_prop(prop), expected[which][i].as_ref());
            }
        }
        let expected_color = if state { pressed } else { None }.or(direct).unwrap_or(inherited);
        assert_eq!(ui.obj_get_style_text_color(child, Part::MAIN), expected_color);
    }
    ui.deinit();
    assert!(ui.objs.is_empty());
});

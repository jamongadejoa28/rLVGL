//! Port of `examples/styles/lv_example_style_transition.c`.

use alloc::rc::Rc;

use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::{SharedRef, StyleTransitionDsc};
use ferrovgl_core::{AnimPath, State, Style, StyleProp, StyleSelector, Ui};

use crate::util::{screen_active, styles_reinit};

/// `static const lv_style_prop_t props[]`
static PROPS: [StyleProp; 4] = [
    StyleProp::BG_COLOR,
    StyleProp::BORDER_COLOR,
    StyleProp::BORDER_WIDTH,
    StyleProp::INV,
];

/// `style_def`, `style_pr`.
struct Styles;

/// `lv_example_style_transition()`: different transitions entering and leaving the pressed state.
pub fn lv_example_style_transition(ui: &mut Ui) {
    let [style_def, style_pr] = styles_reinit::<Styles, 2>(ui, || {
        /* A default transition
         * Make it fast (100ms) and start with some delay (200 ms)*/
        let trans_def = StyleTransitionDsc::new(SharedRef::Static(&PROPS), AnimPath::Linear, 100, 200);

        /* A special transition when going to pressed state
         * Make it slow (500 ms) but start  without delay*/
        let trans_pr = StyleTransitionDsc::new(SharedRef::Static(&PROPS), AnimPath::Linear, 500, 0);

        let mut style_def = Style::new();
        style_def.set_transition(SharedRef::Shared(Rc::new(trans_def)));

        let mut style_pr = Style::new();
        style_pr.set_bg_color(palette::main(Palette::Red));
        style_pr.set_border_width(6);
        style_pr.set_border_color(palette::darken(Palette::Red, 3));
        style_pr.set_transition(SharedRef::Shared(Rc::new(trans_pr)));
        [style_def, style_pr]
    });

    /*Create an object with the new style_pr*/
    let obj = ui.obj_create(Some(screen_active(ui)));
    ui.obj_add_style(obj, style_def, StyleSelector::DEFAULT);
    ui.obj_add_style(obj, style_pr, State::PRESSED.into());

    ui.obj_center(obj);
}

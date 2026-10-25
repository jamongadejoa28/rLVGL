//! Port of `examples/styles/lv_example_style_gradient_buttons.c`.
//!
//! Both sides of `#if LV_USE_DRAW_SW_COMPLEX_GRADIENTS` are ported; the
//! `complex-gradients` feature of this crate selects the first one
//! (the upstream test build, `full.defconfig`, enables it).

#[cfg(feature = "complex-gradients")]
pub use complex::lv_example_style_gradient_buttons;

/// The `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` branch.
#[cfg(feature = "complex-gradients")]
mod complex {
    use alloc::rc::Rc;

    use ferrovgl_core::draw::grad::{GradDir, GradDsc, GradExtend};
    use ferrovgl_core::misc::color::OPA_COVER;
    use ferrovgl_core::misc::style::SharedRef;
    use ferrovgl_core::misc::types::coord;
    use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
    use ferrovgl_widgets::{Button, Label};

    use crate::util::{screen_active, styles_reinit};

    /// `static const lv_color_t grad_colors[2]`
    const GRAD_COLORS: [Color; 2] = [Color::make(0x26, 0xa0, 0xda), Color::make(0x31, 0x47, 0x55)];

    /// `style_with_linear_gradient_bg`, `style_with_radial_gradient_bg`.
    struct Styles;

    /// `lv_example_style_gradient_buttons()`: simple and complex gradients on buttons.
    pub fn lv_example_style_gradient_buttons(ui: &mut Ui) {
        let sel = StyleSelector::DEFAULT;
        let [style_with_linear_gradient_bg, style_with_radial_gradient_bg] = styles_reinit::<Styles, 2>(
            ui,
            || {
                /*Create a linear gradient going from the top left corner to the bottom at an angle, with reflected color map*/
                let mut style_with_linear_gradient_bg = Style::new();
                let mut linear_gradient_dsc = GradDsc::new(); /*NOTE: the gradient descriptor must be static or global variable!*/
                linear_gradient_dsc.init_stops(&GRAD_COLORS, None, None, GRAD_COLORS.len());
                linear_gradient_dsc.linear_init(
                    coord::pct(0),
                    coord::pct(0),
                    coord::pct(20),
                    coord::pct(100),
                    GradExtend::Reflect,
                );
                style_with_linear_gradient_bg.set_bg_grad(SharedRef::Shared(Rc::new(linear_gradient_dsc)));
                style_with_linear_gradient_bg.set_bg_opa(OPA_COVER);

                /*Create a radial gradient with the center in the top left 1/3rd of the object, extending to the bottom right corner, with reflected color map*/
                let mut style_with_radial_gradient_bg = Style::new();
                let mut radial_gradient_dsc = GradDsc::new(); /*NOTE: the gradient descriptor must be static or global variable!*/
                radial_gradient_dsc.init_stops(&GRAD_COLORS, None, None, GRAD_COLORS.len());
                radial_gradient_dsc.radial_init(
                    coord::pct(30),
                    coord::pct(30),
                    coord::pct(100),
                    coord::pct(100),
                    GradExtend::Reflect,
                );
                style_with_radial_gradient_bg.set_bg_grad(SharedRef::Shared(Rc::new(radial_gradient_dsc)));
                style_with_radial_gradient_bg.set_bg_opa(OPA_COVER);
                [style_with_linear_gradient_bg, style_with_radial_gradient_bg]
            },
        );

        /*Create buttons with different gradient styles*/

        /*Simple horizontal gradient*/
        let btn = Button::create(ui, screen_active(ui)).obj();
        ui.obj_set_style_bg_color(btn, GRAD_COLORS[0], sel);
        ui.obj_set_style_bg_grad_color(btn, GRAD_COLORS[1], sel);
        ui.obj_set_style_bg_grad_dir(btn, GradDir::Hor, sel);
        ui.obj_set_size(btn, 150, 50);
        ui.obj_align(btn, Align::Center, 0, -100);

        let label = Label::create(ui, btn);
        label.set_text(ui, "Horizontal");
        ui.obj_center(label.obj());

        /*Simple vertical gradient*/
        let btn = Button::create(ui, screen_active(ui)).obj();
        ui.obj_set_style_bg_color(btn, GRAD_COLORS[0], sel);
        ui.obj_set_style_bg_grad_color(btn, GRAD_COLORS[1], sel);
        ui.obj_set_style_bg_grad_dir(btn, GradDir::Ver, sel);
        ui.obj_set_size(btn, 150, 50);
        ui.obj_align(btn, Align::Center, 0, -40);

        let label = Label::create(ui, btn);
        label.set_text(ui, "Vertical");
        ui.obj_center(label.obj());

        /*Complex linear gradient*/
        let btn = Button::create(ui, screen_active(ui)).obj();
        ui.obj_add_style(btn, style_with_linear_gradient_bg, sel);
        ui.obj_set_size(btn, 150, 50);
        ui.obj_align(btn, Align::Center, 0, 20);

        let label = Label::create(ui, btn);
        label.set_text(ui, "Linear");
        ui.obj_center(label.obj());

        /*Complex radial gradient*/
        let btn = Button::create(ui, screen_active(ui)).obj();
        ui.obj_add_style(btn, style_with_radial_gradient_bg, sel);
        ui.obj_set_size(btn, 150, 50);
        ui.obj_align(btn, Align::Center, 0, 80);

        let label = Label::create(ui, btn);
        label.set_text(ui, "Radial");
        ui.obj_center(label.obj());
    }
}

/// `lv_example_style_gradient_buttons()` without `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` (the `#else`
/// branch): a label reporting the missing feature.
#[cfg(not(feature = "complex-gradients"))]
pub fn lv_example_style_gradient_buttons(ui: &mut ferrovgl_core::Ui) {
    crate::util::complex_gradients_not_enabled(ui);
}

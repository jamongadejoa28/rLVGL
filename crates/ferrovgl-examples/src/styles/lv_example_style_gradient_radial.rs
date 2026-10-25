//! Port of `examples/styles/lv_example_style_gradient_radial.c`.
//!
//! Both sides of `#if LV_USE_DRAW_SW_COMPLEX_GRADIENTS` are ported; the
//! `complex-gradients` feature of this crate selects the first one
//! (the upstream test build, `full.defconfig`, enables it).

#[cfg(feature = "complex-gradients")]
pub use complex::lv_example_style_gradient_radial;

/// The `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` branch.
#[cfg(feature = "complex-gradients")]
mod complex {
    use alloc::rc::Rc;

    use ferrovgl_core::draw::grad::{GradDsc, GradExtend};
    use ferrovgl_core::misc::style::{GRAD_BOTTOM, GRAD_CENTER, GRAD_RIGHT, SharedRef};
    use ferrovgl_core::{Color, Style, StyleSelector, Ui};

    use crate::util::{screen_active, styles_reinit};

    /// `static const lv_color_t grad_colors[2]`
    const GRAD_COLORS: [Color; 2] = [Color::make(0x9B, 0x18, 0x42), Color::make(0x00, 0x00, 0x00)];

    /// `static lv_style_t style;`
    struct StyleRadial;

    /// `lv_example_style_gradient_radial()`: a radial gradient filling the display.
    pub fn lv_example_style_gradient_radial(ui: &mut Ui) {
        let display = ui.display_get_default();
        let width = ui.display_get_horizontal_resolution(display);
        let height = ui.display_get_vertical_resolution(display);

        let [style] = styles_reinit::<StyleRadial, 1>(ui, || {
            let mut style = Style::new();

            /*First define a color gradient. In this example we use a purple to black color map.*/
            let mut grad = GradDsc::new();
            grad.init_stops(&GRAD_COLORS, None, None, GRAD_COLORS.len());

            /*Make a radial gradient with the center in the middle of the object, extending to the farthest corner*/
            grad.radial_init(GRAD_CENTER, GRAD_CENTER, GRAD_RIGHT, GRAD_BOTTOM, GradExtend::Pad);

            /*Set gradient as background*/
            style.set_bg_grad(SharedRef::Shared(Rc::new(grad)));
            [style]
        });

        /*Create an object with the new style*/
        let obj = ui.obj_create(Some(screen_active(ui)));
        ui.obj_add_style(obj, style, StyleSelector::DEFAULT);
        ui.obj_set_size(obj, width, height);
        ui.obj_center(obj);
    }
}

/// `lv_example_style_gradient_radial()` without `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` (the `#else`
/// branch): a label reporting the missing feature.
#[cfg(not(feature = "complex-gradients"))]
pub fn lv_example_style_gradient_radial(ui: &mut ferrovgl_core::Ui) {
    crate::util::complex_gradients_not_enabled(ui);
}

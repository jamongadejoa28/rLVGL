//! Port of `examples/styles/lv_example_style_gradient_conic.c`
//! (`LV_GRADIENT_MAX_STOPS` = 2, the upstream default and
//! `ferrovgl_core::draw::grad::GRADIENT_MAX_STOPS`).
//!
//! Both sides of `#if LV_USE_DRAW_SW_COMPLEX_GRADIENTS` are ported; the
//! `complex-gradients` feature of this crate selects the first one
//! (the upstream test build, `full.defconfig`, enables it).

#[cfg(feature = "complex-gradients")]
pub use complex::lv_example_style_gradient_conic;

/// The `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` branch.
#[cfg(feature = "complex-gradients")]
mod complex {
    use alloc::rc::Rc;

    use ferrovgl_core::draw::grad::{GRADIENT_MAX_STOPS, GradDsc, GradExtend};
    use ferrovgl_core::misc::color::{OPA_50, OPA_COVER};
    use ferrovgl_core::misc::style::{GRAD_CENTER, SharedRef};
    use ferrovgl_core::{Color, Style, StyleSelector, Ui};

    use crate::util::{screen_active, styles_reinit};

    /// `static const lv_color_t grad_colors[2]` (the `LV_GRADIENT_MAX_STOPS < 3` branch).
    const GRAD_COLORS: [Color; 2] = [Color::make(0xe8, 0xe8, 0xe8), Color::make(0x79, 0x79, 0x79)];

    const _: () = assert!(
        GRADIENT_MAX_STOPS < 3,
        "port the LV_GRADIENT_MAX_STOPS >= 3 branch"
    );

    /// `static lv_style_t style;`
    struct StyleConic;

    /// `lv_example_style_gradient_conic()`: a conical gradient background.
    pub fn lv_example_style_gradient_conic(ui: &mut Ui) {
        /*Create a style with gradient background and shadow*/
        let [style] = styles_reinit::<StyleConic, 1>(ui, || {
            let mut style = Style::new();
            style.set_radius(500);
            style.set_bg_opa(OPA_COVER);
            style.set_shadow_color(Color::black());
            style.set_shadow_width(50);
            style.set_shadow_offset_x(20);
            style.set_shadow_offset_y(20);
            style.set_shadow_opa(OPA_50);

            /*First define a color gradient. In this example we use a gray color map with random values.*/
            let mut grad = GradDsc::new();
            grad.init_stops(&GRAD_COLORS, None, None, GRAD_COLORS.len());

            /*Make a conical gradient with the center in the middle of the object*/
            grad.conical_init(GRAD_CENTER, GRAD_CENTER, 45, 110, GradExtend::Reflect);

            /*Set gradient as background*/
            style.set_bg_grad(SharedRef::Shared(Rc::new(grad)));
            [style]
        });

        /*Create an object with the new style*/
        let obj = ui.obj_create(Some(screen_active(ui)));
        ui.obj_add_style(obj, style, StyleSelector::DEFAULT);
        ui.obj_set_size(obj, 200, 200);
        ui.obj_center(obj);
    }
}

/// `lv_example_style_gradient_conic()` without `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` (the `#else`
/// branch): a label reporting the missing feature.
#[cfg(not(feature = "complex-gradients"))]
pub fn lv_example_style_gradient_conic(ui: &mut ferrovgl_core::Ui) {
    crate::util::complex_gradients_not_enabled(ui);
}

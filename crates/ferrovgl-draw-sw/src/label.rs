//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_letter.c` (`lv_draw_sw_label()`).
//!
//! Upstream keeps `lv_draw_sw_label()` in `lv_draw_sw_letter.c`; this
//! module is the Rust home of that entry point. The glyph callback is
//! [`crate::letter::draw_letter_cb`].
//!
//! The FreeType outline event registration
//! (`LV_USE_FREETYPE && LV_USE_VECTOR_GRAPHIC && LV_USE_THORVG`) is not
//! compiled in the parity configuration and is not ported.

use ferrovgl_core::draw::label::{DrawGlyphDsc, LabelDsc};
use ferrovgl_core::draw::rect::FillDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask, DrawTaskRef};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::OPA_MIN;

use crate::SwDrawUnit;
use crate::letter::{draw_letter_cb_impl, task_ref};

/// `lv_draw_sw_label()`: lay the text out with
/// `lv_draw_label_iterate_characters()` and render every glyph with
/// `draw_letter_cb()`.
///
/// `t` is the task being rendered (`unit.task_act`; when that is not set
/// the in-progress task of `t.target_layer` with the same type and area is
/// used to address it in `ctx`).
pub fn label(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LabelDsc, coords: &Area) {
    if dsc.opa <= OPA_MIN {
        return;
    }

    let Some(tref) = task_ref(unit, ctx, t) else {
        return;
    };

    let mut cb =
        |ctx: &mut DrawCtx, _t: DrawTaskRef, g: &mut DrawGlyphDsc, fill: Option<(&FillDsc, &Area)>| {
            draw_letter_cb_impl(unit, ctx, t, g, fill);
        };
    ctx.label_iterate_characters(tref, dsc, coords, &mut cb);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill::tests::{WHITE, ctx_with_layer, px};
    use crate::letter::tests::{BLUE, font};
    use alloc::rc::Rc;
    use ferrovgl_core::draw::{DrawTaskState, LayerId};
    use ferrovgl_core::misc::color::Color;
    use ferrovgl_core::misc::text::TextDecor;

    fn run(txt: &str, decor: TextDecor, opa: u8) -> (DrawCtx, LayerId) {
        let (mut ctx, layer) = ctx_with_layer();
        let mut dsc = LabelDsc::new();
        dsc.text = Rc::from(txt);
        dsc.font = Some(font(false));
        dsc.color = Color::make(0, 0, 0xff);
        dsc.decor = decor;
        dsc.opa = opa;
        let coords = Area::new(10, 10, 40, 29);
        ctx.draw_label(layer, &dsc, &coords);
        let tref = DrawTaskRef { layer, index: 0 };
        if ctx.task(tref).is_none() {
            return (ctx, layer);
        }
        ctx.task_mut(tref).unwrap().state = DrawTaskState::InProgress;
        let t = ctx.task(tref).unwrap().clone();
        let task_dsc = ctx.task_get_label_dsc(tref).unwrap().clone();
        let mut unit = SwDrawUnit::new(0);
        unit.task_act = Some(tref);
        label(&mut unit, &mut ctx, &t, &task_dsc, &coords);
        (ctx, layer)
    }

    #[test]
    fn label_draws_each_glyph() {
        let (ctx, layer) = run("AA", TextDecor::NONE, 255);
        // Glyph boxes at x = 10 and 14 (adv_w 4), y = 10 + 8 - 4 = 14.
        for x0 in [10, 14] {
            assert_eq!(px(&ctx, layer, x0, 14), WHITE);
            assert_eq!(px(&ctx, layer, x0 + 1, 14), BLUE);
            assert_eq!(px(&ctx, layer, x0 + 2, 17), BLUE);
            assert_eq!(px(&ctx, layer, x0 + 3, 15), WHITE);
        }
    }

    #[test]
    fn label_underline_is_filled() {
        let (ctx, layer) = run("AA", TextDecor::UNDERLINE, 255);
        let mut found = false;
        for y in 18..22 {
            if (10..17).all(|x| px(&ctx, layer, x, y) == BLUE) {
                found = true;
            }
        }
        assert!(found, "no underline row");
    }

    #[test]
    fn label_placeholder_for_missing_glyph() {
        let (ctx, layer) = run("~", TextDecor::NONE, 255);
        let mut drawn = 0;
        for y in 10..30 {
            for x in 10..41 {
                if px(&ctx, layer, x, y) != WHITE {
                    drawn += 1;
                }
            }
        }
        assert!(drawn > 0);
    }

    #[test]
    fn transparent_label_draws_nothing() {
        let (ctx, layer) = run("AA", TextDecor::NONE, OPA_MIN);
        for y in 0..30 {
            for x in 0..41 {
                assert_eq!(px(&ctx, layer, x, y), WHITE);
            }
        }
    }
}

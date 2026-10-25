//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_mask_rect.c`.
//!
//! Upstream addresses the layer rows as `lv_color32_t *` (the layer of a
//! mask task is always `ARGB8888`); here the pixel bytes are indexed
//! directly, with the alpha channel at byte 3 of each pixel.

use alloc::vec;
use alloc::vec::Vec;

use ferrovgl_core::draw::mask::MaskRectDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::{OPA_COVER, Opa, opa_mix2};

use crate::SwDrawUnit;
use crate::mask::{self, MaskParam, MaskRadiusParam, MaskRes};

/// `sizeof(lv_color32_t)`
const COLOR32_SIZE: usize = 4;

/// `lv_draw_sw_mask_rect()`: clear the layer outside a rounded rectangle.
pub fn mask_rect(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &MaskRectDsc) {
    let Some(draw_area) = Area::intersect(&dsc.area, &t.clip_area) else {
        return;
    };

    let Some(target_layer) = ctx.layer_mut(t.target_layer) else {
        return;
    };
    let buf_area = target_layer.buf_area;
    let mut clear_area = Area::default();

    if !dsc.keep_outside {
        let mut clear = |clear_area: &mut Area| {
            clear_area.move_by(-buf_area.x1, -buf_area.y1);
            if let Some(draw_buf) = target_layer.draw_buf.as_mut() {
                draw_buf.clear(Some(clear_area));
            }
        };

        /*Clear the top part*/
        clear_area.set(t.clip_area.x1, t.clip_area.y1, t.clip_area.x2, dsc.area.y1 - 1);
        clear(&mut clear_area);

        /*Clear the bottom part*/
        clear_area.set(t.clip_area.x1, dsc.area.y2 + 1, t.clip_area.x2, t.clip_area.y2);
        clear(&mut clear_area);

        /*Clear the left part*/
        clear_area.set(t.clip_area.x1, dsc.area.y1, dsc.area.x1 - 1, dsc.area.y2);
        clear(&mut clear_area);

        /*Clear the right part*/
        clear_area.set(dsc.area.x2 + 1, dsc.area.y1, t.clip_area.x2, dsc.area.y2);
        clear(&mut clear_area);
    }

    /*The rows below are addressed relative to the buffer, so clamp the draw area to it*/
    let Some(draw_area) = Area::intersect(&draw_area, &buf_area) else {
        return;
    };

    let mut param = MaskRadiusParam::default();
    mask::radius_init(&mut unit.circle_cache, &mut param, &dsc.area, dsc.radius, false);

    let mut masks = [MaskParam::Radius(param)];

    let area_w = draw_area.width() as u32;
    let mut mask_buf: Vec<Opa> = vec![0; area_w as usize];

    for y in draw_area.y1..=draw_area.y2 {
        mask_buf.fill(0xff);
        let res = mask::apply(
            &mut unit.circle_cache,
            &mut masks,
            &mut mask_buf,
            draw_area.x1,
            y,
            area_w as i32,
        );
        if res == MaskRes::FullCover {
            continue;
        }

        /*lv_draw_layer_go_to_xy()*/
        let Some(draw_buf) = target_layer.draw_buf.as_mut() else {
            continue;
        };
        let Some(ofs) =
            draw_buf.goto_xy_checked((draw_area.x1 - buf_area.x1) as u32, (y - buf_area.y1) as u32)
        else {
            continue;
        };
        let Some(data) = draw_buf.data_mut() else {
            continue;
        };
        let end = (ofs + area_w as usize * COLOR32_SIZE).min(data.len());
        let Some(c32_buf) = data.get_mut(ofs..end) else {
            continue;
        };

        if res == MaskRes::Transp {
            c32_buf.fill(0);
        } else {
            for (px, &m) in c32_buf
                .as_chunks_mut::<COLOR32_SIZE>()
                .0
                .iter_mut()
                .zip(mask_buf.iter())
            {
                if m != OPA_COVER {
                    px[3] = opa_mix2(px[3], m);
                }
            }
        }
    }

    let [mut param] = masks;
    mask::free_param(&mut unit.circle_cache, &mut param);
}

#[cfg(test)]
mod tests {
    //! The expected hashes come from the upstream C code (lvgl v9.6.0,
    //! `LV_DRAW_SW_COMPLEX=1`) run on the same inputs.

    use super::*;
    use ferrovgl_core::draw::{DrawBuf, DrawDsc, Layer, LayerId};
    use ferrovgl_core::misc::color::ColorFormat;

    fn fnv(bytes: &[u8]) -> u32 {
        let mut h: u32 = 2166136261;
        for &b in bytes {
            h ^= b as u32;
            h = h.wrapping_mul(16777619);
        }
        h
    }

    /// A 60x50 ARGB8888 layer at (2, 4) filled with `i * 31 + 7`.
    fn make_ctx() -> (DrawCtx, LayerId) {
        let (w, h) = (60u32, 50u32);
        let mut ctx = DrawCtx::new();
        let mut layer = Layer::new();
        let mut db = DrawBuf::create(w, h, ColorFormat::Argb8888, 0).unwrap();
        let len = db.header.stride as usize * h as usize;
        let data = db.data_mut().unwrap();
        for (i, b) in data[..len].iter_mut().enumerate() {
            *b = (i as u32).wrapping_mul(31).wrapping_add(7) as u8;
        }
        layer.draw_buf = Some(db);
        layer.color_format = ColorFormat::Argb8888;
        layer.buf_area = Area::new(2, 4, 2 + w as i32 - 1, 4 + h as i32 - 1);
        let id = LayerId(ctx.layers.insert(layer));
        (ctx, id)
    }

    fn data(ctx: &DrawCtx, id: LayerId) -> Vec<u8> {
        let db = ctx.layer(id).unwrap().draw_buf.as_ref().unwrap();
        let len = db.header.stride as usize * db.header.h as usize;
        db.data()[..len].to_vec()
    }

    fn task(layer: LayerId, clip_area: Area) -> DrawTask {
        DrawTask {
            ty: Default::default(),
            area: clip_area,
            real_area: clip_area,
            clip_area,
            target_layer: layer,
            draw_unit: None,
            state: Default::default(),
            dsc: DrawDsc::None,
            opa: OPA_COVER,
            preferred_draw_unit_id: 0,
            preference_score: 0,
        }
    }

    fn run(unit: &mut SwDrawUnit, clip: Area, area: Area, radius: i32, keep_outside: bool) -> Vec<u8> {
        let (mut ctx, id) = make_ctx();
        let t = task(id, clip);
        let mut dsc = MaskRectDsc::new();
        dsc.area = area;
        dsc.radius = radius;
        dsc.keep_outside = keep_outside;
        mask_rect(unit, &mut ctx, &t, &dsc);
        data(&ctx, id)
    }

    #[test]
    fn mask_rect_matches_upstream() {
        let mut u = SwDrawUnit::new(0);
        let full = Area::new(0, 0, 100, 100);
        let a = Area::new(10, 12, 45, 40);
        assert_eq!(fnv(&run(&mut u, full, a, 8, false)), 0x137d7899);
        assert_eq!(fnv(&run(&mut u, full, a, 8, true)), 0x5bbec619);
        assert_eq!(
            fnv(&run(&mut u, Area::new(15, 15, 40, 35), a, 32767, false)),
            0xd5163677
        );
        assert_eq!(
            fnv(&run(&mut u, full, Area::new(-10, 0, 30, 80), 5, false)),
            0x9d1fca6d
        );
        assert!(u.circle_cache.entries.iter().all(|e| e.used_cnt == 0));
    }

    #[test]
    fn mask_rect_invariants() {
        let mut u = SwDrawUnit::new(0);
        let (ctx, id) = make_ctx();
        let orig = data(&ctx, id);
        let stride = 60 * 4;
        let px = |x: i32, y: i32| ((y - 4) as usize) * stride + (x - 2) as usize * 4;

        // Square mask, keep outside: nothing changes.
        let out = run(
            &mut u,
            Area::new(0, 0, 100, 100),
            Area::new(10, 12, 45, 40),
            0,
            true,
        );
        assert_eq!(out, orig);

        // Square mask, clear outside: outside pixels are zeroed (within the
        // clip area), inside pixels are untouched.
        let clip = Area::new(5, 6, 50, 45);
        let out = run(&mut u, clip, Area::new(10, 12, 45, 40), 0, false);
        assert_eq!(
            &out[px(20, 20)..px(20, 20) + 4],
            &orig[px(20, 20)..px(20, 20) + 4]
        );
        assert_eq!(&out[px(6, 20)..px(6, 20) + 4], &[0, 0, 0, 0]);
        assert_eq!(&out[px(20, 44)..px(20, 44) + 4], &[0, 0, 0, 0]);
        // Outside of the clip area nothing is cleared.
        assert_eq!(&out[px(3, 20)..px(3, 20) + 4], &orig[px(3, 20)..px(3, 20) + 4]);

        // Rounded mask: the corner pixels lose alpha only, the colour stays.
        let out = run(&mut u, clip, Area::new(10, 12, 45, 40), 10, true);
        let p = px(12, 14);
        assert_eq!(&out[p..p + 3], &orig[p..p + 3]);
        assert!(out[p + 3] <= orig[p + 3]);
        let p = px(10, 12);
        assert_eq!(&out[p..p + 3], &orig[p..p + 3]);
        assert_eq!(out[p + 3], 0);
    }

    #[test]
    fn mask_rect_outside_of_clip_is_noop() {
        let mut u = SwDrawUnit::new(0);
        let (ctx, id) = make_ctx();
        let orig = data(&ctx, id);
        let out = run(&mut u, Area::new(0, 0, 5, 5), Area::new(10, 12, 45, 40), 4, false);
        assert_eq!(out, orig);
    }
}

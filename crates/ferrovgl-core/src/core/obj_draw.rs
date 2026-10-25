//! Ported from lvgl v9.6.0: `src/core/lv_obj_draw.c`
//! (also `include/lvgl/core/lv_obj_draw.h`, `src/core/lv_obj_draw_private.h`).
//!
//! Initialise draw descriptors from an object's styles and manage the
//! extended draw size.
//!
//! The `lv_obj_get_style_*_internal()` getters of `lv_obj_style_internal_gen.h`
//! are expressed with [`Ui::obj_get_style_prop`] and the `*_filtered_*`
//! variants with [`Ui::obj_style_apply_color_filter`], exactly like the
//! generated C inline functions.

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::{LayerType, Obj, SpecAttr};
use crate::draw::DrawDscBase;
use crate::draw::arc::ArcDsc;
use crate::draw::blur::BlurDsc;
use crate::draw::grad::GradDir;
use crate::draw::image::{DrawImageDsc, image_src_get_type};
use crate::draw::label::LabelDsc;
use crate::draw::line::LineDsc;
use crate::draw::rect::RectDsc;
use crate::font::FontRef;
use crate::misc::check_arg_failed;
use crate::misc::color::{Color, Color32, OPA_MAX, OPA_MIN, OPA_TRANSP, Opa, opa_mix2};
use crate::misc::event::{Event, EventCallback, EventCode, EventParam};
use crate::misc::style::{
    BlendMode, BlurQuality, BorderSide, ImageColorkey, ImageSrc, ImageSrcType, Part, SCALE_NONE, StyleProp,
    StyleValue,
};
use crate::misc::text::{TextAlign, TextDecor};

/// `user_data` of `set_ext_draw_size_event_cb` (`(void *)(lv_uintptr_t)size`).
///
/// The private type doubles as the identity of the callback: only
/// [`Ui::obj_set_ext_draw_size`] registers descriptors carrying it, so
/// "remove every `set_ext_draw_size_event_cb`" is "remove every descriptor
/// whose user data is an `ExtDrawSizeData`".
struct ExtDrawSizeData(i32);

impl Ui {
    /// `lv_obj_init_draw_rect_dsc()`: fill `draw_dsc` from the styles of
    /// `part`. Fields whose opacity is already `OPA_TRANSP` in `draw_dsc` are
    /// skipped (so a caller can disable e.g. the border in advance).
    pub fn obj_init_draw_rect_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut RectDsc) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_init_draw_rect_dsc: obj is valid");
            return;
        }

        draw_dsc.base.obj = Some(obj);
        draw_dsc.base.part = part;

        let opa = self.get_layer_opa(obj, part, &draw_dsc.base);
        if part != Part::MAIN && opa <= OPA_MIN {
            draw_dsc.bg_opa = OPA_TRANSP;
            draw_dsc.bg_image_opa = OPA_TRANSP;
            draw_dsc.border_opa = OPA_TRANSP;
            draw_dsc.outline_opa = OPA_TRANSP;
            draw_dsc.shadow_opa = OPA_TRANSP;
            return;
        }

        draw_dsc.radius = self.style_num(obj, part, StyleProp::RADIUS);

        if draw_dsc.bg_opa != OPA_TRANSP {
            draw_dsc.bg_opa = self.style_opa(obj, part, StyleProp::BG_OPA);
            if draw_dsc.bg_opa > OPA_MIN {
                let bg_color = self.style_color_filtered(obj, part, StyleProp::BG_COLOR);
                draw_dsc.bg_color = self.normal_apply_layer_recolor(obj, part, &draw_dsc.base, bg_color);
                let grad = match self.obj_get_style_prop(obj, part, StyleProp::BG_GRAD) {
                    StyleValue::Grad(g) => Some(g),
                    _ => None,
                };
                if let Some(grad) = grad.filter(|g| g.dir != GradDir::None) {
                    draw_dsc.bg_grad = *grad;
                } else {
                    draw_dsc.bg_grad.dir =
                        GradDir::from_u8(self.style_num(obj, part, StyleProp::BG_GRAD_DIR) as u8);
                    if draw_dsc.bg_grad.dir != GradDir::None {
                        draw_dsc.bg_grad.stops[0].color = draw_dsc.bg_color;
                        let bg_grad_color = self.style_color_filtered(obj, part, StyleProp::BG_GRAD_COLOR);
                        draw_dsc.bg_grad.stops[1].color =
                            self.normal_apply_layer_recolor(obj, part, &draw_dsc.base, bg_grad_color);
                        draw_dsc.bg_grad.stops[0].frac =
                            self.style_num(obj, part, StyleProp::BG_MAIN_STOP) as u8;
                        draw_dsc.bg_grad.stops[1].frac =
                            self.style_num(obj, part, StyleProp::BG_GRAD_STOP) as u8;
                        draw_dsc.bg_grad.stops[0].opa = self.style_opa(obj, part, StyleProp::BG_MAIN_OPA);
                        draw_dsc.bg_grad.stops[1].opa = self.style_opa(obj, part, StyleProp::BG_GRAD_OPA);
                    }
                }
            }
        }

        if draw_dsc.border_opa != OPA_TRANSP {
            draw_dsc.border_width = self.style_num(obj, part, StyleProp::BORDER_WIDTH);
            if draw_dsc.border_width != 0 {
                draw_dsc.border_opa = self.style_opa(obj, part, StyleProp::BORDER_OPA);
                if draw_dsc.border_opa > OPA_MIN {
                    // `lv_border_side_t border_side : 5`
                    draw_dsc.border_side = BorderSide::from_bits_retain(
                        self.style_num(obj, part, StyleProp::BORDER_SIDE) as u8 & 0x1F,
                    );
                    let border_color = self.style_color_filtered(obj, part, StyleProp::BORDER_COLOR);
                    draw_dsc.border_color =
                        self.normal_apply_layer_recolor(obj, part, &draw_dsc.base, border_color);
                }
            }
        }

        if draw_dsc.outline_opa != OPA_TRANSP {
            draw_dsc.outline_width = self.style_num(obj, part, StyleProp::OUTLINE_WIDTH);
            if draw_dsc.outline_width != 0 {
                draw_dsc.outline_opa = self.style_opa(obj, part, StyleProp::OUTLINE_OPA);
                if draw_dsc.outline_opa > OPA_MIN {
                    draw_dsc.outline_pad = self.style_num(obj, part, StyleProp::OUTLINE_PAD);
                    let outline_color = self.style_color_filtered(obj, part, StyleProp::OUTLINE_COLOR);
                    draw_dsc.outline_color =
                        self.normal_apply_layer_recolor(obj, part, &draw_dsc.base, outline_color);
                }
            }
        }

        if draw_dsc.bg_image_opa != OPA_TRANSP {
            draw_dsc.bg_image_src = self.style_image_src(obj, part, StyleProp::BG_IMAGE_SRC);
            if !matches!(draw_dsc.bg_image_src, ImageSrc::None) {
                draw_dsc.bg_image_opa = self.style_opa(obj, part, StyleProp::BG_IMAGE_OPA);
                if draw_dsc.bg_image_opa > OPA_MIN {
                    if image_src_get_type(&draw_dsc.bg_image_src) == ImageSrcType::Symbol {
                        draw_dsc.bg_image_symbol_font = self.style_text_font(obj, part);
                        let text_color = self.style_color_filtered(obj, part, StyleProp::TEXT_COLOR);
                        draw_dsc.bg_image_recolor =
                            self.normal_apply_layer_recolor(obj, part, &draw_dsc.base, text_color);
                    } else {
                        let bg_image_recolor =
                            self.style_color_filtered(obj, part, StyleProp::BG_IMAGE_RECOLOR);
                        let bg_image_recolor_opa = self.style_opa(obj, part, StyleProp::BG_IMAGE_RECOLOR_OPA);
                        let result = self.image_apply_layer_recolor(
                            obj,
                            part,
                            &draw_dsc.base,
                            bg_image_recolor,
                            bg_image_recolor_opa,
                        );
                        draw_dsc.bg_image_recolor_opa = result.alpha;
                        draw_dsc.bg_image_recolor = Color::make(result.red, result.green, result.blue);
                        draw_dsc.bg_image_tiled = self.style_num(obj, part, StyleProp::BG_IMAGE_TILED) != 0;
                    }
                    draw_dsc.bg_image_colorkey = self.style_colorkey(obj, part);
                }
            }
        }

        if draw_dsc.shadow_opa != 0 {
            draw_dsc.shadow_width = self.style_num(obj, part, StyleProp::SHADOW_WIDTH);
            if draw_dsc.shadow_width != 0 && draw_dsc.shadow_opa > OPA_MIN {
                draw_dsc.shadow_opa = self.style_opa(obj, part, StyleProp::SHADOW_OPA);
                if draw_dsc.shadow_opa > OPA_MIN {
                    draw_dsc.shadow_offset_x = self.style_num(obj, part, StyleProp::SHADOW_OFFSET_X);
                    draw_dsc.shadow_offset_y = self.style_num(obj, part, StyleProp::SHADOW_OFFSET_Y);
                    draw_dsc.shadow_spread = self.style_num(obj, part, StyleProp::SHADOW_SPREAD);
                    let shadow_color = self.style_color_filtered(obj, part, StyleProp::SHADOW_COLOR);
                    draw_dsc.shadow_color =
                        self.normal_apply_layer_recolor(obj, part, &draw_dsc.base, shadow_color);
                }
            }
        }

        if opa < OPA_MAX {
            draw_dsc.bg_opa = opa_mix2(draw_dsc.bg_opa, opa);
            draw_dsc.bg_image_opa = opa_mix2(draw_dsc.bg_image_opa, opa);
            draw_dsc.border_opa = opa_mix2(draw_dsc.border_opa, opa);
            draw_dsc.shadow_opa = opa_mix2(draw_dsc.shadow_opa, opa);
            draw_dsc.outline_opa = opa_mix2(draw_dsc.outline_opa, opa);
        }

        self.drop_shadow_init(obj, part, &mut draw_dsc.base);
    }

    /// `lv_obj_init_draw_label_dsc()`: fill `draw_dsc` from the text styles
    /// of `part`. `font` is taken from the `text_font` style, falling back to
    /// [`UiConfig::font_default`](crate::core::UiConfig::font_default)
    /// (`LV_FONT_DEFAULT`).
    pub fn obj_init_draw_label_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut LabelDsc) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_init_draw_label_dsc: obj is valid");
            return;
        }

        draw_dsc.base.obj = Some(obj);
        draw_dsc.base.part = part;

        draw_dsc.opa = self.style_opa(obj, part, StyleProp::TEXT_OPA);
        if draw_dsc.opa <= OPA_MIN {
            return;
        }

        let opa = self.get_layer_opa(obj, part, &draw_dsc.base);
        if opa < OPA_MAX {
            draw_dsc.opa = opa_mix2(draw_dsc.opa, opa);
        }
        if draw_dsc.opa <= OPA_MIN {
            return;
        }

        let text_color = self.style_color_filtered(obj, part, StyleProp::TEXT_COLOR);
        draw_dsc.color = self.normal_apply_layer_recolor(obj, part, &draw_dsc.base, text_color);
        draw_dsc.letter_space = self.style_num(obj, part, StyleProp::TEXT_LETTER_SPACE);
        draw_dsc.line_space = self.style_num(obj, part, StyleProp::TEXT_LINE_SPACE);
        // `lv_text_decor_t decor : 3`
        draw_dsc.decor =
            TextDecor::from_bits_retain(self.style_num(obj, part, StyleProp::TEXT_DECOR) as u8 & 0x07);
        draw_dsc.font = self.style_text_font(obj, part);
        #[cfg(feature = "bidi")]
        {
            draw_dsc.bidi_dir = base_dir_from_num(self.style_num(obj, Part::MAIN, StyleProp::BASE_DIR));
        }

        draw_dsc.align = text_align_from_num(self.style_num(obj, part, StyleProp::TEXT_ALIGN));

        self.drop_shadow_init(obj, part, &mut draw_dsc.base);
    }

    /// `lv_obj_init_draw_image_dsc()`: fill `draw_dsc` from the image styles
    /// of `part` (opacity, recolor, colorkey and, for non-main parts, the
    /// blend mode); the transformation is reset and the pivot centred.
    pub fn obj_init_draw_image_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut DrawImageDsc) {
        let Some(coords) = self.obj(obj).map(|o| o.coords) else {
            check_arg_failed("lv_obj_init_draw_image_dsc: obj is valid");
            return;
        };

        draw_dsc.base.obj = Some(obj);
        draw_dsc.base.part = part;

        draw_dsc.opa = self.style_opa(obj, part, StyleProp::IMAGE_OPA);
        if draw_dsc.opa <= OPA_MIN {
            return;
        }

        let opa = self.get_layer_opa(obj, part, &draw_dsc.base);
        if opa < OPA_MAX {
            draw_dsc.opa = opa_mix2(draw_dsc.opa, opa);
        }
        if draw_dsc.opa <= OPA_MIN {
            return;
        }

        draw_dsc.rotation = 0;
        draw_dsc.scale_x = SCALE_NONE;
        draw_dsc.scale_y = SCALE_NONE;
        draw_dsc.pivot.x = coords.width() / 2;
        draw_dsc.pivot.y = coords.height() / 2;

        let recolor = self.style_color_filtered(obj, part, StyleProp::IMAGE_RECOLOR);
        let recolor_opa = self.style_opa(obj, part, StyleProp::IMAGE_RECOLOR_OPA);
        let result = self.image_apply_layer_recolor(obj, part, &draw_dsc.base, recolor, recolor_opa);
        draw_dsc.recolor_opa = result.alpha;
        draw_dsc.recolor = Color::make(result.red, result.green, result.blue);

        draw_dsc.colorkey = self.style_colorkey(obj, part);

        if part != Part::MAIN {
            // `lv_blend_mode_t blend_mode : 4`
            draw_dsc.blend_mode =
                BlendMode::from_u8(self.style_num(obj, part, StyleProp::BLEND_MODE) as u8 & 0x0F);
        }

        self.drop_shadow_init(obj, part, &mut draw_dsc.base);
    }

    /// `lv_obj_init_draw_line_dsc()`: fill `draw_dsc` from the line styles
    /// of `part`.
    pub fn obj_init_draw_line_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut LineDsc) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_init_draw_line_dsc: obj is valid");
            return;
        }

        draw_dsc.base.obj = Some(obj);
        draw_dsc.base.part = part;

        draw_dsc.opa = self.style_opa(obj, part, StyleProp::LINE_OPA);
        if draw_dsc.opa <= OPA_MIN {
            return;
        }

        let opa = self.get_layer_opa(obj, part, &draw_dsc.base);
        if opa < OPA_MAX {
            draw_dsc.opa = opa_mix2(draw_dsc.opa, opa);
        }
        if draw_dsc.opa <= OPA_MIN {
            return;
        }

        draw_dsc.width = self.style_num(obj, part, StyleProp::LINE_WIDTH);
        if draw_dsc.width == 0 {
            return;
        }

        let line_color = self.style_color_filtered(obj, part, StyleProp::LINE_COLOR);
        draw_dsc.color = self.normal_apply_layer_recolor(obj, part, &draw_dsc.base, line_color);

        draw_dsc.dash_width = self.style_num(obj, part, StyleProp::LINE_DASH_WIDTH);
        if draw_dsc.dash_width != 0 {
            draw_dsc.dash_gap = self.style_num(obj, part, StyleProp::LINE_DASH_GAP);
        }

        draw_dsc.round_start = self.style_num(obj, part, StyleProp::LINE_ROUNDED) != 0;
        draw_dsc.round_end = draw_dsc.round_start;

        self.drop_shadow_init(obj, part, &mut draw_dsc.base);
    }

    /// `lv_obj_init_draw_arc_dsc()`: fill `draw_dsc` from the arc styles of
    /// `part`.
    pub fn obj_init_draw_arc_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut ArcDsc) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_init_draw_arc_dsc: obj is valid");
            return;
        }

        draw_dsc.base.obj = Some(obj);
        draw_dsc.base.part = part;

        draw_dsc.width = self.style_num(obj, part, StyleProp::ARC_WIDTH);
        if draw_dsc.width == 0 {
            return;
        }

        draw_dsc.opa = self.style_opa(obj, part, StyleProp::ARC_OPA);
        if draw_dsc.opa <= OPA_MIN {
            return;
        }

        let opa = self.get_layer_opa(obj, part, &draw_dsc.base);
        if opa < OPA_MAX {
            draw_dsc.opa = opa_mix2(draw_dsc.opa, opa);
        }
        if draw_dsc.opa <= OPA_MIN {
            return;
        }

        let arc_color = self.style_color_filtered(obj, part, StyleProp::ARC_COLOR);
        draw_dsc.color = self.normal_apply_layer_recolor(obj, part, &draw_dsc.base, arc_color);
        draw_dsc.img_src = self.style_image_src(obj, part, StyleProp::ARC_IMAGE_SRC);

        draw_dsc.rounded = self.style_num(obj, part, StyleProp::ARC_ROUNDED) != 0;

        self.drop_shadow_init(obj, part, &mut draw_dsc.base);
    }

    /// `lv_obj_init_draw_blur_dsc()`: fill `draw_dsc` from the blur styles of
    /// `part`. A `corner_radius` already set by the caller is kept.
    pub fn obj_init_draw_blur_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut BlurDsc) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_init_draw_blur_dsc: obj is valid");
            return;
        }

        draw_dsc.base.obj = Some(obj);
        draw_dsc.base.part = part;

        draw_dsc.blur_radius = self.style_num(obj, part, StyleProp::BLUR_RADIUS);
        draw_dsc.quality = BlurQuality::from_u8(self.style_num(obj, part, StyleProp::BLUR_QUALITY) as u8);

        /*Radius might be set earlier as it's already known*/
        if draw_dsc.corner_radius == 0 {
            draw_dsc.corner_radius = self.style_num(obj, part, StyleProp::RADIUS);
        }
    }

    /// `lv_obj_set_ext_draw_size()`: register a `LV_EVENT_REFR_EXT_DRAW_SIZE`
    /// handler requesting at least `size` extra pixels around `obj`
    /// (replacing a previous request).
    pub fn obj_set_ext_draw_size(&mut self, obj: Obj, size: i32) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_set_ext_draw_size: obj is valid");
            return;
        }

        // lv_obj_remove_event_cb(obj, set_ext_draw_size_event_cb);
        self.obj_remove_event_matching(obj, |dsc| {
            dsc.user_data.as_ref().is_some_and(|u| u.is::<ExtDrawSizeData>())
        });
        let cb: EventCallback = Rc::new(set_ext_draw_size_event_cb);
        let user_data: Rc<dyn Any> = Rc::new(ExtDrawSizeData(size));
        self.obj_add_event_cb(obj, cb, EventCode::REFR_EXT_DRAW_SIZE, Some(user_data));
    }

    /// `lv_obj_calculate_ext_draw_size()`: the extra size `part` needs around
    /// the object for its shadow, outline, drop shadow and transform size.
    pub fn obj_calculate_ext_draw_size(&self, obj: Obj, part: Part) -> i32 {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_calculate_ext_draw_size: obj is valid");
            return 0;
        }

        let mut s: i32 = 0;

        let mut sh_width = self.style_num(obj, part, StyleProp::SHADOW_WIDTH);
        if sh_width != 0 {
            let sh_opa = self.style_opa(obj, part, StyleProp::SHADOW_OPA);
            if sh_opa > OPA_MIN {
                sh_width = sh_width / 2 + 1; /*The blur adds only half width*/
                sh_width += self.style_num(obj, part, StyleProp::SHADOW_SPREAD);
                let sh_ofs_x = self.style_num(obj, part, StyleProp::SHADOW_OFFSET_X);
                let sh_ofs_y = self.style_num(obj, part, StyleProp::SHADOW_OFFSET_Y);
                sh_width += sh_ofs_x.abs().max(sh_ofs_y.abs());
                s = s.max(sh_width);
            }
        }

        let outline_width = self.style_num(obj, part, StyleProp::OUTLINE_WIDTH);
        if outline_width != 0 {
            let outline_opa = self.style_opa(obj, part, StyleProp::OUTLINE_OPA);
            if outline_opa > OPA_MIN {
                let outline_pad = self.style_num(obj, part, StyleProp::OUTLINE_PAD);
                s = s.max(outline_pad + outline_width);
            }
        }

        let mut drop_shadow_size: i32 = 0;
        if self.style_opa(obj, part, StyleProp::DROP_SHADOW_OPA) > 0 {
            drop_shadow_size += self
                .style_num(obj, part, StyleProp::DROP_SHADOW_OFFSET_X)
                .abs()
                .max(self.style_num(obj, part, StyleProp::DROP_SHADOW_OFFSET_Y).abs());
            drop_shadow_size += self.style_num(obj, part, StyleProp::DROP_SHADOW_RADIUS) + 1;
        }
        s += drop_shadow_size;

        let w = self.style_num(obj, part, StyleProp::TRANSFORM_WIDTH);
        let h = self.style_num(obj, part, StyleProp::TRANSFORM_HEIGHT);
        let wh = w.max(h);
        if wh > 0 {
            s += wh;
        }

        s
    }

    /// `lv_obj_refresh_ext_draw_size()`: ask the object (and its handlers)
    /// for its extra draw size with `LV_EVENT_REFR_EXT_DRAW_SIZE`, store it
    /// and invalidate the object if it changed.
    pub fn obj_refresh_ext_draw_size(&mut self, obj: Obj) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_refresh_ext_draw_size: obj is valid");
            return;
        }

        let s_old = self.obj_get_ext_draw_size(obj);
        let mut s_new: i32 = 0;
        self.obj_send_event(
            obj,
            EventCode::REFR_EXT_DRAW_SIZE,
            EventParam::ExtDrawSize(&mut s_new),
        );

        // The event may have deleted the object.
        let Some(o) = self.obj_mut(obj) else {
            return;
        };

        /*Store the result if the special attrs already allocated*/
        if let Some(spec) = o.spec.as_deref_mut() {
            spec.ext_draw_size = s_new;
        }
        /*Allocate spec. attrs. only if the result is not zero.
         *Zero is the default value if the spec. attr. are not defined.*/
        else if s_new != 0 {
            let mut spec = Box::new(SpecAttr::new());
            spec.ext_draw_size = s_new;
            o.spec = Some(spec);
        }

        if s_new != s_old {
            self.obj_invalidate(obj);
        }
    }

    /// `lv_obj_get_ext_draw_size()`: the size of the extended draw area
    /// around the object's coordinates.
    pub fn obj_get_ext_draw_size(&self, obj: Obj) -> i32 {
        let Some(o) = self.obj(obj) else {
            check_arg_failed("lv_obj_get_ext_draw_size: obj is valid");
            return 0;
        };
        match o.spec.as_deref() {
            Some(spec) => spec.ext_draw_size,
            None => 0,
        }
    }

    /// `lv_obj_get_layer_type()`
    pub fn obj_get_layer_type(&self, obj: Obj) -> LayerType {
        let Some(o) = self.obj(obj) else {
            check_arg_failed("lv_obj_get_layer_type: obj is valid");
            return LayerType::None;
        };
        match o.spec.as_deref() {
            Some(spec) => spec.layer_type,
            None => LayerType::None,
        }
    }

    /// `get_layer_opa()`
    fn get_layer_opa(&self, obj: Obj, part: Part, base_dsc: &DrawDscBase) -> Opa {
        if let Some(layer) = base_dsc.layer.and_then(|l| self.draw.layers.get(l.0)) {
            /* Accessing the layer opa directly is faster than using get style opa recursive */
            if part == Part::MAIN {
                return layer.opa;
            }
            return opa_mix2(layer.opa, self.style_opa(obj, part, StyleProp::OPA));
        }

        /* fallback to old recursive style opa */
        self.obj_get_style_opa_recursive(obj, part)
    }

    /// The recolor shared by `normal_apply_layer_recolor()` and
    /// `image_apply_layer_recolor()`.
    fn layer_recolor(&self, obj: Obj, part: Part, base_dsc: &DrawDscBase) -> Color32 {
        let mut recolor: Color32;
        if let Some(layer) = base_dsc.layer.and_then(|l| self.draw.layers.get(l.0)) {
            recolor = layer.recolor;
            if part != Part::MAIN {
                recolor = self.obj_style_apply_recolor(obj, part, recolor);
            }
        } else {
            recolor = self.obj_get_style_recolor_recursive(obj, part);
        }
        recolor
    }

    /// `normal_apply_layer_recolor()`
    fn normal_apply_layer_recolor(
        &self,
        obj: Obj,
        part: Part,
        base_dsc: &DrawDscBase,
        color: Color,
    ) -> Color {
        let recolor = self.layer_recolor(obj, part, base_dsc);
        mix_recolor(recolor, color)
    }

    /// `image_apply_layer_recolor()`
    fn image_apply_layer_recolor(
        &self,
        obj: Obj,
        part: Part,
        base_dsc: &DrawDscBase,
        color: Color,
        opa: Opa,
    ) -> Color32 {
        let recolor = self.layer_recolor(obj, part, base_dsc);
        image_recolor_combine(recolor, color, opa)
    }

    /// `drop_shadow_init()`
    fn drop_shadow_init(&self, obj: Obj, part: Part, base_dsc: &mut DrawDscBase) {
        base_dsc.drop_shadow_opa = self.style_opa(obj, part, StyleProp::DROP_SHADOW_OPA);
        if base_dsc.drop_shadow_opa != 0 {
            // `int32_t drop_shadow_blur_radius : 20`
            base_dsc.drop_shadow_blur_radius =
                bitfield20(self.style_num(obj, part, StyleProp::DROP_SHADOW_RADIUS));
            base_dsc.drop_shadow_ofs_x = self.style_num(obj, part, StyleProp::DROP_SHADOW_OFFSET_X) as i16;
            base_dsc.drop_shadow_ofs_y = self.style_num(obj, part, StyleProp::DROP_SHADOW_OFFSET_Y) as i16;
            base_dsc.drop_shadow_color = self.style_color(obj, part, StyleProp::DROP_SHADOW_COLOR);
            base_dsc.drop_shadow_color =
                self.normal_apply_layer_recolor(obj, part, base_dsc, base_dsc.drop_shadow_color);
            // `lv_blur_quality_t drop_shadow_quality : 3`
            base_dsc.drop_shadow_quality =
                BlurQuality::from_u8(self.style_num(obj, part, StyleProp::DROP_SHADOW_QUALITY) as u8 & 0x07);
        }
    }

    /// `lv_obj_get_style_*_color_filtered_internal()`
    fn style_color_filtered(&self, obj: Obj, part: Part, prop: StyleProp) -> Color {
        let v = self.obj_get_style_prop(obj, part, prop);
        self.obj_style_apply_color_filter(obj, part, v).color()
    }

    /// `lv_obj_get_style_bg_image_src_internal()` / `..._arc_image_src_...`
    fn style_image_src(&self, obj: Obj, part: Part, prop: StyleProp) -> ImageSrc {
        match self.obj_get_style_prop(obj, part, prop) {
            StyleValue::Image(src) => src,
            _ => ImageSrc::None,
        }
    }

    /// `lv_obj_get_style_text_font_internal()`; an unset value is
    /// `LV_FONT_DEFAULT` ([`UiConfig::font_default`](crate::core::UiConfig::font_default)).
    fn style_text_font(&self, obj: Obj, part: Part) -> Option<FontRef> {
        match self.obj_get_style_prop(obj, part, StyleProp::TEXT_FONT) {
            StyleValue::Font(f) => Some(f),
            _ => self.config().font_default.clone(),
        }
    }

    /// `lv_obj_get_style_image_colorkey_internal()`
    fn style_colorkey(&self, obj: Obj, part: Part) -> Option<ImageColorkey> {
        match self.obj_get_style_prop(obj, part, StyleProp::IMAGE_COLORKEY) {
            StyleValue::Colorkey(k) => Some(k),
            _ => None,
        }
    }
}

/// `set_ext_draw_size_event_cb()`
fn set_ext_draw_size_event_cb(_ui: &mut Ui, e: &mut Event<'_>) {
    let size = e
        .user_data()
        .and_then(|u| u.downcast_ref::<ExtDrawSizeData>())
        .map_or(0, |d| d.0);
    e.set_ext_draw_size(size);
}

/// Last step of `normal_apply_layer_recolor()`:
/// `lv_color_mix(lv_color_make(recolor.rgb), color, recolor.alpha)`.
fn mix_recolor(recolor: Color32, color: Color) -> Color {
    Color::mix(
        Color::make(recolor.red, recolor.green, recolor.blue),
        color,
        recolor.alpha,
    )
}

/// Last step of `image_apply_layer_recolor()`: combine the layer / style
/// recolor with the image recolor `color` / `opa`.
fn image_recolor_combine(recolor: Color32, color: Color, opa: Opa) -> Color32 {
    if opa > OPA_TRANSP && recolor.alpha > OPA_TRANSP {
        Color32::over32(recolor, color.to_32(opa))
    } else if recolor.alpha > OPA_TRANSP {
        recolor
    } else {
        color.to_32(opa)
    }
}

/// Store `v` in a signed 20-bit C bit field (`int32_t x : 20`).
fn bitfield20(v: i32) -> i32 {
    v.wrapping_shl(12) >> 12
}

/// `(lv_text_align_t)v.num`
fn text_align_from_num(v: i32) -> TextAlign {
    match v {
        1 => TextAlign::Left,
        2 => TextAlign::Center,
        3 => TextAlign::Right,
        _ => TextAlign::Auto,
    }
}

/// `(lv_base_dir_t)v.num`
#[cfg(feature = "bidi")]
fn base_dir_from_num(v: i32) -> crate::misc::text::BaseDir {
    use crate::misc::text::BaseDir;
    match v {
        0x01 => BaseDir::Rtl,
        0x02 => BaseDir::Auto,
        0x20 => BaseDir::Neutral,
        0x21 => BaseDir::Weak,
        _ => BaseDir::Ltr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::core::obj_event::tests::{PLAIN_CLASS, add_obj};
    use crate::draw::{Layer, LayerId};
    use crate::misc::color::OPA_COVER;

    #[test]
    fn set_ext_draw_size_replaces_previous_request() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        ui.obj_set_ext_draw_size(obj, 10);
        ui.obj_set_ext_draw_size(obj, 4);
        assert_eq!(ui.obj_get_event_count(obj), 1);

        let mut s = 0;
        ui.obj_send_event(
            obj,
            EventCode::REFR_EXT_DRAW_SIZE,
            EventParam::ExtDrawSize(&mut s),
        );
        assert_eq!(s, 4);
        // `lv_event_set_ext_draw_size()` keeps the larger value.
        let mut s = 9;
        ui.obj_send_event(
            obj,
            EventCode::REFR_EXT_DRAW_SIZE,
            EventParam::ExtDrawSize(&mut s),
        );
        assert_eq!(s, 9);

        // Other callbacks are kept.
        let other: EventCallback = Rc::new(|_ui: &mut Ui, e: &mut Event<'_>| e.set_ext_draw_size(2));
        ui.obj_add_event_cb(obj, other, EventCode::REFR_EXT_DRAW_SIZE, None);
        ui.obj_set_ext_draw_size(obj, 1);
        assert_eq!(ui.obj_get_event_count(obj), 2);
        let mut s = 0;
        ui.obj_send_event(
            obj,
            EventCode::REFR_EXT_DRAW_SIZE,
            EventParam::ExtDrawSize(&mut s),
        );
        assert_eq!(s, 2);
    }

    #[test]
    fn refresh_ext_draw_size_stores_result() {
        let mut ui = Ui::new(UiConfig::default());
        // No handler: the result is 0 and no special attributes are allocated.
        let plain = add_obj(&mut ui, &PLAIN_CLASS, None);
        assert_eq!(ui.obj_get_ext_draw_size(plain), 0);
        ui.obj_refresh_ext_draw_size(plain);
        assert_eq!(ui.obj_get_ext_draw_size(plain), 0);
        assert!(ui.obj(plain).unwrap().spec.is_none());

        // Unchanged result (no invalidation needed).
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        ui.obj_set_ext_draw_size(obj, 12);
        ui.obj_mut(obj).unwrap().spec.as_mut().unwrap().ext_draw_size = 12;
        ui.obj_refresh_ext_draw_size(obj);
        assert_eq!(ui.obj_get_ext_draw_size(obj), 12);
    }

    #[test]
    fn layer_type_defaults_to_none() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        assert_eq!(ui.obj_get_layer_type(obj), LayerType::None);
        ui.obj_mut(obj).unwrap().spec = Some(Box::new(SpecAttr::new()));
        ui.obj_mut(obj).unwrap().spec.as_mut().unwrap().layer_type = LayerType::Transform;
        assert_eq!(ui.obj_get_layer_type(obj), LayerType::Transform);
    }

    #[test]
    fn main_part_uses_layer_opa_and_recolor() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        let mut layer = Layer::new();
        layer.opa = 100;
        layer.recolor = Color32::make(255, 0, 0, 255);
        let id = LayerId(ui.draw.layers.insert(layer));
        let mut base = DrawDscBase::new();
        base.layer = Some(id);
        assert_eq!(ui.get_layer_opa(obj, Part::MAIN, &base), 100);
        let c = ui.normal_apply_layer_recolor(obj, Part::MAIN, &base, Color::make(0, 0, 255));
        assert_eq!(c, Color::make(255, 0, 0));
        let c32 = ui.image_apply_layer_recolor(obj, Part::MAIN, &base, Color::make(0, 0, 255), 0);
        assert_eq!(c32, Color32::make(255, 0, 0, 255));
    }

    #[test]
    fn recolor_helpers() {
        let none = Color32::make(1, 2, 3, 0);
        let c = Color::make(10, 20, 30);
        // Transparent recolor leaves the colour untouched.
        assert_eq!(mix_recolor(none, c), c);
        assert_eq!(
            mix_recolor(Color32::make(0, 0, 0, OPA_COVER), c),
            Color::make(0, 0, 0)
        );
        assert_eq!(image_recolor_combine(none, c, 77), c.to_32(77));
        let red = Color32::make(255, 0, 0, 128);
        assert_eq!(image_recolor_combine(red, c, 0), red);
        assert_eq!(
            image_recolor_combine(red, c, 200),
            Color32::over32(red, c.to_32(200))
        );
    }

    #[test]
    fn c_bitfield_and_enum_conversions() {
        assert_eq!(bitfield20(5), 5);
        assert_eq!(bitfield20(-5), -5);
        assert_eq!(bitfield20(0x7FFFF), 0x7FFFF);
        assert_eq!(bitfield20(0x80000), -0x80000);
        assert_eq!(bitfield20(0x100005), 5);
        assert_eq!(text_align_from_num(2), TextAlign::Center);
        assert_eq!(text_align_from_num(0), TextAlign::Auto);
        #[cfg(feature = "bidi")]
        {
            use crate::misc::text::BaseDir;
            assert_eq!(base_dir_from_num(1), BaseDir::Rtl);
            assert_eq!(base_dir_from_num(0x21), BaseDir::Weak);
        }
    }
}

//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_rect.h`, `src/draw/lv_draw_rect.c`.
//!
//! Two entry points exist for every `lv_draw_*()` task creator of
//! `lv_draw_rect.c`, `lv_draw_label.c` and `lv_draw_mask.c`:
//!
//! - `DrawCtx::draw_*` (no draw units and no object tree at hand): the
//!   tasks are created exactly like upstream and
//!   `lv_draw_finalize_task_creation()` is reduced to its unit-less part (the
//!   descriptor's `base.layer` is set and a dispatch is requested). The
//!   tasks stay `WAITING` without a preferred unit; the single-threaded
//!   software unit takes them in queue order on the next dispatch.
//! - `Ui::draw_*`: the full upstream flow through
//!   `Ui::draw_finalize_task_creation` (`LV_EVENT_DRAW_TASK_ADDED`, unit
//!   evaluation, dispatch).
//!
//! Both share one generic body per function through the crate-private
//! `TaskSink` trait, so the decomposition logic exists only once.

use alloc::boxed::Box;
use alloc::rc::Rc;

use crate::core::Ui;
use crate::draw::grad::{GradDir, GradDsc};
use crate::draw::image::{DrawImageDsc, image_header_of};
use crate::draw::label::LabelDsc;
use crate::draw::{DrawCtx, DrawDsc, DrawDscBase, DrawTaskRef, DrawTaskType, ImageHeader, LayerId};
use crate::font::FontRef;
use crate::misc::area::{Align, Area, Point};
use crate::misc::color::{Color, OPA_COVER, OPA_MAX, OPA_MIN, Opa};
use crate::misc::event::{EventCode, EventParam};
use crate::misc::style::{BorderSide, ImageColorkey, ImageSrc, ImageSrcType};
use crate::misc::text::{self, TextAttributes, TextFlags};
use crate::misc::types::coord;

/// `LV_RADIUS_CIRCLE`: a radius large enough to always produce a circle.
pub const RADIUS_CIRCLE: i32 = 0x7FFF;

/// `lv_draw_rect_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct RectDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `radius`
    pub radius: i32,
    /// `bg_image_src`
    pub bg_image_src: ImageSrc,
    /// `bg_image_symbol_font`
    pub bg_image_symbol_font: Option<FontRef>,
    /// `bg_image_recolor`
    pub bg_image_recolor: Color,
    /// `bg_image_opa`
    pub bg_image_opa: Opa,
    /// `bg_image_recolor_opa`
    pub bg_image_recolor_opa: Opa,
    /// `bg_image_tiled`
    pub bg_image_tiled: bool,
    /// `bg_opa`
    pub bg_opa: Opa,
    /// `border_opa`
    pub border_opa: Opa,
    /// `outline_opa`
    pub outline_opa: Opa,
    /// `shadow_opa`
    pub shadow_opa: Opa,
    /// `bg_color`
    pub bg_color: Color,
    /// `bg_grad`
    pub bg_grad: GradDsc,
    /// `bg_image_colorkey`
    pub bg_image_colorkey: Option<ImageColorkey>,
    /// `border_color`
    pub border_color: Color,
    /// `border_width`
    pub border_width: i32,
    /// `border_side`
    pub border_side: BorderSide,
    /// `border_post`: draw the border after the children.
    pub border_post: bool,
    /// `outline_color`
    pub outline_color: Color,
    /// `outline_width`
    pub outline_width: i32,
    /// `outline_pad`
    pub outline_pad: i32,
    /// `shadow_color`
    pub shadow_color: Color,
    /// `shadow_width`
    pub shadow_width: i32,
    /// `shadow_offset_x`
    pub shadow_offset_x: i32,
    /// `shadow_offset_y`
    pub shadow_offset_y: i32,
    /// `shadow_spread`
    pub shadow_spread: i32,
}

impl RectDsc {
    /// `lv_draw_rect_dsc_init()`
    ///
    /// `bg_image_symbol_font` is `LV_FONT_DEFAULT` upstream. The default
    /// font lives in `UiConfig::font_default`, which a bare descriptor
    /// cannot see, so it starts as `None` (`lv_obj_init_draw_rect_dsc()`
    /// sets it from the style).
    pub fn new() -> Self {
        let mut bg_grad = GradDsc::new();
        bg_grad.stops[0].color = Color::white();
        bg_grad.stops[1].color = Color::black();
        bg_grad.stops[1].frac = 0xFF;
        bg_grad.stops_count = 2;
        Self {
            base: DrawDscBase::new(),
            radius: 0,
            bg_image_src: ImageSrc::None,
            bg_image_symbol_font: None,
            bg_image_recolor: Color::BLACK,
            bg_image_opa: OPA_COVER,
            bg_image_recolor_opa: 0,
            bg_image_tiled: false,
            bg_opa: OPA_COVER,
            border_opa: OPA_COVER,
            outline_opa: OPA_COVER,
            shadow_opa: OPA_COVER,
            bg_color: Color::white(),
            bg_grad,
            bg_image_colorkey: None,
            border_color: Color::black(),
            border_width: 0,
            border_side: BorderSide::FULL,
            border_post: false,
            outline_color: Color::BLACK,
            outline_width: 0,
            outline_pad: 0,
            shadow_color: Color::black(),
            shadow_width: 0,
            shadow_offset_x: 0,
            shadow_offset_y: 0,
            shadow_spread: 0,
        }
    }
}

impl Default for RectDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_fill_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct FillDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `radius`
    pub radius: i32,
    /// `opa`
    pub opa: Opa,
    /// `color`
    pub color: Color,
    /// `grad`
    pub grad: GradDsc,
}

impl FillDsc {
    /// `lv_draw_fill_dsc_init()`
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            radius: 0,
            opa: OPA_COVER,
            color: Color::BLACK,
            grad: GradDsc::new(),
        }
    }
}

impl Default for FillDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_border_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct BorderDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `radius`
    pub radius: i32,
    /// `color`
    pub color: Color,
    /// `width`
    pub width: i32,
    /// `opa`
    pub opa: Opa,
    /// `side`
    pub side: BorderSide,
}

impl BorderDsc {
    /// `lv_draw_border_dsc_init()`
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            radius: 0,
            color: Color::BLACK,
            width: 0,
            opa: OPA_COVER,
            side: BorderSide::FULL,
        }
    }
}

impl Default for BorderDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_box_shadow_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct BoxShadowDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `radius`
    pub radius: i32,
    /// `color`
    pub color: Color,
    /// `width`
    pub width: i32,
    /// `spread`
    pub spread: i32,
    /// `ofs_x`
    pub ofs_x: i32,
    /// `ofs_y`
    pub ofs_y: i32,
    /// `opa`
    pub opa: Opa,
    /// `bg_cover`: the background fully covers the rectangle, so the shadow
    /// below it can be skipped.
    pub bg_cover: bool,
}

impl BoxShadowDsc {
    /// `lv_draw_box_shadow_dsc_init()`
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            radius: 0,
            color: Color::BLACK,
            width: 0,
            spread: 0,
            ofs_x: 0,
            ofs_y: 0,
            opa: OPA_COVER,
            bg_cover: false,
        }
    }
}

impl Default for BoxShadowDsc {
    fn default() -> Self {
        Self::new()
    }
}

/*
 * Task sink: who finalizes the tasks.
 */

/// The context the `lv_draw_*()` task creators of `lv_draw_rect.c`,
/// `lv_draw_label.c` and `lv_draw_mask.c` add their tasks to (see the
/// module docs).
pub(crate) trait TaskSink {
    /// The draw context owning the layers.
    fn ctx(&mut self) -> &mut DrawCtx;

    /// `lv_draw_finalize_task_creation(layer, t)`
    fn finalize(&mut self, t: DrawTaskRef);

    /// Create a shadow layer with this sink's draw-unit lifecycle notifications.
    fn create_drop_shadow(&mut self, parent: LayerId, base: &DrawDscBase, area: &Area) -> Option<LayerId> {
        self.ctx().layer_create_drop_shadow(parent, base, area)
    }

    /// Finish a drop shadow while retaining this sink's draw-unit dispatch.
    fn finish_drop_shadow(&mut self, layer: LayerId, base: &DrawDscBase) {
        self.ctx().layer_finish_drop_shadow(layer, base);
    }

    /// The `LV_EVENT_DRAW_TASK_ADDED` block of `lv_draw_mask_rect()`, sent
    /// before finalizing (only an object tree can send it).
    fn send_mask_task_added(&mut self, t: DrawTaskRef) {
        let _ = t;
    }
}

impl TaskSink for DrawCtx {
    #[inline]
    fn ctx(&mut self) -> &mut DrawCtx {
        self
    }

    /// The unit-less part of `lv_draw_finalize_task_creation()`: bind the
    /// descriptor to its layer and request a dispatch. Evaluation and
    /// dispatching need the draw units (`Ui::draw_finalize_task_creation`).
    fn finalize(&mut self, t: DrawTaskRef) {
        if let Some(task) = self.task_mut(t)
            && let Some(base_dsc) = task.dsc.base_mut()
        {
            base_dsc.layer = Some(t.layer);
        }
        self.dispatch_request();
    }
}

impl TaskSink for Ui {
    #[inline]
    fn ctx(&mut self) -> &mut DrawCtx {
        &mut self.draw
    }

    #[inline]
    fn finalize(&mut self, t: DrawTaskRef) {
        self.draw_finalize_task_creation(t);
    }

    fn finish_drop_shadow(&mut self, layer: LayerId, base: &DrawDscBase) {
        self.layer_finish_drop_shadow(layer, base);
    }

    fn create_drop_shadow(&mut self, parent: LayerId, base: &DrawDscBase, area: &Area) -> Option<LayerId> {
        self.layer_create_drop_shadow(parent, base, area)
    }

    fn send_mask_task_added(&mut self, t: DrawTaskRef) {
        let obj = self
            .draw
            .task(t)
            .and_then(|task| task.dsc.base())
            .and_then(|b| b.obj);
        if let Some(obj) = obj
            && self.obj_is_send_draw_task_events(obj)
        {
            /*Disable sending LV_EVENT_DRAW_TASK_ADDED first to avoid triggering recursive
             *event calls due draw task adds in the event*/
            self.obj_set_send_draw_task_events(obj, false);
            self.obj_send_event(obj, EventCode::DRAW_TASK_ADDED, EventParam::DrawTask(t));
            self.obj_set_send_draw_task_events(obj, true);
        }
    }
}

/// `LV_CHECK_ARG(layer != NULL, return)` for a layer handle.
pub(crate) fn check_layer(ctx: &DrawCtx, layer: LayerId) -> bool {
    if ctx.layer(layer).is_none() {
        crate::misc::check_arg_failed("layer != NULL");
        return false;
    }
    true
}

/// Store the descriptor of a freshly added task (`t->draw_dsc`).
#[inline]
pub(crate) fn set_task_dsc(ctx: &mut DrawCtx, t: DrawTaskRef, dsc: DrawDsc) {
    if let Some(task) = ctx.task_mut(t) {
        task.dsc = dsc;
    }
}

/// `lv_area_increase(&t->_real_area, w_extra, h_extra)`
fn real_area_increase(ctx: &mut DrawCtx, t: DrawTaskRef, w_extra: i32, h_extra: i32) {
    if let Some(task) = ctx.task_mut(t) {
        task.real_area.increase(w_extra, h_extra);
    }
}

/// `lv_draw_fill()`
fn draw_fill_impl<S: TaskSink + ?Sized>(s: &mut S, layer: LayerId, dsc: &FillDsc, coords: &Area) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    if dsc.opa <= OPA_MIN {
        return;
    }

    if dsc.base.drop_shadow_opa != 0
        && let Some(ds_layer) = s.create_drop_shadow(layer, &dsc.base, coords)
    {
        let mut ds_dsc = dsc.clone();
        ds_dsc.base.drop_shadow_opa = 0; /*Disable drop shadow so rendering below will render plain fill*/
        draw_fill_impl(s, ds_layer, &ds_dsc, coords);
        s.finish_drop_shadow(ds_layer, &dsc.base);
    }

    let t = s.ctx().add_task(layer, coords, DrawTaskType::Fill);

    set_task_dsc(s.ctx(), t, DrawDsc::Fill(Box::new(dsc.clone())));

    s.finalize(t);
}

/// `lv_draw_border()`
fn draw_border_impl<S: TaskSink + ?Sized>(s: &mut S, layer: LayerId, dsc: &BorderDsc, coords: &Area) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    if dsc.opa <= OPA_MIN {
        return;
    }

    if dsc.base.drop_shadow_opa != 0
        && let Some(ds_layer) = s.create_drop_shadow(layer, &dsc.base, coords)
    {
        let mut ds_dsc = dsc.clone();
        ds_dsc.base.drop_shadow_opa = 0; /*Disable drop shadow so rendering below will render plain border*/
        draw_border_impl(s, ds_layer, &ds_dsc, coords);
        s.finish_drop_shadow(ds_layer, &dsc.base);
    }

    let t = s.ctx().add_task(layer, coords, DrawTaskType::Border);

    set_task_dsc(s.ctx(), t, DrawDsc::Border(Box::new(dsc.clone())));

    s.finalize(t);
}

/// `lv_draw_box_shadow()`
fn draw_box_shadow_impl<S: TaskSink + ?Sized>(s: &mut S, layer: LayerId, dsc: &BoxShadowDsc, coords: &Area) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    if dsc.opa <= OPA_MIN {
        return;
    }

    let t = s.ctx().add_task(layer, coords, DrawTaskType::BoxShadow);

    set_task_dsc(s.ctx(), t, DrawDsc::BoxShadow(Box::new(dsc.clone())));

    s.finalize(t);
}

/// `lv_draw_rect()`
fn draw_rect_impl<S: TaskSink + ?Sized>(s: &mut S, layer: LayerId, dsc: &RectDsc, coords: &Area) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    let has_shadow = !(dsc.shadow_width == 0
        || dsc.shadow_opa <= OPA_MIN
        || (dsc.shadow_width == 1
            && dsc.shadow_spread <= 0
            && dsc.shadow_offset_x == 0
            && dsc.shadow_offset_y == 0));

    let has_fill = dsc.bg_opa > OPA_MIN;

    let has_bg_img = !(dsc.bg_image_opa <= OPA_MIN || dsc.bg_image_src.is_none());

    let has_border = !(dsc.border_opa <= OPA_MIN
        || dsc.border_width == 0
        || dsc.border_post
        || dsc.border_side == BorderSide::NONE);

    let has_outline = !(dsc.outline_opa <= OPA_MIN || dsc.outline_width == 0);

    let mut bg_cover = true;
    if dsc.bg_opa < OPA_COVER {
        bg_cover = false;
    } else if dsc.bg_grad.dir != GradDir::None {
        for stop in dsc
            .bg_grad
            .stops
            .iter()
            .take(usize::from(dsc.bg_grad.stops_count))
        {
            if stop.opa != OPA_COVER {
                bg_cover = false;
                break;
            }
        }
    }

    if dsc.base.drop_shadow_opa != 0
        && (has_fill || has_outline)
        && let Some(ds_layer) = s.create_drop_shadow(layer, &dsc.base, coords)
    {
        let mut ds_dsc = dsc.clone();
        ds_dsc.base.drop_shadow_opa = 0; /*Disable drop shadow so rendering below will render plain shadow*/
        ds_dsc.shadow_opa = 0;
        draw_rect_impl(s, ds_layer, &ds_dsc, coords);
        s.finish_drop_shadow(ds_layer, &dsc.base);
    }

    /*Shadow*/
    if has_shadow {
        /*Check whether the shadow is visible*/
        let t = s.ctx().add_task(layer, coords, DrawTaskType::BoxShadow);

        let ctx = s.ctx();
        real_area_increase(ctx, t, dsc.shadow_spread, dsc.shadow_spread);
        real_area_increase(ctx, t, dsc.shadow_width, dsc.shadow_width);
        if let Some(task) = ctx.task_mut(t) {
            task.real_area.move_by(dsc.shadow_offset_x, dsc.shadow_offset_y);
        }
        /*The task's descriptor is zeroed by `lv_draw_add_task()`, then every field is set*/
        let shadow_dsc = BoxShadowDsc {
            base: dsc.base.clone(),
            radius: dsc.radius,
            color: dsc.shadow_color,
            width: dsc.shadow_width,
            spread: dsc.shadow_spread,
            opa: dsc.shadow_opa,
            ofs_x: dsc.shadow_offset_x,
            ofs_y: dsc.shadow_offset_y,
            bg_cover,
        };
        set_task_dsc(ctx, t, DrawDsc::BoxShadow(Box::new(shadow_dsc)));
        s.finalize(t);
    }

    /*Background*/
    if has_fill {
        let mut bg_coords = *coords;
        /*If the border fully covers make the bg area 1px smaller to avoid artifacts on the corners*/
        if dsc.border_width > 1 && dsc.border_opa >= OPA_MAX && dsc.radius != 0 {
            bg_coords.x1 += i32::from(dsc.border_side.contains(BorderSide::LEFT));
            bg_coords.y1 += i32::from(dsc.border_side.contains(BorderSide::TOP));
            bg_coords.x2 -= i32::from(dsc.border_side.contains(BorderSide::RIGHT));
            bg_coords.y2 -= i32::from(dsc.border_side.contains(BorderSide::BOTTOM));
        }

        let t = s.ctx().add_task(layer, &bg_coords, DrawTaskType::Fill);

        let mut bg_dsc = FillDsc::new();
        bg_dsc.base = dsc.base.clone();
        bg_dsc.radius = dsc.radius;
        bg_dsc.color = dsc.bg_color;
        bg_dsc.grad = dsc.bg_grad;
        bg_dsc.opa = dsc.bg_opa;
        set_task_dsc(s.ctx(), t, DrawDsc::Fill(Box::new(bg_dsc)));

        s.finalize(t);
    }

    /*Background image*/
    if has_bg_img {
        let src_type = dsc.bg_image_src.src_type();
        let mut res_ok = true;
        let mut header = ImageHeader::default();
        if src_type == ImageSrcType::Variable || src_type == ImageSrcType::File {
            /*lv_image_decoder_get_info()*/
            match image_header_of(&dsc.bg_image_src) {
                Some(h) => header = h,
                None => res_ok = false,
            }
        } else if src_type == ImageSrcType::Unknown {
            res_ok = false;
        }

        if res_ok {
            if src_type == ImageSrcType::Variable || src_type == ImageSrcType::File {
                let t = if dsc.bg_image_tiled {
                    s.ctx().add_task(layer, coords, DrawTaskType::Image)
                } else {
                    let mut a = Area::new(0, 0, i32::from(header.w) - 1, i32::from(header.h) - 1);
                    Area::align(coords, &mut a, Align::Center, 0, 0);
                    s.ctx().add_task(layer, &a, DrawTaskType::Image)
                };

                let mut bg_image_dsc = DrawImageDsc::new();
                bg_image_dsc.base = dsc.base.clone();
                bg_image_dsc.src = dsc.bg_image_src.clone();
                bg_image_dsc.opa = dsc.bg_image_opa;
                bg_image_dsc.recolor = dsc.bg_image_recolor;
                bg_image_dsc.recolor_opa = dsc.bg_image_recolor_opa;
                bg_image_dsc.tile = dsc.bg_image_tiled;
                bg_image_dsc.colorkey = dsc.bg_image_colorkey;
                bg_image_dsc.header = header;
                bg_image_dsc.clip_radius = dsc.radius;
                bg_image_dsc.image_area = *coords;
                set_task_dsc(s.ctx(), t, DrawDsc::Image(Box::new(bg_image_dsc)));
                s.finalize(t);
            } else if let ImageSrc::Symbol(sym) = &dsc.bg_image_src {
                let mut attributes = TextAttributes::new();
                attributes.text_flags = TextFlags::NONE;
                attributes.max_width = coord::MAX;
                attributes.line_space = 0;
                attributes.letter_space = 0;

                let sz = match &dsc.bg_image_symbol_font {
                    Some(font) => text::get_size_attributes(sym, font.as_font(), &attributes),
                    /*`lv_text_get_size_attributes()` zeroes the size and returns on a NULL font*/
                    None => Point::new(0, 0),
                };

                let mut a = Area::new(0, 0, sz.x - 1, sz.y - 1);
                Area::align(coords, &mut a, Align::Center, 0, 0);
                let t = s.ctx().add_task(layer, &a, DrawTaskType::Label);

                let mut bg_label_dsc = LabelDsc::new();
                bg_label_dsc.base = dsc.base.clone();
                bg_label_dsc.color = dsc.bg_image_recolor;
                bg_label_dsc.font = dsc.bg_image_symbol_font.clone();
                bg_label_dsc.text = Rc::from(&**sym);
                set_task_dsc(s.ctx(), t, DrawDsc::Label(Box::new(bg_label_dsc)));
                if let Some(task) = s.ctx().task_mut(t) {
                    task.ty = DrawTaskType::Label;
                }
                s.finalize(t);
            }
        }
    }

    /*Border*/
    if has_border {
        let t = s.ctx().add_task(layer, coords, DrawTaskType::Border);
        let border_dsc = BorderDsc {
            base: dsc.base.clone(),
            radius: dsc.radius,
            color: dsc.border_color,
            opa: dsc.border_opa,
            width: dsc.border_width,
            side: dsc.border_side,
        };
        set_task_dsc(s.ctx(), t, DrawDsc::Border(Box::new(border_dsc)));
        s.finalize(t);
    }

    /*Outline*/
    if has_outline {
        let mut outline_coords = *coords;
        outline_coords.increase(
            dsc.outline_width + dsc.outline_pad,
            dsc.outline_width + dsc.outline_pad,
        );
        let t = s.ctx().add_task(layer, &outline_coords, DrawTaskType::Border);
        let ctx = s.ctx();
        real_area_increase(ctx, t, dsc.outline_width, dsc.outline_width);
        real_area_increase(ctx, t, dsc.outline_pad, dsc.outline_pad);
        let outline_dsc = BorderDsc {
            base: dsc.base.clone(),
            radius: if dsc.radius == RADIUS_CIRCLE {
                RADIUS_CIRCLE
            } else {
                dsc.radius + dsc.outline_width + dsc.outline_pad
            },
            color: dsc.outline_color,
            opa: dsc.outline_opa,
            width: dsc.outline_width,
            side: BorderSide::FULL,
        };
        set_task_dsc(ctx, t, DrawDsc::Border(Box::new(outline_dsc)));
        s.finalize(t);
    }
}

impl DrawCtx {
    /// `lv_draw_rect()`: split into shadow / fill / border / outline tasks.
    pub fn draw_rect(&mut self, layer: LayerId, dsc: &RectDsc, coords: &Area) {
        draw_rect_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_fill()`
    pub fn draw_fill(&mut self, layer: LayerId, dsc: &FillDsc, coords: &Area) {
        draw_fill_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_border()`
    pub fn draw_border(&mut self, layer: LayerId, dsc: &BorderDsc, coords: &Area) {
        draw_border_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_box_shadow()`
    pub fn draw_box_shadow(&mut self, layer: LayerId, dsc: &BoxShadowDsc, coords: &Area) {
        draw_box_shadow_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_task_get_fill_dsc()`
    pub fn task_get_fill_dsc(&mut self, task: DrawTaskRef) -> Option<&mut FillDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::Fill {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::Fill(d) => Some(d),
            _ => None,
        }
    }

    /// `lv_draw_task_get_border_dsc()`
    pub fn task_get_border_dsc(&mut self, task: DrawTaskRef) -> Option<&mut BorderDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::Border {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::Border(d) => Some(d),
            _ => None,
        }
    }

    /// `lv_draw_task_get_box_shadow_dsc()`
    pub fn task_get_box_shadow_dsc(&mut self, task: DrawTaskRef) -> Option<&mut BoxShadowDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::BoxShadow {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::BoxShadow(d) => Some(d),
            _ => None,
        }
    }
}

impl Ui {
    /// `lv_draw_rect()` with the full `lv_draw_finalize_task_creation()`
    /// (`LV_EVENT_DRAW_TASK_ADDED`, unit evaluation and dispatch).
    pub fn draw_rect(&mut self, layer: LayerId, dsc: &RectDsc, coords: &Area) {
        draw_rect_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_fill()` with the full task finalization.
    pub fn draw_fill(&mut self, layer: LayerId, dsc: &FillDsc, coords: &Area) {
        draw_fill_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_border()` with the full task finalization.
    pub fn draw_border(&mut self, layer: LayerId, dsc: &BorderDsc, coords: &Area) {
        draw_border_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_box_shadow()` with the full task finalization.
    pub fn draw_box_shadow(&mut self, layer: LayerId, dsc: &BoxShadowDsc, coords: &Area) {
        draw_box_shadow_impl(self, layer, dsc, coords);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::grad::GradStop;
    use crate::draw::{DRAW_UNIT_IDLE, DrawTask, DrawTaskState, DrawUnit};
    use crate::font::{Font, GlyphDsc, GlyphFormat};
    use crate::misc::color::ColorFormat;
    use crate::misc::style::SharedRef;
    use alloc::vec::Vec;

    /// 6 px wide letters, 12 px lines.
    struct TestFont;

    impl Font for TestFont {
        fn line_height(&self) -> i32 {
            12
        }
        fn base_line(&self) -> i32 {
            2
        }
        fn underline_position(&self) -> i8 {
            -1
        }
        fn underline_thickness(&self) -> i8 {
            1
        }
        fn glyph_dsc(&self, out: &mut GlyphDsc, _letter: u32, _letter_next: u32) -> bool {
            *out = GlyphDsc {
                adv_w: 6,
                box_w: 5,
                box_h: 7,
                format: GlyphFormat::Image,
                ..GlyphDsc::default()
            };
            true
        }
        fn glyph_bitmap_a8(&self, _g: &GlyphDsc, _out: &mut [u8], _stride: usize) -> bool {
            false
        }
    }

    fn ctx_with_layer() -> (DrawCtx, LayerId) {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 199, 199));
        (ctx, layer)
    }

    fn tasks(ctx: &DrawCtx, layer: LayerId) -> Vec<DrawTask> {
        ctx.layer(layer).unwrap().tasks.iter().cloned().collect()
    }

    fn types(ctx: &DrawCtx, layer: LayerId) -> Vec<DrawTaskType> {
        ctx.layer(layer).unwrap().tasks.iter().map(|t| t.ty).collect()
    }

    const COORDS: Area = Area::new(10, 20, 109, 69);

    #[test]
    fn rect_dsc_init_defaults() {
        let d = RectDsc::new();
        assert_eq!(d.bg_color, Color::white());
        assert_eq!(d.bg_grad.stops[0].color, Color::white());
        assert_eq!(d.bg_grad.stops[1].color, Color::black());
        assert_eq!(d.bg_grad.stops[1].frac, 0xFF);
        assert_eq!(d.bg_grad.stops[0].frac, 0);
        /*memzero: the stop opacities stay 0*/
        assert_eq!(d.bg_grad.stops[0].opa, 0);
        assert_eq!(d.bg_grad.stops_count, 2);
        assert_eq!(d.bg_grad.dir, GradDir::None);
        assert_eq!(d.border_color, Color::black());
        assert_eq!(d.shadow_color, Color::black());
        assert_eq!(d.bg_opa, OPA_COVER);
        assert_eq!(d.bg_image_opa, OPA_COVER);
        assert_eq!(d.outline_opa, OPA_COVER);
        assert_eq!(d.border_opa, OPA_COVER);
        assert_eq!(d.shadow_opa, OPA_COVER);
        assert_eq!(d.border_side, BorderSide::FULL);
        assert_eq!(d.radius, 0);
        assert_eq!(d.border_width, 0);
        assert_eq!(d.outline_width, 0);
        assert_eq!(d.shadow_width, 0);
        assert!(d.bg_image_src.is_none());
        assert!(!d.border_post);
        assert_eq!(d.base, DrawDscBase::new());
        assert_eq!(RectDsc::default(), d);
    }

    #[test]
    fn small_dsc_init_defaults() {
        let f = FillDsc::new();
        assert_eq!(f.opa, OPA_COVER);
        assert_eq!(f.radius, 0);
        assert_eq!(f.grad, GradDsc::new());
        let b = BorderDsc::new();
        assert_eq!(b.opa, OPA_COVER);
        assert_eq!(b.side, BorderSide::FULL);
        assert_eq!(b.width, 0);
        let s = BoxShadowDsc::new();
        assert_eq!(s.opa, OPA_COVER);
        assert!(!s.bg_cover);
        assert_eq!(s.width, 0);
    }

    #[test]
    fn default_rect_is_a_single_fill() {
        let (mut ctx, layer) = ctx_with_layer();
        let mut dsc = RectDsc::new();
        dsc.bg_color = Color::make(1, 2, 3);
        dsc.radius = 4;
        ctx.draw_rect(layer, &dsc, &COORDS);
        assert_eq!(types(&ctx, layer), [DrawTaskType::Fill]);
        let t = DrawTaskRef { layer, index: 0 };
        assert_eq!(ctx.task_get_area(t), COORDS);
        let f = ctx.task_get_fill_dsc(t).unwrap();
        assert_eq!(f.color, Color::make(1, 2, 3));
        assert_eq!(f.radius, 4);
        assert_eq!(f.opa, OPA_COVER);
        assert_eq!(f.grad, dsc.bg_grad);
        /*finalize binds the descriptor to its layer*/
        assert_eq!(f.base.layer, Some(layer));
        assert!(ctx.task_get_border_dsc(t).is_none());
        assert!(ctx.task_get_box_shadow_dsc(t).is_none());
        assert!(ctx.dispatch_req);
    }

    #[test]
    fn full_rect_splits_into_shadow_fill_border_outline() {
        let (mut ctx, layer) = ctx_with_layer();
        let mut dsc = RectDsc::new();
        dsc.radius = 5;
        dsc.shadow_width = 10;
        dsc.shadow_spread = 2;
        dsc.shadow_offset_x = 3;
        dsc.shadow_offset_y = 4;
        dsc.shadow_color = Color::make(9, 9, 9);
        dsc.shadow_opa = 100;
        dsc.border_width = 2;
        dsc.border_color = Color::make(0, 0, 255);
        dsc.outline_width = 3;
        dsc.outline_pad = 1;
        dsc.outline_color = Color::make(255, 0, 0);
        dsc.outline_opa = 200;
        dsc.base.id1 = 7;
        ctx.draw_rect(layer, &dsc, &COORDS);

        assert_eq!(
            types(&ctx, layer),
            [
                DrawTaskType::BoxShadow,
                DrawTaskType::Fill,
                DrawTaskType::Border,
                DrawTaskType::Border
            ]
        );
        let ts = tasks(&ctx, layer);

        /*Shadow: the area is the rect, the real area grows by spread + width and moves*/
        assert_eq!(ts[0].area, COORDS);
        assert_eq!(ts[0].real_area, Area::new(1, 12, 124, 85));
        let s = ctx
            .task_get_box_shadow_dsc(DrawTaskRef { layer, index: 0 })
            .unwrap();
        assert_eq!(s.radius, 5);
        assert_eq!(s.width, 10);
        assert_eq!(s.spread, 2);
        assert_eq!((s.ofs_x, s.ofs_y), (3, 4));
        assert_eq!(s.opa, 100);
        assert_eq!(s.color, Color::make(9, 9, 9));
        assert!(s.bg_cover);
        assert_eq!(s.base.id1, 7);

        /*Background: 1 px smaller on every bordered side (radius != 0, border > 1, opaque)*/
        assert_eq!(ts[1].area, Area::new(11, 21, 108, 68));
        assert_eq!(ts[1].real_area, Area::new(11, 21, 108, 68));

        /*Border*/
        assert_eq!(ts[2].area, COORDS);
        let b = ctx.task_get_border_dsc(DrawTaskRef { layer, index: 2 }).unwrap();
        assert_eq!(b.width, 2);
        assert_eq!(b.radius, 5);
        assert_eq!(b.side, BorderSide::FULL);
        assert_eq!(b.color, Color::make(0, 0, 255));

        /*Outline: area grows by width + pad, the real area once more (like upstream)*/
        assert_eq!(ts[3].area, Area::new(6, 16, 113, 73));
        assert_eq!(ts[3].real_area, Area::new(2, 12, 117, 77));
        let o = ctx.task_get_border_dsc(DrawTaskRef { layer, index: 3 }).unwrap();
        assert_eq!(o.radius, 5 + 3 + 1);
        assert_eq!(o.width, 3);
        assert_eq!(o.opa, 200);
        assert_eq!(o.side, BorderSide::FULL);
        assert_eq!(o.color, Color::make(255, 0, 0));
    }

    #[test]
    fn bg_shrink_follows_the_border_sides() {
        let (mut ctx, layer) = ctx_with_layer();
        let mut dsc = RectDsc::new();
        dsc.radius = 3;
        dsc.border_width = 2;
        dsc.border_side = BorderSide::LEFT | BorderSide::BOTTOM;
        ctx.draw_rect(layer, &dsc, &COORDS);
        assert_eq!(types(&ctx, layer), [DrawTaskType::Fill, DrawTaskType::Border]);
        assert_eq!(tasks(&ctx, layer)[0].area, Area::new(11, 20, 109, 68));

        /*No shrink without radius, with a 1 px border or a translucent border*/
        for (radius, width, opa) in [(0, 2, OPA_COVER), (3, 1, OPA_COVER), (3, 2, OPA_MAX - 1)] {
            let (mut ctx, layer) = ctx_with_layer();
            let mut dsc = RectDsc::new();
            dsc.radius = radius;
            dsc.border_width = width;
            dsc.border_opa = opa;
            ctx.draw_rect(layer, &dsc, &COORDS);
            assert_eq!(tasks(&ctx, layer)[0].area, COORDS);
        }
    }

    #[test]
    fn invisible_parts_are_skipped() {
        let check = |f: &dyn Fn(&mut RectDsc), expected: &[DrawTaskType]| {
            let (mut ctx, layer) = ctx_with_layer();
            let mut dsc = RectDsc::new();
            dsc.shadow_width = 5;
            dsc.border_width = 1;
            dsc.outline_width = 1;
            f(&mut dsc);
            ctx.draw_rect(layer, &dsc, &COORDS);
            assert_eq!(types(&ctx, layer), expected);
        };
        use DrawTaskType::{Border, BoxShadow, Fill};
        check(&|_| {}, &[BoxShadow, Fill, Border, Border]);
        check(&|d| d.shadow_opa = OPA_MIN, &[Fill, Border, Border]);
        check(&|d| d.shadow_width = 0, &[Fill, Border, Border]);
        /*A 1 px shadow without spread and offset is invisible*/
        check(&|d| d.shadow_width = 1, &[Fill, Border, Border]);
        check(
            &|d| {
                d.shadow_width = 1;
                d.shadow_spread = 1;
            },
            &[BoxShadow, Fill, Border, Border],
        );
        check(
            &|d| {
                d.shadow_width = 1;
                d.shadow_offset_y = -1;
            },
            &[BoxShadow, Fill, Border, Border],
        );
        check(&|d| d.bg_opa = OPA_MIN, &[BoxShadow, Border, Border]);
        check(&|d| d.border_opa = OPA_MIN, &[BoxShadow, Fill, Border]);
        check(&|d| d.border_post = true, &[BoxShadow, Fill, Border]);
        check(&|d| d.border_side = BorderSide::NONE, &[BoxShadow, Fill, Border]);
        check(&|d| d.border_width = 0, &[BoxShadow, Fill, Border]);
        check(&|d| d.outline_opa = 0, &[BoxShadow, Fill, Border]);
        check(&|d| d.outline_width = 0, &[BoxShadow, Fill, Border]);
        /*No image source: no image task even when opaque*/
        check(
            &|d| d.bg_image_opa = OPA_COVER,
            &[BoxShadow, Fill, Border, Border],
        );
        /*Symbol source but transparent*/
        check(
            &|d| {
                d.bg_image_src = ImageSrc::Symbol(SharedRef::Static("A"));
                d.bg_image_opa = OPA_MIN;
            },
            &[BoxShadow, Fill, Border, Border],
        );
    }

    #[test]
    fn shadow_bg_cover_depends_on_bg_and_gradient_opacity() {
        let shadow_cover = |f: &dyn Fn(&mut RectDsc)| {
            let (mut ctx, layer) = ctx_with_layer();
            let mut dsc = RectDsc::new();
            dsc.shadow_width = 4;
            f(&mut dsc);
            ctx.draw_rect(layer, &dsc, &COORDS);
            ctx.task_get_box_shadow_dsc(DrawTaskRef { layer, index: 0 })
                .unwrap()
                .bg_cover
        };
        /*No gradient: the (zero) stop opacities are not looked at*/
        assert!(shadow_cover(&|_| {}));
        assert!(!shadow_cover(&|d| d.bg_opa = OPA_COVER - 1));
        /*Fully transparent bg: still no cover (and no fill task)*/
        assert!(!shadow_cover(&|d| d.bg_opa = 0));
        assert!(!shadow_cover(&|d| d.bg_grad.dir = GradDir::Ver));
        assert!(shadow_cover(&|d| {
            d.bg_grad.dir = GradDir::Hor;
            d.bg_grad.stops[0].opa = OPA_COVER;
            d.bg_grad.stops[1].opa = OPA_COVER;
        }));
        /*Stops past stops_count are ignored*/
        assert!(shadow_cover(&|d| {
            d.bg_grad.dir = GradDir::Hor;
            d.bg_grad.stops_count = 1;
            d.bg_grad.stops[0] = GradStop {
                color: Color::BLACK,
                opa: OPA_COVER,
                frac: 0,
            };
        }));
    }

    #[test]
    fn outline_of_a_circle_keeps_the_circle_radius() {
        let (mut ctx, layer) = ctx_with_layer();
        let mut dsc = RectDsc::new();
        dsc.radius = RADIUS_CIRCLE;
        dsc.bg_opa = 0;
        dsc.outline_width = 2;
        dsc.outline_pad = 3;
        ctx.draw_rect(layer, &dsc, &COORDS);
        assert_eq!(types(&ctx, layer), [DrawTaskType::Border]);
        let o = ctx.task_get_border_dsc(DrawTaskRef { layer, index: 0 }).unwrap();
        assert_eq!(o.radius, RADIUS_CIRCLE);
    }

    #[test]
    fn symbol_bg_image_becomes_a_centered_label_task() {
        let (mut ctx, layer) = ctx_with_layer();
        let font = FontRef::Shared(Rc::new(TestFont));
        let mut dsc = RectDsc::new();
        dsc.bg_opa = 0;
        dsc.bg_image_src = ImageSrc::Symbol(SharedRef::Static("AB"));
        dsc.bg_image_symbol_font = Some(font.clone());
        dsc.bg_image_recolor = Color::make(5, 6, 7);
        ctx.draw_rect(layer, &dsc, &COORDS);
        assert_eq!(types(&ctx, layer), [DrawTaskType::Label]);

        /*"AB" is 12 x 12 px, centered in the 100 x 50 rect*/
        let mut expected = Area::new(0, 0, 11, 11);
        Area::align(&COORDS, &mut expected, Align::Center, 0, 0);
        assert_eq!(expected, Area::new(54, 39, 65, 50));
        assert_eq!(tasks(&ctx, layer)[0].area, expected);

        let l = ctx.task_get_label_dsc(DrawTaskRef { layer, index: 0 }).unwrap();
        assert_eq!(&*l.text, "AB");
        assert_eq!(l.font, Some(font));
        assert_eq!(l.color, Color::make(5, 6, 7));
        assert_eq!(l.opa, OPA_COVER);
        assert_eq!(l.base.layer, Some(layer));
    }

    #[test]
    fn single_part_helpers_copy_the_descriptor() {
        let (mut ctx, layer) = ctx_with_layer();
        let mut fill = FillDsc::new();
        fill.color = Color::make(1, 1, 1);
        ctx.draw_fill(layer, &fill, &COORDS);
        let mut border = BorderDsc::new();
        border.width = 3;
        ctx.draw_border(layer, &border, &COORDS);
        let mut shadow = BoxShadowDsc::new();
        shadow.width = 8;
        ctx.draw_box_shadow(layer, &shadow, &COORDS);
        assert_eq!(
            types(&ctx, layer),
            [DrawTaskType::Fill, DrawTaskType::Border, DrawTaskType::BoxShadow]
        );
        /*Unlike lv_draw_rect, lv_draw_box_shadow does not grow the real area*/
        assert_eq!(tasks(&ctx, layer)[2].real_area, COORDS);
        let f = ctx.task_get_fill_dsc(DrawTaskRef { layer, index: 0 }).unwrap();
        assert_eq!(f.color, Color::make(1, 1, 1));
        assert_eq!(f.base.layer, Some(layer));
        assert_eq!(
            ctx.task_get_border_dsc(DrawTaskRef { layer, index: 1 })
                .unwrap()
                .width,
            3
        );
        assert_eq!(
            ctx.task_get_box_shadow_dsc(DrawTaskRef { layer, index: 2 })
                .unwrap()
                .width,
            8
        );

        /*Invisible parts are not added*/
        fill.opa = OPA_MIN;
        border.opa = OPA_MIN;
        shadow.opa = OPA_MIN;
        ctx.draw_fill(layer, &fill, &COORDS);
        ctx.draw_border(layer, &border, &COORDS);
        ctx.draw_box_shadow(layer, &shadow, &COORDS);
        assert_eq!(ctx.layer(layer).unwrap().tasks.len(), 3);
    }

    /// Takes every task and never renders it.
    struct EvalOnlyUnit;

    impl DrawUnit for EvalOnlyUnit {
        fn name(&self) -> &'static str {
            "EVAL"
        }
        fn evaluate(&mut self, task: &mut DrawTask) -> i32 {
            if task.preference_score > 50 {
                task.preference_score = 50;
                task.preferred_draw_unit_id = 1;
            }
            1
        }
        fn dispatch(&mut self, _ctx: &mut DrawCtx, _layer: LayerId) -> i32 {
            DRAW_UNIT_IDLE
        }
    }

    #[test]
    fn ui_draw_rect_evaluates_every_task() {
        use crate::core::UiConfig;
        let mut ui = Ui::new(UiConfig::default());
        ui.draw_units.push(Box::new(EvalOnlyUnit));
        ui.draw.unit_cnt = 1;
        let layer = ui
            .draw
            .layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 199, 199));
        let mut dsc = RectDsc::new();
        dsc.border_width = 1;
        dsc.outline_width = 1;
        dsc.shadow_width = 3;
        ui.draw_rect(layer, &dsc, &COORDS);
        let ts = tasks(&ui.draw, layer);
        assert_eq!(ts.len(), 4);
        for t in &ts {
            assert_eq!(t.preferred_draw_unit_id, 1);
            assert_eq!(t.preference_score, 50);
            assert_eq!(t.state, DrawTaskState::Waiting);
            assert_eq!(t.dsc.base().unwrap().layer, Some(layer));
        }
    }
}

//! Image widget ported from LVGL v9.6.0 `src/widgets/image/lv_image.c`.
//!
//! Sources and bitmap masks retain their `ImageSrc` ownership through queued
//! draw tasks. In-memory descriptors/buffers and font symbols use the existing
//! core decoder seam. File paths currently fail metadata lookup and clear the
//! image, exactly as a C build without a matching filesystem/decoder would.
//! Bitmap masks are forwarded to image descriptors; the pinned software
//! renderer applies them to layer composites only, not direct image tasks.
//! Observer bindings and the optional object-property registry are not exposed.

use alloc::boxed::Box;
use alloc::rc::Rc;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::draw::image::{
    image_buf_get_transformed_area, image_decoder_get_info, image_src_get_type,
};
use ferrovgl_core::draw::{DrawImageDsc, LabelDsc};
use ferrovgl_core::misc::event::{CoverRes, EventParam};
use ferrovgl_core::misc::style::{BlendMode, ImageSrc, ImageSrcType, SCALE_NONE};
use ferrovgl_core::misc::text::{self, TextAttributes, TextFlags};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Area, ColorFormat, Event, EventCode, LvResult, Obj, ObjClass, Part, Point, Ui};

/// `lv_image_align_t`, preserving upstream's numeric values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ImageAlign {
    /// Default alignment (same as top left).
    #[default]
    Default = 0,
    /// Top left.
    TopLeft = 1,
    /// Top center.
    TopMid = 2,
    /// Top right.
    TopRight = 3,
    /// Bottom left.
    BottomLeft = 4,
    /// Bottom center.
    BottomMid = 5,
    /// Bottom right.
    BottomRight = 6,
    /// Left center.
    LeftMid = 7,
    /// Right center.
    RightMid = 8,
    /// Center horizontally and vertically.
    Center = 9,
    /// Upstream's `_LV_IMAGE_ALIGN_AUTO_TRANSFORM` sentinel.
    AutoTransform = 10,
    /// Scale each axis independently to fill the object's size.
    Stretch = 11,
    /// Repeat the image, applying the configured offsets.
    Tile = 12,
    /// Scale uniformly to fit inside the object's size.
    Contain = 13,
    /// Fit inside the object without enlarging the image.
    ContainDownscale = 14,
    /// Scale uniformly to cover the object's size.
    Cover = 15,
}

impl ImageAlign {
    fn automatic(self) -> bool {
        (self as u8) > Self::AutoTransform as u8
    }
    fn resizes(self) -> bool {
        matches!(
            self,
            Self::Stretch | Self::Contain | Self::ContainDownscale | Self::Cover
        )
    }
    fn area_align(self) -> Align {
        match self {
            Self::TopLeft => Align::TopLeft,
            Self::TopMid => Align::TopMid,
            Self::TopRight => Align::TopRight,
            Self::BottomLeft => Align::BottomLeft,
            Self::BottomMid => Align::BottomMid,
            Self::BottomRight => Align::BottomRight,
            Self::LeftMid => Align::LeftMid,
            Self::RightMid => Align::RightMid,
            Self::Center => Align::Center,
            _ => Align::Default,
        }
    }
}

#[derive(Clone)]
struct ImageState {
    src: ImageSrc,
    bitmap_mask_src: ImageSrc,
    offset: Point,
    w: i32,
    h: i32,
    rotation: i32,
    scale_x: u32,
    scale_y: u32,
    pivot: Point,
    src_type: ImageSrcType,
    cf: ColorFormat,
    antialias: bool,
    align: ImageAlign,
    blend_mode: BlendMode,
}

impl ImageState {
    fn new() -> Self {
        Self {
            src: ImageSrc::None,
            bitmap_mask_src: ImageSrc::None,
            offset: Point::new(0, 0),
            w: 0,
            h: 0,
            rotation: 0,
            scale_x: SCALE_NONE as u32,
            scale_y: SCALE_NONE as u32,
            pivot: Point::new(coord::pct(50), coord::pct(50)),
            src_type: ImageSrcType::Unknown,
            cf: ColorFormat::Unknown,
            antialias: true, // LV_COLOR_DEPTH == 32, as in the core draw descriptors.
            align: ImageAlign::Center,
            blend_mode: BlendMode::Normal,
        }
    }
    fn pivot_px(&self) -> Point {
        Point::new(
            coord::pct_to_px(self.pivot.x, (self.w - 1).max(0)),
            coord::pct_to_px(self.pivot.y, (self.h - 1).max(0)),
        )
    }
    fn transformed(&self) -> bool {
        self.rotation != 0 || self.scale_x != SCALE_NONE as u32 || self.scale_y != SCALE_NONE as u32
    }
    fn transformed_area(&self, w: i32, h: i32) -> Area {
        let mut area = Area::default();
        image_buf_get_transformed_area(
            &mut area,
            w,
            h,
            self.rotation,
            self.scale_x as u16,
            self.scale_y as u16,
            self.pivot_px(),
        );
        area
    }
}

ferrovgl_core::impl_widget_state!(ImageState);

/// `lv_image_class`.
pub static IMAGE_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_image",
    new_state: || Box::new(ImageState::new()),
    constructor: Some(constructor),
    destructor: None, // ImageSrc/Rc drops the owned source and mask with widget state.
    event_cb: Some(image_event),
    width_def: coord::SIZE_CONTENT,
    height_def: coord::SIZE_CONTENT,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::Inherit,
    theme_inheritable: false,
};

/// Copyable image handle. Its source and widget state belong to the [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Image(Obj);

impl Image {
    /// `lv_image_create()`. `None` creates an image screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&IMAGE_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }
    /// The base object used for positioning, styles and events.
    pub const fn obj(self) -> Obj {
        self.0
    }
    /// Check that an existing object contains image widget state.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        state(ui, obj).map(|_| Self(obj))
    }

    /// `lv_image_set_src()`: retain the source; `ImageSrc::None` clears it.
    /// Missing/unsupported metadata also clears the source and content size.
    pub fn set_src(self, ui: &mut Ui, src: ImageSrc) {
        if state(ui, self.0).is_none() {
            return;
        }
        ui.obj_invalidate(self.0);
        if state(ui, self.0).is_none() {
            return;
        }
        let src_type = image_src_get_type(&src);
        if src_type == ImageSrcType::Unknown {
            reset_source(ui, self.0);
            return;
        }
        let Some(mut header) = image_decoder_get_info(&ui.draw, &src) else {
            reset_source(ui, self.0);
            return;
        };
        if src_type == ImageSrcType::Symbol {
            let Some(font) = ui.obj_get_style_text_font(self.0, Part::MAIN) else {
                reset_source(ui, self.0);
                return;
            };
            let attrs = text_attributes(ui, self.0);
            let size = text::get_size_attributes(source_text(&src), font.as_font(), &attrs);
            header.w = size.x as u16;
            header.h = size.y as u16;
        }
        if let Some(image) = state_mut(ui, self.0) {
            image.src = src;
            image.src_type = src_type;
            image.w = i32::from(header.w);
            image.h = i32::from(header.h);
            image.cf = header.cf;
        }
        ui.obj_refresh_self_size(self.0);
        update_align(ui, self.0);
        if state(ui, self.0).is_some_and(ImageState::transformed) {
            ui.obj_refresh_ext_draw_size(self.0);
        }
        if state(ui, self.0).is_some() {
            ui.obj_invalidate(self.0);
        }
    }
    /// `lv_image_get_src()`; the returned clone keeps owned sources alive.
    pub fn src(self, ui: &Ui) -> ImageSrc {
        state(ui, self.0).map_or(ImageSrc::None, |s| s.src.clone())
    }
    /// `lv_image_set_offset_x()`.
    pub fn set_offset_x(self, ui: &mut Ui, x: i32) {
        if let Some(s) = state_mut(ui, self.0) {
            s.offset.x = x;
            ui.obj_invalidate(self.0);
        }
    }
    /// `lv_image_set_offset_y()`.
    pub fn set_offset_y(self, ui: &mut Ui, y: i32) {
        if let Some(s) = state_mut(ui, self.0) {
            s.offset.y = y;
            ui.obj_invalidate(self.0);
        }
    }
    /// `lv_image_get_offset_x()`.
    pub fn offset_x(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |s| s.offset.x)
    }
    /// `lv_image_get_offset_y()`.
    pub fn offset_y(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |s| s.offset.y)
    }
    /// `lv_image_set_rotation()`, in clockwise tenths of a degree.
    pub fn set_rotation(self, ui: &mut Ui, angle: i32) {
        let Some(s) = state(ui, self.0) else {
            return;
        };
        let angle = if s.align.automatic() {
            0
        } else {
            angle.rem_euclid(3600)
        };
        if angle != s.rotation {
            change_transform(ui, self.0, Transform::Rotation(angle));
        }
    }
    /// `lv_image_get_rotation()`.
    pub fn rotation(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |s| s.rotation)
    }
    /// `lv_image_set_pivot()`. Coordinates can use `coord::pct()`.
    pub fn set_pivot(self, ui: &mut Ui, x: i32, y: i32) {
        let Some(s) = state(ui, self.0) else {
            return;
        };
        let pivot = if s.align.automatic() {
            Point::new(0, 0)
        } else {
            Point::new(x, y)
        };
        if pivot != s.pivot {
            change_transform(ui, self.0, Transform::Pivot(pivot));
        }
    }
    /// `lv_image_set_pivot_x()`.
    pub fn set_pivot_x(self, ui: &mut Ui, x: i32) {
        if let Some(s) = state(ui, self.0) {
            self.set_pivot(ui, x, s.pivot.y);
        }
    }
    /// `lv_image_set_pivot_y()`.
    pub fn set_pivot_y(self, ui: &mut Ui, y: i32) {
        if let Some(s) = state(ui, self.0) {
            self.set_pivot(ui, s.pivot.x, y);
        }
    }
    /// `lv_image_get_pivot()`: resolve percentages against source width/height - 1.
    pub fn pivot(self, ui: &Ui) -> Point {
        state(ui, self.0).map_or(Point::new(0, 0), ImageState::pivot_px)
    }
    /// `lv_image_set_scale()`: 256 is unscaled; zero is clamped to one.
    pub fn set_scale(self, ui: &mut Ui, scale: u32) {
        let Some(s) = state(ui, self.0) else {
            return;
        };
        if s.align.automatic() || (s.scale_x == scale && s.scale_y == scale) {
            return;
        }
        change_transform(ui, self.0, Transform::Scale(scale.max(1), scale.max(1)));
    }
    /// `lv_image_set_scale_x()`.
    pub fn set_scale_x(self, ui: &mut Ui, scale: u32) {
        let Some(s) = state(ui, self.0) else {
            return;
        };
        if !s.align.automatic() && s.scale_x != scale {
            change_transform(ui, self.0, Transform::Scale(scale.max(1), s.scale_y));
        }
    }
    /// `lv_image_set_scale_y()`.
    pub fn set_scale_y(self, ui: &mut Ui, scale: u32) {
        let Some(s) = state(ui, self.0) else {
            return;
        };
        if !s.align.automatic() && s.scale_y != scale {
            change_transform(ui, self.0, Transform::Scale(s.scale_x, scale.max(1)));
        }
    }
    /// `lv_image_get_scale()`, which returns the X scale.
    pub fn scale(self, ui: &Ui) -> i32 {
        self.scale_x(ui)
    }
    /// `lv_image_get_scale_x()`.
    pub fn scale_x(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |s| s.scale_x as i32)
    }
    /// `lv_image_get_scale_y()`.
    pub fn scale_y(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |s| s.scale_y as i32)
    }
    /// `lv_image_set_blend_mode()`.
    pub fn set_blend_mode(self, ui: &mut Ui, mode: BlendMode) {
        if let Some(s) = state_mut(ui, self.0)
            && s.blend_mode != mode
        {
            s.blend_mode = mode;
            ui.obj_invalidate(self.0);
        }
    }
    /// `lv_image_get_blend_mode()`.
    pub fn blend_mode(self, ui: &Ui) -> BlendMode {
        state(ui, self.0).map_or(BlendMode::Normal, |s| s.blend_mode)
    }
    /// `lv_image_set_antialias()`.
    pub fn set_antialias(self, ui: &mut Ui, enabled: bool) {
        if let Some(s) = state_mut(ui, self.0)
            && s.antialias != enabled
        {
            s.antialias = enabled;
            ui.obj_invalidate(self.0);
        }
    }
    /// `lv_image_get_antialias()`.
    pub fn antialias(self, ui: &Ui) -> bool {
        state(ui, self.0).is_some_and(|s| s.antialias)
    }
    /// `lv_image_set_inner_align()`. Automatic modes control rotation, pivot and scale.
    pub fn set_inner_align(self, ui: &mut Ui, align: ImageAlign) {
        let Some(s) = state(ui, self.0) else {
            return;
        };
        if s.align == align {
            return;
        }
        if s.align.resizes() {
            // C calls the public setter before changing align, so the old
            // automatic mode still prevents this attempted scale reset.
            self.set_scale(ui, SCALE_NONE as u32);
        }
        if let Some(s) = state_mut(ui, self.0) {
            s.align = align;
        }
        update_align(ui, self.0);
        if state(ui, self.0).is_some() {
            ui.obj_invalidate(self.0);
        }
    }
    /// `lv_image_get_inner_align()`.
    pub fn inner_align(self, ui: &Ui) -> ImageAlign {
        state(ui, self.0).map_or(ImageAlign::Default, |s| s.align)
    }
    /// `lv_image_set_bitmap_map_src()` (the upstream public spelling).
    pub fn set_bitmap_map_src(self, ui: &mut Ui, src: ImageSrc) {
        if let Some(s) = state_mut(ui, self.0) {
            s.bitmap_mask_src = src;
            ui.obj_invalidate(self.0);
        }
    }
    /// `lv_image_get_bitmap_map_src()`.
    pub fn bitmap_map_src(self, ui: &Ui) -> ImageSrc {
        state(ui, self.0).map_or(ImageSrc::None, |s| s.bitmap_mask_src.clone())
    }
    /// `lv_image_get_src_width()`: refresh automatic alignment and return source width.
    pub fn src_width(self, ui: &mut Ui) -> i32 {
        update_align(ui, self.0);
        state(ui, self.0).map_or(0, |s| s.w)
    }
    /// `lv_image_get_src_height()`.
    pub fn src_height(self, ui: &mut Ui) -> i32 {
        update_align(ui, self.0);
        state(ui, self.0).map_or(0, |s| s.h)
    }
    /// `lv_image_get_transformed_width()`.
    pub fn transformed_width(self, ui: &mut Ui) -> i32 {
        update_align(ui, self.0);
        state(ui, self.0).map_or(0, |s| s.transformed_area(s.w, s.h).width())
    }
    /// `lv_image_get_transformed_height()`.
    pub fn transformed_height(self, ui: &mut Ui) -> i32 {
        update_align(ui, self.0);
        state(ui, self.0).map_or(0, |s| s.transformed_area(s.w, s.h).height())
    }
}

impl From<Image> for Obj {
    fn from(image: Image) -> Self {
        image.0
    }
}

fn state(ui: &Ui, obj: Obj) -> Option<&ImageState> {
    ui.obj(obj)?.widget.get::<ImageState>()
}
fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut ImageState> {
    ui.obj_mut(obj)?.widget.get_mut::<ImageState>()
}

fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    let (w, h) = (ui.obj_get_width(obj), ui.obj_get_height(obj));
    if let Some(s) = state_mut(ui, obj) {
        s.w = w;
        s.h = h;
    }
    ui.obj_set_clickable(obj, false);
    if state(ui, obj).is_some() {
        ui.obj_set_adv_hittest(obj, true);
    }
}

fn reset_source(ui: &mut Ui, obj: Obj) {
    if let Some(s) = state_mut(ui, obj) {
        s.src = ImageSrc::None;
        s.src_type = ImageSrcType::Unknown;
        s.w = 0;
        s.h = 0;
        s.cf = ColorFormat::Unknown;
        ui.obj_refresh_self_size(obj);
    }
}

fn source_text(src: &ImageSrc) -> &str {
    match src {
        ImageSrc::Symbol(text) | ImageSrc::Path(text) => text.split('\0').next().unwrap_or(""),
        _ => "",
    }
}

fn text_attributes(ui: &Ui, obj: Obj) -> TextAttributes {
    TextAttributes {
        letter_space: ui.obj_get_style_text_letter_space(obj, Part::MAIN),
        line_space: ui.obj_get_style_text_line_space(obj, Part::MAIN),
        max_width: coord::MAX,
        text_flags: TextFlags::NONE,
    }
}

enum Transform {
    Rotation(i32),
    Pivot(Point),
    Scale(u32, u32),
}

fn change_transform(ui: &mut Ui, obj: Obj, change: Transform) {
    ui.obj_update_layout(obj);
    let Some(s) = state(ui, obj) else {
        return;
    };
    let (w, h) = (ui.obj_get_width(obj), ui.obj_get_height(obj));
    let cached_pivot = s.pivot_px();
    let refresh_pivot = matches!(change, Transform::Pivot(_));
    let pad = i32::from(matches!(change, Transform::Scale(..)));
    let mut area = s.transformed_area(w, h);
    let coords = ui.obj_get_coords(obj);
    area.move_by(coords.x1, coords.y1);
    area.increase(pad, pad);
    ui.obj_invalidate_area(obj, &area);
    let Some(s) = state_mut(ui, obj) else {
        return;
    };
    match change {
        Transform::Rotation(angle) => s.rotation = angle,
        Transform::Pivot(pivot) => s.pivot = pivot,
        Transform::Scale(x, y) => {
            s.scale_x = x;
            s.scale_y = y;
        }
    }
    let disp = ui.obj_get_display(obj);
    if let Some(disp) = disp {
        ui.display_enable_invalidation(Some(disp), false);
    }
    ui.obj_refresh_ext_draw_size(obj);
    if let Some(disp) = disp
        && ui.display(disp).is_some()
    {
        ui.display_enable_invalidation(Some(disp), true);
    }
    let Some(s) = state(ui, obj) else {
        return;
    };
    // Rotation/scale preserve the pivot captured before the refresh callback;
    // only lv_image_set_pivot resolves it again afterwards in the C source.
    let mut area = Area::default();
    image_buf_get_transformed_area(
        &mut area,
        w,
        h,
        s.rotation,
        s.scale_x as u16,
        s.scale_y as u16,
        if refresh_pivot { s.pivot_px() } else { cached_pivot },
    );
    let coords = ui.obj_get_coords(obj);
    area.move_by(coords.x1, coords.y1);
    area.increase(pad, pad);
    ui.obj_invalidate_area(obj, &area);
}

fn update_align(ui: &mut Ui, obj: Obj) {
    let Some(s) = state(ui, obj) else {
        return;
    };
    let align = s.align;
    if !align.automatic() {
        return;
    }
    Image(obj).set_rotation(ui, 0);
    Image(obj).set_pivot(ui, 0, 0);
    let Some(s) = state(ui, obj) else {
        return;
    };
    if align == ImageAlign::Tile {
        change_transform(ui, obj, Transform::Scale(SCALE_NONE as u32, SCALE_NONE as u32));
    } else if s.w != 0 && s.h != 0 {
        ui.obj_update_layout(obj);
        let Some(s) = state(ui, obj) else {
            return;
        };
        if s.w == 0 || s.h == 0 {
            return;
        }
        let x = ui.obj_get_width(obj).wrapping_mul(SCALE_NONE) / s.w;
        let y = ui.obj_get_height(obj).wrapping_mul(SCALE_NONE) / s.h;
        let (x, y) = match align {
            ImageAlign::Contain => (x.min(y), x.min(y)),
            ImageAlign::ContainDownscale => (x.min(y).min(SCALE_NONE), x.min(y).min(SCALE_NONE)),
            ImageAlign::Cover => (x.max(y), x.max(y)),
            _ => (x, y),
        };
        change_transform(ui, obj, Transform::Scale(x as u32, y as u32));
    }
}

fn image_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    if ui.obj_event_base(&IMAGE_CLASS, event) != LvResult::Ok {
        return;
    }
    let Some(obj) = event.current_target_obj() else {
        return;
    };
    let Some(s) = state(ui, obj).cloned() else {
        return;
    };
    match event.code {
        EventCode::STYLE_CHANGED => {
            if s.src_type == ImageSrcType::Symbol {
                Image(obj).set_src(ui, s.src);
            } else {
                ui.obj_refresh_ext_draw_size(obj);
            }
        }
        EventCode::REFR_EXT_DRAW_SIZE if s.transformed() => {
            let (w, h) = (ui.obj_get_width(obj), ui.obj_get_height(obj));
            let area = s.transformed_area(w, h);
            event.set_ext_draw_size((-area.x1).max(-area.y1).max(area.x2 - w).max(area.y2 - h));
        }
        EventCode::SIZE_CHANGED if s.align.resizes() => {
            update_align(ui, obj);
            if state(ui, obj).is_some_and(ImageState::transformed) {
                ui.obj_refresh_ext_draw_size(obj);
            }
        }
        EventCode::HIT_TEST => {
            if let Some(info) = event.hit_test_info() {
                let area = if s.w == ui.obj_get_width(obj) && s.h == ui.obj_get_height(obj) && s.transformed()
                {
                    let mut area = s.transformed_area(s.w, s.h);
                    let coords = ui.obj_get_coords(obj);
                    area.move_by(coords.x1, coords.y1);
                    area
                } else {
                    ui.obj_get_click_area(obj)
                };
                info.res = area.is_point_on(info.point, 0);
            }
        }
        EventCode::GET_SELF_SIZE => {
            if let Some(size) = event.self_size_info() {
                size.x = s.w;
                size.y = s.h;
            }
        }
        EventCode::COVER_CHECK => cover_check(ui, obj, &s, event),
        EventCode::DRAW_MAIN => draw_image(ui, obj, &s, event),
        _ => {}
    }
}

fn cover_check(ui: &Ui, obj: Obj, s: &ImageState, event: &mut Event<'_>) {
    let EventParam::CoverCheck(info) = &mut event.param else {
        return;
    };
    if info.res == CoverRes::Masked {
        return;
    }
    if matches!(s.src_type, ImageSrcType::Unknown | ImageSrcType::Symbol)
        || s.cf.has_alpha()
        || ui.obj_get_style_image_opa(obj, Part::MAIN) != 255
        || s.rotation != 0
        || !s.bitmap_mask_src.is_none()
    {
        info.res = CoverRes::NotCover;
        return;
    }
    let coords = ui.obj_get_coords(obj);
    let area = if s.scale_x == SCALE_NONE as u32 && s.scale_y == SCALE_NONE as u32 {
        coords
    } else {
        let mut area = s.transformed_area(coords.width(), coords.height());
        area.move_by(coords.x1, coords.y1);
        area
    };
    if !info.area.is_in(&area, 0) {
        info.res = CoverRes::NotCover;
    }
}

fn draw_image(ui: &mut Ui, obj: Obj, s: &ImageState, event: &Event<'_>) {
    if s.w == 0 || s.h == 0 || s.scale_x == 0 || s.scale_y == 0 || s.src.is_none() {
        return;
    }
    let Some(layer) = event.layer() else {
        return;
    };
    let Some(layer_data) = ui.draw.layer(layer) else {
        return;
    };
    let original_clip = layer_data.clip_area;
    let obj_coords = ui.obj_get_coords(obj);
    if matches!(s.src_type, ImageSrcType::File | ImageSrcType::Variable) {
        let mut dsc = DrawImageDsc::new();
        dsc.base.layer = Some(layer);
        ui.obj_init_draw_image_dsc(obj, Part::MAIN, &mut dsc);
        dsc.pivot = s.pivot_px();
        dsc.scale_x = s.scale_x as i32;
        dsc.scale_y = s.scale_y as i32;
        dsc.rotation = s.rotation;
        dsc.antialias = s.antialias;
        dsc.blend_mode = s.blend_mode;
        dsc.bitmap_mask_src = s.bitmap_mask_src.clone();
        dsc.src = s.src.clone();
        dsc.image_area = Area::new(
            obj_coords.x1,
            obj_coords.y1,
            obj_coords.x1 + s.w - 1,
            obj_coords.y1 + s.h - 1,
        );
        dsc.clip_radius = ui.obj_get_style_radius(obj, Part::MAIN);
        let coords = if (s.align as u8) < ImageAlign::AutoTransform as u8 {
            Area::align(
                &obj_coords,
                &mut dsc.image_area,
                s.align.area_align(),
                s.offset.x,
                s.offset.y,
            );
            dsc.image_area
        } else if matches!(
            s.align,
            ImageAlign::Contain | ImageAlign::Cover | ImageAlign::ContainDownscale
        ) {
            let scale = s.scale_x as i32;
            let x = (obj_coords.width() - s.w.wrapping_mul(scale) / SCALE_NONE) / 2;
            let y = (obj_coords.height() - s.h.wrapping_mul(scale) / SCALE_NONE) / 2;
            dsc.image_area.move_by(x, y);
            dsc.image_area.move_by(s.offset.x, s.offset.y);
            dsc.image_area
        } else if s.align == ImageAlign::Tile {
            let Some(clip) = Area::intersect(&original_clip, &obj_coords) else {
                return;
            };
            if let Some(data) = ui.draw.layer_mut(layer) {
                data.clip_area = clip;
            }
            dsc.image_area.move_by(s.offset.x, s.offset.y);
            dsc.image_area.move_by(
                ((clip.x1 - dsc.image_area.x1 - (s.w - 1)) / s.w) * s.w,
                ((clip.y1 - dsc.image_area.y1 - (s.h - 1)) / s.h) * s.h,
            );
            dsc.tile = true;
            clip
        } else {
            dsc.image_area
        };
        ui.draw_image(layer, &dsc, &coords);
        if let Some(data) = ui.draw.layer_mut(layer) {
            data.clip_area = original_clip;
        }
    } else if s.src_type == ImageSrcType::Symbol {
        let mut dsc = LabelDsc::new();
        dsc.base.layer = Some(layer);
        ui.obj_init_draw_label_dsc(obj, Part::MAIN, &mut dsc);
        dsc.text = Rc::from(source_text(&s.src));
        let same_size = s.w == obj_coords.width() && s.h == obj_coords.height();
        let alignment = (s.align as u8) > ImageAlign::TopLeft as u8 && !same_size;
        let mut coords = obj_coords;
        if (alignment || s.offset.x != 0 || s.offset.y != 0)
            && (s.align as u8) < ImageAlign::AutoTransform as u8
            && let Some(font) = &dsc.font
        {
            let attrs = TextAttributes {
                letter_space: dsc.letter_space,
                line_space: dsc.line_space,
                max_width: coord::MAX,
                text_flags: TextFlags::NONE,
            };
            let size = text::get_size_attributes(&dsc.text, font.as_font(), &attrs);
            // C deliberately uses size.x/y as inclusive x2/y2 here.
            coords = Area::new(0, 0, size.x, size.y);
            Area::align(
                &obj_coords,
                &mut coords,
                s.align.area_align(),
                s.offset.x,
                s.offset.y,
            );
        }
        ui.draw_label(layer, &dsc, &coords);
    }
}

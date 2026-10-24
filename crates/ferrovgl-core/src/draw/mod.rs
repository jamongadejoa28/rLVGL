//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw.h`, `src/draw/lv_draw_private.h`,
//! `src/draw/lv_draw.c`.
//!
//! The draw model: layers own a queue of draw tasks; draw units evaluate and
//! dispatch tasks. Rendering itself lives in `ferrovgl-draw-sw`.
//!
//! Borrowing: the draw units (`unit_head`) live in `Ui::draw_units`, next to
//! (not inside) [`DrawCtx`], so `lv_draw_dispatch_layer` can call
//! `unit.dispatch(&mut ui.draw, layer)` on disjoint fields; the `DrawCtx`
//! functions that consult the units take them as a parameter. A renderer's
//! `execute_drawing(t)` clones the [`DrawTask`] out of the layer queue before
//! rendering it (the task stays linked in the queue like upstream; the clone
//! is the price of handing `&mut DrawCtx` and `&XDsc` to the same function).

#![allow(unused_variables)] // skeleton: bodies are todo!()

pub mod arc;
pub mod blur;
pub mod draw_buf;
pub mod grad;
pub mod image;
pub mod label;
pub mod line;
pub mod mask;
pub mod rect;
pub mod triangle;
pub mod utils;

use alloc::boxed::Box;
use alloc::collections::VecDeque;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::display::DisplayId;
use crate::misc::area::Area;
use crate::misc::arena::{Arena, Key};
use crate::misc::color::{Color, Color32, ColorFormat, Opa};
use crate::misc::event::{EventCode, EventParam};
use crate::misc::style::{BlurQuality, Part};

pub use arc::ArcDsc;
pub use blur::BlurDsc;
pub use draw_buf::{DrawBuf, ImageDsc, ImageHeader};
pub use image::DrawImageDsc;
pub use label::{LabelDsc, LetterDsc};
pub use line::LineDsc;
pub use mask::{MaskBitmapDsc, MaskRectDsc};
pub use rect::{BorderDsc, BoxShadowDsc, FillDsc, RectDsc};
pub use triangle::TriangleDsc;

/// `LV_DRAW_UNIT_NONE`: no task was taken by the unit.
pub const DRAW_UNIT_NONE: i32 = 0;
/// `LV_DRAW_UNIT_IDLE`: the unit is idle (nothing to do).
pub const DRAW_UNIT_IDLE: i32 = -1;

/// `lv_draw_task_type_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DrawTaskType {
    /// `LV_DRAW_TASK_TYPE_NONE`
    #[default]
    None = 0,
    /// `LV_DRAW_TASK_TYPE_FILL`
    Fill,
    /// `LV_DRAW_TASK_TYPE_BORDER`
    Border,
    /// `LV_DRAW_TASK_TYPE_BOX_SHADOW`
    BoxShadow,
    /// `LV_DRAW_TASK_TYPE_LETTER`
    Letter,
    /// `LV_DRAW_TASK_TYPE_LABEL`
    Label,
    /// `LV_DRAW_TASK_TYPE_IMAGE`
    Image,
    /// `LV_DRAW_TASK_TYPE_LAYER`
    Layer,
    /// `LV_DRAW_TASK_TYPE_LINE`
    Line,
    /// `LV_DRAW_TASK_TYPE_ARC`
    Arc,
    /// `LV_DRAW_TASK_TYPE_TRIANGLE`
    Triangle,
    /// `LV_DRAW_TASK_TYPE_MASK_RECTANGLE`
    MaskRectangle,
    /// `LV_DRAW_TASK_TYPE_MASK_BITMAP`
    MaskBitmap,
    /// `LV_DRAW_TASK_TYPE_BLUR`
    Blur,
}

/// `lv_draw_task_state_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DrawTaskState {
    /// `LV_DRAW_TASK_STATE_BLOCKED`
    #[default]
    Blocked = 0,
    /// `LV_DRAW_TASK_STATE_WAITING`
    Waiting,
    /// `LV_DRAW_TASK_STATE_QUEUED`
    Queued,
    /// `LV_DRAW_TASK_STATE_IN_PROGRESS`
    InProgress,
    /// `LV_DRAW_TASK_STATE_FINISHED`
    Finished,
    /// `LV_DRAW_TASK_STATE_FAILED`
    Failed,
}

/// Handle of a layer (`lv_layer_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(pub Key);

/// Reference to a task inside a layer's queue (`lv_draw_task_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DrawTaskRef {
    /// The layer owning the task.
    pub layer: LayerId,
    /// Index into [`Layer::tasks`].
    pub index: usize,
}

/// `lv_draw_dsc_base_t`: common head of every draw descriptor.
///
/// Omitted on purpose: `dsc_size` (only used by `lv_draw_add_task` for the
/// malloc size; the [`DrawDsc`] enum carries the type) and `user_data`
/// (only set by the SDL / OpenGLES backends).
#[derive(Clone, Debug, PartialEq)]
pub struct DrawDscBase {
    /// `obj`: the object drawn (if any).
    pub obj: Option<Obj>,
    /// `part`
    pub part: Part,
    /// `id1`: widget-specific id (e.g. button index).
    pub id1: u32,
    /// `id2`
    pub id2: u32,
    /// `layer`: the target layer, set when the task is added.
    pub layer: Option<LayerId>,
    /// `drop_shadow_ofs_x`
    pub drop_shadow_ofs_x: i16,
    /// `drop_shadow_ofs_y`
    pub drop_shadow_ofs_y: i16,
    /// `drop_shadow_color`
    pub drop_shadow_color: Color,
    /// `drop_shadow_opa`
    pub drop_shadow_opa: Opa,
    /// `drop_shadow_blur_radius`
    pub drop_shadow_blur_radius: i32,
    /// `drop_shadow_quality`
    pub drop_shadow_quality: BlurQuality,
}

impl DrawDscBase {
    /// Zero-initialised base (`lv_memzero`).
    pub const fn new() -> Self {
        Self {
            obj: None,
            part: Part::MAIN,
            id1: 0,
            id2: 0,
            layer: None,
            drop_shadow_ofs_x: 0,
            drop_shadow_ofs_y: 0,
            drop_shadow_color: Color::BLACK,
            drop_shadow_opa: 0,
            drop_shadow_blur_radius: 0,
            drop_shadow_quality: BlurQuality::Auto,
        }
    }
}

impl Default for DrawDscBase {
    fn default() -> Self {
        Self::new()
    }
}

/// `void * draw_dsc`: the typed descriptor of a task.
#[derive(Clone, Debug)]
pub enum DrawDsc {
    /// No descriptor.
    None,
    /// `lv_draw_fill_dsc_t`
    Fill(Box<FillDsc>),
    /// `lv_draw_border_dsc_t`
    Border(Box<BorderDsc>),
    /// `lv_draw_box_shadow_dsc_t`
    BoxShadow(Box<BoxShadowDsc>),
    /// `lv_draw_letter_dsc_t`
    Letter(Box<LetterDsc>),
    /// `lv_draw_label_dsc_t`
    Label(Box<LabelDsc>),
    /// `lv_draw_image_dsc_t`
    Image(Box<DrawImageDsc>),
    /// `lv_draw_image_dsc_t` whose `src` is a layer.
    Layer(Box<DrawImageDsc>),
    /// `lv_draw_line_dsc_t`
    Line(Box<LineDsc>),
    /// `lv_draw_arc_dsc_t`
    Arc(Box<ArcDsc>),
    /// `lv_draw_triangle_dsc_t`
    Triangle(Box<TriangleDsc>),
    /// `lv_draw_mask_rect_dsc_t`
    MaskRect(Box<MaskRectDsc>),
    /// Bitmap mask (no upstream struct, see [`MaskBitmapDsc`]).
    MaskBitmap(Box<MaskBitmapDsc>),
    /// `lv_draw_blur_dsc_t`
    Blur(Box<BlurDsc>),
}

impl DrawDsc {
    /// The common base of the descriptor, if any.
    pub fn base(&self) -> Option<&DrawDscBase> {
        match self {
            DrawDsc::None => None,
            DrawDsc::Fill(d) => Some(&d.base),
            DrawDsc::Border(d) => Some(&d.base),
            DrawDsc::BoxShadow(d) => Some(&d.base),
            DrawDsc::Letter(d) => Some(&d.base),
            DrawDsc::Label(d) => Some(&d.base),
            DrawDsc::Image(d) | DrawDsc::Layer(d) => Some(&d.base),
            DrawDsc::Line(d) => Some(&d.base),
            DrawDsc::Arc(d) => Some(&d.base),
            DrawDsc::Triangle(d) => Some(&d.base),
            DrawDsc::MaskRect(d) => Some(&d.base),
            DrawDsc::MaskBitmap(d) => Some(&d.base),
            DrawDsc::Blur(d) => Some(&d.base),
        }
    }

    /// Mutable access to the common base.
    pub fn base_mut(&mut self) -> Option<&mut DrawDscBase> {
        match self {
            DrawDsc::None => None,
            DrawDsc::Fill(d) => Some(&mut d.base),
            DrawDsc::Border(d) => Some(&mut d.base),
            DrawDsc::BoxShadow(d) => Some(&mut d.base),
            DrawDsc::Letter(d) => Some(&mut d.base),
            DrawDsc::Label(d) => Some(&mut d.base),
            DrawDsc::Image(d) | DrawDsc::Layer(d) => Some(&mut d.base),
            DrawDsc::Line(d) => Some(&mut d.base),
            DrawDsc::Arc(d) => Some(&mut d.base),
            DrawDsc::Triangle(d) => Some(&mut d.base),
            DrawDsc::MaskRect(d) => Some(&mut d.base),
            DrawDsc::MaskBitmap(d) => Some(&mut d.base),
            DrawDsc::Blur(d) => Some(&mut d.base),
        }
    }
}

/// `lv_draw_task_t`
#[derive(Clone, Debug)]
pub struct DrawTask {
    /// `type`
    pub ty: DrawTaskType,
    /// `area`: the area where the task should draw (not clipped).
    pub area: Area,
    /// `_real_area`: the real draw area (e.g. with shadow / outline).
    pub real_area: Area,
    /// `clip_area`
    pub clip_area: Area,
    /// `target_layer`
    pub target_layer: LayerId,
    /// `draw_unit`: index into `Ui::draw_units` of the unit that took the task.
    pub draw_unit: Option<u8>,
    /// `state`
    pub state: DrawTaskState,
    /// `draw_dsc`
    pub dsc: DrawDsc,
    /// `opa`
    pub opa: Opa,
    /// `preferred_draw_unit_id`
    pub preferred_draw_unit_id: u8,
    /// `preference_score`
    pub preference_score: u8,
}

/// `lv_layer_t`
///
/// `user_data` is omitted (only the NanoVG backend uses it).
pub struct Layer {
    /// `draw_buf`: target buffer (`None` until allocated).
    pub draw_buf: Option<DrawBuf>,
    /// `draw_task_head`: the task queue.
    pub tasks: VecDeque<DrawTask>,
    /// `display`
    pub display: Option<DisplayId>,
    /// `parent`
    pub parent: Option<LayerId>,
    /// `next`: the display's singly linked list of live layers
    /// (`DisplayData::layer_head` -> `next`); `lv_draw_layer_init` appends
    /// at the tail, `lv_draw_layer_deinit` unlinks.
    pub next: Option<LayerId>,
    /// `buf_area`: the absolute coordinates of the buffer.
    pub buf_area: Area,
    /// `phy_clip_area`: the physical clipping area relative to the display.
    pub phy_clip_area: Area,
    /// `_clip_area`: the current clip area, set by the drawing code.
    pub clip_area: Area,
    /// `partial_y_offset`: y offset when the layer is rendered in chunks.
    pub partial_y_offset: i32,
    /// `recolor`
    pub recolor: Color32,
    /// `color_format`
    pub color_format: ColorFormat,
    /// `all_tasks_added`
    pub all_tasks_added: bool,
    /// `opa`
    pub opa: Opa,
    /// `buffer_owned`: the draw buffer was allocated by the layer (so
    /// `lv_draw_layer` frees it).
    pub buffer_owned: bool,
}

impl Layer {
    /// `lv_layer_init()`: a zeroed layer (`next = None`) with
    /// `color_format = ARGB8888` and `opa = COVER`.
    pub fn new() -> Self {
        todo!()
    }

    /// `lv_layer_reset()`
    pub fn reset(&mut self) {
        todo!()
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_unit_t`: a rendering backend.
///
/// Return codes of `evaluate` / `dispatch` follow `lv_draw_private.h`:
/// [`DRAW_UNIT_NONE`] when nothing was taken, [`DRAW_UNIT_IDLE`] when idle,
/// positive when a task was taken.
pub trait DrawUnit {
    /// `name`
    fn name(&self) -> &'static str;
    /// `evaluate_cb`: decide whether this unit can render `task` and set
    /// `preferred_draw_unit_id` / `preference_score`.
    fn evaluate(&mut self, task: &mut DrawTask) -> i32;
    /// `dispatch_cb`: take and render ready tasks of `layer`.
    fn dispatch(&mut self, ctx: &mut DrawCtx, layer: LayerId) -> i32;
    /// `wait_for_finish_cb`
    fn wait_for_finish(&mut self) -> i32 {
        0
    }
    /// `delete_cb`
    fn delete(&mut self) {}
    /// `event_cb`: display / refresh related notifications sent by
    /// `lv_draw_unit_send_event()`. `param` is the `void * param`
    /// (`EventParam::Layer` for `CHILD_CREATED` / `CHILD_DELETED`, the
    /// display for refresh events, ...).
    fn on_event(&mut self, code: EventCode, param: &mut EventParam<'_>) {
        let _ = (code, param);
    }
}

/// The registered draw units (`unit_head`), owned by `Ui::draw_units`.
pub type DrawUnits = [Box<dyn DrawUnit>];

/// `lv_draw_global_info_t`: the draw subsystem state owned by [`Ui`]
/// (except `unit_head`, which is `Ui::draw_units`).
pub struct DrawCtx {
    /// All live layers.
    pub layers: Arena<Layer>,
    /// `used_memory_for_layers`
    pub used_memory_for_layers: u32,
    /// `dispatch_req`
    pub dispatch_req: bool,
    /// `task_running`
    pub task_running: bool,
}

impl DrawCtx {
    /// `lv_draw_init()`: layers only; units are registered with
    /// `Ui::draw_create_unit`.
    pub fn new() -> Self {
        Self {
            layers: Arena::new(),
            used_memory_for_layers: 0,
            dispatch_req: false,
            task_running: false,
        }
    }

    /// `lv_draw_add_task()`: append a task of `ty` to `layer`.
    pub fn add_task(&mut self, layer: LayerId, coords: &Area, ty: DrawTaskType) -> DrawTaskRef {
        todo!()
    }

    /// `lv_draw_finalize_task_creation()`: evaluate the task with every unit
    /// of `units` (`Ui::draw_units`) and request a dispatch.
    pub fn finalize_task_creation(&mut self, units: &mut DrawUnits, t: DrawTaskRef) {
        todo!()
    }

    /// `lv_draw_dispatch()`: dispatch every display's layers through `units`.
    pub fn dispatch(&mut self, units: &mut DrawUnits) {
        todo!()
    }

    /// `lv_draw_dispatch_layer()`: returns `true` if a task was dispatched.
    /// `u->dispatch_cb(u, layer)` becomes `unit.dispatch(self, layer)` for
    /// every unit of `units`.
    pub fn dispatch_layer(&mut self, units: &mut DrawUnits, disp: Option<DisplayId>, layer: LayerId) -> bool {
        todo!()
    }

    /// `lv_draw_dispatch_wait_for_request()`
    pub fn dispatch_wait_for_request(&mut self) {
        todo!()
    }

    /// `lv_draw_wait_for_finish()`: `wait_for_finish` on every unit.
    pub fn wait_for_finish(&mut self, units: &mut DrawUnits) {
        todo!()
    }

    /// `lv_draw_dispatch_request()`
    pub fn dispatch_request(&mut self) {
        todo!()
    }

    /// `lv_draw_get_available_task()`
    pub fn get_available_task(
        &self,
        layer: LayerId,
        t_prev: Option<DrawTaskRef>,
        draw_unit_id: u8,
    ) -> Option<DrawTaskRef> {
        todo!()
    }

    /// `lv_draw_get_next_available_task()`
    pub fn get_next_available_task(
        &self,
        layer: LayerId,
        t_prev: Option<DrawTaskRef>,
        draw_unit_id: u8,
    ) -> Option<DrawTaskRef> {
        todo!()
    }

    /// `lv_draw_get_dependent_count()`
    pub fn get_dependent_count(&self, t_check: DrawTaskRef) -> u32 {
        todo!()
    }

    /// `lv_draw_cleanup_task()`
    pub fn cleanup_task(&mut self, t: DrawTaskRef) {
        todo!()
    }

    /// `lv_draw_layer_create()`
    pub fn layer_create(
        &mut self,
        parent: Option<LayerId>,
        color_format: ColorFormat,
        area: &Area,
    ) -> LayerId {
        todo!()
    }

    /// `lv_draw_layer_init()`
    pub fn layer_init(
        &mut self,
        layer: LayerId,
        parent: Option<LayerId>,
        color_format: ColorFormat,
        area: &Area,
    ) {
        todo!()
    }

    /// `lv_draw_layer_alloc_buf()`: returns `false` if allocation failed.
    pub fn layer_alloc_buf(&mut self, layer: LayerId) -> bool {
        todo!()
    }

    /// `lv_draw_layer_go_to_xy()`: byte offset of `(x, y)` in the layer buffer.
    pub fn layer_go_to_xy(&self, layer: LayerId, x: i32, y: i32) -> Option<usize> {
        todo!()
    }

    /// `lv_draw_layer_create_drop_shadow()`
    pub fn layer_create_drop_shadow(
        &mut self,
        parent: LayerId,
        base: &DrawDscBase,
        area: &Area,
    ) -> Option<LayerId> {
        todo!()
    }

    /// `lv_draw_layer_finish_drop_shadow()`
    pub fn layer_finish_drop_shadow(&mut self, drop_shadow_layer: LayerId, base: &DrawDscBase) {
        todo!()
    }

    /// `lv_draw_layer_dealloc_buf()`
    pub fn layer_dealloc_buf(&mut self, layer: LayerId) {
        todo!()
    }

    /// `lv_draw_layer_deinit()`
    pub fn layer_deinit(&mut self, layer: LayerId) {
        todo!()
    }

    /// `lv_draw_layer_delete()`
    pub fn layer_delete(&mut self, layer: LayerId) {
        todo!()
    }

    /// `lv_draw_task_get_type()`
    pub fn task_get_type(&self, t: DrawTaskRef) -> DrawTaskType {
        todo!()
    }

    /// `lv_draw_task_get_draw_dsc()`
    pub fn task_get_draw_dsc(&self, t: DrawTaskRef) -> Option<&DrawDsc> {
        todo!()
    }

    /// Mutable `lv_draw_task_get_draw_dsc()`.
    pub fn task_get_draw_dsc_mut(&mut self, t: DrawTaskRef) -> Option<&mut DrawDsc> {
        todo!()
    }

    /// `lv_draw_task_get_area()`
    pub fn task_get_area(&self, t: DrawTaskRef) -> Area {
        todo!()
    }

    /// Borrow a task.
    #[inline]
    pub fn task(&self, t: DrawTaskRef) -> Option<&DrawTask> {
        self.layers.get(t.layer.0)?.tasks.get(t.index)
    }

    /// Mutably borrow a task.
    #[inline]
    pub fn task_mut(&mut self, t: DrawTaskRef) -> Option<&mut DrawTask> {
        self.layers.get_mut(t.layer.0)?.tasks.get_mut(t.index)
    }

    /// Borrow a layer.
    #[inline]
    pub fn layer(&self, l: LayerId) -> Option<&Layer> {
        self.layers.get(l.0)
    }

    /// Mutably borrow a layer.
    #[inline]
    pub fn layer_mut(&mut self, l: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(l.0)
    }
}

impl Default for DrawCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    /// `lv_draw_unit_send_event()`: send `code` with `param` to the draw unit
    /// called `name` (`None` = every unit, like a `NULL` name).
    pub fn draw_unit_send_event(&mut self, name: Option<&str>, code: EventCode, param: EventParam<'_>) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_type_order_matches_header() {
        assert_eq!(DrawTaskType::Fill as u8, 1);
        assert_eq!(DrawTaskType::Layer as u8, 7);
        assert_eq!(DrawTaskType::MaskRectangle as u8, 11);
        assert_eq!(DrawTaskType::Blur as u8, 13);
        assert_eq!(DrawTaskState::Failed as u8, 5);
        assert_eq!(DRAW_UNIT_IDLE, -1);
    }
}

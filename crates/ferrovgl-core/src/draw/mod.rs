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
//!
//! Display coupling: upstream `lv_draw_layer_init()` appends the layer to
//! `disp->layer_head` (where `disp` is the parent's display or
//! `lv_refr_get_disp_refreshing()`) and `lv_draw_dispatch()` walks that
//! list. [`DrawCtx`] cannot see the displays, so it keeps the same singly
//! linked `next` chains itself, one per display (keyed by [`Layer::display`]):
//! [`DrawCtx::dispatch`] walks the chain of the refreshing display only, like
//! upstream. The display-aware entry points are the `Ui::draw_*` methods at
//! the bottom of this file: they bind root layers to `Ui::disp_refresh`,
//! dispatch the chain of `Ui::disp_refresh`, forward
//! `LV_EVENT_CHILD_CREATED` / `LV_EVENT_CHILD_DELETED` to the draw units and
//! send `LV_EVENT_DRAW_TASK_ADDED` to the drawn object.
//!
//! Known deviation: when no display is refreshing, upstream
//! `lv_draw_dispatch()` dispatches nothing, while [`DrawCtx::dispatch`] with
//! `disp == None` walks every chain. Layers created outside a refresh (the
//! `None`-keyed chain, which upstream cannot have: `lv_draw_layer_init()`
//! would dereference a `NULL` display) and roots retained from a snapshot
//! are only rendered that way.
//!
//! Unit-less entry points: the `DrawCtx` methods that do not take the draw
//! units ([`DrawCtx::layer_create`], [`DrawCtx::layer_init`],
//! [`DrawCtx::layer_create_drop_shadow`], [`DrawCtx::layer_deinit`],
//! [`DrawCtx::layer_delete`]) behave like upstream with an empty
//! `unit_head`: no `LV_EVENT_CHILD_CREATED` / `LV_EVENT_CHILD_DELETED` and
//! no `lv_draw_wait_for_finish()`. With registered units use the matching
//! `Ui::draw_*` method, which reaches every unit.
//!
//! Task handles: a [`DrawTaskRef`] is an index into the layer queue, so it is
//! stable exactly as long as no finished task is removed in front of it. Like
//! upstream, removal only happens in [`DrawCtx::dispatch_layer`] and
//! [`DrawCtx::layer_deinit`]; callers must not keep a handle across those.

pub mod arc;
pub mod blur;
pub mod draw_buf;
pub mod grad;
pub mod image;
pub mod label;
pub mod line;
pub mod mask;
pub mod rect;
#[cfg(feature = "snapshot")]
pub mod snapshot;
pub mod triangle;
pub mod utils;

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::core::ui::DRAW_LAYER_MAX_MEMORY;
use crate::display::DisplayId;
use crate::misc::area::Area;
use crate::misc::arena::{Arena, Key};
use crate::misc::color::{Color, Color32, ColorFormat, OPA_COVER, Opa};
use crate::misc::event::{EventCode, EventParam};
use crate::misc::style::{BlurQuality, ImageSrc, Part};

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

/// `LV_DRAW_UNIT_NONE` as stored in `preferred_draw_unit_id`.
const DRAW_UNIT_NONE_ID: u8 = DRAW_UNIT_NONE as u8;

/// `lv_draw_task_type_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
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
    /// `LV_DRAW_TASK_TYPE_VECTOR` (`#if LV_USE_VECTOR_GRAPHIC`, on in the
    /// parity config). No task of this type is created yet:
    /// `lv_draw_vector()` / `lv_draw_vector_dsc_t` are not ported, so
    /// [`DrawDsc`] has no matching variant.
    Vector,
}

/// `lv_draw_task_state_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(pub Key);

/// Reference to a task inside a layer's queue (`lv_draw_task_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    #[must_use]
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
#[non_exhaustive]
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
    #[must_use]
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
    #[must_use]
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

    /// The layer a `LV_DRAW_TASK_TYPE_LAYER` task blends (`draw_dsc->src`
    /// of its `lv_draw_image_dsc_t`), if the descriptor is one.
    fn src_layer(&self) -> Option<LayerId> {
        match self {
            DrawDsc::Layer(d) | DrawDsc::Image(d) => match d.src {
                ImageSrc::Layer(l) => Some(l),
                _ => None,
            },
            _ => None,
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
#[derive(Debug)]
#[non_exhaustive]
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
    /// `lv_layer_init()`: a zeroed layer (`lv_memzero`, so `next = None` and
    /// `color_format = UNKNOWN`) followed by [`Layer::reset`]
    /// (`opa = COVER`, transparent `recolor`, `all_tasks_added = false`).
    #[must_use]
    pub fn new() -> Self {
        let mut layer = Self {
            draw_buf: None,
            tasks: VecDeque::new(),
            display: None,
            parent: None,
            next: None,
            buf_area: Area::default(),
            phy_clip_area: Area::default(),
            clip_area: Area::default(),
            partial_y_offset: 0,
            recolor: Color32::default(),
            color_format: ColorFormat::Unknown,
            all_tasks_added: false,
            opa: 0,
            buffer_owned: false,
        };
        layer.reset();
        layer
    }

    /// `lv_layer_reset()`
    pub fn reset(&mut self) {
        self.opa = OPA_COVER;
        self.recolor = Color32::make(0, 0, 0, 0);
        self.all_tasks_added = false;
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
    #[must_use]
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

impl core::fmt::Debug for dyn DrawUnit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DrawUnit")
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}

/// The registered draw units (`unit_head`), owned by `Ui::draw_units`.
pub type DrawUnits = [Box<dyn DrawUnit>];

/// `lv_draw_unit_send_event(NULL, code, layer)` on an optional unit list
/// (the `DrawCtx`-level entry points have no units at hand and pass `None`).
fn send_layer_event(units: Option<&mut DrawUnits>, code: EventCode, layer: LayerId) {
    if let Some(units) = units {
        let mut param = EventParam::Layer(layer);
        for u in units.iter_mut() {
            u.on_event(code, &mut param);
        }
    }
}

/// Reborrow an optional unit list for one more call.
fn reborrow<'a>(units: &'a mut Option<&mut DrawUnits>) -> Option<&'a mut DrawUnits> {
    match units {
        Some(u) => Some(&mut **u),
        None => None,
    }
}

/// `lv_draw_global_info_t`: the draw subsystem state owned by [`Ui`]
/// (except `unit_head`, which is `Ui::draw_units`).
#[derive(Debug)]
pub struct DrawCtx {
    /// All live layers.
    pub layers: Arena<Layer>,
    /// `used_memory_for_layers`
    pub used_memory_for_layers: u32,
    /// `dispatch_req`
    pub dispatch_req: bool,
    /// `task_running`
    pub task_running: bool,
    /// `unit_cnt`: number of registered draw units. `Ui::draw_create_unit`
    /// keeps it equal to `Ui::draw_units.len()`; [`DrawCtx::get_available_task`]
    /// uses it to pick the single-unit fast path.
    pub unit_cnt: u32,
    /// Heads of the per-display `next` chains (`disp->layer_head`), keyed by
    /// the layers' [`Layer::display`] (`None` for layers created without a
    /// known display). Appended in creation order like upstream.
    layer_heads: Vec<(Option<DisplayId>, LayerId)>,
    /// The file-scope state of `lv_bidi.c` (`custom_neutrals`), here so that
    /// the label drawing and the widgets share it (`LV_USE_BIDI`).
    #[cfg(feature = "bidi")]
    pub bidi: crate::misc::bidi::BidiState,
}

impl DrawCtx {
    /// `lv_draw_init()`: layers only; units are registered with
    /// `Ui::draw_create_unit`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: Arena::new(),
            used_memory_for_layers: 0,
            dispatch_req: false,
            task_running: false,
            unit_cnt: 0,
            layer_heads: Vec::new(),
            #[cfg(feature = "bidi")]
            bidi: crate::misc::bidi::BidiState::new(),
        }
    }

    /// `lv_draw_add_task()`: append a task of `ty` to `layer`.
    ///
    /// The new task saves the layer's current `_clip_area` and `opa`, starts
    /// in `WAITING` with `area == _real_area == coords` and an empty
    /// descriptor ([`DrawDsc::None`], the zeroed `draw_dsc` slot upstream);
    /// the caller stores its descriptor into [`DrawTask::dsc`] before
    /// finalizing. Panics on a stale `layer` (upstream: `LV_CHECK_ARG`).
    #[must_use]
    pub fn add_task(&mut self, layer: LayerId, coords: &Area, ty: DrawTaskType) -> DrawTaskRef {
        let lyr = &mut self.layers[layer.0];
        let new_task = DrawTask {
            ty,
            area: *coords,
            real_area: *coords,
            clip_area: lyr.clip_area,
            target_layer: layer,
            draw_unit: None,
            state: DrawTaskState::Waiting,
            dsc: DrawDsc::None,
            opa: lyr.opa,
            preferred_draw_unit_id: 0,
            preference_score: 0,
        };
        /*Find the tail*/
        lyr.tasks.push_back(new_task);
        DrawTaskRef {
            layer,
            index: lyr.tasks.len() - 1,
        }
    }

    /// "Let the draw units set their preference score" part of
    /// `lv_draw_finalize_task_creation()`.
    fn evaluate_task(&mut self, units: &mut DrawUnits, t: DrawTaskRef) {
        let Some(task) = self.task_mut(t) else {
            return;
        };
        task.preference_score = 100;
        task.preferred_draw_unit_id = 0;
        for u in units.iter_mut() {
            u.evaluate(task);
        }
    }

    /// `lv_draw_finalize_task_creation()`: "not taken by any unit" handling.
    fn fail_untaken_task(&mut self, t: DrawTaskRef) {
        let Some(task) = self.task_mut(t) else {
            return;
        };
        if task.preferred_draw_unit_id == DRAW_UNIT_NONE_ID {
            /* mark a non-taken layer task as BLOCKED so we don't try to release it
             * as one of its draw tasks might be in progress already */
            if task.ty == DrawTaskType::Layer {
                task.state = DrawTaskState::Blocked;
            } else {
                /* Other tasks won't have anything depending on them so we can just mark them as FAILED*/
                task.state = DrawTaskState::Failed;
            }
        }
    }

    /// `lv_draw_finalize_task_creation()`: evaluate the task with every unit
    /// of `units` (`Ui::draw_units`) and request a dispatch.
    ///
    /// This is the object-less variant: `LV_EVENT_DRAW_TASK_ADDED` is sent by
    /// `Ui::draw_finalize_task_creation`, which needs the object tree.
    /// `disp` is the refreshing display handed to [`DrawCtx::dispatch`].
    pub fn finalize_task_creation(&mut self, units: &mut DrawUnits, disp: Option<DisplayId>, t: DrawTaskRef) {
        let Some(task) = self.task_mut(t) else {
            return;
        };
        if let Some(base_dsc) = task.dsc.base_mut() {
            base_dsc.layer = Some(t.layer);
        }

        /*Dispatch only on the "main" draw_task and not on the draw tasks added in the event
         *as dispatching might remove the "main" draw task while it's still being used in the event*/
        if !self.task_running {
            /*Let the draw units set their preference score*/
            self.evaluate_task(units, t);
            self.fail_untaken_task(t);
            self.dispatch(units, disp);
        } else {
            /*Let the draw units set their preference score*/
            self.evaluate_task(units, t);
        }
    }

    /// `lv_draw_dispatch()`: dispatch the layers of `disp` through `units`.
    ///
    /// `disp` is `lv_refr_get_disp_refreshing()` (`Ui::disp_refresh`, which
    /// [`Ui::draw_dispatch`] passes). Like upstream, only the `layer_head`
    /// chain of that display is walked; the chains of the other displays are
    /// left alone until their own refresh. `None` walks every chain (a known
    /// deviation, see the module docs). Then, if no task was dispatched,
    /// `lv_draw_wait_for_finish()` + `lv_draw_dispatch_request()`.
    pub fn dispatch(&mut self, units: &mut DrawUnits, disp: Option<DisplayId>) {
        let mut task_dispatched = false;
        let heads: Vec<(Option<DisplayId>, LayerId)> = match disp {
            /*`lv_display_t * disp = lv_refr_get_disp_refreshing(); if(disp != NULL)`*/
            Some(_) => self.chain_head(disp).map(|h| (disp, h)).into_iter().collect(),
            /*No display is refreshing (see the module docs)*/
            None => self.layer_heads.clone(),
        };
        for (disp, head) in heads {
            let mut layer = Some(head);
            while let Some(l) = layer {
                if !self.layers.contains(l.0) {
                    break;
                }
                if self.dispatch_layer(units, disp, l) {
                    task_dispatched = true;
                }
                layer = self.layers.get(l.0).and_then(|x| x.next);
            }
        }
        if !task_dispatched {
            self.wait_for_finish(units);
            self.dispatch_request();
        }
    }

    /// `lv_draw_dispatch_layer()`: returns `true` if a task was dispatched.
    /// `u->dispatch_cb(u, layer)` becomes `unit.dispatch(self, layer)` for
    /// every unit of `units`.
    pub fn dispatch_layer(&mut self, units: &mut DrawUnits, disp: Option<DisplayId>, layer: LayerId) -> bool {
        let _ = disp; /*LV_UNUSED(disp)*/

        /*Remove the finished tasks first*/
        let mut remove_task = false;
        let mut removed: Vec<DrawTask> = Vec::new();
        {
            let Some(lyr) = self.layers.get_mut(layer.0) else {
                return false;
            };
            let mut i = 0;
            while i < lyr.tasks.len() {
                let state = lyr.tasks[i].state;
                if state == DrawTaskState::Finished || state == DrawTaskState::Failed {
                    if let Some(t) = lyr.tasks.remove(i) {
                        removed.push(t);
                    }
                    remove_task = true;
                } else {
                    i += 1;
                }
            }
        }
        for t in removed {
            self.cleanup_task_owned(Some(&mut *units), t);
        }

        let mut task_dispatched = false;

        let (parent, layer_ready, has_tasks) = {
            let lyr = &self.layers[layer.0];
            (
                lyr.parent,
                lyr.all_tasks_added && lyr.tasks.is_empty(),
                !lyr.tasks.is_empty(),
            )
        };

        /*This layer is ready, enable blending its buffer*/
        if let (Some(parent), true) = (parent, layer_ready) {
            /*Find a draw task with TYPE_LAYER in the layer where the src is this layer*/
            let mut request = false;
            if let Some(p) = self.layers.get_mut(parent.0) {
                for t_src in p.tasks.iter_mut() {
                    if t_src.ty == DrawTaskType::Layer
                        && t_src.state == DrawTaskState::Blocked
                        && t_src.dsc.src_layer() == Some(layer)
                    {
                        t_src.state = if t_src.preferred_draw_unit_id == DRAW_UNIT_NONE_ID {
                            DrawTaskState::Failed
                        } else {
                            DrawTaskState::Waiting
                        };
                        request = true;
                        break;
                    }
                }
            }
            if request {
                self.dispatch_request();
            }
        }
        /*Assign draw tasks to the draw_units*/
        else if remove_task || has_tasks {
            /*Find a draw unit which is not busy and can take at least one task*/
            /*Let all draw units to pick draw tasks*/
            for u in units.iter_mut() {
                let taken_cnt = u.dispatch(self, layer);
                if taken_cnt != DRAW_UNIT_IDLE {
                    task_dispatched = true;
                }
            }
        }

        task_dispatched
    }

    /// `lv_draw_dispatch_wait_for_request()`
    ///
    /// Single-thread (`LV_USE_OS == 0`) variant: upstream spins
    /// `while(!dispatch_req);` and clears the flag. Nothing else can set the
    /// flag on this thread, so the only reachable behaviour is the fall
    /// through with the flag cleared.
    pub fn dispatch_wait_for_request(&mut self) {
        self.dispatch_req = false;
    }

    /// `lv_draw_wait_for_finish()`: `wait_for_finish` on every unit.
    pub fn wait_for_finish(&mut self, units: &mut DrawUnits) {
        for u in units.iter_mut() {
            u.wait_for_finish();
        }
    }

    /// `lv_draw_dispatch_request()`
    pub fn dispatch_request(&mut self) {
        self.dispatch_req = true;
    }

    /// `lv_draw_get_available_task()`
    #[must_use]
    pub fn get_available_task(
        &self,
        layer: LayerId,
        t_prev: Option<DrawTaskRef>,
        draw_unit_id: u8,
    ) -> Option<DrawTaskRef> {
        if !self.layers.contains(layer.0) {
            return None;
        }
        if self.unit_cnt == 1 {
            self.get_first_available_task(layer)
        } else {
            self.get_next_available_task(layer, t_prev, draw_unit_id)
        }
    }

    /// `lv_draw_get_next_available_task()`
    #[must_use]
    pub fn get_next_available_task(
        &self,
        layer: LayerId,
        t_prev: Option<DrawTaskRef>,
        draw_unit_id: u8,
    ) -> Option<DrawTaskRef> {
        let lyr = self.layers.get(layer.0)?;

        /*If the first task covers the whole layer, there cannot be independent areas.*/
        if let Some(t) = lyr.tasks.front()
            && t.state != DrawTaskState::Waiting
            && t.area.x1 <= lyr.buf_area.x1
            && t.area.x2 >= lyr.buf_area.x2
            && t.area.y1 <= lyr.buf_area.y1
            && t.area.y2 >= lyr.buf_area.y2
        {
            return None;
        }

        let start = match t_prev {
            Some(p) => p.index + 1,
            None => 0,
        };
        for (index, t) in lyr.tasks.iter().enumerate().skip(start) {
            /*Find a draw task for this draw unit which is waiting and independent?*/
            if (t.preferred_draw_unit_id == draw_unit_id || t.preferred_draw_unit_id == DRAW_UNIT_NONE_ID)
                && t.state == DrawTaskState::Waiting
                && is_independent(lyr, index, draw_unit_id)
            {
                return Some(DrawTaskRef { layer, index });
            }
        }

        None
    }

    /// `lv_draw_get_dependent_count()`
    #[must_use]
    pub fn get_dependent_count(&self, t_check: DrawTaskRef) -> u32 {
        let Some(lyr) = self.layers.get(t_check.layer.0) else {
            return 0;
        };
        let Some(check) = lyr.tasks.get(t_check.index) else {
            return 0;
        };
        if t_check.index + 1 >= lyr.tasks.len() {
            return 0;
        }

        let mut cnt = 0;
        for t in lyr.tasks.iter().skip(t_check.index + 1) {
            if (t.state == DrawTaskState::Waiting || t.state == DrawTaskState::Blocked)
                && check.area.is_on(&t.area)
            {
                cnt += 1;
            }
        }
        cnt
    }

    /// `lv_draw_cleanup_task()`: free a draw task and everything it owns.
    ///
    /// Freeing means removing the task from its layer queue (the caller
    /// unlinks it upstream); the handles of the tasks behind it shift by one.
    /// The layer freed with a `LV_DRAW_TASK_TYPE_LAYER` task is deleted
    /// through `units` (`lv_draw_layer_delete()`: `LV_EVENT_CHILD_DELETED`).
    pub fn cleanup_task(&mut self, units: &mut DrawUnits, t: DrawTaskRef) {
        let Some(task) = self
            .layers
            .get_mut(t.layer.0)
            .and_then(|l| l.tasks.remove(t.index))
        else {
            return;
        };
        self.cleanup_task_owned(Some(units), task);
    }

    /// `lv_draw_cleanup_task()` on a task already taken out of its queue.
    fn cleanup_task_owned(&mut self, units: Option<&mut DrawUnits>, mut task: DrawTask) {
        debug_assert!(
            task.state != DrawTaskState::InProgress,
            "task (type {:?}) is still used by a draw unit",
            task.ty
        );
        if task.ty == DrawTaskType::Line {
            if let DrawDsc::Line(draw_line_dsc) = &mut task.dsc {
                draw_line_dsc.points = None;
            }
        }
        /*If it was layer drawing free the layer too*/
        else if task.ty == DrawTaskType::Layer
            && let Some(layer_drawn) = task.dsc.src_layer()
        {
            self.layer_delete_impl(units, layer_drawn);
        }
        /*A local label text (`text_local`) is freed with the descriptor*/
        drop(task);
    }

    /// `lv_draw_layer_create()` without draw units (no
    /// `LV_EVENT_CHILD_CREATED`; see the module docs and
    /// [`Ui::draw_layer_create`]).
    #[must_use]
    pub fn layer_create(
        &mut self,
        parent: Option<LayerId>,
        color_format: ColorFormat,
        area: &Area,
    ) -> LayerId {
        self.layer_create_impl(None, parent, color_format, area, None)
    }

    /// `lv_draw_layer_create()` with the units to notify and the refreshing
    /// display for a root layer.
    fn layer_create_impl(
        &mut self,
        units: Option<&mut DrawUnits>,
        parent: Option<LayerId>,
        color_format: ColorFormat,
        area: &Area,
        disp_refreshing: Option<DisplayId>,
    ) -> LayerId {
        let new_layer = LayerId(self.layers.insert(Layer::new()));

        self.layer_init_impl(units, new_layer, parent, color_format, area, disp_refreshing);

        /*Inherits transparency from parent*/
        if let Some(parent) = parent.and_then(|p| self.layers.get(p.0)) {
            let (opa, recolor) = (parent.opa, parent.recolor);
            let l = &mut self.layers[new_layer.0];
            l.opa = opa;
            l.recolor = recolor;
        }

        new_layer
    }

    /// `lv_draw_layer_init()`
    ///
    /// Without a parent the layer's display stays `None` (upstream binds it
    /// to `lv_refr_get_disp_refreshing()`, see `Ui::draw_layer_init`).
    /// Without draw units: no `LV_EVENT_CHILD_CREATED` (see the module docs).
    pub fn layer_init(
        &mut self,
        layer: LayerId,
        parent: Option<LayerId>,
        color_format: ColorFormat,
        area: &Area,
    ) {
        self.layer_init_impl(None, layer, parent, color_format, area, None);
    }

    /// `lv_draw_layer_init()` with the units to notify and the refreshing
    /// display for a root layer.
    fn layer_init_impl(
        &mut self,
        units: Option<&mut DrawUnits>,
        layer: LayerId,
        parent: Option<LayerId>,
        color_format: ColorFormat,
        area: &Area,
        disp_refreshing: Option<DisplayId>,
    ) {
        if !self.layers.contains(layer.0) {
            return;
        }
        let disp = match parent {
            Some(p) => self.layers.get(p.0).and_then(|l| l.display),
            None => disp_refreshing,
        };

        {
            let l = &mut self.layers[layer.0];
            *l = Layer::new(); /*lv_layer_init()*/
            l.parent = parent;
            l.clip_area = *area;
            l.buf_area = *area;
            l.phy_clip_area = *area;
            l.color_format = color_format;
            l.display = disp;
        }

        send_layer_event(units, EventCode::CHILD_CREATED, layer);

        self.chain_append(disp, layer);
    }

    /// The head of the `next` chain of `disp` (`disp->layer_head`).
    fn chain_head(&self, disp: Option<DisplayId>) -> Option<LayerId> {
        self.layer_heads.iter().find(|(d, _)| *d == disp).map(|(_, h)| *h)
    }

    /// Append `layer` at the tail of the `next` chain of `disp`
    /// (`disp->layer_head` handling of `lv_draw_layer_init()`).
    fn chain_append(&mut self, disp: Option<DisplayId>, layer: LayerId) {
        if let Some(head) = self.chain_head(disp) {
            let mut tail = head;
            loop {
                if tail == layer {
                    return; /*already linked*/
                }
                match self.layers.get(tail.0).and_then(|l| l.next) {
                    Some(n) => tail = n,
                    None => break,
                }
            }
            if let Some(t) = self.layers.get_mut(tail.0) {
                t.next = Some(layer);
            }
        } else {
            self.layer_heads.push((disp, layer));
        }
    }

    /// Unlink `layer` from its `next` chain (`lv_draw_layer_deinit()`).
    fn chain_unlink(&mut self, layer: LayerId) {
        let Some((disp, next)) = self.layers.get(layer.0).map(|l| (l.display, l.next)) else {
            return;
        };
        /*The chain of the layer's own display first, then the others*/
        let mut order: Vec<usize> = Vec::with_capacity(self.layer_heads.len());
        order.extend(
            self.layer_heads
                .iter()
                .enumerate()
                .filter(|(_, (d, _))| *d == disp)
                .map(|(i, _)| i),
        );
        order.extend(
            self.layer_heads
                .iter()
                .enumerate()
                .filter(|(_, (d, _))| *d != disp)
                .map(|(i, _)| i),
        );

        for ci in order {
            let head = self.layer_heads[ci].1;
            if head == layer {
                match next {
                    Some(n) => self.layer_heads[ci].1 = n,
                    None => {
                        self.layer_heads.remove(ci);
                    }
                }
                self.layers[layer.0].next = None;
                return;
            }
            let mut l = Some(head);
            while let Some(cur) = l {
                let cur_next = self.layers.get(cur.0).and_then(|x| x.next);
                if cur_next == Some(layer) {
                    self.layers[cur.0].next = next;
                    self.layers[layer.0].next = None;
                    return;
                }
                l = cur_next;
            }
        }
    }

    /// `lv_draw_layer_alloc_buf()`: returns `false` if allocation failed.
    #[must_use]
    pub fn layer_alloc_buf(&mut self, layer: LayerId) -> bool {
        let Some(l) = self.layers.get(layer.0) else {
            return false;
        };
        /*If the buffer of the layer is already allocated return it*/
        if l.draw_buf.is_some() {
            return true;
        }

        /*If the buffer of the layer is not allocated yet, allocate it now*/
        let w = l.buf_area.width();
        let h = l.buf_area.height();
        let color_format = l.color_format;
        let layer_size_byte = (h as u32).wrapping_mul(DrawBuf::width_to_stride(w as u32, color_format));

        /* Do not allocate the layer if the sum of allocated layer sizes
         * will exceed `LV_DRAW_LAYER_MAX_MEMORY` (`#if LV_DRAW_LAYER_MAX_MEMORY > 0`) */
        if DRAW_LAYER_MAX_MEMORY != 0
            && self.used_memory_for_layers.wrapping_add(layer_size_byte) > DRAW_LAYER_MAX_MEMORY
        {
            return false;
        }

        let Some(mut draw_buf) = DrawBuf::create(w as u32, h as u32, color_format, 0) else {
            return false;
        };

        self.used_memory_for_layers = self.used_memory_for_layers.wrapping_add(layer_size_byte);

        if color_format.has_alpha() {
            draw_buf.clear(None);
        }

        let l = &mut self.layers[layer.0];
        l.draw_buf = Some(draw_buf);
        l.buffer_owned = true;
        true
    }

    /// `lv_draw_layer_go_to_xy()`: byte offset of `(x, y)` in the layer buffer.
    #[must_use]
    pub fn layer_go_to_xy(&self, layer: LayerId, x: i32, y: i32) -> Option<usize> {
        self.layers
            .get(layer.0)?
            .draw_buf
            .as_ref()
            .and_then(|buf| buf.goto_xy_checked(x as u32, y as u32))
    }

    /// `lv_draw_layer_create_drop_shadow()` without draw units (no
    /// `LV_EVENT_CHILD_CREATED`; see [`Ui::draw_layer_create_drop_shadow`]).
    #[must_use]
    pub fn layer_create_drop_shadow(
        &mut self,
        parent: LayerId,
        base: &DrawDscBase,
        area: &Area,
    ) -> Option<LayerId> {
        self.layer_create_drop_shadow_impl(None, parent, base, area)
    }

    fn layer_create_drop_shadow_impl(
        &mut self,
        units: Option<&mut DrawUnits>,
        parent: LayerId,
        base: &DrawDscBase,
        area: &Area,
    ) -> Option<LayerId> {
        if !self.layers.contains(parent.0) {
            return None;
        }

        let mut drop_shadow_area = *area;
        let blur_radius = base.drop_shadow_blur_radius;

        /* x2 to have some extra space for cleaner blurring */
        drop_shadow_area.increase(blur_radius.wrapping_mul(2), blur_radius.wrapping_mul(2));

        Some(self.layer_create_impl(units, Some(parent), ColorFormat::A8, &drop_shadow_area, None))
    }

    /// `lv_draw_layer_finish_drop_shadow()`
    pub fn layer_finish_drop_shadow(&mut self, drop_shadow_layer: LayerId, base: &DrawDscBase) {
        let Some(l) = self.layers.get(drop_shadow_layer.0) else {
            return;
        };
        let mut drop_shadow_area = l.buf_area;
        let buf_area = l.buf_area;
        let parent = l.parent;

        let mut blur_dsc = BlurDsc::new();
        blur_dsc.blur_radius = base.drop_shadow_blur_radius;
        blur_dsc.quality = base.drop_shadow_quality;
        /* Forward the shadow color so backends that bake the recolor into the
         * blur pass (e.g. NanoVG, where the subsequent layer composite cannot
         * easily apply recolor to a pre-existing FBO image) can pick it up. */
        blur_dsc.base.drop_shadow_color = base.drop_shadow_color;
        blur_dsc.base.drop_shadow_opa = base.drop_shadow_opa;
        self.draw_blur(drop_shadow_layer, &blur_dsc, &buf_area);

        drop_shadow_area.move_by(
            i32::from(base.drop_shadow_ofs_x),
            i32::from(base.drop_shadow_ofs_y),
        );

        let mut layer_draw_dsc = DrawImageDsc::new();
        layer_draw_dsc.src = ImageSrc::Layer(drop_shadow_layer);
        layer_draw_dsc.recolor = base.drop_shadow_color;
        layer_draw_dsc.opa = base.drop_shadow_opa;
        if let Some(parent) = parent {
            self.draw_layer(parent, &layer_draw_dsc, &drop_shadow_area);
        }
    }

    /// `lv_draw_layer_dealloc_buf()`
    pub fn layer_dealloc_buf(&mut self, layer: LayerId) {
        let Some(l) = self.layers.get_mut(layer.0) else {
            return;
        };
        let Some(draw_buf) = l.draw_buf.as_ref() else {
            return;
        };
        if !l.buffer_owned {
            return;
        }

        let layer_size_byte = (l.buf_area.height() as u32).wrapping_mul(u32::from(draw_buf.header.stride));
        if self.used_memory_for_layers >= layer_size_byte {
            self.used_memory_for_layers -= layer_size_byte;
        } else {
            self.used_memory_for_layers = 0;
        }

        l.draw_buf = None; /*lv_draw_buf_destroy()*/
        l.buffer_owned = false;
    }

    /// `lv_draw_layer_deinit()`
    ///
    /// Without draw units: no `lv_draw_wait_for_finish()` and no
    /// `LV_EVENT_CHILD_DELETED`, also not for the layers freed with nested
    /// `LV_DRAW_TASK_TYPE_LAYER` tasks. Those go through
    /// `Ui::draw_layer_deinit` (or a dispatch, which has the units).
    pub fn layer_deinit(&mut self, layer: LayerId) {
        self.layer_deinit_impl(None, layer);
    }

    /// `lv_draw_layer_deinit()` with the units to notify.
    fn layer_deinit_impl(&mut self, mut units: Option<&mut DrawUnits>, layer: LayerId) {
        let Some(l) = self.layers.get_mut(layer.0) else {
            return;
        };

        if l.tasks.iter().any(|t| t.state == DrawTaskState::InProgress) {
            /*Deiniting a layer with a task in progress, waiting for the draw units*/
            if let Some(u) = reborrow(&mut units) {
                self.wait_for_finish(u);
            }
        }

        let tasks: Vec<DrawTask> = self.layers[layer.0].tasks.drain(..).collect();
        for t in tasks {
            self.cleanup_task_owned(reborrow(&mut units), t);
        }

        self.chain_unlink(layer);

        send_layer_event(reborrow(&mut units), EventCode::CHILD_DELETED, layer);

        self.layer_dealloc_buf(layer);
    }

    /// `lv_draw_layer_delete()` without draw units (see
    /// [`DrawCtx::layer_deinit`] and [`Ui::draw_layer_delete`]).
    pub fn layer_delete(&mut self, layer: LayerId) {
        self.layer_delete_impl(None, layer);
    }

    /// `lv_draw_layer_delete()` with the units to notify.
    fn layer_delete_impl(&mut self, units: Option<&mut DrawUnits>, layer: LayerId) {
        if !self.layers.contains(layer.0) {
            return;
        }
        self.layer_deinit_impl(units, layer);
        self.layers.remove(layer.0); /*lv_free(layer)*/
    }

    /// `lv_draw_task_get_type()`
    #[must_use]
    pub fn task_get_type(&self, t: DrawTaskRef) -> DrawTaskType {
        self.task(t).map_or(DrawTaskType::None, |task| task.ty)
    }

    /// `lv_draw_task_get_draw_dsc()`
    #[must_use]
    pub fn task_get_draw_dsc(&self, t: DrawTaskRef) -> Option<&DrawDsc> {
        self.task(t).map(|task| &task.dsc)
    }

    /// Mutable `lv_draw_task_get_draw_dsc()`.
    #[must_use]
    pub fn task_get_draw_dsc_mut(&mut self, t: DrawTaskRef) -> Option<&mut DrawDsc> {
        self.task_mut(t).map(|task| &mut task.dsc)
    }

    /// `lv_draw_task_get_area()`
    #[must_use]
    pub fn task_get_area(&self, t: DrawTaskRef) -> Area {
        self.task(t).map_or_else(Area::default, |task| task.area)
    }

    /// Borrow a task.
    #[inline]
    #[must_use]
    pub fn task(&self, t: DrawTaskRef) -> Option<&DrawTask> {
        self.layers.get(t.layer.0)?.tasks.get(t.index)
    }

    /// Mutably borrow a task.
    #[inline]
    #[must_use]
    pub fn task_mut(&mut self, t: DrawTaskRef) -> Option<&mut DrawTask> {
        self.layers.get_mut(t.layer.0)?.tasks.get_mut(t.index)
    }

    /// Borrow a layer.
    #[inline]
    #[must_use]
    pub fn layer(&self, l: LayerId) -> Option<&Layer> {
        self.layers.get(l.0)
    }

    /// Mutably borrow a layer.
    #[inline]
    #[must_use]
    pub fn layer_mut(&mut self, l: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(l.0)
    }

    /// `get_first_available_task()`
    fn get_first_available_task(&self, layer: LayerId) -> Option<DrawTaskRef> {
        /* If there is only 1 draw unit the task can be consumed linearly as
         * they are added in the correct order. However, it can happen that
         * there is a `LV_DRAW_TASK_TYPE_LAYER` which can be blended only when
         * all its tasks are ready. As other areas might be on top of that
         * layer-to-blend don't skip it. Instead stop there, so that the
         * draw tasks of that layer can be consumed and can be finished.
         * After that this layer-to-blenf will have `LV_DRAW_TASK_STATE_WAITING`
         * so it can be blended normally.*/
        let lyr = self.layers.get(layer.0)?;
        let t = lyr.tasks.front()?;
        /*Not waiting to be rendered, leave this layer while the first task is ready (i.e. not blocked)*/
        if t.state != DrawTaskState::Waiting {
            None
        }
        /*Waiting to be rendered, use it*/
        else {
            Some(DrawTaskRef { layer, index: 0 })
        }
    }
}

/// `is_independent()`: check if there are older draw tasks overlapping the
/// area of `t_check` (the task at `check_index` of `layer`).
///
/// Returns `true` if `t_check` is not overlapping with older tasks so it's
/// independent.
fn is_independent(layer: &Layer, check_index: usize, draw_unit_id: u8) -> bool {
    let Some(t_check) = layer.tasks.get(check_index) else {
        return true;
    };

    /*If t_check is outside of the older tasks then it's independent*/
    for t in layer.tasks.iter().take(check_index) {
        /*It's independent of finished draw tasks, and queued draw tasks of the same draw unit,
         *so no need to check it*/
        if t.state == DrawTaskState::Finished
            || (t.state == DrawTaskState::Queued && t.preferred_draw_unit_id == draw_unit_id)
        {
            continue;
        }

        if Area::intersect(&t.real_area, &t_check.real_area).is_some() {
            return false;
        }
    }

    true
}

impl Default for DrawCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    /// `lv_draw_unit_send_event()`: send `code` with `param` to the draw unit
    /// called `name` (`None` = every unit, like a `NULL` name).
    pub fn draw_unit_send_event(&mut self, name: Option<&str>, code: EventCode, mut param: EventParam<'_>) {
        for u in self.draw_units.iter_mut() {
            let matches = match name {
                None => true,
                Some(n) => n == u.name(),
            };
            if matches {
                u.on_event(code, &mut param);
            }
        }
    }

    /// `lv_draw_finalize_task_creation()`: the full upstream flow. Sends
    /// `LV_EVENT_DRAW_TASK_ADDED` to the drawn object (when it has
    /// `LV_OBJ_FLAG_SEND_DRAW_TASK_EVENTS`), lets every draw unit evaluate
    /// the task and dispatches.
    pub fn draw_finalize_task_creation(&mut self, t: DrawTaskRef) {
        let Some(task) = self.draw.task_mut(t) else {
            return;
        };
        let mut obj = None;
        if let Some(base_dsc) = task.dsc.base_mut() {
            base_dsc.layer = Some(t.layer);
            obj = base_dsc.obj;
        }

        /*Send LV_EVENT_DRAW_TASK_ADDED and dispatch only on the "main" draw_task
         *and not on the draw tasks added in the event.
         *Sending LV_EVENT_DRAW_TASK_ADDED events might cause recursive event sends and besides
         *dispatching might remove the "main" draw task while it's still being used in the event*/

        if !self.draw.task_running {
            if let Some(obj) = obj
                && self.obj_is_send_draw_task_events(obj)
            {
                self.draw.task_running = true;
                self.obj_send_event(obj, EventCode::DRAW_TASK_ADDED, EventParam::DrawTask(t));
                self.draw.task_running = false;
            }

            /*Let the draw units set their preference score*/
            self.draw.evaluate_task(&mut self.draw_units, t);
            self.draw.fail_untaken_task(t);
            let disp = self.disp_refresh;
            self.draw.dispatch(&mut self.draw_units, disp);
        } else {
            /*Let the draw units set their preference score*/
            self.draw.evaluate_task(&mut self.draw_units, t);
        }
    }

    /// `lv_draw_dispatch()`
    pub fn draw_dispatch(&mut self) {
        let disp = self.disp_refresh;
        self.draw.dispatch(&mut self.draw_units, disp);
    }

    /// `lv_draw_layer_create()`: a root layer (no parent) is bound to the
    /// refreshing display (`Ui::disp_refresh`) and the draw units get
    /// `LV_EVENT_CHILD_CREATED`.
    #[must_use]
    pub fn draw_layer_create(
        &mut self,
        parent: Option<LayerId>,
        color_format: ColorFormat,
        area: &Area,
    ) -> LayerId {
        let disp = self.disp_refresh;
        self.draw
            .layer_create_impl(Some(&mut self.draw_units), parent, color_format, area, disp)
    }

    /// `lv_draw_layer_init()`: like [`Ui::draw_layer_create`] for a layer
    /// already in `Ui::draw.layers`.
    pub fn draw_layer_init(
        &mut self,
        layer: LayerId,
        parent: Option<LayerId>,
        color_format: ColorFormat,
        area: &Area,
    ) {
        let disp = self.disp_refresh;
        self.draw.layer_init_impl(
            Some(&mut self.draw_units),
            layer,
            parent,
            color_format,
            area,
            disp,
        );
    }

    /// `lv_draw_layer_deinit()`: also sends `LV_EVENT_CHILD_DELETED` to the
    /// draw units.
    pub fn draw_layer_deinit(&mut self, layer: LayerId) {
        self.draw.layer_deinit_impl(Some(&mut self.draw_units), layer);
    }

    /// `lv_draw_layer_delete()`: also sends `LV_EVENT_CHILD_DELETED` to the
    /// draw units.
    pub fn draw_layer_delete(&mut self, layer: LayerId) {
        self.draw.layer_delete_impl(Some(&mut self.draw_units), layer);
    }

    /// `lv_draw_cleanup_task()`: [`DrawCtx::cleanup_task`] with the
    /// registered draw units.
    pub fn draw_cleanup_task(&mut self, t: DrawTaskRef) {
        self.draw.cleanup_task(&mut self.draw_units, t);
    }
}

impl Ui {
    /// `lv_draw_layer_create_drop_shadow()` with child-layer draw-unit notifications.
    #[must_use]
    pub fn draw_layer_create_drop_shadow(
        &mut self,
        parent: LayerId,
        base: &DrawDscBase,
        area: &Area,
    ) -> Option<LayerId> {
        self.draw
            .layer_create_drop_shadow_impl(Some(&mut self.draw_units), parent, base, area)
    }

    /// `lv_draw_layer_finish_drop_shadow()` with draw-unit evaluation and dispatch.
    /// Call this after drawing the shadow silhouette into the child A8 layer.
    pub fn draw_layer_finish_drop_shadow(&mut self, layer: LayerId, base: &DrawDscBase) {
        let Some(data) = self.draw.layer(layer) else {
            return;
        };
        let bounds = data.buf_area;
        let parent = data.parent;
        let mut blur = BlurDsc::new();
        blur.blur_radius = base.drop_shadow_blur_radius;
        blur.quality = base.drop_shadow_quality;
        blur.base.drop_shadow_color = base.drop_shadow_color;
        blur.base.drop_shadow_opa = base.drop_shadow_opa;
        self.draw_blur(layer, &blur, &bounds);
        let mut destination = bounds;
        destination.move_by(
            i32::from(base.drop_shadow_ofs_x),
            i32::from(base.drop_shadow_ofs_y),
        );
        let mut image = DrawImageDsc::new();
        image.src = ImageSrc::Layer(layer);
        image.recolor = base.drop_shadow_color;
        image.opa = base.drop_shadow_opa;
        if let Some(parent) = parent {
            self.draw_layer(parent, &image, &destination);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::area::Point;
    use crate::misc::style::BlendMode;
    use alloc::rc::Rc;
    use alloc::vec;
    use core::cell::RefCell;

    #[test]
    fn task_type_order_matches_header() {
        assert_eq!(DrawTaskType::Fill as u8, 1);
        assert_eq!(DrawTaskType::Layer as u8, 7);
        assert_eq!(DrawTaskType::MaskRectangle as u8, 11);
        assert_eq!(DrawTaskType::Blur as u8, 13);
        /*`#if LV_USE_VECTOR_GRAPHIC` (tests/configs/full.defconfig)*/
        assert_eq!(DrawTaskType::Vector as u8, 14);
        assert_eq!(DrawTaskState::Failed as u8, 5);
        assert_eq!(DRAW_UNIT_IDLE, -1);
    }

    /// The events a [`MockUnit`] received, shared with the test through an
    /// `Rc` because the unit itself is only reachable as a `dyn DrawUnit`.
    type EventLog = Rc<RefCell<Vec<(EventCode, Option<LayerId>)>>>;

    /// A unit that takes `Fill` and `Layer` tasks only (like `lv_draw_sw`
    /// with a restricted type set) and finishes them synchronously.
    struct MockUnit {
        id: u8,
        events: EventLog,
    }

    impl MockUnit {
        fn boxed(id: u8) -> Box<dyn DrawUnit> {
            Self::recording(id).0
        }

        /// A unit plus the log of the events it will receive.
        fn recording(id: u8) -> (Box<dyn DrawUnit>, EventLog) {
            let events: EventLog = Rc::new(RefCell::new(Vec::new()));
            (
                Box::new(MockUnit {
                    id,
                    events: events.clone(),
                }),
                events,
            )
        }
    }

    impl DrawUnit for MockUnit {
        fn name(&self) -> &'static str {
            "MOCK"
        }

        fn evaluate(&mut self, task: &mut DrawTask) -> i32 {
            if task.ty == DrawTaskType::Fill || task.ty == DrawTaskType::Layer {
                if task.preference_score > 90 {
                    task.preference_score = 90;
                    task.preferred_draw_unit_id = self.id;
                }
                return 1;
            }
            0
        }

        fn dispatch(&mut self, ctx: &mut DrawCtx, layer: LayerId) -> i32 {
            /*lv_draw_sw dispatch (single thread)*/
            let Some(t) = ctx.get_available_task(layer, None, self.id) else {
                return DRAW_UNIT_IDLE;
            };
            let task = ctx.task_mut(t).unwrap();
            if task.preferred_draw_unit_id != self.id {
                task.state = DrawTaskState::Failed;
                return DRAW_UNIT_IDLE;
            }
            task.state = DrawTaskState::InProgress;
            task.state = DrawTaskState::Finished;
            ctx.dispatch_request();
            1
        }

        fn on_event(&mut self, code: EventCode, param: &mut EventParam<'_>) {
            let layer = match param {
                EventParam::Layer(l) => Some(*l),
                _ => None,
            };
            self.events.borrow_mut().push((code, layer));
        }
    }

    fn layer_dsc(src: LayerId) -> DrawDsc {
        DrawDsc::Layer(Box::new(DrawImageDsc {
            base: DrawDscBase::new(),
            src: ImageSrc::Layer(src),
            header: ImageHeader::default(),
            clip_radius: 0,
            rotation: 0,
            scale_x: 256,
            scale_y: 256,
            skew_x: 0,
            skew_y: 0,
            pivot: Point::new(0, 0),
            recolor: Color::BLACK,
            recolor_opa: 0,
            opa: OPA_COVER,
            blend_mode: BlendMode::Normal,
            antialias: false,
            tile: false,
            colorkey: None,
            sup: None,
            image_area: Area::default(),
            bitmap_mask_src: ImageSrc::None,
        }))
    }

    fn ctx_with_layer(area: Area) -> (DrawCtx, LayerId) {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &area);
        (ctx, layer)
    }

    #[test]
    fn layer_init_and_reset() {
        let l = Layer::new();
        assert_eq!(l.opa, OPA_COVER);
        assert_eq!(l.recolor, Color32::make(0, 0, 0, 0));
        assert!(!l.all_tasks_added);
        assert_eq!(l.color_format, ColorFormat::Unknown);
        assert!(l.next.is_none() && l.parent.is_none() && l.draw_buf.is_none());
    }

    #[test]
    fn layer_create_sets_areas_and_inherits_parent() {
        let area = Area::new(10, 20, 109, 219);
        let (mut ctx, root) = ctx_with_layer(area);
        {
            let l = ctx.layer(root).unwrap();
            assert_eq!(l.buf_area, area);
            assert_eq!(l.clip_area, area);
            assert_eq!(l.phy_clip_area, area);
            assert_eq!(l.color_format, ColorFormat::Argb8888);
            assert_eq!(l.parent, None);
            assert_eq!(l.display, None);
        }
        ctx.layer_mut(root).unwrap().opa = 100;
        ctx.layer_mut(root).unwrap().recolor = Color32::make(1, 2, 3, 4);

        let child_area = Area::new(20, 30, 49, 59);
        let child = ctx.layer_create(Some(root), ColorFormat::A8, &child_area);
        let c = ctx.layer(child).unwrap();
        assert_eq!(c.parent, Some(root));
        assert_eq!(c.opa, 100);
        assert_eq!(c.recolor, Color32::make(1, 2, 3, 4));
        assert_eq!(c.color_format, ColorFormat::A8);
        /*Appended at the tail of the display's list*/
        assert_eq!(ctx.layer(root).unwrap().next, Some(child));
        assert_eq!(c.next, None);
    }

    #[test]
    fn layer_chain_links_and_unlinks_in_creation_order() {
        let (mut ctx, root) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let a = ctx.layer_create(Some(root), ColorFormat::Argb8888, &Area::new(0, 0, 9, 9));
        let b = ctx.layer_create(Some(root), ColorFormat::Argb8888, &Area::new(0, 0, 9, 9));
        let c = ctx.layer_create(Some(a), ColorFormat::Argb8888, &Area::new(0, 0, 9, 9));
        assert_eq!(ctx.layer(root).unwrap().next, Some(a));
        assert_eq!(ctx.layer(a).unwrap().next, Some(b));
        assert_eq!(ctx.layer(b).unwrap().next, Some(c));
        assert_eq!(ctx.layer(c).unwrap().next, None);

        /*Unlink from the middle*/
        ctx.layer_delete(b);
        assert!(ctx.layer(b).is_none());
        assert_eq!(ctx.layer(a).unwrap().next, Some(c));

        /*Deinit keeps the layer alive but unlinked*/
        ctx.layer_deinit(c);
        assert!(ctx.layer(c).is_some());
        assert_eq!(ctx.layer(a).unwrap().next, None);
        assert_eq!(ctx.layer(c).unwrap().next, None);

        /*Removing the head advances the head*/
        ctx.layer_delete(root);
        assert_eq!(ctx.layer_heads.len(), 1);
        assert_eq!(ctx.layer_heads[0].1, a);
        ctx.layer_delete(a);
        assert!(ctx.layer_heads.is_empty());
    }

    #[test]
    fn add_task_saves_clip_area_and_opa() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        ctx.layer_mut(layer).unwrap().clip_area = Area::new(5, 5, 50, 50);
        ctx.layer_mut(layer).unwrap().opa = 128;

        let coords = Area::new(-10, -10, 200, 200);
        let t = ctx.add_task(layer, &coords, DrawTaskType::Fill);
        assert_eq!(t.layer, layer);
        assert_eq!(t.index, 0);

        /*Changing the layer's clip area afterwards must not affect the task*/
        ctx.layer_mut(layer).unwrap().clip_area = Area::new(0, 0, 1, 1);
        ctx.layer_mut(layer).unwrap().opa = OPA_COVER;

        let task = ctx.task(t).unwrap();
        assert_eq!(task.ty, DrawTaskType::Fill);
        assert_eq!(task.area, coords);
        assert_eq!(task.real_area, coords);
        assert_eq!(task.clip_area, Area::new(5, 5, 50, 50));
        assert_eq!(task.opa, 128);
        assert_eq!(task.state, DrawTaskState::Waiting);
        assert_eq!(task.target_layer, layer);
        assert_eq!(task.preferred_draw_unit_id, 0);
        assert!(matches!(task.dsc, DrawDsc::None));

        let t2 = ctx.add_task(layer, &coords, DrawTaskType::Border);
        assert_eq!(t2.index, 1);
        assert_eq!(ctx.task(t2).unwrap().clip_area, Area::new(0, 0, 1, 1));
        assert_eq!(ctx.task_get_type(t2), DrawTaskType::Border);
        assert_eq!(ctx.task_get_area(t2), coords);
        assert_eq!(ctx.layer(layer).unwrap().tasks.len(), 2);
    }

    #[test]
    fn next_available_task_skips_overlapping_areas() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let a = ctx.add_task(layer, &Area::new(0, 0, 49, 49), DrawTaskType::Fill);
        let b = ctx.add_task(layer, &Area::new(40, 40, 60, 60), DrawTaskType::Fill); /*overlaps a*/
        let c = ctx.add_task(layer, &Area::new(70, 70, 99, 99), DrawTaskType::Fill); /*independent*/
        let d = ctx.add_task(layer, &Area::new(50, 50, 55, 55), DrawTaskType::Fill); /*overlaps b only*/

        /*Everything waiting: the first task is available*/
        assert_eq!(ctx.get_next_available_task(layer, None, 1), Some(a));
        /*Continue after a: b overlaps the (not finished) a, so c is next*/
        assert_eq!(ctx.get_next_available_task(layer, Some(a), 1), Some(c));
        assert_eq!(ctx.get_next_available_task(layer, Some(c), 1), None);

        /*a in progress: b and d are blocked (d transitively through b), c is free*/
        ctx.task_mut(a).unwrap().state = DrawTaskState::InProgress;
        assert_eq!(ctx.get_next_available_task(layer, None, 1), Some(c));

        /*a finished: b becomes available, d still waits for b*/
        ctx.task_mut(a).unwrap().state = DrawTaskState::Finished;
        assert_eq!(ctx.get_next_available_task(layer, None, 1), Some(b));
        assert_eq!(ctx.get_next_available_task(layer, Some(b), 1), Some(c));

        /*b queued for the same unit: d is independent for that unit but not for another*/
        ctx.task_mut(b).unwrap().state = DrawTaskState::Queued;
        ctx.task_mut(b).unwrap().preferred_draw_unit_id = 1;
        ctx.task_mut(d).unwrap().preferred_draw_unit_id = 1;
        assert_eq!(ctx.get_next_available_task(layer, Some(c), 1), Some(d));
        assert_eq!(ctx.get_next_available_task(layer, Some(c), 2), None);

        /*preferred_draw_unit_id filters tasks of other units (NONE matches any)*/
        ctx.task_mut(c).unwrap().preferred_draw_unit_id = 2;
        assert_eq!(ctx.get_next_available_task(layer, Some(b), 1), Some(d));
        assert_eq!(ctx.get_next_available_task(layer, Some(b), 2), Some(c));
    }

    #[test]
    fn next_available_task_uses_real_area_for_dependencies() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let a = ctx.add_task(layer, &Area::new(0, 0, 19, 19), DrawTaskType::BoxShadow);
        /*A shadow spreads further than its `area`*/
        ctx.task_mut(a).unwrap().real_area = Area::new(-10, -10, 39, 39);
        ctx.task_mut(a).unwrap().state = DrawTaskState::InProgress;
        let b = ctx.add_task(layer, &Area::new(30, 30, 39, 39), DrawTaskType::Fill);
        assert_eq!(ctx.get_next_available_task(layer, None, 1), None);
        ctx.task_mut(a).unwrap().real_area = Area::new(0, 0, 19, 19);
        assert_eq!(ctx.get_next_available_task(layer, None, 1), Some(b));
    }

    #[test]
    fn next_available_task_stops_when_first_task_covers_the_layer() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(10, 10, 59, 59));
        let a = ctx.add_task(layer, &Area::new(0, 0, 99, 99), DrawTaskType::Fill);
        let b = ctx.add_task(layer, &Area::new(200, 200, 210, 210), DrawTaskType::Fill);
        /*The first task is in progress and covers the whole layer: nothing can be independent*/
        ctx.task_mut(a).unwrap().state = DrawTaskState::InProgress;
        assert_eq!(ctx.get_next_available_task(layer, None, 1), None);
        /*A smaller first task does not block the far away b*/
        ctx.task_mut(a).unwrap().area = Area::new(10, 10, 59, 58);
        assert_eq!(ctx.get_next_available_task(layer, None, 1), Some(b));
        /*A waiting covering task is simply the next task*/
        ctx.task_mut(a).unwrap().area = Area::new(0, 0, 99, 99);
        ctx.task_mut(a).unwrap().state = DrawTaskState::Waiting;
        assert_eq!(ctx.get_next_available_task(layer, None, 1), Some(a));
    }

    #[test]
    fn blocked_layer_task_blocks_overlapping_tasks_only() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let child = ctx.layer_create(Some(layer), ColorFormat::Argb8888, &Area::new(0, 0, 49, 49));
        let lt = ctx.add_task(layer, &Area::new(0, 0, 49, 49), DrawTaskType::Layer);
        ctx.task_mut(lt).unwrap().dsc = layer_dsc(child);
        ctx.task_mut(lt).unwrap().state = DrawTaskState::Blocked;
        let over = ctx.add_task(layer, &Area::new(40, 40, 60, 60), DrawTaskType::Fill);
        let apart = ctx.add_task(layer, &Area::new(60, 0, 99, 39), DrawTaskType::Fill);

        assert_eq!(ctx.get_next_available_task(layer, None, 1), Some(apart));
        assert_eq!(ctx.get_next_available_task(layer, Some(apart), 1), None);

        /*Once the layer task is ready it comes first again*/
        ctx.task_mut(lt).unwrap().state = DrawTaskState::Waiting;
        assert_eq!(ctx.get_next_available_task(layer, None, 1), Some(lt));
        assert_eq!(ctx.get_next_available_task(layer, Some(lt), 1), Some(apart));
        let _ = over;
    }

    #[test]
    fn single_unit_takes_tasks_strictly_in_order() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        ctx.unit_cnt = 1;
        let a = ctx.add_task(layer, &Area::new(0, 0, 9, 9), DrawTaskType::Layer);
        let b = ctx.add_task(layer, &Area::new(50, 50, 59, 59), DrawTaskType::Fill);

        assert_eq!(ctx.get_available_task(layer, None, 1), Some(a));
        /*A blocked layer task at the head stops the single unit, even though b is independent*/
        ctx.task_mut(a).unwrap().state = DrawTaskState::Blocked;
        assert_eq!(ctx.get_available_task(layer, None, 1), None);
        assert_eq!(ctx.get_next_available_task(layer, None, 1), Some(b));
        /*With several units the dependency walk is used instead*/
        ctx.unit_cnt = 2;
        assert_eq!(ctx.get_available_task(layer, None, 1), Some(b));
        /*Empty layer*/
        ctx.unit_cnt = 1;
        ctx.layer_mut(layer).unwrap().tasks.clear();
        assert_eq!(ctx.get_available_task(layer, None, 1), None);
    }

    #[test]
    fn dependent_count_counts_waiting_and_blocked_overlaps_behind() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let before = ctx.add_task(layer, &Area::new(0, 0, 99, 99), DrawTaskType::Fill);
        let check = ctx.add_task(layer, &Area::new(10, 10, 29, 29), DrawTaskType::Fill);
        let t1 = ctx.add_task(layer, &Area::new(29, 29, 40, 40), DrawTaskType::Fill); /*touches*/
        let t2 = ctx.add_task(layer, &Area::new(30, 30, 40, 40), DrawTaskType::Fill); /*outside*/
        let t3 = ctx.add_task(layer, &Area::new(0, 0, 99, 99), DrawTaskType::Layer); /*blocked, overlaps*/
        let t4 = ctx.add_task(layer, &Area::new(10, 10, 29, 29), DrawTaskType::Fill); /*finished*/
        ctx.task_mut(t3).unwrap().state = DrawTaskState::Blocked;
        ctx.task_mut(t4).unwrap().state = DrawTaskState::Finished;

        assert_eq!(ctx.get_dependent_count(check), 2);
        let _ = (t1, t2);
        /*Older tasks are not counted (check, t1, t2, t3 depend on `before`),
         *the last task has no dependents*/
        assert_eq!(ctx.get_dependent_count(before), 4);
        assert_eq!(ctx.get_dependent_count(t4), 0);
        assert_eq!(ctx.get_dependent_count(DrawTaskRef { layer, index: 99 }), 0);
    }

    #[test]
    fn dispatch_layer_removes_finished_and_failed_tasks() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut units: Vec<Box<dyn DrawUnit>> = Vec::new();
        let a = ctx.add_task(layer, &Area::new(0, 0, 9, 9), DrawTaskType::Fill);
        let b = ctx.add_task(layer, &Area::new(10, 10, 19, 19), DrawTaskType::Fill);
        let c = ctx.add_task(layer, &Area::new(20, 20, 29, 29), DrawTaskType::Fill);
        let d = ctx.add_task(layer, &Area::new(30, 30, 39, 39), DrawTaskType::Fill);
        ctx.task_mut(a).unwrap().state = DrawTaskState::Finished;
        ctx.task_mut(c).unwrap().state = DrawTaskState::Failed;
        let _ = (b, d);

        /*No units: nothing can be dispatched but the queue is cleaned*/
        assert!(!ctx.dispatch_layer(&mut units, None, layer));
        let tasks = &ctx.layer(layer).unwrap().tasks;
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].area, Area::new(10, 10, 19, 19));
        assert_eq!(tasks[1].area, Area::new(30, 30, 39, 39));
    }

    #[test]
    fn finished_child_layer_unblocks_its_layer_task() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut units: Vec<Box<dyn DrawUnit>> = Vec::new();
        let child = ctx.layer_create(Some(layer), ColorFormat::Argb8888, &Area::new(0, 0, 49, 49));
        let lt = ctx.add_task(layer, &Area::new(0, 0, 49, 49), DrawTaskType::Layer);
        ctx.task_mut(lt).unwrap().dsc = layer_dsc(child);
        ctx.task_mut(lt).unwrap().state = DrawTaskState::Blocked;
        ctx.task_mut(lt).unwrap().preferred_draw_unit_id = 1;

        /*Child not finished yet: still has a task*/
        let ct = ctx.add_task(child, &Area::new(0, 0, 9, 9), DrawTaskType::Fill);
        ctx.layer_mut(child).unwrap().all_tasks_added = true;
        ctx.dispatch_req = false;
        assert!(!ctx.dispatch_layer(&mut units, None, child));
        assert_eq!(ctx.task(lt).unwrap().state, DrawTaskState::Blocked);
        assert!(!ctx.dispatch_req);

        /*Child's task finished: it is removed and the layer task is released*/
        ctx.task_mut(ct).unwrap().state = DrawTaskState::Finished;
        assert!(!ctx.dispatch_layer(&mut units, None, child));
        assert!(ctx.layer(child).unwrap().tasks.is_empty());
        assert_eq!(ctx.task(lt).unwrap().state, DrawTaskState::Waiting);
        assert!(ctx.dispatch_req);

        /*A layer task nobody took fails instead*/
        ctx.task_mut(lt).unwrap().state = DrawTaskState::Blocked;
        ctx.task_mut(lt).unwrap().preferred_draw_unit_id = DRAW_UNIT_NONE_ID;
        assert!(!ctx.dispatch_layer(&mut units, None, child));
        assert_eq!(ctx.task(lt).unwrap().state, DrawTaskState::Failed);

        /*all_tasks_added == false keeps the layer task blocked*/
        ctx.task_mut(lt).unwrap().state = DrawTaskState::Blocked;
        ctx.layer_mut(child).unwrap().all_tasks_added = false;
        assert!(!ctx.dispatch_layer(&mut units, None, child));
        assert_eq!(ctx.task(lt).unwrap().state, DrawTaskState::Blocked);
    }

    #[test]
    fn cleanup_of_a_layer_task_deletes_the_drawn_layer() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut units: Vec<Box<dyn DrawUnit>> = Vec::new();
        let child = ctx.layer_create(Some(layer), ColorFormat::Argb8888, &Area::new(0, 0, 49, 49));
        let grandchild = ctx.layer_create(Some(child), ColorFormat::Argb8888, &Area::new(0, 0, 9, 9));
        let gt = ctx.add_task(child, &Area::new(0, 0, 9, 9), DrawTaskType::Layer);
        ctx.task_mut(gt).unwrap().dsc = layer_dsc(grandchild);
        ctx.task_mut(gt).unwrap().state = DrawTaskState::Waiting;

        let lt = ctx.add_task(layer, &Area::new(0, 0, 49, 49), DrawTaskType::Layer);
        ctx.task_mut(lt).unwrap().dsc = layer_dsc(child);
        ctx.task_mut(lt).unwrap().state = DrawTaskState::Finished;
        assert_eq!(ctx.layers.len(), 3);

        assert!(!ctx.dispatch_layer(&mut units, None, layer));
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
        /*The child and, through its own layer task, the grandchild are gone*/
        assert!(ctx.layer(child).is_none());
        assert!(ctx.layer(grandchild).is_none());
        assert_eq!(ctx.layers.len(), 1);
        assert_eq!(ctx.layer(layer).unwrap().next, None);
    }

    #[test]
    fn cleanup_task_removes_it_from_the_queue() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let a = ctx.add_task(layer, &Area::new(0, 0, 9, 9), DrawTaskType::Fill);
        let b = ctx.add_task(layer, &Area::new(10, 10, 19, 19), DrawTaskType::Fill);
        ctx.task_mut(a).unwrap().state = DrawTaskState::Finished;
        ctx.cleanup_task(&mut [], a);
        assert_eq!(ctx.layer(layer).unwrap().tasks.len(), 1);
        assert_eq!(ctx.task_get_area(a), Area::new(10, 10, 19, 19));
        let _ = b;
        /*Stale handle: no-op*/
        ctx.cleanup_task(&mut [], DrawTaskRef { layer, index: 5 });
        assert_eq!(ctx.layer(layer).unwrap().tasks.len(), 1);
    }

    #[test]
    fn finalize_evaluates_dispatches_and_fails_untaken_tasks() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut units: Vec<Box<dyn DrawUnit>> = vec![MockUnit::boxed(1)];
        ctx.unit_cnt = 1;

        /*A Fill task is taken by the mock unit and rendered right away*/
        let t = ctx.add_task(layer, &Area::new(0, 0, 9, 9), DrawTaskType::Fill);
        ctx.dispatch_req = false;
        ctx.finalize_task_creation(&mut units, None, t);
        {
            let task = ctx.task(t).unwrap();
            assert_eq!(task.preferred_draw_unit_id, 1);
            assert_eq!(task.preference_score, 90);
            assert_eq!(task.state, DrawTaskState::Finished);
        }
        assert!(ctx.dispatch_req);

        /*A task no unit wants fails ("not taken by any unit")*/
        let t2 = ctx.add_task(layer, &Area::new(10, 10, 19, 19), DrawTaskType::Arc);
        ctx.evaluate_task(&mut units, t2);
        ctx.fail_untaken_task(t2);
        {
            let task = ctx.task(t2).unwrap();
            assert_eq!(task.preference_score, 100);
            assert_eq!(task.preferred_draw_unit_id, DRAW_UNIT_NONE_ID);
            assert_eq!(task.state, DrawTaskState::Failed);
        }
        /*... and the dispatch that follows in lv_draw_finalize_task_creation
         *removes it (together with the finished Fill task) right away*/
        ctx.dispatch(&mut units, None);
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
        let t3 = ctx.add_task(layer, &Area::new(10, 10, 19, 19), DrawTaskType::Arc);
        ctx.finalize_task_creation(&mut units, None, t3);
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());

        /*An untaken LAYER task is blocked instead of failed, so it stays*/
        let mut no_units: Vec<Box<dyn DrawUnit>> = Vec::new();
        let child = ctx.layer_create(Some(layer), ColorFormat::Argb8888, &Area::new(0, 0, 9, 9));
        let lt = ctx.add_task(layer, &Area::new(0, 0, 9, 9), DrawTaskType::Layer);
        ctx.task_mut(lt).unwrap().dsc = layer_dsc(child);
        ctx.finalize_task_creation(&mut no_units, None, lt);
        let tasks = &ctx.layer(layer).unwrap().tasks;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].ty, DrawTaskType::Layer);
        assert_eq!(tasks[0].state, DrawTaskState::Blocked);
        assert_eq!(tasks[0].dsc.base().unwrap().layer, Some(layer));
    }

    #[test]
    fn finalize_while_a_task_is_running_only_evaluates() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut units: Vec<Box<dyn DrawUnit>> = vec![MockUnit::boxed(1)];
        ctx.unit_cnt = 1;
        ctx.task_running = true;
        let t = ctx.add_task(layer, &Area::new(0, 0, 9, 9), DrawTaskType::Fill);
        ctx.dispatch_req = false;
        ctx.finalize_task_creation(&mut units, None, t);
        let task = ctx.task(t).unwrap();
        assert_eq!(task.preferred_draw_unit_id, 1);
        assert_eq!(task.state, DrawTaskState::Waiting);
        assert!(!ctx.dispatch_req);
    }

    #[test]
    fn dispatch_walks_every_layer_and_requests_when_idle() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut units: Vec<Box<dyn DrawUnit>> = vec![MockUnit::boxed(1)];
        ctx.unit_cnt = 1;
        let child = ctx.layer_create(Some(layer), ColorFormat::Argb8888, &Area::new(0, 0, 49, 49));
        let a = ctx.add_task(layer, &Area::new(0, 0, 9, 9), DrawTaskType::Fill);
        let b = ctx.add_task(child, &Area::new(20, 20, 29, 29), DrawTaskType::Fill);
        for t in [a, b] {
            ctx.task_mut(t).unwrap().preferred_draw_unit_id = 1;
        }

        ctx.dispatch(&mut units, None);
        assert_eq!(ctx.task(a).unwrap().state, DrawTaskState::Finished);
        assert_eq!(ctx.task(b).unwrap().state, DrawTaskState::Finished);

        /*Nothing left to do: the finished tasks are removed and a dispatch is requested*/
        ctx.dispatch_req = false;
        ctx.dispatch(&mut units, None);
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
        assert!(ctx.layer(child).unwrap().tasks.is_empty());
        assert!(ctx.dispatch_req);

        ctx.dispatch_wait_for_request();
        assert!(!ctx.dispatch_req);
    }

    #[test]
    fn dispatch_walks_only_the_refreshing_display_chain() {
        /*lv_draw_dispatch(): only `lv_refr_get_disp_refreshing()->layer_head`*/
        let mut keys = Arena::<()>::new();
        let d1 = DisplayId(keys.insert(()));
        let d2 = DisplayId(keys.insert(()));
        let d3 = DisplayId(keys.insert(()));
        let mut ctx = DrawCtx::new();
        let mut units: Vec<Box<dyn DrawUnit>> = vec![MockUnit::boxed(1)];
        ctx.unit_cnt = 1;
        let area = Area::new(0, 0, 9, 9);
        let r1 = ctx.layer_create_impl(None, None, ColorFormat::Argb8888, &area, Some(d1));
        let r2 = ctx.layer_create_impl(None, None, ColorFormat::Argb8888, &area, Some(d2));
        let r0 = ctx.layer_create(None, ColorFormat::Argb8888, &area);
        let mut tasks = Vec::new();
        for l in [r1, r2, r0] {
            let t = ctx.add_task(l, &area, DrawTaskType::Fill);
            ctx.task_mut(t).unwrap().preferred_draw_unit_id = 1;
            tasks.push(t);
        }
        let state = |ctx: &DrawCtx, i: usize| ctx.task(tasks[i]).unwrap().state;

        /*d1 refreshes: the WAITING tasks of d2 and of the display-less chain stay*/
        ctx.dispatch_req = false;
        ctx.dispatch(&mut units, Some(d1));
        assert_eq!(state(&ctx, 0), DrawTaskState::Finished);
        assert_eq!(state(&ctx, 1), DrawTaskState::Waiting);
        assert_eq!(state(&ctx, 2), DrawTaskState::Waiting);

        /*A refreshing display without layers dispatches nothing and requests*/
        ctx.dispatch_req = false;
        ctx.dispatch(&mut units, Some(d3));
        assert_eq!(state(&ctx, 1), DrawTaskState::Waiting);
        assert!(ctx.dispatch_req);

        ctx.dispatch(&mut units, Some(d2));
        assert_eq!(state(&ctx, 1), DrawTaskState::Finished);
        assert_eq!(state(&ctx, 2), DrawTaskState::Waiting);

        /*No display refreshing: every chain (documented deviation)*/
        ctx.dispatch(&mut units, None);
        assert_eq!(state(&ctx, 2), DrawTaskState::Finished);
    }

    #[test]
    fn ui_dispatch_uses_disp_refresh() {
        use crate::core::UiConfig;
        let mut ui = Ui::new(UiConfig::default());
        ui.draw_units.push(MockUnit::boxed(1));
        ui.draw.unit_cnt = 1;
        let mut keys = Arena::<()>::new();
        let d1 = DisplayId(keys.insert(()));
        let d2 = DisplayId(keys.insert(()));
        let area = Area::new(0, 0, 9, 9);
        ui.disp_refresh = Some(d1);
        let r1 = ui.draw_layer_create(None, ColorFormat::Argb8888, &area);
        ui.disp_refresh = Some(d2);
        let t = ui.draw.add_task(r1, &area, DrawTaskType::Fill);
        ui.draw.task_mut(t).unwrap().preferred_draw_unit_id = 1;
        /*d2 refreshing: the task of d1 is not taken*/
        ui.draw_dispatch();
        assert_eq!(ui.draw.task(t).unwrap().state, DrawTaskState::Waiting);
        ui.disp_refresh = Some(d1);
        ui.draw_dispatch();
        assert_eq!(ui.draw.task(t).unwrap().state, DrawTaskState::Finished);
    }

    #[test]
    fn cleanup_of_a_layer_task_notifies_the_units() {
        use crate::core::UiConfig;
        let mut ui = Ui::new(UiConfig::default());
        let (unit, events) = MockUnit::recording(1);
        ui.draw_units.push(unit);
        ui.draw.unit_cnt = 1;
        let area = Area::new(0, 0, 9, 9);
        let root = ui.draw_layer_create(None, ColorFormat::Argb8888, &area);
        let child = ui.draw_layer_create(Some(root), ColorFormat::Argb8888, &area);
        let lt = ui.draw.add_task(root, &area, DrawTaskType::Layer);
        ui.draw.task_mut(lt).unwrap().dsc = layer_dsc(child);
        ui.draw.task_mut(lt).unwrap().state = DrawTaskState::Finished;
        /*lv_draw_cleanup_task() -> lv_draw_layer_delete(layer_drawn) -> CHILD_DELETED*/
        ui.draw_cleanup_task(lt);
        assert!(ui.draw.layer(child).is_none());
        assert!(ui.draw.layer(root).unwrap().tasks.is_empty());
        assert_eq!(
            *events.borrow(),
            vec![
                (EventCode::CHILD_CREATED, Some(root)),
                (EventCode::CHILD_CREATED, Some(child)),
                (EventCode::CHILD_DELETED, Some(child)),
            ]
        );
    }

    #[test]
    fn full_layer_flow_blends_the_child_once_it_is_ready() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut units: Vec<Box<dyn DrawUnit>> = vec![MockUnit::boxed(1)];
        ctx.unit_cnt = 1;

        /*lv_draw_layer(): the LAYER task of the parent starts BLOCKED*/
        let child = ctx.layer_create(Some(layer), ColorFormat::Argb8888, &Area::new(10, 10, 49, 49));
        let lt = ctx.add_task(layer, &Area::new(10, 10, 49, 49), DrawTaskType::Layer);
        ctx.task_mut(lt).unwrap().dsc = layer_dsc(child);
        ctx.task_mut(lt).unwrap().state = DrawTaskState::Blocked;
        ctx.finalize_task_creation(&mut units, None, lt);
        assert_eq!(ctx.task(lt).unwrap().state, DrawTaskState::Blocked);
        assert_eq!(ctx.task(lt).unwrap().preferred_draw_unit_id, 1);

        /*Draw into the child, then mark it complete*/
        let ct = ctx.add_task(child, &Area::new(10, 10, 19, 19), DrawTaskType::Fill);
        ctx.finalize_task_creation(&mut units, None, ct);
        assert_eq!(ctx.task(ct).unwrap().state, DrawTaskState::Finished);
        ctx.layer_mut(child).unwrap().all_tasks_added = true;

        /*Next dispatch: the parent is visited first (its LAYER task is still
         *blocked), then the child's queue is cleaned and, being complete, it
         *releases the LAYER task and requests another dispatch*/
        ctx.dispatch_req = false;
        ctx.dispatch(&mut units, None);
        assert_eq!(ctx.task(lt).unwrap().state, DrawTaskState::Waiting);
        assert!(ctx.dispatch_req);
        /*That dispatch blends the layer*/
        ctx.dispatch(&mut units, None);
        assert_eq!(ctx.task(lt).unwrap().state, DrawTaskState::Finished);
        assert!(ctx.layer(child).is_some());
        /*And the one after that frees the LAYER task together with the child layer*/
        ctx.dispatch(&mut units, None);
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
        assert!(ctx.layer(child).is_none());
        assert_eq!(ctx.layer(layer).unwrap().next, None);
    }

    #[test]
    fn layer_deinit_cleans_tasks_and_dealloc_accounting_is_safe() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let _ = ctx.add_task(layer, &Area::new(0, 0, 9, 9), DrawTaskType::Fill);
        let _ = ctx.add_task(layer, &Area::new(0, 0, 9, 9), DrawTaskType::Fill);
        ctx.layer_deinit(layer);
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
        /*No buffer: dealloc is a no-op and the counter stays*/
        ctx.used_memory_for_layers = 7;
        ctx.layer_dealloc_buf(layer);
        assert_eq!(ctx.used_memory_for_layers, 7);
        assert_eq!(ctx.layer_go_to_xy(layer, 1, 1), None);
        /*Stale layers are ignored everywhere*/
        ctx.layer_delete(layer);
        assert!(ctx.layer(layer).is_none());
        ctx.layer_delete(layer);
        ctx.layer_deinit(layer);
        assert!(!ctx.layer_alloc_buf(layer));
        assert_eq!(ctx.get_next_available_task(layer, None, 1), None);
        assert_eq!(
            ctx.task_get_type(DrawTaskRef { layer, index: 0 }),
            DrawTaskType::None
        );
    }

    #[test]
    fn drop_shadow_layer_is_an_a8_child_grown_by_twice_the_radius() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut base = DrawDscBase::new();
        base.drop_shadow_blur_radius = 3;
        let ds = ctx
            .layer_create_drop_shadow(layer, &base, &Area::new(10, 20, 29, 39))
            .unwrap();
        let l = ctx.layer(ds).unwrap();
        assert_eq!(l.color_format, ColorFormat::A8);
        assert_eq!(l.parent, Some(layer));
        assert_eq!(l.buf_area, Area::new(4, 14, 35, 45));
        assert_eq!(l.clip_area, Area::new(4, 14, 35, 45));
        ctx.layer_delete(layer);
        assert!(
            ctx.layer_create_drop_shadow(layer, &base, &Area::new(0, 0, 1, 1))
                .is_none()
        );
    }

    #[test]
    fn ui_shadow_creation_notifies_units_and_inherits_parent_display() {
        use crate::core::UiConfig;
        let mut ui = Ui::new(UiConfig::default());
        let (unit, events) = MockUnit::recording(1);
        ui.draw_units.push(unit);
        ui.draw.unit_cnt = 1;
        let disp = DisplayId(Arena::<()>::new().insert(()));
        ui.disp_refresh = Some(disp);
        let area = Area::new(10, 10, 19, 19);
        let root = ui.draw_layer_create(None, ColorFormat::Xrgb8888, &area);
        let mut base = DrawDscBase::new();
        base.drop_shadow_blur_radius = 2;
        let child = rect::TaskSink::create_drop_shadow(&mut ui, root, &base, &area).unwrap();
        let shadow = ui.draw.layer(child).unwrap();
        assert_eq!(shadow.display, Some(disp));
        assert_eq!(shadow.parent, Some(root));
        assert_eq!(shadow.color_format, ColorFormat::A8);
        assert_eq!(shadow.buf_area, Area::new(6, 6, 23, 23));
        ui.draw_layer_delete(child);
        assert_eq!(
            *events.borrow(),
            vec![
                (EventCode::CHILD_CREATED, Some(root)),
                (EventCode::CHILD_CREATED, Some(child)),
                (EventCode::CHILD_DELETED, Some(child)),
            ]
        );
    }

    #[test]
    fn ui_wrappers_bind_root_layers_to_the_refreshing_display_and_notify_units() {
        use crate::core::UiConfig;
        let mut ui = Ui::new(UiConfig::default());
        let (unit, events) = MockUnit::recording(1);
        ui.draw_units.push(unit);
        ui.draw.unit_cnt = 1;
        /*DrawCtx never dereferences the display, any live key will do*/
        let disp = DisplayId(Arena::<()>::new().insert(()));
        ui.disp_refresh = Some(disp);

        let root = ui.draw_layer_create(None, ColorFormat::Rgb565, &Area::new(0, 0, 9, 9));
        let child = ui.draw_layer_create(Some(root), ColorFormat::Argb8888, &Area::new(0, 0, 4, 4));
        assert_eq!(ui.draw.layer(root).unwrap().display, Some(disp));
        assert_eq!(ui.draw.layer(child).unwrap().display, Some(disp));
        assert_eq!(ui.draw.layer(root).unwrap().next, Some(child));

        /*A layer created with an unknown display goes to its own chain*/
        ui.disp_refresh = None;
        let other = ui.draw_layer_create(None, ColorFormat::Rgb565, &Area::new(0, 0, 9, 9));
        assert_eq!(ui.draw.layer(other).unwrap().display, None);
        assert_eq!(ui.draw.layer(child).unwrap().next, None);

        /*Tasks on every chain are dispatched*/
        let t = ui
            .draw
            .add_task(other, &Area::new(0, 0, 1, 1), DrawTaskType::Fill);
        ui.draw.task_mut(t).unwrap().preferred_draw_unit_id = 1;
        let t2 = ui
            .draw
            .add_task(child, &Area::new(0, 0, 1, 1), DrawTaskType::Fill);
        ui.draw.task_mut(t2).unwrap().preferred_draw_unit_id = 1;
        ui.draw_dispatch();
        assert_eq!(ui.draw.task(t).unwrap().state, DrawTaskState::Finished);
        assert_eq!(ui.draw.task(t2).unwrap().state, DrawTaskState::Finished);

        ui.draw_layer_delete(child);
        ui.draw_layer_deinit(other);
        assert_eq!(ui.draw.layer(root).unwrap().next, None);

        /*CHILD_CREATED x3 then CHILD_DELETED x2 reached the unit*/
        assert_eq!(
            *events.borrow(),
            vec![
                (EventCode::CHILD_CREATED, Some(root)),
                (EventCode::CHILD_CREATED, Some(child)),
                (EventCode::CHILD_CREATED, Some(other)),
                (EventCode::CHILD_DELETED, Some(child)),
                (EventCode::CHILD_DELETED, Some(other)),
            ]
        );

        /*draw_unit_send_event filters on the unit name*/
        ui.draw_unit_send_event(Some("OTHER"), EventCode::REFR_START, EventParam::None);
        assert_eq!(events.borrow().len(), 5);
        ui.draw_unit_send_event(Some("MOCK"), EventCode::REFR_START, EventParam::None);
        ui.draw_unit_send_event(None, EventCode::REFR_READY, EventParam::None);
        assert_eq!(events.borrow().len(), 7);
        assert_eq!(events.borrow()[5], (EventCode::REFR_START, None));
        assert_eq!(events.borrow()[6], (EventCode::REFR_READY, None));
    }
}

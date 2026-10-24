//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_event.h`, `src/misc/lv_event_private.h`,
//! `src/misc/lv_event.c`, `include/lvgl/core/lv_obj_event.h` (hit test / cover check info).
//!
//! Event callbacks are stored in an arena inside [`Ui`] (never inside the
//! object tree) as `Rc<dyn Fn>`; dispatch snapshots the id list and clones
//! the `Rc` out before calling, so callbacks may add/remove callbacks or
//! delete their own target, exactly like in C.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::display::{DisplayId, ScreenLoadAnim};
use crate::draw::{DrawTaskRef, LayerId};
use crate::indev::IndevId;
use crate::misc::anim::AnimId;
use crate::misc::area::{Area, Point};
use crate::misc::arena::Key;
use crate::misc::style::{State, StyleProp};

/// `lv_result_t` (named `LvResult` to avoid shadowing `core::result::Result`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LvResult {
    /// `LV_RESULT_INVALID`: the object was deleted (became invalid) in the
    /// action or the operation failed.
    #[default]
    Invalid = 0,
    /// `LV_RESULT_OK`: the object is still valid after the action.
    Ok = 1,
}

/// `lv_event_code_t`: type of event.
///
/// Like the C enum this is an *open* value: the built-in codes are the
/// associated constants below (numbers identical to `lv_event.h`) and ids
/// returned by [`Ui::event_register_id`] (`LV_EVENT_LAST + 1 ..= LV_EVENT_LAST_CUSTOM`)
/// are plain `EventCode`s too, so they can be sent, filtered and stored
/// exactly like built-in ones. Constants can be used in `match` patterns.
///
/// `LV_EVENT_LAST == 80` matches the configuration the upstream reference
/// images are rendered with (`tests/configs/full.defconfig` sets
/// `CONFIG_LV_USE_TRANSLATION=y`, so `LV_EVENT_TRANSLATION_LANGUAGE_CHANGED`
/// exists); a default `lv_conf.h` build would have `LV_EVENT_LAST == 79`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct EventCode(pub u16);

impl EventCode {
    /// `LV_EVENT_ALL`
    pub const ALL: EventCode = EventCode(0);
    /// `LV_EVENT_PRESSED`: object has been pressed.
    pub const PRESSED: EventCode = EventCode(1);
    /// `LV_EVENT_PRESSING`: object is being pressed (sent continuously while pressing).
    pub const PRESSING: EventCode = EventCode(2);
    /// `LV_EVENT_PRESS_LOST`: object is still being pressed but slid cursor/finger off.
    pub const PRESS_LOST: EventCode = EventCode(3);
    /// `LV_EVENT_SHORT_CLICKED`: released before long press time, not scrolled.
    pub const SHORT_CLICKED: EventCode = EventCode(4);
    /// `LV_EVENT_SINGLE_CLICKED`
    pub const SINGLE_CLICKED: EventCode = EventCode(5);
    /// `LV_EVENT_DOUBLE_CLICKED`
    pub const DOUBLE_CLICKED: EventCode = EventCode(6);
    /// `LV_EVENT_TRIPLE_CLICKED`
    pub const TRIPLE_CLICKED: EventCode = EventCode(7);
    /// `LV_EVENT_LONG_PRESSED`
    pub const LONG_PRESSED: EventCode = EventCode(8);
    /// `LV_EVENT_LONG_PRESSED_REPEAT`
    pub const LONG_PRESSED_REPEAT: EventCode = EventCode(9);
    /// `LV_EVENT_CLICKED`: released without scrolling (regardless of long press).
    pub const CLICKED: EventCode = EventCode(10);
    /// `LV_EVENT_RELEASED`
    pub const RELEASED: EventCode = EventCode(11);
    /// `LV_EVENT_SCROLL_BEGIN`
    pub const SCROLL_BEGIN: EventCode = EventCode(12);
    /// `LV_EVENT_SCROLL_THROW_BEGIN`
    pub const SCROLL_THROW_BEGIN: EventCode = EventCode(13);
    /// `LV_EVENT_SCROLL_END`
    pub const SCROLL_END: EventCode = EventCode(14);
    /// `LV_EVENT_SCROLL`
    pub const SCROLL: EventCode = EventCode(15);
    /// `LV_EVENT_GESTURE`
    pub const GESTURE: EventCode = EventCode(16);
    /// `LV_EVENT_GESTURE_UP`
    pub const GESTURE_UP: EventCode = EventCode(17);
    /// `LV_EVENT_GESTURE_DOWN`
    pub const GESTURE_DOWN: EventCode = EventCode(18);
    /// `LV_EVENT_GESTURE_LEFT`
    pub const GESTURE_LEFT: EventCode = EventCode(19);
    /// `LV_EVENT_GESTURE_RIGHT`
    pub const GESTURE_RIGHT: EventCode = EventCode(20);
    /// `LV_EVENT_KEY`
    pub const KEY: EventCode = EventCode(21);
    /// `LV_EVENT_ROTARY`
    pub const ROTARY: EventCode = EventCode(22);
    /// `LV_EVENT_FOCUSED`
    pub const FOCUSED: EventCode = EventCode(23);
    /// `LV_EVENT_DEFOCUSED`
    pub const DEFOCUSED: EventCode = EventCode(24);
    /// `LV_EVENT_LEAVE`
    pub const LEAVE: EventCode = EventCode(25);
    /// `LV_EVENT_HIT_TEST`
    pub const HIT_TEST: EventCode = EventCode(26);
    /// `LV_EVENT_INDEV_RESET`
    pub const INDEV_RESET: EventCode = EventCode(27);
    /// `LV_EVENT_HOVER_OVER`
    pub const HOVER_OVER: EventCode = EventCode(28);
    /// `LV_EVENT_HOVER_LEAVE`
    pub const HOVER_LEAVE: EventCode = EventCode(29);
    /// `LV_EVENT_COVER_CHECK`
    pub const COVER_CHECK: EventCode = EventCode(30);
    /// `LV_EVENT_REFR_EXT_DRAW_SIZE`
    pub const REFR_EXT_DRAW_SIZE: EventCode = EventCode(31);
    /// `LV_EVENT_DRAW_MAIN_BEGIN`
    pub const DRAW_MAIN_BEGIN: EventCode = EventCode(32);
    /// `LV_EVENT_DRAW_MAIN`
    pub const DRAW_MAIN: EventCode = EventCode(33);
    /// `LV_EVENT_DRAW_MAIN_END`
    pub const DRAW_MAIN_END: EventCode = EventCode(34);
    /// `LV_EVENT_DRAW_POST_BEGIN`
    pub const DRAW_POST_BEGIN: EventCode = EventCode(35);
    /// `LV_EVENT_DRAW_POST`
    pub const DRAW_POST: EventCode = EventCode(36);
    /// `LV_EVENT_DRAW_POST_END`
    pub const DRAW_POST_END: EventCode = EventCode(37);
    /// `LV_EVENT_DRAW_TASK_ADDED`
    pub const DRAW_TASK_ADDED: EventCode = EventCode(38);
    /// `LV_EVENT_VALUE_CHANGED`
    pub const VALUE_CHANGED: EventCode = EventCode(39);
    /// `LV_EVENT_INSERT`
    pub const INSERT: EventCode = EventCode(40);
    /// `LV_EVENT_REFRESH`
    pub const REFRESH: EventCode = EventCode(41);
    /// `LV_EVENT_READY`
    pub const READY: EventCode = EventCode(42);
    /// `LV_EVENT_CANCEL`
    pub const CANCEL: EventCode = EventCode(43);
    /// `LV_EVENT_STATE_CHANGED`
    pub const STATE_CHANGED: EventCode = EventCode(44);
    /// `LV_EVENT_CHECKED`
    pub const CHECKED: EventCode = EventCode(45);
    /// `LV_EVENT_UNCHECKED`
    pub const UNCHECKED: EventCode = EventCode(46);
    /// `LV_EVENT_CREATE`
    pub const CREATE: EventCode = EventCode(47);
    /// `LV_EVENT_DELETE`
    pub const DELETE: EventCode = EventCode(48);
    /// `LV_EVENT_CHILD_CHANGED`
    pub const CHILD_CHANGED: EventCode = EventCode(49);
    /// `LV_EVENT_CHILD_CREATED`
    pub const CHILD_CREATED: EventCode = EventCode(50);
    /// `LV_EVENT_CHILD_DELETED`
    pub const CHILD_DELETED: EventCode = EventCode(51);
    /// `LV_EVENT_SCREEN_UNLOAD_START`
    pub const SCREEN_UNLOAD_START: EventCode = EventCode(52);
    /// `LV_EVENT_SCREEN_LOAD_START`
    pub const SCREEN_LOAD_START: EventCode = EventCode(53);
    /// `LV_EVENT_SCREEN_LOADED`
    pub const SCREEN_LOADED: EventCode = EventCode(54);
    /// `LV_EVENT_SCREEN_UNLOADED`
    pub const SCREEN_UNLOADED: EventCode = EventCode(55);
    /// `LV_EVENT_SIZE_CHANGED`
    pub const SIZE_CHANGED: EventCode = EventCode(56);
    /// `LV_EVENT_STYLE_CHANGED`
    pub const STYLE_CHANGED: EventCode = EventCode(57);
    /// `LV_EVENT_LAYOUT_CHANGED`
    pub const LAYOUT_CHANGED: EventCode = EventCode(58);
    /// `LV_EVENT_GET_SELF_SIZE`
    pub const GET_SELF_SIZE: EventCode = EventCode(59);
    /// `LV_EVENT_INVALIDATE_AREA`
    pub const INVALIDATE_AREA: EventCode = EventCode(60);
    /// `LV_EVENT_RESOLUTION_CHANGED`
    pub const RESOLUTION_CHANGED: EventCode = EventCode(61);
    /// `LV_EVENT_COLOR_FORMAT_CHANGED`
    pub const COLOR_FORMAT_CHANGED: EventCode = EventCode(62);
    /// `LV_EVENT_REFR_REQUEST`
    pub const REFR_REQUEST: EventCode = EventCode(63);
    /// `LV_EVENT_REFR_START`
    pub const REFR_START: EventCode = EventCode(64);
    /// `LV_EVENT_REFR_READY`
    pub const REFR_READY: EventCode = EventCode(65);
    /// `LV_EVENT_RENDER_START`
    pub const RENDER_START: EventCode = EventCode(66);
    /// `LV_EVENT_RENDER_READY`
    pub const RENDER_READY: EventCode = EventCode(67);
    /// `LV_EVENT_FLUSH_START`
    pub const FLUSH_START: EventCode = EventCode(68);
    /// `LV_EVENT_FLUSH_FINISH`
    pub const FLUSH_FINISH: EventCode = EventCode(69);
    /// `LV_EVENT_FLUSH_WAIT_START`
    pub const FLUSH_WAIT_START: EventCode = EventCode(70);
    /// `LV_EVENT_FLUSH_WAIT_FINISH`
    pub const FLUSH_WAIT_FINISH: EventCode = EventCode(71);
    /// `LV_EVENT_SYNC_START`
    pub const SYNC_START: EventCode = EventCode(72);
    /// `LV_EVENT_SYNC_FINISH`
    pub const SYNC_FINISH: EventCode = EventCode(73);
    /// `LV_EVENT_SYNC_WAIT_START`
    pub const SYNC_WAIT_START: EventCode = EventCode(74);
    /// `LV_EVENT_SYNC_WAIT_FINISH`
    pub const SYNC_WAIT_FINISH: EventCode = EventCode(75);
    /// `LV_EVENT_UPDATE_LAYOUT_COMPLETED`
    pub const UPDATE_LAYOUT_COMPLETED: EventCode = EventCode(76);
    /// `LV_EVENT_VSYNC`
    pub const VSYNC: EventCode = EventCode(77);
    /// `LV_EVENT_VSYNC_REQUEST`
    pub const VSYNC_REQUEST: EventCode = EventCode(78);
    /// `LV_EVENT_TRANSLATION_LANGUAGE_CHANGED` (`LV_USE_TRANSLATION`).
    pub const TRANSLATION_LANGUAGE_CHANGED: EventCode = EventCode(79);
    /// `LV_EVENT_LAST`: number of default events.
    pub const LAST: EventCode = EventCode(80);
    /// `LV_EVENT_LAST_CUSTOM`: last id usable for custom events.
    pub const LAST_CUSTOM: EventCode = EventCode(0x7FFF);

    /// `LV_EVENT_PREPROCESS`: flag ORed into a filter to call the callback
    /// before the class event handler.
    pub const PREPROCESS: u32 = 0x8000;
    /// `LV_EVENT_MARKED_DELETING`: flag set on a descriptor removed during dispatch.
    pub const MARKED_DELETING: u32 = 0x10000;
    /// Mask selecting the code inside a raw `filter` (`LV_EVENT_PREPROCESS - 1`).
    pub const CODE_MASK: u32 = 0x7FFF;

    /// Raw code.
    #[inline]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Wrap a raw code (the `(lv_event_code_t)` cast).
    #[inline]
    pub const fn from_u16(v: u16) -> EventCode {
        EventCode(v)
    }

    /// `true` for a code registered with `lv_event_register_id()`.
    #[inline]
    pub const fn is_custom(self) -> bool {
        self.0 > Self::LAST.0 && self.0 <= Self::LAST_CUSTOM.0
    }

    /// `lv_event_code_get_name()`
    pub fn name(self) -> &'static str {
        todo!()
    }
}

/// Filter of an event descriptor: `lv_event_code_t | LV_EVENT_PREPROCESS`.
///
/// Stored in [`EventDsc::filter`] as a raw `u32` exactly like upstream; this
/// is the typed view used by the `*_add_event_cb` APIs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventFilter {
    /// The code to match (`EventCode::ALL` matches everything).
    pub code: EventCode,
    /// `LV_EVENT_PREPROCESS`: run before the class handler.
    pub preprocess: bool,
}

impl EventFilter {
    /// Build a filter.
    pub const fn new(code: EventCode, preprocess: bool) -> Self {
        Self { code, preprocess }
    }

    /// The raw `uint32_t filter` (`code | LV_EVENT_PREPROCESS`).
    #[inline]
    pub const fn to_u32(self) -> u32 {
        self.code.0 as u32 | if self.preprocess { EventCode::PREPROCESS } else { 0 }
    }

    /// Decode a raw filter (ignores `LV_EVENT_MARKED_DELETING`).
    #[inline]
    pub const fn from_u32(filter: u32) -> Self {
        Self {
            code: EventCode((filter & EventCode::CODE_MASK) as u16),
            preprocess: filter & EventCode::PREPROCESS != 0,
        }
    }
}

impl From<EventCode> for EventFilter {
    fn from(code: EventCode) -> Self {
        EventFilter {
            code,
            preprocess: false,
        }
    }
}

/// `lv_event_cb_t`
pub type EventCallback = Rc<dyn for<'p> Fn(&mut Ui, &mut Event<'p>)>;

/// Handle of an event descriptor (`lv_event_dsc_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventDscId(pub Key);

/// `lv_event_dsc_t`: one registered callback.
pub struct EventDsc {
    /// `cb`
    pub cb: EventCallback,
    /// `user_data`
    pub user_data: Option<Rc<dyn Any>>,
    /// `filter`: `code | LV_EVENT_PREPROCESS | LV_EVENT_MARKED_DELETING`,
    /// raw like upstream so custom codes need no extra representation.
    pub filter: u32,
}

impl EventDsc {
    /// The code part of `filter`.
    #[inline]
    pub const fn code(&self) -> EventCode {
        EventCode((self.filter & EventCode::CODE_MASK) as u16)
    }

    /// `filter & LV_EVENT_PREPROCESS`
    #[inline]
    pub const fn is_preprocess(&self) -> bool {
        self.filter & EventCode::PREPROCESS != 0
    }

    /// `filter & LV_EVENT_MARKED_DELETING`
    #[inline]
    pub const fn is_marked_deleting(&self) -> bool {
        self.filter & EventCode::MARKED_DELETING != 0
    }
}

/// `lv_event_list_t`: callbacks attached to one object/display/indev.
#[derive(Clone, Debug, Default)]
pub struct EventList {
    /// `array`: descriptor ids in registration order.
    pub dscs: Vec<EventDscId>,
    /// `is_traversing`
    pub is_traversing: bool,
    /// `has_marked_deleting`
    pub has_marked_deleting: bool,
}

impl EventList {
    /// An empty list.
    pub const fn new() -> Self {
        Self {
            dscs: Vec::new(),
            is_traversing: false,
            has_marked_deleting: false,
        }
    }

    /// `lv_event_get_count()`
    #[inline]
    pub fn len(&self) -> usize {
        self.dscs.len()
    }

    /// `true` if no callback is registered.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dscs.is_empty()
    }
}

/// The object an event is sent to (`void * target`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum EventTarget {
    /// An object.
    Obj(Obj),
    /// A display.
    Display(DisplayId),
    /// An input device.
    Indev(IndevId),
    /// A draw unit, identified by its name (`lv_draw_unit_send_event()`).
    DrawUnit(&'static str),
}

impl EventTarget {
    /// The object if the target is one.
    #[inline]
    pub fn obj(self) -> Option<Obj> {
        match self {
            EventTarget::Obj(o) => Some(o),
            _ => None,
        }
    }
}

/// `lv_hit_test_info_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct HitTestInfo {
    /// `point`: where the object was pressed, in absolute coordinates.
    pub point: Point,
    /// `res`: `true` if the object was hit.
    pub res: bool,
}

/// `lv_cover_res_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CoverRes {
    /// `LV_COVER_RES_COVER`: the area is fully covered.
    #[default]
    Cover = 0,
    /// `LV_COVER_RES_NOT_COVER`: the area is not covered.
    NotCover = 1,
    /// `LV_COVER_RES_MASKED`: a mask is active, so no decision.
    Masked = 2,
}

/// `lv_cover_check_info_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CoverCheckInfo {
    /// `res`
    pub res: CoverRes,
    /// `area`: the area to check.
    pub area: Area,
}

/// Typed replacement of the `void * param` of an event.
pub enum EventParam<'p> {
    /// No parameter.
    None,
    /// A point (e.g. `LV_EVENT_PRESSED` original point).
    Point(Point),
    /// `LV_EVENT_KEY`: the key.
    Key(u32),
    /// `LV_EVENT_ROTARY`: the rotary diff.
    RotaryDiff(i32),
    /// Input-related events: the input device.
    Indev(IndevId),
    /// `LV_EVENT_SIZE_CHANGED` (old size), `LV_EVENT_INVALIDATE_AREA`, ...
    Area(Area),
    /// A mutable area (e.g. `LV_EVENT_INVALIDATE_AREA` on a display).
    AreaMut(&'p mut Area),
    /// `LV_EVENT_DRAW_*`: the layer to draw to.
    Layer(LayerId),
    /// `LV_EVENT_DRAW_TASK_ADDED`: the task.
    DrawTask(DrawTaskRef),
    /// `LV_EVENT_HIT_TEST`
    HitTest(&'p mut HitTestInfo),
    /// `LV_EVENT_COVER_CHECK`
    CoverCheck(&'p mut CoverCheckInfo),
    /// `LV_EVENT_GET_SELF_SIZE`
    SelfSize(&'p mut Point),
    /// `LV_EVENT_REFR_EXT_DRAW_SIZE`
    ExtDrawSize(&'p mut i32),
    /// `LV_EVENT_STYLE_CHANGED`: the changed property (or `Any`).
    StyleProp(StyleProp),
    /// `LV_EVENT_STATE_CHANGED`: the previous state.
    State(State),
    /// `LV_EVENT_CHILD_*`: the child.
    Obj(Obj),
    /// `LV_EVENT_SCROLL_*`: the scroll animation.
    Anim(AnimId),
    /// `LV_EVENT_INSERT`: inserted text.
    Text(&'p str),
    /// Screen load animation type.
    ScreenLoadAnim(ScreenLoadAnim),
    /// Any other data.
    Any(&'p dyn Any),
}

impl core::fmt::Debug for EventParam<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EventParam::None => f.write_str("None"),
            EventParam::Point(p) => f.debug_tuple("Point").field(p).finish(),
            EventParam::Key(k) => f.debug_tuple("Key").field(k).finish(),
            EventParam::RotaryDiff(d) => f.debug_tuple("RotaryDiff").field(d).finish(),
            EventParam::Indev(i) => f.debug_tuple("Indev").field(i).finish(),
            EventParam::Area(a) => f.debug_tuple("Area").field(a).finish(),
            EventParam::AreaMut(a) => f.debug_tuple("AreaMut").field(a).finish(),
            EventParam::Layer(l) => f.debug_tuple("Layer").field(l).finish(),
            EventParam::DrawTask(t) => f.debug_tuple("DrawTask").field(t).finish(),
            EventParam::HitTest(h) => f.debug_tuple("HitTest").field(h).finish(),
            EventParam::CoverCheck(c) => f.debug_tuple("CoverCheck").field(c).finish(),
            EventParam::SelfSize(p) => f.debug_tuple("SelfSize").field(p).finish(),
            EventParam::ExtDrawSize(s) => f.debug_tuple("ExtDrawSize").field(s).finish(),
            EventParam::StyleProp(p) => f.debug_tuple("StyleProp").field(p).finish(),
            EventParam::State(s) => f.debug_tuple("State").field(s).finish(),
            EventParam::Obj(o) => f.debug_tuple("Obj").field(o).finish(),
            EventParam::Anim(a) => f.debug_tuple("Anim").field(a).finish(),
            EventParam::Text(t) => f.debug_tuple("Text").field(t).finish(),
            EventParam::ScreenLoadAnim(a) => f.debug_tuple("ScreenLoadAnim").field(a).finish(),
            EventParam::Any(_) => f.write_str("Any(..)"),
        }
    }
}

/// One entry of the event stack: the safe-Rust form of the
/// `lv_event_t.prev` chain rooted at `lv_global_t::event_header`.
///
/// `lv_event_push()` pushes a frame, `lv_event_pop()` pops it and
/// `lv_event_mark_deleted()` walks the stack. `current_target` must be kept
/// in step with [`Event::current_target`] while bubbling / trickling.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct EventFrame {
    /// `original_target`
    pub original_target: EventTarget,
    /// `current_target`
    pub current_target: EventTarget,
    /// `deleted`
    pub deleted: bool,
}

/// `lv_event_t`
pub struct Event<'p> {
    /// `code`
    pub code: EventCode,
    /// `original_target`
    pub target: EventTarget,
    /// `current_target`: the object whose callback is running (differs from
    /// `target` while bubbling / trickling).
    pub current_target: EventTarget,
    /// `param`
    pub param: EventParam<'p>,
    /// `user_data` of the running callback.
    pub user_data: Option<Rc<dyn Any>>,
    /// `stop_processing`
    pub stop_processing: bool,
    /// `stop_bubbling`
    pub stop_bubbling: bool,
    /// `stop_trickling`
    pub stop_trickling: bool,
    /// `deleted`: the target was deleted during dispatch. `event_send`
    /// copies it back from the [`EventFrame`] after every callback.
    pub deleted: bool,
    /// `prev`: index of this event's frame in [`Ui::event_stack`] while it
    /// is pushed (`None` before `event_push` / after `event_pop`).
    pub frame: Option<usize>,
}

impl<'p> Event<'p> {
    /// `lv_event_get_target()`
    #[inline]
    pub fn target(&self) -> EventTarget {
        self.target
    }

    /// `lv_event_get_current_target()`
    #[inline]
    pub fn current_target(&self) -> EventTarget {
        self.current_target
    }

    /// `lv_event_get_target_obj()`
    #[inline]
    pub fn target_obj(&self) -> Option<Obj> {
        self.target.obj()
    }

    /// `lv_event_get_current_target_obj()`
    #[inline]
    pub fn current_target_obj(&self) -> Option<Obj> {
        self.current_target.obj()
    }

    /// `lv_event_get_code()`
    #[inline]
    pub fn code(&self) -> EventCode {
        self.code
    }

    /// `lv_event_get_param()`
    #[inline]
    pub fn param(&mut self) -> &mut EventParam<'p> {
        &mut self.param
    }

    /// `lv_event_get_user_data()`
    #[inline]
    pub fn user_data(&self) -> Option<&Rc<dyn Any>> {
        self.user_data.as_ref()
    }

    /// `lv_event_stop_bubbling()`
    #[inline]
    pub fn stop_bubbling(&mut self) {
        self.stop_bubbling = true;
    }

    /// `lv_event_stop_trickling()`
    #[inline]
    pub fn stop_trickling(&mut self) {
        self.stop_trickling = true;
    }

    /// `lv_event_stop_processing()`
    #[inline]
    pub fn stop_processing(&mut self) {
        self.stop_processing = true;
    }

    /// `lv_event_get_indev()`
    pub fn indev(&self) -> Option<IndevId> {
        todo!()
    }

    /// `lv_event_get_layer()`
    pub fn layer(&self) -> Option<LayerId> {
        todo!()
    }

    /// `lv_event_get_old_size()`
    pub fn old_size(&self) -> Option<&Area> {
        todo!()
    }

    /// `lv_event_get_key()`
    pub fn key(&self) -> u32 {
        todo!()
    }

    /// `lv_event_get_rotary_diff()`
    pub fn rotary_diff(&self) -> i32 {
        todo!()
    }

    /// `lv_event_get_scroll_anim()`
    pub fn scroll_anim(&self) -> Option<AnimId> {
        todo!()
    }

    /// `lv_event_set_ext_draw_size()`
    pub fn set_ext_draw_size(&mut self, size: i32) {
        todo!()
    }

    /// `lv_event_get_self_size_info()`
    pub fn self_size_info(&mut self) -> Option<&mut Point> {
        todo!()
    }

    /// `lv_event_get_hit_test_info()`
    pub fn hit_test_info(&mut self) -> Option<&mut HitTestInfo> {
        todo!()
    }

    /// `lv_event_get_cover_area()`
    pub fn cover_area(&self) -> Option<&Area> {
        todo!()
    }

    /// `lv_event_set_cover_res()`
    pub fn set_cover_res(&mut self, res: CoverRes) {
        todo!()
    }

    /// `lv_event_get_draw_task()`
    pub fn draw_task(&self) -> Option<DrawTaskRef> {
        todo!()
    }

    /// `lv_event_get_prev_state()`
    pub fn prev_state(&self) -> State {
        todo!()
    }

    /// `lv_event_get_invalidated_area()`
    pub fn invalidated_area(&mut self) -> Option<&mut Area> {
        todo!()
    }
}

impl Ui {
    /// `lv_event_add()`: register `cb` on `list`.
    pub fn event_add(
        &mut self,
        list: &mut EventList,
        cb: EventCallback,
        filter: EventFilter,
        user_data: Option<Rc<dyn Any>>,
    ) -> EventDscId {
        todo!()
    }

    /// `lv_event_remove_dsc()`
    pub fn event_remove_dsc(&mut self, list: &mut EventList, dsc: EventDscId) -> bool {
        todo!()
    }

    /// `lv_event_remove()`: remove the callback at `index`.
    pub fn event_remove(&mut self, list: &mut EventList, index: usize) -> bool {
        todo!()
    }

    /// `lv_event_remove_all()`
    pub fn event_remove_all(&mut self, list: &mut EventList) {
        todo!()
    }

    /// `lv_event_get_dsc()`
    pub fn event_get_dsc(&self, list: &EventList, index: usize) -> Option<&EventDsc> {
        todo!()
    }

    /// `lv_event_dsc_get_cb()`
    pub fn event_dsc_get_cb(&self, dsc: EventDscId) -> Option<EventCallback> {
        todo!()
    }

    /// `lv_event_dsc_get_user_data()`
    pub fn event_dsc_get_user_data(&self, dsc: EventDscId) -> Option<Rc<dyn Any>> {
        todo!()
    }

    /// `lv_event_send()`: call every matching callback of `list` on `e`.
    /// `preprocess` selects the `LV_EVENT_PREPROCESS` callbacks.
    ///
    /// The list is passed by value snapshot because the owner (object,
    /// display, indev) lives inside `self`.
    pub fn event_send(&mut self, list: EventList, e: &mut Event<'_>, preprocess: bool) -> LvResult {
        todo!()
    }

    /// `lv_event_push()`: push `e` on the event stack and record its frame.
    pub fn event_push(&mut self, e: &mut Event<'_>) {
        todo!()
    }

    /// `lv_event_pop()`
    pub fn event_pop(&mut self, e: &mut Event<'_>) {
        todo!()
    }

    /// `lv_event_push_and_send()`
    pub fn event_push_and_send(
        &mut self,
        list: EventList,
        code: EventCode,
        original_target: EventTarget,
        param: EventParam<'_>,
    ) -> LvResult {
        todo!()
    }

    /// `lv_event_register_id()`: a new custom event code
    /// (`EventCode::LAST` if `LAST_CUSTOM` is exhausted).
    pub fn event_register_id(&mut self) -> EventCode {
        todo!()
    }

    /// `lv_event_mark_deleted()`: mark every running event (every frame of
    /// [`Ui::event_stack`]) whose target is `target` as deleted.
    pub fn event_mark_deleted(&mut self, target: EventTarget) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_codes_match_upstream() {
        assert_eq!(EventCode::PRESSED.0, 1);
        assert_eq!(EventCode::CLICKED.0, 10);
        assert_eq!(EventCode::KEY.0, 21);
        assert_eq!(EventCode::COVER_CHECK.0, 30);
        assert_eq!(EventCode::DRAW_MAIN.0, 33);
        assert_eq!(EventCode::VALUE_CHANGED.0, 39);
        assert_eq!(EventCode::DELETE.0, 48);
        assert_eq!(EventCode::SIZE_CHANGED.0, 56);
        assert_eq!(EventCode::GET_SELF_SIZE.0, 59);
        assert_eq!(EventCode::UPDATE_LAYOUT_COMPLETED.0, 76);
        assert_eq!(EventCode::VSYNC_REQUEST.0, 78);
        assert_eq!(EventCode::TRANSLATION_LANGUAGE_CHANGED.0, 79);
        // Pinned to tests/configs/full.defconfig (LV_USE_TRANSLATION = 1).
        assert_eq!(EventCode::LAST.0, 80);
        assert_eq!(EventCode::LAST_CUSTOM.0, 0x7FFF);
        assert_eq!(EventCode::PREPROCESS, 0x8000);
        assert_eq!(EventCode::MARKED_DELETING, 0x10000);
        assert_eq!(EventCode::default(), EventCode::ALL);
        assert_eq!(CoverRes::Masked as u8, 2);
    }

    #[test]
    fn custom_codes_round_trip_through_filters() {
        let custom = EventCode(EventCode::LAST.0 + 1);
        assert!(custom.is_custom());
        assert!(!EventCode::LAST.is_custom());
        assert!(!EventCode::LAST_CUSTOM.is_custom() || EventCode::LAST_CUSTOM.0 == 0x7FFF);
        let f = EventFilter::new(custom, true);
        assert_eq!(f.to_u32(), 81 | 0x8000);
        assert_eq!(EventFilter::from_u32(f.to_u32() | EventCode::MARKED_DELETING), f);
        let f2: EventFilter = EventCode::CLICKED.into();
        assert_eq!(f2.to_u32(), 10);
        // Constants are usable as match patterns.
        let which = match custom {
            EventCode::CLICKED => "clicked",
            EventCode::LAST => "last",
            _ => "other",
        };
        assert_eq!(which, "other");
    }
}

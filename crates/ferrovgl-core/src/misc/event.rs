//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_event.h`, `src/misc/lv_event_private.h`,
//! `src/misc/lv_event.c`, `include/lvgl/core/lv_obj_event.h` (hit test / cover check info).
//!
//! Event callbacks are stored in an arena inside [`Ui`] (never inside the
//! object tree) as `Rc<dyn Fn>`; dispatch snapshots the id list and clones
//! the `Rc` out before calling, so callbacks may add/remove callbacks or
//! delete their own target, exactly like in C.

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
use crate::misc::arena::{Arena, Key};
use crate::misc::check_arg_failed;
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

    /// `lv_event_code_get_name()`: the name of the code without the `LV_`
    /// prefix (e.g. `"EVENT_CLICKED"`), `"EVENT_UNKNOWN"` for custom codes,
    /// `LV_EVENT_LAST` and `LV_EVENT_LAST_CUSTOM`.
    pub fn name(self) -> &'static str {
        // Remove the preprocess flag
        let code = EventCode(self.0 & !(EventCode::PREPROCESS as u16));

        match code {
            EventCode::ALL => "EVENT_ALL",

            // Input device events
            EventCode::PRESSED => "EVENT_PRESSED",
            EventCode::PRESSING => "EVENT_PRESSING",
            EventCode::PRESS_LOST => "EVENT_PRESS_LOST",
            EventCode::SHORT_CLICKED => "EVENT_SHORT_CLICKED",
            EventCode::SINGLE_CLICKED => "EVENT_SINGLE_CLICKED",
            EventCode::DOUBLE_CLICKED => "EVENT_DOUBLE_CLICKED",
            EventCode::TRIPLE_CLICKED => "EVENT_TRIPLE_CLICKED",
            EventCode::LONG_PRESSED => "EVENT_LONG_PRESSED",
            EventCode::LONG_PRESSED_REPEAT => "EVENT_LONG_PRESSED_REPEAT",
            EventCode::CLICKED => "EVENT_CLICKED",
            EventCode::RELEASED => "EVENT_RELEASED",
            EventCode::SCROLL_BEGIN => "EVENT_SCROLL_BEGIN",
            EventCode::SCROLL_THROW_BEGIN => "EVENT_SCROLL_THROW_BEGIN",
            EventCode::SCROLL_END => "EVENT_SCROLL_END",
            EventCode::SCROLL => "EVENT_SCROLL",
            EventCode::GESTURE => "EVENT_GESTURE",
            EventCode::GESTURE_UP => "EVENT_GESTURE_UP",
            EventCode::GESTURE_DOWN => "EVENT_GESTURE_DOWN",
            EventCode::GESTURE_LEFT => "EVENT_GESTURE_LEFT",
            EventCode::GESTURE_RIGHT => "EVENT_GESTURE_RIGHT",
            EventCode::KEY => "EVENT_KEY",
            EventCode::ROTARY => "EVENT_ROTARY",
            EventCode::FOCUSED => "EVENT_FOCUSED",
            EventCode::DEFOCUSED => "EVENT_DEFOCUSED",
            EventCode::LEAVE => "EVENT_LEAVE",
            EventCode::HIT_TEST => "EVENT_HIT_TEST",
            EventCode::INDEV_RESET => "EVENT_INDEV_RESET",
            EventCode::HOVER_OVER => "EVENT_HOVER_OVER",
            EventCode::HOVER_LEAVE => "EVENT_HOVER_LEAVE",

            // Drawing events
            EventCode::COVER_CHECK => "EVENT_COVER_CHECK",
            EventCode::REFR_EXT_DRAW_SIZE => "EVENT_REFR_EXT_DRAW_SIZE",
            EventCode::DRAW_MAIN_BEGIN => "EVENT_DRAW_MAIN_BEGIN",
            EventCode::DRAW_MAIN => "EVENT_DRAW_MAIN",
            EventCode::DRAW_MAIN_END => "EVENT_DRAW_MAIN_END",
            EventCode::DRAW_POST_BEGIN => "EVENT_DRAW_POST_BEGIN",
            EventCode::DRAW_POST => "EVENT_DRAW_POST",
            EventCode::DRAW_POST_END => "EVENT_DRAW_POST_END",
            EventCode::DRAW_TASK_ADDED => "EVENT_DRAW_TASK_ADDED",

            // Special events
            EventCode::VALUE_CHANGED => "EVENT_VALUE_CHANGED",
            EventCode::INSERT => "EVENT_INSERT",
            EventCode::REFRESH => "EVENT_REFRESH",
            EventCode::READY => "EVENT_READY",
            EventCode::CANCEL => "EVENT_CANCEL",
            EventCode::STATE_CHANGED => "EVENT_STATE_CHANGED",
            EventCode::CHECKED => "EVENT_CHECKED",
            EventCode::UNCHECKED => "EVENT_UNCHECKED",

            // Other events
            EventCode::CREATE => "EVENT_CREATE",
            EventCode::DELETE => "EVENT_DELETE",
            EventCode::CHILD_CHANGED => "EVENT_CHILD_CHANGED",
            EventCode::CHILD_CREATED => "EVENT_CHILD_CREATED",
            EventCode::CHILD_DELETED => "EVENT_CHILD_DELETED",
            EventCode::SCREEN_UNLOAD_START => "EVENT_SCREEN_UNLOAD_START",
            EventCode::SCREEN_LOAD_START => "EVENT_SCREEN_LOAD_START",
            EventCode::SCREEN_LOADED => "EVENT_SCREEN_LOADED",
            EventCode::SCREEN_UNLOADED => "EVENT_SCREEN_UNLOADED",
            EventCode::SIZE_CHANGED => "EVENT_SIZE_CHANGED",
            EventCode::STYLE_CHANGED => "EVENT_STYLE_CHANGED",
            EventCode::LAYOUT_CHANGED => "EVENT_LAYOUT_CHANGED",
            EventCode::GET_SELF_SIZE => "EVENT_GET_SELF_SIZE",
            EventCode::UPDATE_LAYOUT_COMPLETED => "EVENT_UPDATE_LAYOUT_COMPLETED",

            // Events of optional LVGL components
            EventCode::INVALIDATE_AREA => "EVENT_INVALIDATE_AREA",
            EventCode::RESOLUTION_CHANGED => "EVENT_RESOLUTION_CHANGED",
            EventCode::COLOR_FORMAT_CHANGED => "EVENT_COLOR_FORMAT_CHANGED",
            EventCode::REFR_REQUEST => "EVENT_REFR_REQUEST",
            EventCode::REFR_START => "EVENT_REFR_START",
            EventCode::REFR_READY => "EVENT_REFR_READY",
            EventCode::RENDER_START => "EVENT_RENDER_START",
            EventCode::RENDER_READY => "EVENT_RENDER_READY",
            EventCode::FLUSH_START => "EVENT_FLUSH_START",
            EventCode::FLUSH_FINISH => "EVENT_FLUSH_FINISH",
            EventCode::FLUSH_WAIT_START => "EVENT_FLUSH_WAIT_START",
            EventCode::FLUSH_WAIT_FINISH => "EVENT_FLUSH_WAIT_FINISH",
            EventCode::SYNC_START => "EVENT_SYNC_START",
            EventCode::SYNC_FINISH => "EVENT_SYNC_FINISH",
            EventCode::SYNC_WAIT_START => "EVENT_SYNC_WAIT_START",
            EventCode::SYNC_WAIT_FINISH => "EVENT_SYNC_WAIT_FINISH",

            EventCode::VSYNC => "EVENT_VSYNC",
            EventCode::VSYNC_REQUEST => "EVENT_VSYNC_REQUEST",

            // LV_USE_TRANSLATION
            EventCode::TRANSLATION_LANGUAGE_CHANGED => "EVENT_TRANSLATION_LANGUAGE_CHANGED",

            // Special event flags (`LV_EVENT_LAST`, `LV_EVENT_LAST_CUSTOM`)
            // and custom codes.
            _ => "EVENT_UNKNOWN",
        }
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

    /// `lv_event_get_code()`: the code without the `LV_EVENT_PREPROCESS` flag.
    #[inline]
    pub fn code(&self) -> EventCode {
        EventCode(self.code.0 & !(EventCode::PREPROCESS as u16))
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

    /// `lv_event_get_indev()`: the input device of an input related event.
    pub fn indev(&self) -> Option<IndevId> {
        const INDEV_CODES: [EventCode; 22] = [
            EventCode::PRESSED,
            EventCode::PRESSING,
            EventCode::PRESS_LOST,
            EventCode::SHORT_CLICKED,
            EventCode::LONG_PRESSED,
            EventCode::LONG_PRESSED_REPEAT,
            EventCode::CLICKED,
            EventCode::RELEASED,
            EventCode::SCROLL_BEGIN,
            EventCode::SCROLL_END,
            EventCode::SCROLL,
            EventCode::GESTURE,
            EventCode::GESTURE_UP,
            EventCode::GESTURE_DOWN,
            EventCode::GESTURE_LEFT,
            EventCode::GESTURE_RIGHT,
            EventCode::KEY,
            EventCode::FOCUSED,
            EventCode::DEFOCUSED,
            EventCode::LEAVE,
            EventCode::HOVER_OVER,
            EventCode::HOVER_LEAVE,
        ];
        if !check_event_code(event_code_in_array(self.code, &INDEV_CODES)) {
            return None;
        }
        match self.param {
            EventParam::Indev(i) => Some(i),
            _ => None,
        }
    }

    /// `lv_event_get_layer()`: the layer of a `LV_EVENT_DRAW_*` event.
    pub fn layer(&self) -> Option<LayerId> {
        const DRAW_CODES: [EventCode; 6] = [
            EventCode::DRAW_MAIN,
            EventCode::DRAW_MAIN_BEGIN,
            EventCode::DRAW_MAIN_END,
            EventCode::DRAW_POST,
            EventCode::DRAW_POST_BEGIN,
            EventCode::DRAW_POST_END,
        ];
        if !check_event_code(event_code_in_array(self.code, &DRAW_CODES)) {
            return None;
        }
        match self.param {
            EventParam::Layer(l) => Some(l),
            _ => None,
        }
    }

    /// `lv_event_get_old_size()`: the old area of `LV_EVENT_SIZE_CHANGED`.
    pub fn old_size(&self) -> Option<&Area> {
        if !check_event_code(self.code == EventCode::SIZE_CHANGED) {
            return None;
        }
        match &self.param {
            EventParam::Area(a) => Some(a),
            EventParam::AreaMut(a) => Some(&**a),
            _ => None,
        }
    }

    /// `lv_event_get_key()`: the key of `LV_EVENT_KEY` (0 if none).
    pub fn key(&self) -> u32 {
        if !check_event_code(self.code == EventCode::KEY) {
            return 0;
        }
        match self.param {
            EventParam::Key(k) => k,
            _ => 0,
        }
    }

    /// `lv_event_get_rotary_diff()`: the diff of `LV_EVENT_ROTARY` (0 if none).
    pub fn rotary_diff(&self) -> i32 {
        if !check_event_code(self.code == EventCode::ROTARY) {
            return 0;
        }
        match self.param {
            EventParam::RotaryDiff(r) => r,
            _ => 0,
        }
    }

    /// `lv_event_get_scroll_anim()`: the animation of `LV_EVENT_SCROLL_BEGIN`.
    pub fn scroll_anim(&self) -> Option<AnimId> {
        if !check_event_code(self.code == EventCode::SCROLL_BEGIN) {
            return None;
        }
        match self.param {
            EventParam::Anim(a) => Some(a),
            _ => None,
        }
    }

    /// `lv_event_set_ext_draw_size()`: keep the larger of the current and
    /// the new extra draw size in `LV_EVENT_REFR_EXT_DRAW_SIZE`.
    pub fn set_ext_draw_size(&mut self, size: i32) {
        if !check_event_code(self.code == EventCode::REFR_EXT_DRAW_SIZE) {
            return;
        }
        if let EventParam::ExtDrawSize(cur_size) = &mut self.param {
            **cur_size = core::cmp::max(**cur_size, size);
        }
    }

    /// `lv_event_get_self_size_info()`: the point to fill in `LV_EVENT_GET_SELF_SIZE`.
    pub fn self_size_info(&mut self) -> Option<&mut Point> {
        if !check_event_code(self.code == EventCode::GET_SELF_SIZE) {
            return None;
        }
        match &mut self.param {
            EventParam::SelfSize(p) => Some(&mut **p),
            _ => None,
        }
    }

    /// `lv_event_get_hit_test_info()`: the info of `LV_EVENT_HIT_TEST`.
    pub fn hit_test_info(&mut self) -> Option<&mut HitTestInfo> {
        if !check_event_code(self.code == EventCode::HIT_TEST) {
            return None;
        }
        match &mut self.param {
            EventParam::HitTest(h) => Some(&mut **h),
            _ => None,
        }
    }

    /// `lv_event_get_cover_area()`: the area to check in `LV_EVENT_COVER_CHECK`.
    pub fn cover_area(&self) -> Option<&Area> {
        if !check_event_code(self.code == EventCode::COVER_CHECK) {
            return None;
        }
        match &self.param {
            EventParam::CoverCheck(p) => Some(&p.area),
            _ => None,
        }
    }

    /// `lv_event_set_cover_res()`: save only "stronger" results in
    /// `LV_EVENT_COVER_CHECK`.
    pub fn set_cover_res(&mut self, res: CoverRes) {
        if !check_event_code(self.code == EventCode::COVER_CHECK) {
            return;
        }
        if let EventParam::CoverCheck(p) = &mut self.param {
            // Save only "stronger" results
            if res as u8 > p.res as u8 {
                p.res = res;
            }
        }
    }

    /// `lv_event_get_draw_task()`: the task of `LV_EVENT_DRAW_TASK_ADDED`.
    pub fn draw_task(&self) -> Option<DrawTaskRef> {
        if !check_event_code(self.code == EventCode::DRAW_TASK_ADDED) {
            return None;
        }
        match self.param {
            EventParam::DrawTask(t) => Some(t),
            _ => None,
        }
    }

    /// `lv_event_get_prev_state()`: the previous state in
    /// `LV_EVENT_STATE_CHANGED` (empty if none).
    pub fn prev_state(&self) -> State {
        if !check_event_code(self.code == EventCode::STATE_CHANGED) {
            return State::empty();
        }
        match self.param {
            EventParam::State(s) => s,
            _ => State::empty(),
        }
    }

    /// `lv_event_get_invalidated_area()`: the area of a display's
    /// `LV_EVENT_INVALIDATE_AREA` (may be modified).
    pub fn invalidated_area(&mut self) -> Option<&mut Area> {
        if !check_event_code(self.code == EventCode::INVALIDATE_AREA) {
            return None;
        }
        match &mut self.param {
            EventParam::AreaMut(a) => Some(&mut **a),
            _ => None,
        }
    }
}

/// `LV_CHECK_ARG_FORMAT_MSG(<code check>, return ..., "invalid event code")`.
#[inline]
fn check_event_code(ok: bool) -> bool {
    if !ok {
        check_arg_failed("invalid event code");
    }
    ok
}

/// `event_code_in_array()` (`src/core/lv_obj_event.c`).
fn event_code_in_array(code: EventCode, arr: &[EventCode]) -> bool {
    for &c in arr {
        if code == c {
            return true;
        }
    }
    false
}

/// `lv_event_free_user_data_cb()`: an event callback releasing the event's
/// `user_data`.
///
/// Upstream calls `lv_free()` on the pointer. Here `user_data` is an `Rc`
/// shared with the descriptor, so this drops the event's reference; the
/// data itself is released when the last reference (usually the
/// descriptor's) goes away.
pub fn event_free_user_data_cb(_ui: &mut Ui, e: &mut Event<'_>) {
    let p = e.user_data.take();
    drop(p);
}

/// A zeroed `lv_event_t` (`lv_memzero(&e, sizeof(e))` + the fields set by
/// `lv_event_push_and_send()`).
fn event_new<'p>(code: EventCode, original_target: EventTarget, param: EventParam<'p>) -> Event<'p> {
    Event {
        code,
        target: original_target,
        current_target: original_target,
        param,
        user_data: None,
        stop_processing: false,
        stop_bubbling: false,
        stop_trickling: false,
        deleted: false,
        frame: None,
    }
}

impl Ui {
    /// `lv_event_add()`: register `cb` on `list`.
    ///
    /// The descriptor is stored in [`Ui::event_dscs`]; `list` only keeps its
    /// id. Like upstream, adding never triggers a cleanup and a callback
    /// added while `list` is being traversed is not called by that dispatch.
    pub fn event_add(
        &mut self,
        list: &mut EventList,
        cb: EventCallback,
        filter: EventFilter,
        user_data: Option<Rc<dyn Any>>,
    ) -> EventDscId {
        let dsc = EventDsc {
            cb,
            user_data,
            filter: filter.to_u32(),
        };
        let id = EventDscId(self.event_dscs.insert(dsc));
        list.dscs.push(id);
        id
    }

    /// `lv_event_remove_dsc()`: remove `dsc` from `list`. Returns `false`
    /// if `dsc` is not in `list`.
    ///
    /// While `list` is traversed the descriptor is only marked and the
    /// dispatch loop frees it when it finishes.
    pub fn event_remove_dsc(&mut self, list: &mut EventList, dsc: EventDscId) -> bool {
        let size = list.dscs.len();
        for i in 0..size {
            let event = list.dscs[i];
            if event == dsc {
                event_mark_deleting(&mut self.event_dscs, list, event);
                cleanup_event_list(&mut self.event_dscs, list);
                return true;
            }
        }

        false
    }

    /// `lv_event_get_dsc()`: the descriptor at `index`.
    pub fn event_get_dsc(&self, list: &EventList, index: usize) -> Option<&EventDsc> {
        let dsc = list.dscs.get(index)?;
        self.event_dscs.get(dsc.0)
    }

    /// `lv_event_dsc_get_cb()`
    pub fn event_dsc_get_cb(&self, dsc: EventDscId) -> Option<EventCallback> {
        self.event_dscs.get(dsc.0).map(|d| d.cb.clone())
    }

    /// `lv_event_dsc_get_user_data()`
    pub fn event_dsc_get_user_data(&self, dsc: EventDscId) -> Option<Rc<dyn Any>> {
        self.event_dscs.get(dsc.0).and_then(|d| d.user_data.clone())
    }

    /// `lv_event_remove()`: remove the callback at `index`. Returns `false`
    /// if there is no descriptor at `index`.
    pub fn event_remove(&mut self, list: &mut EventList, index: usize) -> bool {
        let Some(&dsc) = list.dscs.get(index) else {
            return false;
        };
        event_mark_deleting(&mut self.event_dscs, list, dsc);
        cleanup_event_list(&mut self.event_dscs, list);
        true
    }

    /// `lv_event_remove_all()`
    pub fn event_remove_all(&mut self, list: &mut EventList) {
        let size = list.dscs.len();
        for i in 0..size {
            let dsc = list.dscs[i];
            event_mark_deleting(&mut self.event_dscs, list, dsc);
        }

        cleanup_event_list(&mut self.event_dscs, list);
    }

    /// `lv_event_send()`: call every matching callback of `list` on `e`.
    /// `preprocess` selects the `LV_EVENT_PREPROCESS` callbacks.
    ///
    /// The list is passed by value snapshot because the owner (object,
    /// display, indev) lives inside `self`. Like every upstream caller,
    /// `list` must be (a clone of) the event list of `e.current_target`,
    /// which must stay in place in its owner: the traversal state
    /// (`is_traversing`, deferred removal of descriptors marked while
    /// traversing) is then kept on that live list, so callbacks removing
    /// callbacks, nesting sends or deleting the target behave exactly like
    /// upstream. A list not owned by `e.current_target` is traversed as a
    /// standalone list (its cleanup happens on the snapshot).
    ///
    /// Returns [`LvResult::Invalid`] if the target was deleted
    /// (`e.deleted`), [`LvResult::Ok`] otherwise.
    pub fn event_send(&mut self, list: EventList, e: &mut Event<'_>, preprocess: bool) -> LvResult {
        let mut list = list;
        self.event_send_list(&mut list, e, preprocess)
    }

    /// Body of [`Ui::event_send`] working on `local`, which is refreshed
    /// from the live list when the dispatch ends so a following pass (see
    /// `lv_event_push_and_send()`) sees callbacks added meanwhile.
    fn event_send_list(&mut self, local: &mut EventList, e: &mut Event<'_>, preprocess: bool) -> LvResult {
        self.event_frame_sync(e);
        if e.deleted {
            return LvResult::Invalid;
        }

        let target = e.current_target;
        // `local` is the live list of the target when the ids are the same.
        let live = matches!(self.event_parts(target).1, Some(l) if l.dscs == local.dscs);

        // When obj is deleted in its own event, the list in the object is
        // released, so back up the header (the ids) in advance to be able to
        // release the descriptors.
        let mut back_array_head = local.dscs.clone();

        // Dealing with the problem of nested event deletion event
        let is_traversing = {
            let (_, live_list) = self.event_parts(target);
            let list = match live_list.filter(|_| live) {
                Some(l) => l,
                None => &mut *local,
            };
            let is_traversing = list.is_traversing;
            list.is_traversing = true;
            is_traversing
        };

        let mut res = LvResult::Ok;
        // Removal is deferred while traversing and additions are appended,
        // so the first `size` ids of the live list are the backed up ones.
        let size = back_array_head.len();
        let mut i = 0;
        while i < size && !e.deleted {
            let id = back_array_head[i];
            i += 1;
            // `dsc->cb == NULL`: a descriptor freed outside this list.
            let Some(dsc) = self.event_dscs.get(id.0) else {
                continue;
            };
            if dsc.is_marked_deleting() {
                continue;
            }
            let is_preprocessed = (dsc.filter & EventCode::PREPROCESS) != 0;
            if is_preprocessed != preprocess {
                continue;
            }
            let filter = dsc.filter & !EventCode::PREPROCESS;
            if filter == EventCode::ALL.0 as u32 || filter == e.code.0 as u32 {
                e.user_data = dsc.user_data.clone();
                let cb = dsc.cb.clone();
                cb(self, e);
                self.event_frame_sync(e);
                if e.stop_processing {
                    break;
                }

                // Stop if the object is deleted
                if e.deleted {
                    res = LvResult::Invalid;
                    break;
                }
            }
        }

        if is_traversing {
            self.event_refresh_local(live, target, local);
            return res;
        }

        if e.deleted {
            cleanup_event_list_core(&mut self.event_dscs, &mut back_array_head);
        } else {
            let (dscs, live_list) = self.event_parts(target);
            match live_list.filter(|_| live) {
                Some(list) => {
                    list.is_traversing = false;
                    cleanup_event_list(dscs, list);
                }
                None if live => {
                    // The owner disappeared without the event being marked
                    // deleted (no frame pushed): release like the deleted case.
                    cleanup_event_list_core(dscs, &mut back_array_head);
                }
                None => {
                    local.is_traversing = false;
                    cleanup_event_list(dscs, local);
                }
            }
        }

        self.event_refresh_local(live, target, local);
        res
    }

    /// Copy the live list of `target` back into `local` after a dispatch.
    fn event_refresh_local(&mut self, live: bool, target: EventTarget, local: &mut EventList) {
        if !live {
            return;
        }
        if let (_, Some(l)) = self.event_parts(target) {
            local.clone_from(l);
        }
    }

    /// The descriptor arena and the live event list owned by `target`
    /// (`obj->spec_attr->event_list`, `disp->event_list`,
    /// `indev->event_list`), borrowed together.
    fn event_parts(&mut self, target: EventTarget) -> (&mut Arena<EventDsc>, Option<&mut EventList>) {
        let list = match target {
            EventTarget::Obj(obj) => self
                .objs
                .get_mut(obj.0.0)
                .and_then(|o| o.spec.as_deref_mut())
                .map(|s| &mut s.event_list),
            EventTarget::Display(disp) => self.displays.get_mut(disp.0).map(|d| &mut d.event_list),
            EventTarget::Indev(indev) => self.indevs.get_mut(indev.0).map(|i| &mut i.event_list),
            EventTarget::DrawUnit(_) => None,
        };
        (&mut self.event_dscs, list)
    }

    /// Keep `e` and its frame of [`Ui::event_stack`] in step (in C they are
    /// the same memory): the frame follows the targets of `e`, and `deleted`
    /// set on either side is visible on both.
    fn event_frame_sync(&mut self, e: &mut Event<'_>) {
        let Some(frame) = e.frame.and_then(|i| self.event_stack.get_mut(i)) else {
            return;
        };
        frame.original_target = e.target;
        frame.current_target = e.current_target;
        if frame.deleted {
            e.deleted = true;
        } else if e.deleted {
            frame.deleted = true;
        }
    }

    /// `lv_event_push()`: push `e` on the event stack and record its frame.
    ///
    /// Builds a simple linked list (here a stack) from the events in flight:
    /// it's important to know if the target was deleted by a nested event
    /// called from an event callback.
    pub fn event_push(&mut self, e: &mut Event<'_>) {
        self.event_stack.push(EventFrame {
            original_target: e.target,
            current_target: e.current_target,
            deleted: e.deleted,
        });
        e.frame = Some(self.event_stack.len() - 1);
    }

    /// `lv_event_pop()`: `event_head = e->prev`.
    pub fn event_pop(&mut self, e: &mut Event<'_>) {
        if let Some(idx) = e.frame
            && let Some(frame) = self.event_stack.get(idx)
        {
            if frame.deleted {
                e.deleted = true;
            }
            self.event_stack.truncate(idx);
        }
        e.frame = None;
    }

    /// `lv_event_push_and_send()`: build an event, push it, run the
    /// preprocess callbacks and then (unless processing was stopped or the
    /// target deleted) the normal callbacks of `list`.
    pub fn event_push_and_send(
        &mut self,
        list: EventList,
        code: EventCode,
        original_target: EventTarget,
        param: EventParam<'_>,
    ) -> LvResult {
        let mut list = list;
        let mut e = event_new(code, original_target, param);

        self.event_push(&mut e);
        let mut res = self.event_send_list(&mut list, &mut e, true);
        if !(res != LvResult::Ok || e.stop_processing) {
            res = self.event_send_list(&mut list, &mut e, false);
        }

        self.event_pop(&mut e);
        res
    }

    /// `lv_event_register_id()`: a new custom event code
    /// (`EventCode::LAST` if `LAST_CUSTOM` is exhausted).
    pub fn event_register_id(&mut self) -> EventCode {
        if self.event_last_register_id.wrapping_add(1) > EventCode::LAST_CUSTOM.0 as u32 {
            #[cfg(feature = "log")]
            log::error!("No more event ids can be registered");
            return EventCode::LAST;
        }

        self.event_last_register_id = self.event_last_register_id.wrapping_add(1);
        EventCode(self.event_last_register_id as u16)
    }

    /// `lv_event_mark_deleted()`: mark every running event (every frame of
    /// [`Ui::event_stack`]) whose target is `target` as deleted.
    pub fn event_mark_deleted(&mut self, target: EventTarget) {
        for e in self.event_stack.iter_mut().rev() {
            if e.original_target == target || e.current_target == target {
                e.deleted = true;
            }
        }
    }
}

/// `cleanup_event_list_core()`: free the descriptors marked for deletion
/// and compact `array`.
fn cleanup_event_list_core(dscs: &mut Arena<EventDsc>, array: &mut Vec<EventDscId>) {
    let size = array.len();
    let mut kept_count = 0;
    for i in 0..size {
        let dsc_i = array[i];
        if event_is_marked_deleting(dscs, dsc_i) {
            dscs.remove(dsc_i.0);
        } else {
            array[kept_count] = dsc_i;
            kept_count += 1;
        }
    }

    if kept_count == 0 {
        *array = Vec::new();
    } else {
        array.truncate(kept_count);
    }
}

/// `cleanup_event_list()`: traverse the list to delete the descriptors
/// marked for deletion (not while it is being traversed).
fn cleanup_event_list(dscs: &mut Arena<EventDsc>, list: &mut EventList) {
    if list.is_traversing {
        return;
    }
    if !list.has_marked_deleting {
        return;
    }

    cleanup_event_list_core(dscs, &mut list.dscs);

    list.has_marked_deleting = false;
}

/// `event_mark_deleting()`
fn event_mark_deleting(dscs: &mut Arena<EventDsc>, list: &mut EventList, dsc: EventDscId) {
    list.has_marked_deleting = true;
    if let Some(d) = dscs.get_mut(dsc.0) {
        d.filter |= EventCode::MARKED_DELETING;
    }
}

/// `event_is_marked_deleting()`. An id whose descriptor was already freed
/// (only possible for a stale snapshot) counts as marked.
fn event_is_marked_deleting(dscs: &Arena<EventDsc>, dsc: EventDscId) -> bool {
    dscs.get(dsc.0).is_none_or(|d| d.is_marked_deleting())
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
    // ---------------------------------------------------------------------
    // Helpers: objects are built directly in the arena (the obj-level APIs
    // of lv_obj_event.c are ported elsewhere), with a `spec_attr` so that
    // `event_send` finds their live list through the event target.
    // ---------------------------------------------------------------------

    use alloc::boxed::Box;
    use core::cell::Cell;

    use crate::core::obj::{OBJ_CLASS, ObjData, ObjFlags, ObjId, SpecAttr};
    use crate::core::ui::UiConfig;

    fn new_ui() -> Ui {
        Ui::new(UiConfig::default())
    }

    fn make_obj(ui: &mut Ui) -> Obj {
        let key = ui.objs.insert(ObjData {
            class: &OBJ_CLASS,
            parent: None,
            spec: Some(Box::new(SpecAttr::new())),
            styles: Vec::new(),
            style_main_prop_is_set: 0,
            style_other_prop_is_set: 0,
            user_data: None,
            coords: Area::default(),
            flags: ObjFlags::empty(),
            radio_button: false,
            state: State::empty(),
            layout_inv: false,
            readjust_scroll_after_layout: false,
            scr_layout_inv: false,
            skip_trans: false,
            h_layout: false,
            w_layout: false,
            h_ignore_size: false,
            w_ignore_size: false,
            is_deleting: false,
            rendered: false,
            has_blur: false,
            widget: Box::new(()),
        });
        Obj(ObjId(key))
    }

    /// Run `f` on the live list of `obj` (taken out and put back, the way
    /// the `lv_obj_*_event*` wrappers access `obj->spec_attr->event_list`).
    fn with_list<R>(ui: &mut Ui, obj: Obj, f: impl FnOnce(&mut Ui, &mut EventList) -> R) -> R {
        let spec = ui.objs[obj.0.0].spec.as_mut().unwrap();
        let mut list = core::mem::take(&mut spec.event_list);
        let r = f(ui, &mut list);
        ui.objs[obj.0.0].spec.as_mut().unwrap().event_list = list;
        r
    }

    fn list_of(ui: &Ui, obj: Obj) -> EventList {
        ui.objs[obj.0.0].spec.as_ref().unwrap().event_list.clone()
    }

    fn count(ui: &Ui, obj: Obj) -> usize {
        ui.objs[obj.0.0].spec.as_ref().unwrap().event_list.len()
    }

    fn add(ui: &mut Ui, obj: Obj, cb: EventCallback, filter: impl Into<EventFilter>) -> EventDscId {
        let filter = filter.into();
        with_list(ui, obj, |ui, l| ui.event_add(l, cb, filter, None))
    }

    /// Like `lv_obj_send_event()` without the class handler.
    fn send(ui: &mut Ui, obj: Obj, code: EventCode) -> LvResult {
        let list = list_of(ui, obj);
        ui.event_push_and_send(list, code, EventTarget::Obj(obj), EventParam::None)
    }

    /// Like `lv_obj_delete()` as far as the event module is concerned.
    fn delete(ui: &mut Ui, obj: Obj) {
        with_list(ui, obj, |ui, l| ui.event_remove_all(l));
        ui.event_mark_deleted(EventTarget::Obj(obj));
        ui.objs.remove(obj.0.0);
    }

    fn counter_cb(cnt: &Rc<Cell<u32>>) -> EventCallback {
        let cnt = cnt.clone();
        Rc::new(move |_ui: &mut Ui, _e: &mut Event<'_>| cnt.set(cnt.get() + 1))
    }

    fn noop_cb() -> EventCallback {
        Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {})
    }

    // ---------------------------------------------------------------------
    // lv_event.c
    // ---------------------------------------------------------------------

    #[test]
    fn code_get_name() {
        assert_eq!(EventCode::ALL.name(), "EVENT_ALL");
        assert_eq!(EventCode::PRESSED.name(), "EVENT_PRESSED");
        assert_eq!(EventCode::HOVER_LEAVE.name(), "EVENT_HOVER_LEAVE");
        assert_eq!(EventCode::DRAW_TASK_ADDED.name(), "EVENT_DRAW_TASK_ADDED");
        assert_eq!(
            EventCode::UPDATE_LAYOUT_COMPLETED.name(),
            "EVENT_UPDATE_LAYOUT_COMPLETED"
        );
        assert_eq!(EventCode::VSYNC_REQUEST.name(), "EVENT_VSYNC_REQUEST");
        assert_eq!(
            EventCode::TRANSLATION_LANGUAGE_CHANGED.name(),
            "EVENT_TRANSLATION_LANGUAGE_CHANGED"
        );
        // The preprocess flag is removed.
        assert_eq!(EventCode(EventCode::CLICKED.0 | 0x8000).name(), "EVENT_CLICKED");
        // Special values and custom codes.
        assert_eq!(EventCode::LAST.name(), "EVENT_UNKNOWN");
        assert_eq!(EventCode::LAST_CUSTOM.name(), "EVENT_UNKNOWN");
        assert_eq!(EventCode(EventCode::LAST.0 + 1).name(), "EVENT_UNKNOWN");
        // Every built-in code has its own name.
        for c in 0..EventCode::LAST.0 {
            let n = EventCode(c).name();
            assert_ne!(n, "EVENT_UNKNOWN", "code {c}");
            for d in 0..c {
                assert_ne!(EventCode(d).name(), n);
            }
        }
    }

    /// `test_event_register_id_stops_at_last_custom`
    #[test]
    fn register_id_stops_at_last_custom() {
        let mut ui = new_ui();
        assert_eq!(ui.event_register_id(), EventCode(EventCode::LAST.0 + 1));
        assert_eq!(ui.event_register_id(), EventCode(EventCode::LAST.0 + 2));

        ui.event_last_register_id = EventCode::LAST_CUSTOM.0 as u32 - 1;
        assert_eq!(ui.event_register_id(), EventCode::LAST_CUSTOM);

        assert_eq!(ui.event_register_id(), EventCode::LAST);
        assert_eq!(ui.event_register_id(), EventCode::LAST);
        assert_eq!(ui.event_last_register_id, EventCode::LAST_CUSTOM.0 as u32);
    }

    /// `test_event_remove_event_dsc` (on the event lists directly).
    #[test]
    fn remove_event_dsc() {
        let mut ui = new_ui();
        let obj = make_obj(&mut ui);

        let dsc_1 = add(&mut ui, obj, noop_cb(), EventCode::CLICKED);
        let dsc_2 = add(&mut ui, obj, noop_cb(), EventCode::PRESSED);
        assert_eq!(count(&ui, obj), 2);

        assert!(with_list(&mut ui, obj, |ui, l| ui.event_remove_dsc(l, dsc_1)));
        assert_eq!(count(&ui, obj), 1);
        assert_eq!(list_of(&ui, obj).dscs[0], dsc_2);
        let l = list_of(&ui, obj);
        assert_eq!(ui.event_get_dsc(&l, 0).unwrap().code(), EventCode::PRESSED);

        // Removing the same descriptor twice must fail
        assert!(!with_list(&mut ui, obj, |ui, l| ui.event_remove_dsc(l, dsc_1)));
        assert_eq!(count(&ui, obj), 1);

        assert!(with_list(&mut ui, obj, |ui, l| ui.event_remove_dsc(l, dsc_2)));
        assert_eq!(count(&ui, obj), 0);

        // A descriptor of another widget doesn't belong to this list
        let other = make_obj(&mut ui);
        let dsc_other = add(&mut ui, other, noop_cb(), EventCode::CLICKED);
        assert!(!with_list(&mut ui, obj, |ui, l| ui.event_remove_dsc(l, dsc_other)));
        assert_eq!(count(&ui, other), 1);

        delete(&mut ui, other);
        delete(&mut ui, obj);
        assert!(ui.event_dscs.is_empty());
    }

    /// `test_event_remove_event_cb` / `test_event_should_not_memory_lean`
    #[test]
    fn remove_and_remove_all_free_descriptors() {
        let mut ui = new_ui();
        let obj = make_obj(&mut ui);
        let cb = noop_cb();
        add(&mut ui, obj, cb.clone(), EventCode::CLICKED);
        add(&mut ui, obj, noop_cb(), EventCode::ALL);
        add(&mut ui, obj, cb.clone(), EventCode::PRESSED);
        assert_eq!(count(&ui, obj), 3);
        assert_eq!(ui.event_dscs.len(), 3);

        // lv_obj_remove_event_cb(): from the last index down.
        let mut removed = 0;
        with_list(&mut ui, obj, |ui, l| {
            for i in (0..l.len()).rev() {
                let same = ui.event_get_dsc(l, i).is_some_and(|d| Rc::ptr_eq(&d.cb, &cb));
                if same {
                    assert!(ui.event_remove(l, i));
                    removed += 1;
                }
            }
        });
        assert_eq!(removed, 2);
        assert_eq!(count(&ui, obj), 1);
        assert_eq!(ui.event_dscs.len(), 1);
        assert!(!with_list(&mut ui, obj, |ui, l| ui.event_remove(l, 1)));

        for _ in 0..10 {
            add(&mut ui, obj, noop_cb(), EventCode::ALL);
        }
        assert_eq!(ui.event_dscs.len(), 11);
        with_list(&mut ui, obj, |ui, l| ui.event_remove_all(l));
        assert_eq!(count(&ui, obj), 0);
        assert!(ui.event_dscs.is_empty());
        assert!(!list_of(&ui, obj).has_marked_deleting);
    }

    #[test]
    fn dsc_getters() {
        let mut ui = new_ui();
        let mut list = EventList::new();
        let cb = noop_cb();
        let data: Rc<dyn Any> = Rc::new(7u32);
        let id = ui.event_add(
            &mut list,
            cb.clone(),
            EventFilter::new(EventCode::KEY, true),
            Some(data.clone()),
        );
        assert!(Rc::ptr_eq(&ui.event_dsc_get_cb(id).unwrap(), &cb));
        let ud = ui.event_dsc_get_user_data(id).unwrap();
        assert_eq!(ud.downcast_ref::<u32>(), Some(&7));
        let dsc = ui.event_get_dsc(&list, 0).unwrap();
        assert_eq!(dsc.filter, EventCode::KEY.0 as u32 | EventCode::PREPROCESS);
        assert!(dsc.is_preprocess());
        assert!(ui.event_get_dsc(&list, 1).is_none());
        assert!(ui.event_remove_dsc(&mut list, id));
        assert!(ui.event_dsc_get_cb(id).is_none());
        assert!(ui.event_dsc_get_user_data(id).is_none());
    }

    /// `test_event_stop_processing` (the class handler / indev parts are
    /// replaced by a direct `lv_event_push_and_send()`).
    #[test]
    fn stop_processing() {
        let mut ui = new_ui();
        let btn = make_obj(&mut ui);
        let pre_1 = Rc::new(Cell::new(0u32));
        let pre_2 = Rc::new(Cell::new(0u32));
        let post_1 = Rc::new(Cell::new(0u32));
        let post_2 = Rc::new(Cell::new(0u32));
        let pre_stop_1 = Rc::new(Cell::new(false));
        let post_stop_1 = Rc::new(Cell::new(false));

        let (c, s) = (pre_1.clone(), pre_stop_1.clone());
        let cb: EventCallback = Rc::new(move |_ui: &mut Ui, e: &mut Event<'_>| {
            c.set(c.get() + 1);
            if s.get() {
                e.stop_processing();
            }
        });
        add(&mut ui, btn, cb, EventFilter::new(EventCode::CLICKED, true));
        add(
            &mut ui,
            btn,
            counter_cb(&pre_2),
            EventFilter::new(EventCode::CLICKED, true),
        );
        let (c, s) = (post_1.clone(), post_stop_1.clone());
        let cb: EventCallback = Rc::new(move |_ui: &mut Ui, e: &mut Event<'_>| {
            c.set(c.get() + 1);
            if s.get() {
                e.stop_processing();
            }
        });
        add(&mut ui, btn, cb, EventCode::CLICKED);
        add(&mut ui, btn, counter_cb(&post_2), EventCode::CLICKED);

        let reset = |pre: bool, post: bool| {
            for c in [&pre_1, &pre_2, &post_1, &post_2] {
                c.set(0);
            }
            pre_stop_1.set(pre);
            post_stop_1.set(post);
        };
        let get = || [pre_1.get(), pre_2.get(), post_1.get(), post_2.get()];

        reset(false, false);
        assert_eq!(send(&mut ui, btn, EventCode::CLICKED), LvResult::Ok);
        assert_eq!(get(), [1, 1, 1, 1]);

        reset(true, false);
        assert_eq!(send(&mut ui, btn, EventCode::CLICKED), LvResult::Ok);
        assert_eq!(get(), [1, 0, 0, 0]);

        reset(false, true);
        assert_eq!(send(&mut ui, btn, EventCode::CLICKED), LvResult::Ok);
        assert_eq!(get(), [1, 1, 1, 0]);

        // Other codes reach none of them.
        reset(false, false);
        assert_eq!(send(&mut ui, btn, EventCode::PRESSED), LvResult::Ok);
        assert_eq!(get(), [0, 0, 0, 0]);
        assert!(ui.event_stack.is_empty());
    }

    #[test]
    fn filter_all_matches_every_code_and_user_data_is_passed() {
        let mut ui = new_ui();
        let obj = make_obj(&mut ui);
        let seen = Rc::new(core::cell::RefCell::new(Vec::new()));
        let s = seen.clone();
        let cb: EventCallback = Rc::new(move |_ui: &mut Ui, e: &mut Event<'_>| {
            let ud = e.user_data().and_then(|d| d.downcast_ref::<u32>()).copied();
            s.borrow_mut().push((e.code(), ud));
        });
        let ud: Rc<dyn Any> = Rc::new(5u32);
        with_list(&mut ui, obj, |ui, l| {
            ui.event_add(l, cb.clone(), EventCode::ALL.into(), Some(ud.clone()));
            ui.event_add(l, cb.clone(), EventCode::KEY.into(), None);
            ui.event_add(l, cb.clone(), EventFilter::new(EventCode::ALL, true), None);
        });
        let custom = ui.event_register_id();
        send(&mut ui, obj, EventCode::KEY);
        send(&mut ui, obj, custom);
        assert_eq!(
            *seen.borrow(),
            [
                (EventCode::KEY, None),
                (EventCode::KEY, Some(5)),
                (EventCode::KEY, None),
                (custom, None),
                (custom, Some(5)),
            ]
        );
    }

    #[test]
    fn removal_during_dispatch_is_deferred() {
        let mut ui = new_ui();
        let obj = make_obj(&mut ui);
        let b_cnt = Rc::new(Cell::new(0u32));
        let b_id = Rc::new(Cell::new(None::<EventDscId>));
        let inner_count = Rc::new(Cell::new(0usize));

        let (bid, ic) = (b_id.clone(), inner_count.clone());
        let cb_a: EventCallback = Rc::new(move |ui: &mut Ui, e: &mut Event<'_>| {
            let obj = e.current_target_obj().unwrap();
            let id = bid.get().unwrap();
            assert!(with_list(ui, obj, |ui, l| ui.event_remove_dsc(l, id)));
            // Still in the list (only marked) while it is being traversed.
            ic.set(count(ui, obj));
            assert!(ui.event_dscs[id.0].is_marked_deleting());
            // A callback added now is not called by this dispatch.
            add(ui, obj, noop_cb(), EventCode::ALL);
        });
        add(&mut ui, obj, cb_a, EventCode::CLICKED);
        b_id.set(Some(add(&mut ui, obj, counter_cb(&b_cnt), EventCode::CLICKED)));

        assert_eq!(send(&mut ui, obj, EventCode::CLICKED), LvResult::Ok);
        assert_eq!(b_cnt.get(), 0);
        assert_eq!(inner_count.get(), 2);
        // Cleaned up after the traversal: A and the added no-op remain.
        assert_eq!(count(&ui, obj), 2);
        assert_eq!(ui.event_dscs.len(), 2);
        let l = list_of(&ui, obj);
        assert!(!l.is_traversing && !l.has_marked_deleting);
    }

    /// Two removals of index 0 during a traversal hit the same (marked)
    /// descriptor, like upstream.
    #[test]
    fn remove_by_index_during_dispatch_keeps_indices() {
        let mut ui = new_ui();
        let obj = make_obj(&mut ui);
        let b_cnt = Rc::new(Cell::new(0u32));
        let cb: EventCallback = Rc::new(move |ui: &mut Ui, e: &mut Event<'_>| {
            let obj = e.current_target_obj().unwrap();
            assert!(with_list(ui, obj, |ui, l| ui.event_remove(l, 0)));
            assert!(with_list(ui, obj, |ui, l| ui.event_remove(l, 0)));
        });
        add(&mut ui, obj, cb, EventCode::CLICKED);
        add(&mut ui, obj, counter_cb(&b_cnt), EventCode::CLICKED);
        send(&mut ui, obj, EventCode::CLICKED);
        assert_eq!(b_cnt.get(), 1);
        assert_eq!(count(&ui, obj), 1);
        assert_eq!(ui.event_dscs.len(), 1);
    }

    /// `test_event_delete_obj_in_recursive_event_call`
    #[test]
    fn delete_obj_in_recursive_event_call() {
        let mut ui = new_ui();
        let obj = make_obj(&mut ui);
        let click_count = Rc::new(Cell::new(0u32));
        let cc = click_count.clone();
        let cb: EventCallback = Rc::new(move |ui: &mut Ui, e: &mut Event<'_>| {
            let obj = e.target_obj().unwrap();
            cc.set(cc.get() + 1);

            if cc.get() == 10 {
                with_list(ui, obj, |ui, l| ui.event_remove(l, 0));
            } else if cc.get() == 15 {
                delete(ui, obj);
            } else {
                send(ui, obj, EventCode::CLICKED);
            }
        });
        add(&mut ui, obj, cb.clone(), EventCode::CLICKED);
        add(&mut ui, obj, cb, EventCode::CLICKED);

        assert_eq!(send(&mut ui, obj, EventCode::CLICKED), LvResult::Invalid);
        assert_eq!(click_count.get(), 15);
        assert!(!ui.objs.contains(obj.0.0));
        // No leaked descriptors and no dangling frames.
        assert!(ui.event_dscs.is_empty());
        assert!(ui.event_stack.is_empty());
    }

    #[test]
    fn deleted_event_is_not_sent_again() {
        let mut ui = new_ui();
        let obj = make_obj(&mut ui);
        let cnt = Rc::new(Cell::new(0u32));
        add(&mut ui, obj, counter_cb(&cnt), EventCode::ALL);
        let list = list_of(&ui, obj);
        let mut e = event_new(EventCode::CLICKED, EventTarget::Obj(obj), EventParam::None);
        ui.event_push(&mut e);
        ui.event_mark_deleted(EventTarget::Obj(obj));
        assert_eq!(ui.event_send(list, &mut e, false), LvResult::Invalid);
        assert!(e.deleted);
        assert_eq!(cnt.get(), 0);
        ui.event_pop(&mut e);
        assert!(ui.event_stack.is_empty());
        assert_eq!(e.frame, None);
    }

    #[test]
    fn push_pop_and_mark_deleted() {
        let mut ui = new_ui();
        let a = make_obj(&mut ui);
        let b = make_obj(&mut ui);
        let mut e1 = event_new(EventCode::CLICKED, EventTarget::Obj(a), EventParam::None);
        let mut e2 = event_new(EventCode::PRESSED, EventTarget::Obj(b), EventParam::None);
        let mut e3 = event_new(EventCode::KEY, EventTarget::Obj(b), EventParam::None);
        e3.current_target = EventTarget::Obj(a);
        ui.event_push(&mut e1);
        ui.event_push(&mut e2);
        ui.event_push(&mut e3);
        assert_eq!((e1.frame, e2.frame, e3.frame), (Some(0), Some(1), Some(2)));

        ui.event_mark_deleted(EventTarget::Obj(a));
        let deleted: Vec<bool> = ui.event_stack.iter().map(|f| f.deleted).collect();
        // Matched by original or current target.
        assert_eq!(deleted, [true, false, true]);

        ui.event_pop(&mut e3);
        assert!(e3.deleted);
        ui.event_pop(&mut e2);
        assert!(!e2.deleted);
        assert_eq!(ui.event_stack.len(), 1);
        ui.event_pop(&mut e1);
        assert!(e1.deleted);
        assert!(ui.event_stack.is_empty());
    }

    #[test]
    fn standalone_list_without_owner() {
        let mut ui = new_ui();
        let mut list = EventList::new();
        let cnt = Rc::new(Cell::new(0u32));
        ui.event_add(&mut list, counter_cb(&cnt), EventCode::ALL.into(), None);
        let id = ui.event_add(&mut list, counter_cb(&cnt), EventCode::READY.into(), None);
        let target = EventTarget::DrawUnit("sw");
        let res = ui.event_push_and_send(list.clone(), EventCode::READY, target, EventParam::None);
        assert_eq!(res, LvResult::Ok);
        assert_eq!(cnt.get(), 2);

        // A stale descriptor in a snapshot is skipped.
        let snapshot = list.clone();
        assert!(ui.event_remove_dsc(&mut list, id));
        let res = ui.event_push_and_send(snapshot, EventCode::READY, target, EventParam::None);
        assert_eq!(res, LvResult::Ok);
        assert_eq!(cnt.get(), 3);
        assert_eq!(ui.event_dscs.len(), 1);
    }

    #[test]
    fn nested_send_on_same_list() {
        let mut ui = new_ui();
        let obj = make_obj(&mut ui);
        let depth = Rc::new(Cell::new(0u32));
        let d = depth.clone();
        let cb: EventCallback = Rc::new(move |ui: &mut Ui, e: &mut Event<'_>| {
            let obj = e.current_target_obj().unwrap();
            // The live list is flagged while traversing.
            assert!(list_of(ui, obj).is_traversing);
            d.set(d.get() + 1);
            if d.get() < 3 {
                send(ui, obj, EventCode::VALUE_CHANGED);
                // The outer traversal is still running.
                assert!(list_of(ui, obj).is_traversing);
            }
        });
        add(&mut ui, obj, cb, EventCode::VALUE_CHANGED);
        assert_eq!(send(&mut ui, obj, EventCode::VALUE_CHANGED), LvResult::Ok);
        assert_eq!(depth.get(), 3);
        assert!(!list_of(&ui, obj).is_traversing);
    }

    #[test]
    fn callbacks_added_in_preprocess_run_in_the_normal_pass() {
        let mut ui = new_ui();
        let obj = make_obj(&mut ui);
        let cnt = Rc::new(Cell::new(0u32));
        let c = cnt.clone();
        let cb: EventCallback = Rc::new(move |ui: &mut Ui, e: &mut Event<'_>| {
            let obj = e.current_target_obj().unwrap();
            add(ui, obj, counter_cb(&c), EventCode::CLICKED);
        });
        add(&mut ui, obj, cb, EventFilter::new(EventCode::CLICKED, true));
        send(&mut ui, obj, EventCode::CLICKED);
        // lv_event_push_and_send() reads the list again for the second pass.
        assert_eq!(cnt.get(), 1);
        assert_eq!(count(&ui, obj), 2);
    }

    #[test]
    fn free_user_data_cb_drops_the_event_reference() {
        let mut ui = new_ui();
        let mut list = EventList::new();
        let data: Rc<dyn Any> = Rc::new(1u8);
        ui.event_add(
            &mut list,
            Rc::new(event_free_user_data_cb),
            EventCode::DELETE.into(),
            Some(data.clone()),
        );
        ui.event_push_and_send(
            list.clone(),
            EventCode::DELETE,
            EventTarget::DrawUnit("x"),
            EventParam::None,
        );
        assert_eq!(Rc::strong_count(&data), 2);
        ui.event_remove_all(&mut list);
        assert_eq!(Rc::strong_count(&data), 1);
    }

    // ---------------------------------------------------------------------
    // lv_event_get_* parameter accessors (lv_obj_event.c, lv_display.c)
    // ---------------------------------------------------------------------

    fn ev<'p>(code: EventCode, param: EventParam<'p>) -> Event<'p> {
        event_new(code, EventTarget::DrawUnit("t"), param)
    }

    #[test]
    fn get_code_removes_preprocess_flag() {
        let e = ev(EventCode(EventCode::CLICKED.0 | 0x8000), EventParam::None);
        assert_eq!(e.code(), EventCode::CLICKED);
    }

    #[test]
    fn param_accessors() {
        assert_eq!(ev(EventCode::KEY, EventParam::Key(13)).key(), 13);
        assert_eq!(ev(EventCode::KEY, EventParam::None).key(), 0);
        assert_eq!(
            ev(EventCode::ROTARY, EventParam::RotaryDiff(-3)).rotary_diff(),
            -3
        );
        let st = State::PRESSED;
        assert_eq!(
            ev(EventCode::STATE_CHANGED, EventParam::State(st)).prev_state(),
            st
        );
        let area = Area::new(1, 2, 3, 4);
        let e = ev(EventCode::SIZE_CHANGED, EventParam::Area(area));
        assert_eq!(e.old_size(), Some(&area));

        let mut ext = 5;
        let mut e = ev(EventCode::REFR_EXT_DRAW_SIZE, EventParam::ExtDrawSize(&mut ext));
        e.set_ext_draw_size(3);
        e.set_ext_draw_size(9);
        e.set_ext_draw_size(7);
        drop(e);
        assert_eq!(ext, 9);

        let mut info = CoverCheckInfo {
            res: CoverRes::Cover,
            area,
        };
        let mut e = ev(EventCode::COVER_CHECK, EventParam::CoverCheck(&mut info));
        assert_eq!(e.cover_area(), Some(&area));
        e.set_cover_res(CoverRes::Masked);
        // Only "stronger" results are saved.
        e.set_cover_res(CoverRes::NotCover);
        drop(e);
        assert_eq!(info.res, CoverRes::Masked);

        let mut hit = HitTestInfo {
            point: Point::new(1, 1),
            res: true,
        };
        let mut e = ev(EventCode::HIT_TEST, EventParam::HitTest(&mut hit));
        e.hit_test_info().unwrap().res = false;
        drop(e);
        assert!(!hit.res);

        let mut p = Point::new(0, 0);
        let mut e = ev(EventCode::GET_SELF_SIZE, EventParam::SelfSize(&mut p));
        *e.self_size_info().unwrap() = Point::new(10, 20);
        drop(e);
        assert_eq!(p, Point::new(10, 20));

        let mut inv = area;
        let mut e = ev(EventCode::INVALIDATE_AREA, EventParam::AreaMut(&mut inv));
        e.invalidated_area().unwrap().x1 = 0;
        drop(e);
        assert_eq!(inv.x1, 0);
    }

    /// With checks that only log (no `check-arg-panic`), a wrong code
    /// returns the neutral value like upstream's `LV_CHECK_ARG`.
    #[cfg(not(feature = "check-arg-panic"))]
    #[test]
    fn param_accessors_check_the_code() {
        assert_eq!(ev(EventCode::PRESSED, EventParam::Key(13)).key(), 0);
        assert_eq!(ev(EventCode::KEY, EventParam::RotaryDiff(2)).rotary_diff(), 0);
        assert_eq!(
            ev(EventCode::PRESSED, EventParam::State(State::PRESSED)).prev_state(),
            State::empty()
        );
        let area = Area::new(1, 2, 3, 4);
        assert!(
            ev(EventCode::CLICKED, EventParam::Area(area))
                .old_size()
                .is_none()
        );
        // SINGLE_CLICKED is not in upstream's indev code table.
        let mut e = ev(EventCode::SINGLE_CLICKED, EventParam::None);
        assert!(e.indev().is_none());
        e.code = EventCode::DRAW_TASK_ADDED;
        assert!(e.layer().is_none());
        let mut ext = 1;
        let mut e = ev(EventCode::DRAW_MAIN, EventParam::ExtDrawSize(&mut ext));
        e.set_ext_draw_size(10);
        drop(e);
        assert_eq!(ext, 1);
    }

    #[cfg(feature = "check-arg-panic")]
    #[test]
    #[should_panic(expected = "invalid event code")]
    fn param_accessors_check_the_code_panics() {
        let _ = ev(EventCode::PRESSED, EventParam::Key(13)).key();
    }
}

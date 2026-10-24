//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_obj.h`, `src/core/lv_obj_private.h`,
//! `src/core/lv_obj_class_private.h`, `src/core/lv_obj.c`.
//!
//! Objects live in [`Ui::objs`]; [`Obj`] is a `Copy` handle. Widget specific
//! state is a [`WidgetState`] trait object inside [`ObjData`], queried
//! COM-style by `TypeId` to mimic C struct embedding.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::{Any, TypeId};

use crate::core::Ui;
use crate::core::obj_style::ObjStyle;
use crate::display::ScreenLoadAnim;
use crate::group::GroupId;
use crate::misc::area::{Area, Dir, Point};
use crate::misc::arena::Key;
use crate::misc::event::{Event, EventCode, EventList};
use crate::misc::style::State;

/// Handle of an object slot in [`Ui::objs`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjId(pub Key);

/// `lv_obj_t *`: a `Copy` handle to an object. Widget handles are newtypes
/// over `Obj`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Obj(pub ObjId);

impl From<ObjId> for Obj {
    fn from(id: ObjId) -> Self {
        Obj(id)
    }
}

impl From<Obj> for ObjId {
    fn from(o: Obj) -> Self {
        o.0
    }
}

impl Obj {
    /// The underlying id.
    #[inline]
    pub const fn id(self) -> ObjId {
        self.0
    }
}

bitflags::bitflags! {
    /// `lv_obj_flag_t`
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct ObjFlags: u32 {
        /// `LV_OBJ_FLAG_HIDDEN`
        const HIDDEN = 1 << 0;
        /// `LV_OBJ_FLAG_CLICKABLE`
        const CLICKABLE = 1 << 1;
        /// `LV_OBJ_FLAG_CLICK_FOCUSABLE`
        const CLICK_FOCUSABLE = 1 << 2;
        /// `LV_OBJ_FLAG_CHECKABLE`
        const CHECKABLE = 1 << 3;
        /// `LV_OBJ_FLAG_SCROLLABLE`
        const SCROLLABLE = 1 << 4;
        /// `LV_OBJ_FLAG_SCROLL_ELASTIC`
        const SCROLL_ELASTIC = 1 << 5;
        /// `LV_OBJ_FLAG_SCROLL_MOMENTUM`
        const SCROLL_MOMENTUM = 1 << 6;
        /// `LV_OBJ_FLAG_SCROLL_ONE`
        const SCROLL_ONE = 1 << 7;
        /// `LV_OBJ_FLAG_SCROLL_CHAIN_HOR`
        const SCROLL_CHAIN_HOR = 1 << 8;
        /// `LV_OBJ_FLAG_SCROLL_CHAIN_VER`
        const SCROLL_CHAIN_VER = 1 << 9;
        /// `LV_OBJ_FLAG_SCROLL_CHAIN`
        const SCROLL_CHAIN = Self::SCROLL_CHAIN_HOR.bits() | Self::SCROLL_CHAIN_VER.bits();
        /// `LV_OBJ_FLAG_SCROLL_ON_FOCUS`
        const SCROLL_ON_FOCUS = 1 << 10;
        /// `LV_OBJ_FLAG_SCROLL_WITH_ARROW`
        const SCROLL_WITH_ARROW = 1 << 11;
        /// `LV_OBJ_FLAG_SNAPPABLE`
        const SNAPPABLE = 1 << 12;
        /// `LV_OBJ_FLAG_PRESS_LOCK`
        const PRESS_LOCK = 1 << 13;
        /// `LV_OBJ_FLAG_EVENT_BUBBLE`
        const EVENT_BUBBLE = 1 << 14;
        /// `LV_OBJ_FLAG_GESTURE_BUBBLE`
        const GESTURE_BUBBLE = 1 << 15;
        /// `LV_OBJ_FLAG_ADV_HITTEST`
        const ADV_HITTEST = 1 << 16;
        /// `LV_OBJ_FLAG_IGNORE_LAYOUT`
        const IGNORE_LAYOUT = 1 << 17;
        /// `LV_OBJ_FLAG_FLOATING`
        const FLOATING = 1 << 18;
        /// `LV_OBJ_FLAG_SEND_DRAW_TASK_EVENTS`
        const SEND_DRAW_TASK_EVENTS = 1 << 19;
        /// `LV_OBJ_FLAG_OVERFLOW_VISIBLE`
        const OVERFLOW_VISIBLE = 1 << 20;
        /// `LV_OBJ_FLAG_EVENT_TRICKLE`
        const EVENT_TRICKLE = 1 << 21;
        /// `LV_OBJ_FLAG_STATE_TRICKLE`
        const STATE_TRICKLE = 1 << 22;
        /// `LV_OBJ_FLAG_LAYOUT_1`
        const LAYOUT_1 = 1 << 23;
        /// `LV_OBJ_FLAG_LAYOUT_2`
        const LAYOUT_2 = 1 << 24;
        /// `LV_OBJ_FLAG_FLEX_IN_NEW_TRACK` (alias of `LAYOUT_1`).
        const FLEX_IN_NEW_TRACK = 1 << 23;
        /// `LV_OBJ_FLAG_WIDGET_1`
        const WIDGET_1 = 1 << 25;
        /// `LV_OBJ_FLAG_WIDGET_2`
        const WIDGET_2 = 1 << 26;
        /// `LV_OBJ_FLAG_USER_1`
        const USER_1 = 1 << 27;
        /// `LV_OBJ_FLAG_USER_2`
        const USER_2 = 1 << 28;
        /// `LV_OBJ_FLAG_USER_3`
        const USER_3 = 1 << 29;
        /// `LV_OBJ_FLAG_USER_4`
        const USER_4 = 1 << 30;
    }
}

/// `lv_scrollbar_mode_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ScrollbarMode {
    /// `LV_SCROLLBAR_MODE_OFF`
    Off = 0,
    /// `LV_SCROLLBAR_MODE_ON`
    On = 1,
    /// `LV_SCROLLBAR_MODE_ACTIVE`
    Active = 2,
    /// `LV_SCROLLBAR_MODE_AUTO`
    #[default]
    Auto = 3,
}

/// `lv_scroll_snap_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ScrollSnap {
    /// `LV_SCROLL_SNAP_NONE`
    #[default]
    None = 0,
    /// `LV_SCROLL_SNAP_START`
    Start = 1,
    /// `LV_SCROLL_SNAP_END`
    End = 2,
    /// `LV_SCROLL_SNAP_CENTER`
    Center = 3,
}

/// `lv_layer_type_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum LayerType {
    /// `LV_LAYER_TYPE_NONE`
    #[default]
    None = 0,
    /// `LV_LAYER_TYPE_SIMPLE`
    Simple = 1,
    /// `LV_LAYER_TYPE_TRANSFORM`
    Transform = 2,
}

/// `lv_obj_class_editable_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObjClassEditable {
    /// `LV_OBJ_CLASS_EDITABLE_INHERIT`
    #[default]
    Inherit = 0,
    /// `LV_OBJ_CLASS_EDITABLE_TRUE`
    True = 1,
    /// `LV_OBJ_CLASS_EDITABLE_FALSE`
    False = 2,
}

/// `lv_obj_class_group_def_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObjClassGroupDef {
    /// `LV_OBJ_CLASS_GROUP_DEF_INHERIT`
    #[default]
    Inherit = 0,
    /// `LV_OBJ_CLASS_GROUP_DEF_TRUE`
    True = 1,
    /// `LV_OBJ_CLASS_GROUP_DEF_FALSE`
    False = 2,
}

/// `lv_obj_tree_walk_res_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObjTreeWalkRes {
    /// `LV_OBJ_TREE_WALK_NEXT`
    #[default]
    Next = 0,
    /// `LV_OBJ_TREE_WALK_SKIP_CHILDREN`
    SkipChildren = 1,
    /// `LV_OBJ_TREE_WALK_END`
    End = 2,
}

/// Widget specific state stored in [`ObjData::widget`].
///
/// Inheritance is expressed as embedding (`SliderState { bar: BarState, .. }`)
/// plus `query`, which returns `self` or an embedded base state whose
/// `TypeId` matches, like COM's `QueryInterface`. Use
/// [`impl_widget_state!`](crate::impl_widget_state) to implement it.
pub trait WidgetState: Any {
    /// Return the state whose type id is `id`: `self` or an embedded base.
    fn query(&self, id: TypeId) -> Option<&dyn Any>;
    /// Mutable [`WidgetState::query`].
    fn query_mut(&mut self, id: TypeId) -> Option<&mut dyn Any>;
}

impl dyn WidgetState {
    /// Downcast to `T`, walking the embedded base states.
    #[inline]
    pub fn get<T: Any>(&self) -> Option<&T> {
        self.query(TypeId::of::<T>())?.downcast_ref::<T>()
    }

    /// Mutable [`get`](Self::get).
    #[inline]
    pub fn get_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.query_mut(TypeId::of::<T>())?.downcast_mut::<T>()
    }
}

/// Implement [`WidgetState`] for a widget state struct, optionally naming the
/// field that embeds the base class state.
///
/// ```ignore
/// impl_widget_state!(BarState);
/// impl_widget_state!(SliderState, base = bar);
/// ```
#[macro_export]
macro_rules! impl_widget_state {
    ($t:ty) => {
        impl $crate::core::obj::WidgetState for $t {
            fn query(&self, id: ::core::any::TypeId) -> Option<&dyn ::core::any::Any> {
                if id == ::core::any::TypeId::of::<$t>() {
                    Some(self)
                } else {
                    None
                }
            }
            fn query_mut(&mut self, id: ::core::any::TypeId) -> Option<&mut dyn ::core::any::Any> {
                if id == ::core::any::TypeId::of::<$t>() {
                    Some(self)
                } else {
                    None
                }
            }
        }
    };
    ($t:ty, base = $field:ident) => {
        impl $crate::core::obj::WidgetState for $t {
            fn query(&self, id: ::core::any::TypeId) -> Option<&dyn ::core::any::Any> {
                if id == ::core::any::TypeId::of::<$t>() {
                    Some(self)
                } else {
                    $crate::core::obj::WidgetState::query(&self.$field, id)
                }
            }
            fn query_mut(&mut self, id: ::core::any::TypeId) -> Option<&mut dyn ::core::any::Any> {
                if id == ::core::any::TypeId::of::<$t>() {
                    Some(self)
                } else {
                    $crate::core::obj::WidgetState::query_mut(&mut self.$field, id)
                }
            }
        }
    };
}

impl_widget_state!(());

/// `constructor_cb` / `destructor_cb`
pub type ObjClassLifecycleCallback = fn(&'static ObjClass, &mut Ui, Obj);
/// `event_cb` of a class.
pub type ObjClassEventCallback = fn(&'static ObjClass, &mut Ui, &mut Event<'_>);

/// `lv_obj_class_t`: describes a widget type. Instances are `static`s and
/// compared by address.
pub struct ObjClass {
    /// `base_class`
    pub base: Option<&'static ObjClass>,
    /// `name`
    pub name: &'static str,
    /// Replaces `instance_size`: allocate the widget state.
    pub new_state: fn() -> Box<dyn WidgetState>,
    /// `constructor_cb`
    pub constructor: Option<ObjClassLifecycleCallback>,
    /// `destructor_cb`
    pub destructor: Option<ObjClassLifecycleCallback>,
    /// `event_cb`
    pub event_cb: Option<ObjClassEventCallback>,
    /// `width_def`
    pub width_def: i32,
    /// `height_def`
    pub height_def: i32,
    /// `editable`
    pub editable: ObjClassEditable,
    /// `group_def`
    pub group_def: ObjClassGroupDef,
    /// `theme_inheritable`
    pub theme_inheritable: bool,
}

impl PartialEq for ObjClass {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl core::fmt::Debug for ObjClass {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ObjClass({})", self.name)
    }
}

/// `lv_delete_cb_t`
pub type DeleteCallback = Rc<dyn Fn(&mut Ui, Obj)>;

/// `lv_screen_create_cb_t`
pub type ScreenCreateCallback = fn(&mut Ui) -> Obj;

/// `lv_obj_spec_attr_t`: rarely used attributes, allocated on demand.
pub struct SpecAttr {
    /// `children`
    pub children: Vec<ObjId>,
    /// `group_p`
    pub group: Option<GroupId>,
    /// `event_list`
    pub event_list: EventList,
    /// `name` (`LV_USE_OBJ_NAME`)
    pub name: Option<Rc<str>>,
    /// `scroll`: the current scroll offset.
    pub scroll: Point,
    /// `ext_click_pad`
    pub ext_click_pad: i32,
    /// `ext_draw_size`
    pub ext_draw_size: i32,
    /// `scrollbar_mode`
    pub scrollbar_mode: ScrollbarMode,
    /// `scroll_snap_x`
    pub scroll_snap_x: ScrollSnap,
    /// `scroll_snap_y`
    pub scroll_snap_y: ScrollSnap,
    /// `scroll_dir`
    pub scroll_dir: Dir,
    /// `layer_type`
    pub layer_type: LayerType,
    /// `name_static`
    pub name_static: bool,
    /// `user_flags`: `LAYOUT_1/2`, `WIDGET_1/2`, `USER_1..4` bits.
    pub user_flags: u8,
}

impl SpecAttr {
    /// `lv_obj_allocate_spec_attr()` defaults.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            group: None,
            event_list: EventList::new(),
            name: None,
            scroll: Point::new(0, 0),
            ext_click_pad: 0,
            ext_draw_size: 0,
            scrollbar_mode: ScrollbarMode::Auto,
            scroll_snap_x: ScrollSnap::None,
            scroll_snap_y: ScrollSnap::None,
            scroll_dir: Dir::ALL,
            layer_type: LayerType::None,
            name_static: false,
            user_flags: 0,
        }
    }
}

impl Default for SpecAttr {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_obj_t`: the data of one object.
pub struct ObjData {
    /// `class_p`
    pub class: &'static ObjClass,
    /// `parent`
    pub parent: Option<ObjId>,
    /// `spec_attr`
    pub spec: Option<Box<SpecAttr>>,
    /// `styles` (+ `style_cnt`)
    pub styles: Vec<ObjStyle>,
    /// `style_main_prop_is_set` (`LV_OBJ_STYLE_CACHE`)
    pub style_main_prop_is_set: u32,
    /// `style_other_prop_is_set`
    pub style_other_prop_is_set: u32,
    /// `user_data`
    pub user_data: Option<Box<dyn Any>>,
    /// `coords`
    pub coords: Area,
    /// The `hidden .. state_trickle` bit fields, as [`ObjFlags`].
    pub flags: ObjFlags,
    /// `radio_button`
    pub radio_button: bool,
    /// `state`
    pub state: State,
    /// `layout_inv`
    pub layout_inv: bool,
    /// `readjust_scroll_after_layout`
    pub readjust_scroll_after_layout: bool,
    /// `scr_layout_inv`
    pub scr_layout_inv: bool,
    /// `skip_trans`
    pub skip_trans: bool,
    /// `h_layout`
    pub h_layout: bool,
    /// `w_layout`
    pub w_layout: bool,
    /// `h_ignore_size`
    pub h_ignore_size: bool,
    /// `w_ignore_size`
    pub w_ignore_size: bool,
    /// `is_deleting`
    pub is_deleting: bool,
    /// `rendered`
    pub rendered: bool,
    /// `has_blur`
    pub has_blur: bool,
    /// Widget specific state (the bytes after `lv_obj_t` in C).
    pub widget: Box<dyn WidgetState>,
}

/// `lv_obj_class`
pub static OBJ_CLASS: ObjClass = ObjClass {
    base: None,
    name: "lv_obj",
    new_state: || Box::new(()),
    constructor: Some(obj_constructor),
    destructor: Some(obj_destructor),
    event_cb: Some(obj_event),
    width_def: crate::core::ui::DPI_DEF,
    height_def: crate::core::ui::DPI_DEF,
    editable: ObjClassEditable::False,
    group_def: ObjClassGroupDef::False,
    theme_inheritable: false,
};

/// `LV_OBJ_DEF_WIDTH`: `LV_DPX(100)`
pub const OBJ_DEF_WIDTH_DPX: i32 = 100;
/// `LV_OBJ_DEF_HEIGHT`: `LV_DPX(50)`
pub const OBJ_DEF_HEIGHT_DPX: i32 = 50;
/// `STYLE_TRANSITION_MAX`
pub const STYLE_TRANSITION_MAX: usize = 32;

/// `lv_obj_constructor()`
fn obj_constructor(class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_obj_destructor()`
fn obj_destructor(class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_obj_event()`: the base class event handler.
fn obj_event(class: &'static ObjClass, ui: &mut Ui, e: &mut Event<'_>) {
    todo!()
}

macro_rules! flag_accessors {
    ($( ($set:ident, $is:ident, $flag:ident, $lv_set:literal, $lv_is:literal) ),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("`", $lv_set, "()`")]
                pub fn $set(&mut self, obj: Obj, en: bool) {
                    self.obj_set_flag(obj, ObjFlags::$flag, en);
                }

                #[doc = concat!("`", $lv_is, "()`")]
                pub fn $is(&self, obj: Obj) -> bool {
                    self.obj_has_flag(obj, ObjFlags::$flag)
                }
            )*
        }
    };
}

flag_accessors! {
    (obj_set_hidden, obj_is_hidden, HIDDEN, "lv_obj_set_hidden", "lv_obj_is_hidden"),
    (obj_set_clickable, obj_is_clickable, CLICKABLE, "lv_obj_set_clickable", "lv_obj_is_clickable"),
    (obj_set_click_focusable, obj_is_click_focusable, CLICK_FOCUSABLE, "lv_obj_set_click_focusable", "lv_obj_is_click_focusable"),
    (obj_set_checkable, obj_is_checkable, CHECKABLE, "lv_obj_set_checkable", "lv_obj_is_checkable"),
    (obj_set_scrollable, obj_is_scrollable, SCROLLABLE, "lv_obj_set_scrollable", "lv_obj_is_scrollable"),
    (obj_set_scroll_elastic, obj_is_scroll_elastic, SCROLL_ELASTIC, "lv_obj_set_scroll_elastic", "lv_obj_is_scroll_elastic"),
    (obj_set_scroll_momentum, obj_is_scroll_momentum, SCROLL_MOMENTUM, "lv_obj_set_scroll_momentum", "lv_obj_is_scroll_momentum"),
    (obj_set_scroll_one, obj_is_scroll_one, SCROLL_ONE, "lv_obj_set_scroll_one", "lv_obj_is_scroll_one"),
    (obj_set_scroll_chain_hor, obj_is_scroll_chain_hor, SCROLL_CHAIN_HOR, "lv_obj_set_scroll_chain_hor", "lv_obj_is_scroll_chain_hor"),
    (obj_set_scroll_chain_ver, obj_is_scroll_chain_ver, SCROLL_CHAIN_VER, "lv_obj_set_scroll_chain_ver", "lv_obj_is_scroll_chain_ver"),
    (obj_set_scroll_on_focus, obj_is_scroll_on_focus, SCROLL_ON_FOCUS, "lv_obj_set_scroll_on_focus", "lv_obj_is_scroll_on_focus"),
    (obj_set_scroll_with_arrow, obj_is_scroll_with_arrow, SCROLL_WITH_ARROW, "lv_obj_set_scroll_with_arrow", "lv_obj_is_scroll_with_arrow"),
    (obj_set_snappable, obj_is_snappable, SNAPPABLE, "lv_obj_set_snappable", "lv_obj_is_snappable"),
    (obj_set_press_lock, obj_is_press_lock, PRESS_LOCK, "lv_obj_set_press_lock", "lv_obj_is_press_lock"),
    (obj_set_event_bubble, obj_is_event_bubble, EVENT_BUBBLE, "lv_obj_set_event_bubble", "lv_obj_is_event_bubble"),
    (obj_set_gesture_bubble, obj_is_gesture_bubble, GESTURE_BUBBLE, "lv_obj_set_gesture_bubble", "lv_obj_is_gesture_bubble"),
    (obj_set_adv_hittest, obj_is_adv_hittest, ADV_HITTEST, "lv_obj_set_adv_hittest", "lv_obj_is_adv_hittest"),
    (obj_set_ignore_layout, obj_is_ignore_layout, IGNORE_LAYOUT, "lv_obj_set_ignore_layout", "lv_obj_is_ignore_layout"),
    (obj_set_floating, obj_is_floating, FLOATING, "lv_obj_set_floating", "lv_obj_is_floating"),
    (obj_set_send_draw_task_events, obj_is_send_draw_task_events, SEND_DRAW_TASK_EVENTS, "lv_obj_set_send_draw_task_events", "lv_obj_is_send_draw_task_events"),
    (obj_set_overflow_visible, obj_is_overflow_visible, OVERFLOW_VISIBLE, "lv_obj_set_overflow_visible", "lv_obj_is_overflow_visible"),
    (obj_set_event_trickle, obj_is_event_trickle, EVENT_TRICKLE, "lv_obj_set_event_trickle", "lv_obj_is_event_trickle"),
    (obj_set_state_trickle, obj_is_state_trickle, STATE_TRICKLE, "lv_obj_set_state_trickle", "lv_obj_is_state_trickle"),
    (obj_set_flex_in_new_track, obj_is_flex_in_new_track, FLEX_IN_NEW_TRACK, "lv_obj_set_flex_in_new_track", "lv_obj_is_flex_in_new_track"),
}

macro_rules! state_accessors {
    ($( ($set:ident, $is:ident, $state:ident, $lv_set:literal, $lv_is:literal) ),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("`", $lv_set, "()`")]
                pub fn $set(&mut self, obj: Obj, en: bool) {
                    self.obj_set_state(obj, State::$state, en);
                }

                #[doc = concat!("`", $lv_is, "()`")]
                pub fn $is(&self, obj: Obj) -> bool {
                    self.obj_has_state(obj, State::$state)
                }
            )*
        }
    };
}

state_accessors! {
    (obj_set_alt, obj_is_alt, ALT, "lv_obj_set_alt", "lv_obj_is_alt"),
    (obj_set_checked, obj_is_checked, CHECKED, "lv_obj_set_checked", "lv_obj_is_checked"),
    (obj_set_focused, obj_is_focused, FOCUSED, "lv_obj_set_focused", "lv_obj_is_focused"),
    (obj_set_focus_key, obj_is_focus_key, FOCUS_KEY, "lv_obj_set_focus_key", "lv_obj_is_focus_key"),
    (obj_set_edited, obj_is_edited, EDITED, "lv_obj_set_edited", "lv_obj_is_edited"),
    (obj_set_hovered, obj_is_hovered, HOVERED, "lv_obj_set_hovered", "lv_obj_is_hovered"),
    (obj_set_pressed, obj_is_pressed, PRESSED, "lv_obj_set_pressed", "lv_obj_is_pressed"),
    (obj_set_scrolled, obj_is_scrolled, SCROLLED, "lv_obj_set_scrolled", "lv_obj_is_scrolled"),
    (obj_set_disabled, obj_is_disabled, DISABLED, "lv_obj_set_disabled", "lv_obj_is_disabled"),
    (obj_set_state_user_1, obj_is_state_user_1, USER_1, "lv_obj_set_state_user_1", "lv_obj_is_state_user_1"),
    (obj_set_state_user_2, obj_is_state_user_2, USER_2, "lv_obj_set_state_user_2", "lv_obj_is_state_user_2"),
    (obj_set_state_user_3, obj_is_state_user_3, USER_3, "lv_obj_set_state_user_3", "lv_obj_is_state_user_3"),
    (obj_set_state_user_4, obj_is_state_user_4, USER_4, "lv_obj_set_state_user_4", "lv_obj_is_state_user_4"),
}

impl Ui {
    /// `lv_obj_create()`: create a base object. `None` creates a screen on
    /// the default display.
    pub fn obj_create(&mut self, parent: Option<Obj>) -> Obj {
        todo!()
    }

    /// `lv_obj_add_flag()`
    pub fn obj_add_flag(&mut self, obj: Obj, f: ObjFlags) {
        todo!()
    }

    /// `lv_obj_remove_flag()`
    pub fn obj_remove_flag(&mut self, obj: Obj, f: ObjFlags) {
        todo!()
    }

    /// `lv_obj_set_flag()`
    pub fn obj_set_flag(&mut self, obj: Obj, f: ObjFlags, v: bool) {
        todo!()
    }

    /// `lv_obj_set_radio_button()`
    pub fn obj_set_radio_button(&mut self, obj: Obj, en: bool) {
        todo!()
    }

    /// `lv_obj_is_radio_button()`
    pub fn obj_is_radio_button(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_add_state()`
    pub fn obj_add_state(&mut self, obj: Obj, state: State) {
        todo!()
    }

    /// `lv_obj_remove_state()`
    pub fn obj_remove_state(&mut self, obj: Obj, state: State) {
        todo!()
    }

    /// `lv_obj_set_state()`
    pub fn obj_set_state(&mut self, obj: Obj, state: State, v: bool) {
        todo!()
    }

    /// `lv_obj_set_user_data()`
    pub fn obj_set_user_data(&mut self, obj: Obj, user_data: Option<Box<dyn Any>>) {
        todo!()
    }

    /// `lv_obj_set_user_flag()`
    pub fn obj_set_user_flag(&mut self, obj: Obj, bit: u32, v: bool) {
        todo!()
    }

    /// `lv_obj_has_flag()`: `true` if all of `f` are set.
    pub fn obj_has_flag(&self, obj: Obj, f: ObjFlags) -> bool {
        todo!()
    }

    /// `lv_obj_has_flag_any()`: `true` if any of `f` is set.
    pub fn obj_has_flag_any(&self, obj: Obj, f: ObjFlags) -> bool {
        todo!()
    }

    /// `lv_obj_get_state()`
    pub fn obj_get_state(&self, obj: Obj) -> State {
        todo!()
    }

    /// `lv_obj_has_state()`
    pub fn obj_has_state(&self, obj: Obj, state: State) -> bool {
        todo!()
    }

    /// `lv_obj_get_group()`
    pub fn obj_get_group(&self, obj: Obj) -> Option<GroupId> {
        todo!()
    }

    /// `lv_obj_get_user_data()`
    pub fn obj_get_user_data(&self, obj: Obj) -> Option<&dyn Any> {
        todo!()
    }

    /// Mutable `lv_obj_get_user_data()`.
    pub fn obj_get_user_data_mut(&mut self, obj: Obj) -> Option<&mut dyn Any> {
        todo!()
    }

    /// `lv_obj_get_user_flag()`
    pub fn obj_get_user_flag(&self, obj: Obj, bit: u32) -> bool {
        todo!()
    }

    /// `lv_obj_check_type()`: exact class match.
    pub fn obj_check_type(&self, obj: Obj, class: &'static ObjClass) -> bool {
        todo!()
    }

    /// `lv_obj_has_class()`: class or any base class matches.
    pub fn obj_has_class(&self, obj: Obj, class: &'static ObjClass) -> bool {
        todo!()
    }

    /// `lv_obj_get_class()`
    pub fn obj_get_class(&self, obj: Obj) -> &'static ObjClass {
        todo!()
    }

    /// `lv_obj_is_valid()` / `lv_obj_is_in_widget_tree()`: the handle refers
    /// to a live object.
    pub fn obj_is_valid(&self, obj: Obj) -> bool {
        self.objs.contains(obj.0.0)
    }

    /// `lv_obj_add_delete_cb()`
    pub fn obj_add_delete_cb(&mut self, obj: Obj, cb: DeleteCallback) {
        todo!()
    }

    /// `lv_obj_add_screen_load_event()`
    pub fn obj_add_screen_load_event(
        &mut self,
        obj: Obj,
        trigger: EventCode,
        screen: Obj,
        anim_type: ScreenLoadAnim,
        duration: u32,
        delay: u32,
    ) {
        todo!()
    }

    /// `lv_obj_add_screen_create_event()`
    pub fn obj_add_screen_create_event(
        &mut self,
        obj: Obj,
        trigger: EventCode,
        screen_create_cb: ScreenCreateCallback,
        anim_type: ScreenLoadAnim,
        duration: u32,
        delay: u32,
    ) {
        todo!()
    }

    /// Borrow an object's data (`None` for a stale handle).
    #[inline]
    pub fn obj(&self, obj: Obj) -> Option<&ObjData> {
        self.objs.get(obj.0.0)
    }

    /// Mutably borrow an object's data.
    #[inline]
    pub fn obj_mut(&mut self, obj: Obj) -> Option<&mut ObjData> {
        self.objs.get_mut(obj.0.0)
    }

    /// Borrow the widget state of `obj` as `T` (walks base classes).
    pub fn widget<T: Any>(&self, obj: Obj) -> Option<&T> {
        self.obj(obj)?.widget.get::<T>()
    }

    /// Mutable [`widget`](Self::widget).
    pub fn widget_mut<T: Any>(&mut self, obj: Obj) -> Option<&mut T> {
        self.obj_mut(obj)?.widget.get_mut::<T>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_bits_match_upstream() {
        assert_eq!(ObjFlags::HIDDEN.bits(), 1);
        assert_eq!(ObjFlags::SCROLL_CHAIN.bits(), 0x300);
        assert_eq!(ObjFlags::ADV_HITTEST.bits(), 1 << 16);
        assert_eq!(ObjFlags::LAYOUT_1, ObjFlags::FLEX_IN_NEW_TRACK);
        assert_eq!(ObjFlags::USER_4.bits(), 1 << 30);
        assert_eq!(ScrollbarMode::Auto as u8, 3);
        assert_eq!(LayerType::Transform as u8, 2);
    }

    struct Base {
        v: i32,
    }
    impl_widget_state!(Base);
    struct Derived {
        base: Base,
        w: i32,
    }
    impl_widget_state!(Derived, base = base);

    #[test]
    fn widget_state_query_walks_bases() {
        let d: Box<dyn WidgetState> = Box::new(Derived {
            base: Base { v: 1 },
            w: 2,
        });
        assert_eq!(d.get::<Derived>().map(|d| d.w), Some(2));
        assert_eq!(d.get::<Base>().map(|b| b.v), Some(1));
        assert!(d.get::<()>().is_none());
        let unit: Box<dyn WidgetState> = Box::new(());
        assert!(unit.get::<()>().is_some());
        assert_eq!(OBJ_CLASS.name, "lv_obj");
        assert!(OBJ_CLASS == OBJ_CLASS);
    }
}

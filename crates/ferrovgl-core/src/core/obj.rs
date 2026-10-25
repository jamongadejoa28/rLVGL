//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_obj.h`, `src/core/lv_obj_private.h`,
//! `src/core/lv_obj_class_private.h`, `src/core/lv_obj.c`.
//!
//! Objects live in [`Ui::objs`]; [`Obj`] is a `Copy` handle. Widget specific
//! state is a [`WidgetState`] trait object inside [`ObjData`], queried
//! COM-style by `TypeId` to mimic C struct embedding.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::{Any, TypeId};
use core::cell::Cell;

use crate::core::Ui;
use crate::core::obj_style::{ObjStyle, ObjStyleSrc, ObjStyleTransitionDsc, StyleStateCmp};
use crate::display::ScreenLoadAnim;
use crate::draw::LayerId;
use crate::draw::blur::BlurDsc;
use crate::draw::grad::GradDir;
use crate::draw::rect::RectDsc;
use crate::group::{GroupId, KEY_DOWN, KEY_ENTER, KEY_LEFT, KEY_RIGHT, KEY_UP};
use crate::indev::IndevType;
use crate::misc::anim::{AnimEnable, AnimTarget};
use crate::misc::area::{Area, Dir, Point};
use crate::misc::arena::Key;
use crate::misc::check_arg_failed;
use crate::misc::color::{OPA_MAX, OPA_MIN, OPA_TRANSP, opa_mix2};
use crate::misc::event::{
    CoverRes, Event, EventCallback, EventCode, EventList, EventParam, EventTarget, LvResult,
    event_free_user_data_cb,
};
use crate::misc::style::{Part, State, StyleProp, StyleValue};

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

    /// Mutable version of `get`.
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

/// The `free_cb` of an object's `lv_ext_data_t` (`void (*)(void * data)`).
pub type ObjExtDataFreeCb = fn(Option<Rc<dyn Any>>);

/// `lv_ext_data_t` of an object (`LV_USE_EXT_DATA`), set by
/// [`Ui::obj_set_external_data`] and released by `lv_obj_destruct()`.
#[derive(Default)]
pub struct ObjExtData {
    /// `data`; independent of the object's application `user_data`.
    pub data: Option<Rc<dyn Any>>,
    /// `free_cb`
    pub free_cb: Option<ObjExtDataFreeCb>,
}

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
    /// `ext_data` (`LV_USE_EXT_DATA`).
    pub ext_data: ObjExtData,
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
    /// `id` (`LV_USE_OBJ_ID`): the `void *` object id as `lv_uintptr_t`,
    /// 0 for `NULL`. Not the arena handle [`ObjId`]; see
    /// [`obj_id`](crate::core::obj_id).
    #[cfg(feature = "obj-id")]
    pub id: usize,
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

/*=====================
 * Static data
 *====================*/

/// `screen_load_anim_dsc_t::target`
#[derive(Copy, Clone)]
enum ScreenLoadTarget {
    /// `target.screen`
    Screen(Obj),
    /// `target.create_cb`
    CreateCb(ScreenCreateCallback),
}

/// `screen_load_anim_dsc_t`: the user data of the screen load trigger events.
struct ScreenLoadAnimDsc {
    anim_type: ScreenLoadAnim,
    duration: u32,
    delay: u32,
    target: ScreenLoadTarget,
}

/// `struct _lv_delete_dsc_t`: the user data of `call_delete_cb`.
///
/// `event_cb` is the `call_delete_cb` callback this descriptor was
/// registered with, so `lv_obj_remove_delete_cb()` can match the
/// `(cb, user_data)` pair like upstream (callbacks compare by `Rc` identity).
struct DeleteDsc {
    obj: Obj,
    cb: DeleteCallback,
    event_cb: EventCallback,
}

/// `lv_delete_dsc_t *`: the descriptor returned by [`Ui::obj_add_delete_cb`],
/// passed to [`Ui::obj_remove_delete_cb`] to cancel the callback.
///
/// It is the `user_data` of the `call_delete_cb` event descriptor; clones
/// refer to the same descriptor (`Rc` identity, like the C pointer).
#[derive(Clone)]
pub struct DeleteDscRef(Rc<dyn Any>);

impl core::fmt::Debug for DeleteDscRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let obj = self.0.downcast_ref::<DeleteDsc>().map(|d| d.obj);
        f.debug_struct("DeleteDscRef").field("obj", &obj).finish()
    }
}

/// Bits of [`ObjFlags`] stored in [`ObjData::flags`] (`hidden` ..
/// `state_trickle`); everything from `LV_OBJ_FLAG_LAYOUT_1` up lives in
/// [`SpecAttr::user_flags`] like upstream.
const OBJ_FLAG_BITS_MASK: u32 = ObjFlags::LAYOUT_1.bits() - 1;
/// Shift from an [`ObjFlags`] bit to its [`SpecAttr::user_flags`] bit
/// (`LV_OBJ_FLAG_LAYOUT_1` is `user_flags` bit 0).
const USER_FLAGS_SHIFT: u32 = 23;

/// `lv_obj_constructor()`
fn obj_constructor(class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    let Some(parent) = ui.obj(obj).and_then(|d| d.parent).map(Obj::from) else {
        set_constructor_attributes(class, ui, obj, false);
        return;
    };

    let sl = ui.obj_get_scroll_left(parent);
    let st = ui.obj_get_scroll_top(parent);

    let parent_coords = ui.obj(parent).map(|d| d.coords).unwrap_or_default();
    let pad_top = ui.style_num(parent, Part::MAIN, StyleProp::PAD_TOP);
    let pad_left = ui.style_num(parent, Part::MAIN, StyleProp::PAD_LEFT);
    if let Some(d) = ui.obj_mut(obj) {
        d.coords.y1 = parent_coords.y1 + pad_top - st;
        d.coords.y2 = d.coords.y1 - 1;
        d.coords.x1 = parent_coords.x1 + pad_left - sl;
        d.coords.x2 = d.coords.x1 - 1;
    }

    set_constructor_attributes(class, ui, obj, true);
}

/// The "Set attributes" part of `lv_obj_constructor()` and the
/// `LV_OBJ_ID_AUTO_ASSIGN` id assignment that ends it.
fn set_constructor_attributes(class: &'static ObjClass, ui: &mut Ui, obj: Obj, has_parent: bool) {
    let Some(d) = ui.obj_mut(obj) else {
        return;
    };
    d.flags.insert(ObjFlags::CLICKABLE);
    d.flags.insert(ObjFlags::SNAPPABLE);
    d.flags.insert(ObjFlags::CLICK_FOCUSABLE);
    d.flags.insert(ObjFlags::SCROLLABLE);
    d.flags.insert(ObjFlags::SCROLL_ELASTIC);
    d.flags.insert(ObjFlags::SCROLL_MOMENTUM);
    d.flags.insert(ObjFlags::SCROLL_WITH_ARROW);
    if has_parent {
        d.flags.insert(ObjFlags::PRESS_LOCK);
        d.flags.insert(ObjFlags::SCROLL_CHAIN_HOR);
        d.flags.insert(ObjFlags::SCROLL_CHAIN_VER);
        d.flags.insert(ObjFlags::GESTURE_BUBBLE);
    }

    /*`LV_USE_OBJ_ID && LV_OBJ_ID_AUTO_ASSIGN`*/
    #[cfg(feature = "obj-id")]
    ui.obj_assign_id(class, obj);
    #[cfg(not(feature = "obj-id"))]
    let _ = class;
}

/// `lv_obj_destructor()`
fn obj_destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    ui.event_mark_deleted(EventTarget::Obj(obj));

    /*Remove all style*/
    ui.obj_enable_style_refresh(false); /*No need to refresh the style because the object will be deleted*/
    ui.obj_remove_style_all(obj);
    ui.obj_enable_style_refresh(true);

    /*Remove the animations from this object*/
    ui.anim_delete(AnimTarget::Obj(obj), None);

    /*Delete from the group*/
    let group = ui.obj_get_group(obj);
    if group.is_some() {
        ui.group_remove_obj(obj);
    }

    let spec = ui.obj_mut(obj).and_then(|d| d.spec.take());
    if let Some(mut spec) = spec {
        spec.children = Vec::new();

        ui.event_remove_all(&mut spec.event_list);
        /*`name` is reference counted: dropping `spec` releases it (the
         *`name_static` case owns nothing).*/
        drop(spec);
    }

    /*`LV_USE_OBJ_ID && LV_OBJ_ID_AUTO_ASSIGN`*/
    #[cfg(feature = "obj-id")]
    ui.obj_free_id(obj);
}

/// `lv_obj_draw()`
fn obj_draw(ui: &mut Ui, e: &mut Event<'_>) {
    let code = e.code();
    let Some(obj) = e.current_target_obj() else {
        return;
    };
    if code == EventCode::COVER_CHECK {
        let (res, area) = match e.param() {
            EventParam::CoverCheck(info) => (info.res, info.area),
            _ => return,
        };
        if res == CoverRes::Masked {
            return;
        }
        let res = cover_check(ui, obj, &area);
        if let EventParam::CoverCheck(info) = e.param() {
            info.res = res;
        }
    } else if code == EventCode::DRAW_MAIN {
        let Some(layer) = e.layer() else {
            return;
        };

        let mut draw_dsc = RectDsc::new();
        draw_dsc.base.layer = Some(layer);

        let coords = ui.transformed_coords(obj);

        let backdrop_blur = ui.style_bool(obj, Part::MAIN, StyleProp::BLUR_BACKDROP);
        if backdrop_blur {
            let mut blur_dsc = BlurDsc::new();
            ui.obj_init_draw_blur_dsc(obj, Part::MAIN, &mut blur_dsc);
            blur_dsc.corner_radius = draw_dsc.radius;
            blur_dsc.base.layer = Some(layer);
            ui.draw_blur(layer, &blur_dsc, &coords);
        }

        ui.obj_init_draw_rect_dsc(obj, Part::MAIN, &mut draw_dsc);
        /*If the border is drawn later disable loading its properties*/
        if ui.style_bool(obj, Part::MAIN, StyleProp::BORDER_POST) {
            draw_dsc.border_post = true;
        }

        ui.draw_rect(layer, &draw_dsc, &coords);
    } else if code == EventCode::DRAW_MAIN_END {
        /*Draw the non backdrop blur when the main content is rendered the the children are not yet */
        let Some(layer) = e.layer() else {
            return;
        };
        let backdrop_blur = ui.style_bool(obj, Part::MAIN, StyleProp::BLUR_BACKDROP);
        if !backdrop_blur {
            let coords = ui.transformed_coords(obj);

            let mut blur_dsc = BlurDsc::new();
            ui.obj_init_draw_blur_dsc(obj, Part::MAIN, &mut blur_dsc);
            blur_dsc.corner_radius = ui.style_num(obj, Part::MAIN, StyleProp::RADIUS);
            blur_dsc.base.layer = Some(layer);
            ui.draw_blur(layer, &blur_dsc, &coords);
        }
    } else if code == EventCode::DRAW_POST {
        let Some(layer) = e.layer() else {
            return;
        };
        draw_scrollbar(ui, obj, layer);

        /*If the border is drawn later disable loading other properties*/
        if ui.style_num(obj, Part::MAIN, StyleProp::BORDER_WIDTH) != 0
            && ui.style_bool(obj, Part::MAIN, StyleProp::BORDER_POST)
        {
            let mut draw_dsc = RectDsc::new();
            draw_dsc.bg_opa = OPA_TRANSP;
            draw_dsc.bg_image_opa = OPA_TRANSP;
            draw_dsc.outline_opa = OPA_TRANSP;
            draw_dsc.shadow_opa = OPA_TRANSP;
            draw_dsc.base.layer = Some(layer);
            ui.obj_init_draw_rect_dsc(obj, Part::MAIN, &mut draw_dsc);

            let coords = ui.transformed_coords(obj);

            ui.draw_rect(layer, &draw_dsc, &coords);
        }
    }
}

/// The `LV_EVENT_COVER_CHECK` branch of `lv_obj_draw()` after the
/// `LV_COVER_RES_MASKED` early return: the new `info->res`.
fn cover_check(ui: &Ui, obj: Obj, info_area: &Area) -> CoverRes {
    if ui.style_bool(obj, Part::MAIN, StyleProp::CLIP_CORNER) {
        return CoverRes::Masked;
    }

    /*Most trivial test. Is the mask fully IN the object? If no it surely doesn't cover it*/
    let r = ui.style_num(obj, Part::MAIN, StyleProp::RADIUS);
    let coords = ui.transformed_coords(obj);

    if !info_area.is_in(&coords, r) {
        return CoverRes::NotCover;
    }

    if ui.style_opa(obj, Part::MAIN, StyleProp::BG_OPA) < OPA_MAX {
        return CoverRes::NotCover;
    }

    if ui.style_opa(obj, Part::MAIN, StyleProp::OPA) < OPA_MAX {
        return CoverRes::NotCover;
    }

    if ui.style_num(obj, Part::MAIN, StyleProp::BG_GRAD_DIR) as u8 != GradDir::None as u8
        && (ui.style_opa(obj, Part::MAIN, StyleProp::BG_GRAD_OPA) < OPA_MAX
            || ui.style_opa(obj, Part::MAIN, StyleProp::BG_MAIN_OPA) < OPA_MAX)
    {
        return CoverRes::NotCover;
    }
    if let StyleValue::Grad(grad_dsc) = ui.obj_get_style_prop(obj, Part::MAIN, StyleProp::BG_GRAD) {
        let stops_count = grad_dsc.stops_count as usize;
        for i in 0..stops_count {
            if grad_dsc.stops[i].opa < OPA_MAX {
                return CoverRes::NotCover;
            }
        }
    }
    CoverRes::Cover
}

/// `draw_scrollbar()`
fn draw_scrollbar(ui: &mut Ui, obj: Obj, layer: LayerId) {
    let (hor_area, ver_area) = ui.obj_get_scrollbar_area(obj);

    /*`lv_area_get_size()` is unsigned: `<= 0` is `== 0`*/
    if hor_area.size() == 0 && ver_area.size() == 0 {
        return;
    }

    let mut rect_dsc = RectDsc::new();
    let sb_res = scrollbar_init_draw_dsc(ui, obj, &mut rect_dsc);
    if sb_res != LvResult::Ok {
        return;
    }

    let backdrop_blur = ui.style_bool(obj, Part::SCROLLBAR, StyleProp::BLUR_BACKDROP);
    let mut blur_dsc = BlurDsc::new();
    blur_dsc.corner_radius = rect_dsc.radius;
    blur_dsc.blur_radius = ui.style_num(obj, Part::SCROLLBAR, StyleProp::BLUR_RADIUS);

    if hor_area.size() > 0 {
        if backdrop_blur {
            ui.obj_init_draw_blur_dsc(obj, Part::SCROLLBAR, &mut blur_dsc);
        }
        blur_dsc.base.id1 = 0;
        ui.draw_blur(layer, &blur_dsc, &hor_area);

        rect_dsc.base.id1 = 0;
        ui.draw_rect(layer, &rect_dsc, &hor_area);

        if !backdrop_blur {
            ui.draw_blur(layer, &blur_dsc, &hor_area);
        }
    }
    if ver_area.size() > 0 {
        blur_dsc.base.id1 = 1;
        if backdrop_blur {
            ui.draw_blur(layer, &blur_dsc, &ver_area);
        }

        rect_dsc.base.id1 = 1;
        ui.draw_rect(layer, &rect_dsc, &ver_area);

        if !backdrop_blur {
            ui.draw_blur(layer, &blur_dsc, &ver_area);
        }
    }
}

/// `scrollbar_init_draw_dsc()`: initialize the draw descriptor for the
/// scrollbar. Returns [`LvResult::Ok`] if the scrollbar is visible,
/// [`LvResult::Invalid`] if not.
fn scrollbar_init_draw_dsc(ui: &Ui, obj: Obj, dsc: &mut RectDsc) -> LvResult {
    *dsc = RectDsc::new();
    dsc.bg_opa = ui.style_opa(obj, Part::SCROLLBAR, StyleProp::BG_OPA);
    if dsc.bg_opa > OPA_MIN {
        dsc.bg_color = ui.style_color(obj, Part::SCROLLBAR, StyleProp::BG_COLOR);
    }

    dsc.border_opa = ui.style_opa(obj, Part::SCROLLBAR, StyleProp::BORDER_OPA);
    if dsc.border_opa > OPA_MIN {
        dsc.border_width = ui.style_num(obj, Part::SCROLLBAR, StyleProp::BORDER_WIDTH);
        if dsc.border_width > 0 {
            dsc.border_color = ui.style_color(obj, Part::SCROLLBAR, StyleProp::BORDER_COLOR);
        } else {
            dsc.border_opa = OPA_TRANSP;
        }
    }

    dsc.shadow_opa = ui.style_opa(obj, Part::SCROLLBAR, StyleProp::SHADOW_OPA);
    if dsc.shadow_opa > OPA_MIN {
        dsc.shadow_width = ui.style_num(obj, Part::SCROLLBAR, StyleProp::SHADOW_WIDTH);
        if dsc.shadow_width > 0 {
            dsc.shadow_spread = ui.style_num(obj, Part::SCROLLBAR, StyleProp::SHADOW_SPREAD);
            dsc.shadow_color = ui.style_color(obj, Part::SCROLLBAR, StyleProp::SHADOW_COLOR);
        } else {
            dsc.shadow_opa = OPA_TRANSP;
        }
    }

    let opa = ui.obj_get_style_opa_recursive(obj, Part::SCROLLBAR);
    if opa < OPA_MAX {
        let v = opa_mix2(dsc.bg_opa, opa);
        dsc.bg_opa = v;
        dsc.border_opa = v;
        dsc.shadow_opa = v;
    }

    if dsc.bg_opa != OPA_TRANSP || dsc.border_opa != OPA_TRANSP || dsc.shadow_opa != OPA_TRANSP {
        dsc.radius = ui.style_num(obj, Part::SCROLLBAR, StyleProp::RADIUS);
        LvResult::Ok
    } else {
        LvResult::Invalid
    }
}

/// `lv_obj_event()`: the base class event handler.
fn obj_event(_class: &'static ObjClass, ui: &mut Ui, e: &mut Event<'_>) {
    let code = e.code();
    let Some(obj) = e.current_target_obj() else {
        return;
    };
    if code == EventCode::PRESSED {
        ui.obj_add_state(obj, State::PRESSED);
    } else if code == EventCode::RELEASED {
        ui.obj_remove_state(obj, State::PRESSED);
        let indev = e.indev();
        /*Go the checked state if enabled*/
        let no_scroll_obj = match indev {
            None => true,
            Some(indev) => ui.indev_get_scroll_obj(indev).is_none(),
        };
        if no_scroll_obj && ui.obj_is_checkable(obj) {
            let was_checked = ui.has_state(obj, State::CHECKED);
            if !ui.obj_get_state(obj).intersects(State::CHECKED) {
                ui.obj_add_state(obj, State::CHECKED);
            }
            /*Radio buttons can't be checked off directly*/
            else if !ui.obj_is_radio_button(obj) {
                ui.obj_remove_state(obj, State::CHECKED);
            }
            if was_checked != ui.has_state(obj, State::CHECKED) {
                let res = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
                if res != LvResult::Ok {
                    return;
                }

                let code = if was_checked {
                    EventCode::UNCHECKED
                } else {
                    EventCode::CHECKED
                };
                /*`if(res != LV_RESULT_OK) return;` ends the handler either way*/
                let _res = ui.obj_send_event(obj, code, EventParam::None);
            }
        }
    } else if code == EventCode::VALUE_CHANGED {
        if ui.obj_is_radio_button(obj) && ui.has_state(obj, State::CHECKED) {
            let parent = ui.obj(obj).and_then(|d| d.parent).map(Obj::from);
            if let Some(parent) = parent {
                let child_cnt = ui.child_count(parent);
                for i in 0..child_cnt {
                    let Some(sibling) = ui.child_at(parent, i) else {
                        continue;
                    };
                    if obj == sibling {
                        continue;
                    }

                    if ui.obj_is_radio_button(sibling) && ui.has_state(sibling, State::CHECKED) {
                        ui.obj_remove_state(sibling, State::CHECKED);
                        let res = ui.obj_send_event(sibling, EventCode::VALUE_CHANGED, EventParam::None);
                        if res != LvResult::Ok {
                            return;
                        }

                        let res = ui.obj_send_event(sibling, EventCode::UNCHECKED, EventParam::None);
                        if res != LvResult::Ok {
                            return;
                        }
                    }
                }
            }
        }
    } else if code == EventCode::PRESS_LOST {
        ui.obj_remove_state(obj, State::PRESSED);
    } else if code == EventCode::STYLE_CHANGED {
        let child_cnt = ui.child_count(obj);
        for i in 0..child_cnt {
            if let Some(child) = ui.child_at(obj, i) {
                ui.obj_mark_layout_as_dirty(child);
            }
        }
    } else if code == EventCode::KEY {
        if ui.obj_is_checkable(obj) {
            let c = e.key();
            let was_checked = ui.has_state(obj, State::CHECKED);
            if c == KEY_RIGHT || c == KEY_UP {
                ui.obj_add_state(obj, State::CHECKED);
            } else if c == KEY_LEFT || c == KEY_DOWN {
                /*Radio buttons can't be checked off directly*/
                if !ui.obj_is_radio_button(obj) {
                    ui.obj_remove_state(obj, State::CHECKED);
                }
            }

            /*With Enter LV_EVENT_RELEASED will send VALUE_CHANGE event*/
            if c != KEY_ENTER && was_checked != ui.has_state(obj, State::CHECKED) {
                let res = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
                if res != LvResult::Ok {
                    return;
                }

                let code = if was_checked {
                    EventCode::UNCHECKED
                } else {
                    EventCode::CHECKED
                };
                /*`if(res != LV_RESULT_OK) return;` ends the handler either way*/
                let _res = ui.obj_send_event(obj, code, EventParam::None);
            }
        } else if ui.obj_is_scrollable(obj) && ui.obj_is_scroll_with_arrow(obj) && !ui.obj_is_editable(obj) {
            /*scroll by keypad or encoder*/
            let anim_enable: AnimEnable = false;
            let sl = ui.obj_get_scroll_left(obj);
            let sr = ui.obj_get_scroll_right(obj);
            let c = e.key();
            if c == KEY_DOWN {
                /*use scroll_to_x/y functions to enforce scroll limits*/
                let y = ui.obj_get_scroll_y(obj) + ui.obj_get_height(obj) / 4;
                ui.obj_scroll_to_y(obj, y, anim_enable);
            } else if c == KEY_UP {
                let y = ui.obj_get_scroll_y(obj) - ui.obj_get_height(obj) / 4;
                ui.obj_scroll_to_y(obj, y, anim_enable);
            } else if c == KEY_RIGHT {
                /*If the object can't be scrolled horizontally then scroll it vertically*/
                if !(ui.obj_get_scroll_dir(obj).intersects(Dir::HOR) && (sl > 0 || sr > 0)) {
                    let y = ui.obj_get_scroll_y(obj) + ui.obj_get_height(obj) / 4;
                    ui.obj_scroll_to_y(obj, y, anim_enable);
                } else {
                    let x = ui.obj_get_scroll_x(obj) + ui.obj_get_width(obj) / 4;
                    ui.obj_scroll_to_x(obj, x, anim_enable);
                }
            } else if c == KEY_LEFT {
                /*If the object can't be scrolled horizontally then scroll it vertically*/
                if !(ui.obj_get_scroll_dir(obj).intersects(Dir::HOR) && (sl > 0 || sr > 0)) {
                    let y = ui.obj_get_scroll_y(obj) - ui.obj_get_height(obj) / 4;
                    ui.obj_scroll_to_y(obj, y, anim_enable);
                } else {
                    let x = ui.obj_get_scroll_x(obj) - ui.obj_get_width(obj) / 4;
                    ui.obj_scroll_to_x(obj, x, anim_enable);
                }
            }
        }
    } else if code == EventCode::FOCUSED {
        if ui.obj_is_scroll_on_focus(obj) {
            ui.obj_scroll_to_view_recursive(obj, true);
        }

        let group = ui.obj_get_group(obj);
        let editing = match group {
            Some(g) => ui.group_get_editing(g),
            None => false,
        };
        let mut state = State::FOCUSED;

        /* Use the indev for then indev handler.
         * But if the obj was focused manually it returns NULL so try to
         * use the indev from the event*/
        let mut indev = ui.indev_active();
        if indev.is_none() {
            indev = e.indev();
        }

        let indev_type = match indev {
            Some(i) => ui.indev_get_type(i),
            None => IndevType::None,
        };
        if indev_type == IndevType::Keypad || indev_type == IndevType::Encoder {
            state |= State::FOCUS_KEY;
        }
        if editing {
            state |= State::EDITED;
            ui.obj_add_state(obj, state);
        } else {
            ui.obj_add_state(obj, state);
            ui.obj_remove_state(obj, State::EDITED);
        }
    } else if code == EventCode::SCROLL_BEGIN {
        ui.obj_add_state(obj, State::SCROLLED);
    } else if code == EventCode::SCROLL_END {
        ui.obj_remove_state(obj, State::SCROLLED);
        if ui.obj_get_scrollbar_mode(obj) == ScrollbarMode::Active {
            let (hor_area, ver_area) = ui.obj_get_scrollbar_area(obj);
            ui.obj_invalidate_area(obj, &hor_area);
            ui.obj_invalidate_area(obj, &ver_area);
        }
    } else if code == EventCode::DEFOCUSED {
        ui.obj_remove_state(obj, State::FOCUSED | State::EDITED | State::FOCUS_KEY);
    } else if code == EventCode::SIZE_CHANGED {
        let align = ui.style_num(obj, Part::MAIN, StyleProp::ALIGN);
        let layout = ui.style_num(obj, Part::MAIN, StyleProp::LAYOUT) as u16;
        if layout != 0 || align != 0 {
            ui.obj_mark_layout_as_dirty(obj);
        }

        let child_cnt = ui.child_count(obj);
        for i in 0..child_cnt {
            if let Some(child) = ui.child_at(obj, i) {
                ui.obj_mark_layout_as_dirty(child);
            }
        }
    } else if code == EventCode::CHILD_CHANGED {
        let align = ui.style_num(obj, Part::MAIN, StyleProp::ALIGN);
        let layout = ui.style_num(obj, Part::MAIN, StyleProp::LAYOUT) as u16;
        if layout != 0
            || align != 0
            || ui.obj_is_style_any_width_content(obj)
            || ui.obj_is_style_any_height_content(obj)
        {
            ui.obj_mark_layout_as_dirty(obj);
        }
    } else if code == EventCode::CHILD_DELETED {
        if let Some(d) = ui.obj_mut(obj) {
            d.readjust_scroll_after_layout = true;
        }
        ui.obj_mark_layout_as_dirty(obj);
    } else if code == EventCode::REFR_EXT_DRAW_SIZE {
        let d = ui.obj_calculate_ext_draw_size(obj, Part::MAIN);
        e.set_ext_draw_size(d);
    } else if code == EventCode::DRAW_MAIN
        || code == EventCode::DRAW_POST
        || code == EventCode::DRAW_MAIN_END
        || code == EventCode::COVER_CHECK
    {
        obj_draw(ui, e);
    } else if code == EventCode::INDEV_RESET {
        ui.obj_remove_state(obj, State::PRESSED);
        ui.obj_remove_state(obj, State::SCROLLED);
    } else if code == EventCode::HOVER_OVER {
        ui.obj_add_state(obj, State::HOVERED);
    } else if code == EventCode::HOVER_LEAVE {
        ui.obj_remove_state(obj, State::HOVERED);
    }
}

/// `null_on_delete_cb()`
fn null_on_delete_cb(_ui: &mut Ui, e: &mut Event<'_>) {
    if let Some(obj_ptr) = e.user_data().and_then(|u| u.downcast_ref::<Cell<Option<Obj>>>()) {
        obj_ptr.set(None);
    }
}

/// `screen_load_on_trigger_event_cb()`
fn screen_load_on_trigger_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(dsc) = e.user_data().and_then(|u| u.downcast_ref::<ScreenLoadAnimDsc>()) else {
        return;
    };
    let ScreenLoadTarget::Screen(screen) = dsc.target else {
        return;
    };
    let (anim_type, duration, delay) = (dsc.anim_type, dsc.duration, dsc.delay);
    ui.screen_load_anim(screen, anim_type, duration, delay, false);
}

/// `screen_create_on_trigger_event_cb()`
fn screen_create_on_trigger_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(dsc) = e.user_data().and_then(|u| u.downcast_ref::<ScreenLoadAnimDsc>()) else {
        return;
    };
    let ScreenLoadTarget::CreateCb(create_cb) = dsc.target else {
        return;
    };
    let (anim_type, duration, delay) = (dsc.anim_type, dsc.duration, dsc.delay);

    let screen = create_cb(ui);
    ui.screen_load_anim(screen, anim_type, duration, delay, false);
    let cb: EventCallback = Rc::new(delete_on_screen_unloaded_event_cb);
    ui.obj_add_event_cb(screen, cb, EventCode::SCREEN_UNLOADED, None);
}

/// `delete_on_screen_unloaded_event_cb()`
fn delete_on_screen_unloaded_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    if let Some(obj) = e.target_obj() {
        ui.obj_delete(obj);
    }
}

/// `call_delete_cb()`
fn call_delete_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let obj = e.target_obj();
    let Some(user_data) = e.user_data().cloned() else {
        return;
    };
    let Some(dsc) = user_data.downcast_ref::<DeleteDsc>() else {
        return;
    };
    debug_assert!(obj == Some(dsc.obj));

    let cb = dsc.cb.clone();
    cb(ui, dsc.obj);
    ui.obj_remove_delete_cb(Some(&DeleteDscRef(user_data)));
}

/*-------------------
 * OTHER FUNCTIONS
 *------------------*/

/// `lv_obj_add_child()`: append `child` to `parent->spec_attr->children`.
///
/// Defined in `lv_obj.c` upstream; the `Ui::obj_add_child` declaration of
/// `obj_tree.rs` delegates here. Like upstream the caller allocates the
/// parent's `spec_attr` first (`lv_obj_class_create_obj()`,
/// `lv_obj_set_parent()`); C dereferences it unchecked, here a missing
/// `spec_attr` returns [`LvResult::Invalid`] (the realloc-failure result).
pub fn obj_add_child(ui: &mut Ui, parent: Obj, child: Obj) -> LvResult {
    if !ui.check_obj(parent) {
        return LvResult::Invalid;
    }
    /*`child != NULL`: a handle can't be NULL*/

    let Some(spec_attr) = ui.obj_mut(parent).and_then(|d| d.spec.as_deref_mut()) else {
        return LvResult::Invalid;
    };

    spec_attr.children.push(child.id());
    LvResult::Ok
}

/// `lv_obj_remove_child()`: remove `child` from
/// `parent->spec_attr->children`, shifting the following children down.
///
/// Defined in `lv_obj.c` upstream; the `Ui::obj_remove_child` declaration
/// of `obj_tree.rs` delegates here. Like upstream the start index is
/// `lv_obj_get_index(child)` (looked up in the child's own parent), the
/// array is freed when it held one child and shrunk by one otherwise.
pub fn obj_remove_child(ui: &mut Ui, parent: Obj, child: Obj) {
    if !ui.check_obj(parent) || !ui.check_obj(child) {
        return;
    }

    let index = obj_get_index_raw(ui, child);

    let Some(spec_attr) = ui.obj_mut(parent).and_then(|d| d.spec.as_deref_mut()) else {
        return;
    };
    let child_cnt = spec_attr.children.len() as i32;

    /*C reads `children[-1]` when the child isn't found (LV_ASSERT(0) in
     *lv_obj_get_index); the shift is skipped then*/
    if index >= 0 {
        let mut i = index;
        while i < child_cnt - 1 {
            spec_attr.children[i as usize] = spec_attr.children[(i + 1) as usize];
            i += 1;
        }
    }

    /* No more children*/
    if child_cnt == 1 {
        spec_attr.children = Vec::new();
        return;
    }
    /*`child_cnt--` + shrinking realloc (C underflows on an empty array)*/
    spec_attr.children.pop();
}

/// `lv_obj_get_index()` read straight from `spec_attr` (`-1` without a
/// parent or when the parent doesn't list `obj`).
fn obj_get_index_raw(ui: &Ui, obj: Obj) -> i32 {
    let Some(parent) = ui.obj(obj).and_then(|d| d.parent) else {
        return -1;
    };
    let Some(spec) = ui.objs.get(parent.0).and_then(|d| d.spec.as_deref()) else {
        return -1;
    };
    match spec.children.iter().position(|&c| c == obj.id()) {
        Some(i) => i as i32,
        /*Shouldn't reach this point*/
        None => -1,
    }
}

macro_rules! simple_flag_setters {
    ($( ($set:ident, $flag:ident, $lv_set:literal) ),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("`", $lv_set, "()`")]
                pub fn $set(&mut self, obj: Obj, en: bool) {
                    self.set_flag_bit(obj, ObjFlags::$flag, en);
                }
            )*
        }
    };
}

macro_rules! flag_getters {
    ($( ($is:ident, $flag:ident, $lv_is:literal) ),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("`", $lv_is, "()`")]
                pub fn $is(&self, obj: Obj) -> bool {
                    self.get_flag_bit(obj, ObjFlags::$flag)
                }
            )*
        }
    };
}

macro_rules! state_accessors {
    ($( ($set:ident, $is:ident, $state:ident, $lv_set:literal, $lv_is:literal) ),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("`", $lv_set, "()`")]
                pub fn $set(&mut self, obj: Obj, en: bool) {
                    if !self.check_obj(obj) {
                        return;
                    }
                    self.set_state_internal(obj, State::$state, en);
                }

                #[doc = concat!("`", $lv_is, "()`")]
                pub fn $is(&self, obj: Obj) -> bool {
                    if !self.check_obj(obj) {
                        return false;
                    }
                    self.has_state(obj, State::$state)
                }
            )*
        }
    };
}

#[allow(clippy::collapsible_if)] // keep the C `if` nesting
impl Ui {
    /// `lv_obj_create()`: create a base object. `None` creates a screen on
    /// the default display.
    pub fn obj_create(&mut self, parent: Option<Obj>) -> Obj {
        let obj = self.obj_class_create_obj(&OBJ_CLASS, parent);

        self.obj_class_init_obj(obj);

        obj
    }

    /*=====================
     * Setter functions
     *====================*/

    /// `lv_obj_add_flag()`
    pub fn obj_add_flag(&mut self, obj: Obj, f: ObjFlags) {
        if !self.check_obj(obj) {
            return;
        }

        if f.bits() >= ObjFlags::LAYOUT_1.bits() {
            self.obj_allocate_spec_attr(obj);
        }

        /* If all requested flags are already set, do nothing */
        if self.obj_has_flag(obj, f) {
            return;
        }

        self.apply_flag_setters(obj, f, true);
        let user = user_flag_bits(f);
        if user != 0 {
            if let Some(spec) = self.obj_mut(obj).and_then(|d| d.spec.as_deref_mut()) {
                spec.user_flags |= user;
            }
        }

        if f.intersects(ObjFlags::LAYOUT_1) || f.intersects(ObjFlags::LAYOUT_2) {
            self.mark_parent_and_self_layout_dirty(obj);
        }
    }

    /// `lv_obj_remove_flag()`
    pub fn obj_remove_flag(&mut self, obj: Obj, f: ObjFlags) {
        if !self.check_obj(obj) {
            return;
        }

        if f.bits() >= ObjFlags::LAYOUT_1.bits() {
            self.obj_allocate_spec_attr(obj);
        }

        /* If none of the requested flags are set, do nothing */
        self.apply_flag_setters(obj, f, false);
        let user = user_flag_bits(f);
        if user != 0 {
            if let Some(spec) = self.obj_mut(obj).and_then(|d| d.spec.as_deref_mut()) {
                spec.user_flags &= !user;
            }
        }

        if f.intersects(ObjFlags::LAYOUT_1) || f.intersects(ObjFlags::LAYOUT_2) {
            self.mark_parent_and_self_layout_dirty(obj);
        }
    }

    /// `lv_obj_set_flag()`
    pub fn obj_set_flag(&mut self, obj: Obj, f: ObjFlags, v: bool) {
        if !self.check_obj(obj) {
            return;
        }

        if v {
            self.obj_add_flag(obj, f);
        } else {
            self.obj_remove_flag(obj, f);
        }
    }

    /// `lv_obj_set_hidden()`
    pub fn obj_set_hidden(&mut self, obj: Obj, en: bool) {
        if !self.check_obj(obj) {
            return;
        }
        if self.raw_flag(obj, ObjFlags::HIDDEN) == en {
            return;
        }

        let was_on_layout = self.obj_is_layout_positioned(obj);

        /*Invalidate while still visible*/
        if !self.raw_flag(obj, ObjFlags::HIDDEN) {
            self.obj_invalidate(obj);
        }

        self.write_flag(obj, ObjFlags::HIDDEN, en);

        /*Invalidate when visible again*/
        if !self.raw_flag(obj, ObjFlags::HIDDEN) {
            self.obj_invalidate(obj);
        }

        /*Focus the next next item when this one get hidden*/
        if self.raw_flag(obj, ObjFlags::HIDDEN) && self.has_state(obj, State::FOCUSED) {
            let group = self.obj_get_group(obj);
            if let Some(group) = group {
                self.group_focus_next(group);
                let next_obj = self.group_get_focused(group);
                if let Some(next_obj) = next_obj {
                    self.obj_invalidate(next_obj);
                }
            }
        }

        if was_on_layout != self.obj_is_layout_positioned(obj) {
            self.mark_parent_and_self_layout_dirty(obj);
        }
    }
}

simple_flag_setters! {
    (obj_set_clickable, CLICKABLE, "lv_obj_set_clickable"),
    (obj_set_click_focusable, CLICK_FOCUSABLE, "lv_obj_set_click_focusable"),
    (obj_set_checkable, CHECKABLE, "lv_obj_set_checkable"),
}

impl Ui {
    /// `lv_obj_set_scrollable()`
    pub fn obj_set_scrollable(&mut self, obj: Obj, en: bool) {
        if !self.check_obj(obj) {
            return;
        }

        if self.raw_flag(obj, ObjFlags::SCROLLABLE) == en {
            return;
        }

        /*Invalidate the scrollbars while scrollable*/
        if self.raw_flag(obj, ObjFlags::SCROLLABLE) {
            let (hor_area, ver_area) = self.obj_get_scrollbar_area(obj);
            self.obj_invalidate_area(obj, &hor_area);
            self.obj_invalidate_area(obj, &ver_area);
        }

        self.write_flag(obj, ObjFlags::SCROLLABLE, en);

        /*Invalidate the scrollbars while scrollable again*/
        if self.raw_flag(obj, ObjFlags::SCROLLABLE) {
            let (hor_area, ver_area) = self.obj_get_scrollbar_area(obj);
            self.obj_invalidate_area(obj, &hor_area);
            self.obj_invalidate_area(obj, &ver_area);
        }
    }
}

simple_flag_setters! {
    (obj_set_scroll_elastic, SCROLL_ELASTIC, "lv_obj_set_scroll_elastic"),
    (obj_set_scroll_momentum, SCROLL_MOMENTUM, "lv_obj_set_scroll_momentum"),
    (obj_set_scroll_one, SCROLL_ONE, "lv_obj_set_scroll_one"),
    (obj_set_scroll_chain, SCROLL_CHAIN, "lv_obj_set_scroll_chain"),
    (obj_set_scroll_chain_hor, SCROLL_CHAIN_HOR, "lv_obj_set_scroll_chain_hor"),
    (obj_set_scroll_chain_ver, SCROLL_CHAIN_VER, "lv_obj_set_scroll_chain_ver"),
    (obj_set_scroll_on_focus, SCROLL_ON_FOCUS, "lv_obj_set_scroll_on_focus"),
    (obj_set_scroll_with_arrow, SCROLL_WITH_ARROW, "lv_obj_set_scroll_with_arrow"),
    (obj_set_snappable, SNAPPABLE, "lv_obj_set_snappable"),
    (obj_set_press_lock, PRESS_LOCK, "lv_obj_set_press_lock"),
    (obj_set_event_bubble, EVENT_BUBBLE, "lv_obj_set_event_bubble"),
    (obj_set_gesture_bubble, GESTURE_BUBBLE, "lv_obj_set_gesture_bubble"),
    (obj_set_adv_hittest, ADV_HITTEST, "lv_obj_set_adv_hittest"),
}

impl Ui {
    /// `lv_obj_set_ignore_layout()`
    pub fn obj_set_ignore_layout(&mut self, obj: Obj, en: bool) {
        self.set_layout_affecting_flag(obj, ObjFlags::IGNORE_LAYOUT, en);
    }

    /// `lv_obj_set_floating()`
    pub fn obj_set_floating(&mut self, obj: Obj, en: bool) {
        self.set_layout_affecting_flag(obj, ObjFlags::FLOATING, en);
    }
}

simple_flag_setters! {
    (obj_set_send_draw_task_events, SEND_DRAW_TASK_EVENTS, "lv_obj_set_send_draw_task_events"),
    (obj_set_overflow_visible, OVERFLOW_VISIBLE, "lv_obj_set_overflow_visible"),
    (obj_set_event_trickle, EVENT_TRICKLE, "lv_obj_set_event_trickle"),
    (obj_set_state_trickle, STATE_TRICKLE, "lv_obj_set_state_trickle"),
}

impl Ui {
    /// `lv_obj_set_radio_button()`
    pub fn obj_set_radio_button(&mut self, obj: Obj, en: bool) {
        if !self.check_obj(obj) {
            return;
        }
        if let Some(d) = self.obj_mut(obj) {
            d.radio_button = en;
        }
    }

    /// `lv_obj_set_flex_in_new_track()`
    pub fn obj_set_flex_in_new_track(&mut self, obj: Obj, en: bool) {
        if !self.check_obj(obj) {
            return;
        }

        /*TODO For backward compatibility use it as an LV_OBJ_FLAG_LAYOUT_1
         *It will have its own bit in lv_obj_t after v9.5*/
        if en {
            /*Need to allocate spec_attr to set the bit*/
            if let Some(spec) = self.obj_allocate_spec_attr(obj) {
                spec.user_flags |= 1 << 0;
            }
        } else {
            /*If spec_attr is not allocated,
             *its not needed to clear it as 0 is the default value*/
            if let Some(spec) = self.obj_mut(obj).and_then(|d| d.spec.as_deref_mut()) {
                spec.user_flags &= !(1 << 0);
            }
        }

        self.mark_parent_and_self_layout_dirty(obj);
    }

    /// `lv_obj_add_state()`
    pub fn obj_add_state(&mut self, obj: Obj, state: State) {
        if !self.check_obj(obj) {
            return;
        }

        let cur = self.raw_state(obj);
        let new_state = cur | state;
        if cur != new_state {
            self.update_obj_state(obj, new_state);
            if self.raw_flag(obj, ObjFlags::STATE_TRICKLE) {
                self.obj_children_add_state(obj, state);
            }
        }
    }

    /// `lv_obj_remove_state()`
    pub fn obj_remove_state(&mut self, obj: Obj, state: State) {
        if !self.check_obj(obj) {
            return;
        }

        let cur = self.raw_state(obj);
        let new_state = State::from_bits_retain(cur.bits() & !state.bits());
        if cur != new_state {
            self.update_obj_state(obj, new_state);
            if self.raw_flag(obj, ObjFlags::STATE_TRICKLE) {
                self.obj_children_remove_state(obj, state);
            }
        }
    }

    /// `lv_obj_set_state()`
    pub fn obj_set_state(&mut self, obj: Obj, state: State, v: bool) {
        if !self.check_obj(obj) {
            return;
        }
        self.set_state_internal(obj, state, v);
    }
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

/*=======================
 * Getter functions
 *======================*/

impl Ui {
    /// `lv_obj_has_flag()`: `true` if all of `f` are set.
    pub fn obj_has_flag(&self, obj: Obj, f: ObjFlags) -> bool {
        if !self.check_obj(obj) {
            return false;
        }
        let Some(d) = self.obj(obj) else {
            return false;
        };

        if f.bits() >= ObjFlags::LAYOUT_1.bits() && d.spec.is_none() {
            return false;
        }

        let low = f.bits() & OBJ_FLAG_BITS_MASK;
        if d.flags.bits() & low != low {
            return false;
        }
        let user = user_flag_bits(f);
        if user != 0 {
            let user_flags = d.spec.as_ref().map_or(0, |s| s.user_flags);
            if user_flags & user != user {
                return false;
            }
        }

        true
    }

    /// `lv_obj_has_flag_any()`: `true` if any of `f` is set.
    pub fn obj_has_flag_any(&self, obj: Obj, f: ObjFlags) -> bool {
        if !self.check_obj(obj) {
            return false;
        }
        let Some(d) = self.obj(obj) else {
            return false;
        };

        if d.flags.bits() & f.bits() & OBJ_FLAG_BITS_MASK != 0 {
            return true;
        }

        let Some(spec) = d.spec.as_deref() else {
            return false;
        };
        if f.bits() < ObjFlags::LAYOUT_1.bits() {
            return false;
        }

        spec.user_flags & user_flag_bits(f) != 0
    }
}

flag_getters! {
    (obj_is_hidden, HIDDEN, "lv_obj_is_hidden"),
    (obj_is_clickable, CLICKABLE, "lv_obj_is_clickable"),
    (obj_is_click_focusable, CLICK_FOCUSABLE, "lv_obj_is_click_focusable"),
    (obj_is_checkable, CHECKABLE, "lv_obj_is_checkable"),
    (obj_is_scrollable, SCROLLABLE, "lv_obj_is_scrollable"),
    (obj_is_scroll_elastic, SCROLL_ELASTIC, "lv_obj_is_scroll_elastic"),
    (obj_is_scroll_momentum, SCROLL_MOMENTUM, "lv_obj_is_scroll_momentum"),
    (obj_is_scroll_one, SCROLL_ONE, "lv_obj_is_scroll_one"),
    (obj_is_scroll_chain_hor, SCROLL_CHAIN_HOR, "lv_obj_is_scroll_chain_hor"),
    (obj_is_scroll_chain_ver, SCROLL_CHAIN_VER, "lv_obj_is_scroll_chain_ver"),
    (obj_is_scroll_on_focus, SCROLL_ON_FOCUS, "lv_obj_is_scroll_on_focus"),
    (obj_is_scroll_with_arrow, SCROLL_WITH_ARROW, "lv_obj_is_scroll_with_arrow"),
    (obj_is_snappable, SNAPPABLE, "lv_obj_is_snappable"),
    (obj_is_press_lock, PRESS_LOCK, "lv_obj_is_press_lock"),
    (obj_is_event_bubble, EVENT_BUBBLE, "lv_obj_is_event_bubble"),
    (obj_is_gesture_bubble, GESTURE_BUBBLE, "lv_obj_is_gesture_bubble"),
    (obj_is_adv_hittest, ADV_HITTEST, "lv_obj_is_adv_hittest"),
    (obj_is_ignore_layout, IGNORE_LAYOUT, "lv_obj_is_ignore_layout"),
    (obj_is_floating, FLOATING, "lv_obj_is_floating"),
    (obj_is_send_draw_task_events, SEND_DRAW_TASK_EVENTS, "lv_obj_is_send_draw_task_events"),
    (obj_is_overflow_visible, OVERFLOW_VISIBLE, "lv_obj_is_overflow_visible"),
    (obj_is_event_trickle, EVENT_TRICKLE, "lv_obj_is_event_trickle"),
    (obj_is_state_trickle, STATE_TRICKLE, "lv_obj_is_state_trickle"),
}

#[allow(clippy::collapsible_if)] // keep the C `if` nesting
impl Ui {
    /// `lv_obj_is_radio_button()`
    pub fn obj_is_radio_button(&self, obj: Obj) -> bool {
        if !self.check_obj(obj) {
            return false;
        }
        self.obj(obj).is_some_and(|d| d.radio_button)
    }

    /// `lv_obj_is_flex_in_new_track()`
    pub fn obj_is_flex_in_new_track(&self, obj: Obj) -> bool {
        if !self.check_obj(obj) {
            return false;
        }

        /*For backward compatibility use it as as LV_OBJ_FLAG_LAYOUT_1
         *It will have it's on bin in lv_obj_t after v9.5*/
        let Some(spec) = self.obj(obj).and_then(|d| d.spec.as_deref()) else {
            return false;
        };

        spec.user_flags & (1 << 0) != 0
    }

    /// `lv_obj_get_state()`
    pub fn obj_get_state(&self, obj: Obj) -> State {
        if !self.check_obj(obj) {
            return State::DEFAULT;
        }

        self.raw_state(obj)
    }

    /// `lv_obj_has_state()`
    pub fn obj_has_state(&self, obj: Obj, state: State) -> bool {
        if !self.check_obj(obj) {
            return false;
        }
        self.has_state(obj, state)
    }

    /// `lv_obj_get_group()`
    pub fn obj_get_group(&self, obj: Obj) -> Option<GroupId> {
        if !self.check_obj(obj) {
            return None;
        }

        self.obj(obj)?.spec.as_deref()?.group
    }

    /*-------------------
     * OTHER FUNCTIONS
     *------------------*/

    /// `lv_obj_allocate_spec_attr()`: allocate the rarely used attributes
    /// (`scroll_dir = LV_DIR_ALL`, `scrollbar_mode = LV_SCROLLBAR_MODE_AUTO`,
    /// everything else zero) if not allocated yet.
    pub fn obj_allocate_spec_attr(&mut self, obj: Obj) -> Option<&mut SpecAttr> {
        if !self.check_obj(obj) {
            return None;
        }

        let d = self.obj_mut(obj)?;
        if d.spec.is_none() {
            d.spec = Some(Box::new(SpecAttr::new()));
        }
        d.spec.as_deref_mut()
    }

    /// `lv_obj_check_type()`: exact class match.
    pub fn obj_check_type(&self, obj: Obj, class: &'static ObjClass) -> bool {
        let Some(d) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return false;
        };
        core::ptr::eq(d.class, class)
    }

    /// `lv_obj_has_class()`: class or any base class matches.
    pub fn obj_has_class(&self, obj: Obj, class: &'static ObjClass) -> bool {
        let Some(d) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return false;
        };

        let mut obj_class = Some(d.class);
        while let Some(c) = obj_class {
            if core::ptr::eq(c, class) {
                return true;
            }
            obj_class = c.base;
        }

        false
    }

    /// `lv_obj_get_class()`
    ///
    /// For a stale handle (the C `NULL` object) the argument check fails and
    /// [`OBJ_CLASS`] is returned, as the signature has no `NULL`.
    pub fn obj_get_class(&self, obj: Obj) -> &'static ObjClass {
        match self.obj(obj) {
            Some(d) => d.class,
            None => {
                check_arg_failed("obj != NULL");
                &OBJ_CLASS
            }
        }
    }

    /// `lv_obj_is_valid()` / `lv_obj_is_in_widget_tree()`: the handle refers
    /// to a live object.
    pub fn obj_is_valid(&self, obj: Obj) -> bool {
        self.objs.contains(obj.0.0)
    }

    /// `lv_obj_is_in_widget_tree()`: walk up to the screen and check that
    /// the screen belongs to a display.
    pub fn obj_is_in_widget_tree(&self, obj: Obj) -> bool {
        let Some(mut d) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return false;
        };

        let mut current = obj.id();
        while let Some(parent) = d.parent {
            current = parent;
            match self.objs.get(parent.0) {
                Some(p) => d = p,
                None => return false,
            }
        }

        /* current is now a screen (parent == NULL); verify it belongs to a display */
        for disp in &self.display_order {
            if let Some(disp) = self.displays.get(disp.0) {
                if disp.screens.contains(&current) {
                    return true;
                }
            }
        }

        false
    }

    /// `lv_obj_null_on_delete()`: set `obj_ptr` to `None` when the object it
    /// holds is deleted (`obj_ptr` replaces the `lv_obj_t **`).
    pub fn obj_null_on_delete(&mut self, obj_ptr: Rc<Cell<Option<Obj>>>) {
        let Some(obj) = obj_ptr.get() else {
            check_arg_failed("*obj_ptr != NULL");
            return;
        };

        let cb: EventCallback = Rc::new(null_on_delete_cb);
        let user_data: Rc<dyn Any> = obj_ptr;
        self.obj_add_event_cb(obj, cb, EventCode::DELETE, Some(user_data));
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
        if !self.check_obj(obj) {
            return;
        }

        let dsc: Rc<dyn Any> = Rc::new(ScreenLoadAnimDsc {
            anim_type,
            duration,
            delay,
            target: ScreenLoadTarget::Screen(screen),
        });

        let cb: EventCallback = Rc::new(screen_load_on_trigger_event_cb);
        self.obj_add_event_cb(obj, cb, trigger, Some(dsc.clone()));
        let free_cb: EventCallback = Rc::new(event_free_user_data_cb);
        self.obj_add_event_cb(obj, free_cb, EventCode::DELETE, Some(dsc));
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
        if !self.check_obj(obj) {
            return;
        }

        let dsc: Rc<dyn Any> = Rc::new(ScreenLoadAnimDsc {
            anim_type,
            duration,
            delay,
            target: ScreenLoadTarget::CreateCb(screen_create_cb),
        });

        let cb: EventCallback = Rc::new(screen_create_on_trigger_event_cb);
        self.obj_add_event_cb(obj, cb, trigger, Some(dsc.clone()));
        let free_cb: EventCallback = Rc::new(event_free_user_data_cb);
        self.obj_add_event_cb(obj, free_cb, EventCode::DELETE, Some(dsc));
    }

    /// `lv_obj_set_user_data()`
    pub fn obj_set_user_data(&mut self, obj: Obj, user_data: Option<Box<dyn Any>>) {
        if !self.check_obj(obj) {
            return;
        }
        if let Some(d) = self.obj_mut(obj) {
            d.user_data = user_data;
        }
    }

    /// `lv_obj_set_user_flag()`
    pub fn obj_set_user_flag(&mut self, obj: Obj, bit: u32, v: bool) {
        if !self.check_obj(obj) {
            return;
        }
        if bit >= 4 {
            check_arg_failed("bit < 4");
            return;
        }

        let Some(spec_attr) = self.obj_allocate_spec_attr(obj) else {
            return;
        };

        if v {
            spec_attr.user_flags |= 1 << (bit + 4);
        } else {
            spec_attr.user_flags &= !(1 << (bit + 4));
        }
    }

    /// `lv_obj_get_user_data()`
    pub fn obj_get_user_data(&self, obj: Obj) -> Option<&dyn Any> {
        if !self.check_obj(obj) {
            return None;
        }

        self.obj(obj)?.user_data.as_deref()
    }

    /// Mutable `lv_obj_get_user_data()`.
    pub fn obj_get_user_data_mut(&mut self, obj: Obj) -> Option<&mut dyn Any> {
        if !self.check_obj(obj) {
            return None;
        }

        self.obj_mut(obj)?.user_data.as_deref_mut()
    }

    /// `lv_obj_get_user_flag()`
    pub fn obj_get_user_flag(&self, obj: Obj, bit: u32) -> bool {
        if !self.check_obj(obj) {
            return false;
        }
        if bit >= 4 {
            check_arg_failed("bit < 4");
            return false;
        }

        match self.obj(obj).and_then(|d| d.spec.as_deref()) {
            None => false,
            Some(spec) => spec.user_flags & (1 << (bit + 4)) != 0,
        }
    }

    /// `lv_obj_add_delete_cb()`: call `cb` once when the object is deleted
    /// (on `LV_EVENT_DELETE`); the callback removes itself afterwards.
    ///
    /// Returns the descriptor to pass to [`Ui::obj_remove_delete_cb`], or
    /// `None` (C `NULL`) for an invalid object. The C `user_data` argument
    /// is captured by the `cb` closure instead.
    pub fn obj_add_delete_cb(&mut self, obj: Obj, cb: DeleteCallback) -> Option<DeleteDscRef> {
        if !self.check_obj(obj) {
            return None;
        }

        let event_cb: EventCallback = Rc::new(call_delete_cb);
        let dsc: Rc<dyn Any> = Rc::new(DeleteDsc {
            obj,
            cb,
            event_cb: event_cb.clone(),
        });

        /*`lv_obj_add_event_cb()` can't fail (no allocation failure in Rust)*/
        self.obj_add_event_cb(obj, event_cb, EventCode::DELETE, Some(dsc.clone()));
        Some(DeleteDscRef(dsc))
    }

    /// `lv_obj_remove_delete_cb()`: cancel a callback added with
    /// [`Ui::obj_add_delete_cb`]. `None` (C `NULL`) is ignored.
    pub fn obj_remove_delete_cb(&mut self, dsc: Option<&DeleteDscRef>) {
        let Some(dsc) = dsc else {
            return;
        };
        let Some(d) = dsc.0.downcast_ref::<DeleteDsc>() else {
            return;
        };
        let (obj, event_cb) = (d.obj, d.event_cb.clone());

        let count = self.obj_remove_event_cb_with_user_data(obj, &event_cb, &dsc.0);
        #[cfg(feature = "log")]
        if count == 0 {
            log::warn!("Delete callback descriptor not found on object or already removed");
        }
        #[cfg(not(feature = "log"))]
        let _ = count;
        /*`lv_free(dsc)` (only when found): the descriptor is released with its last `Rc`*/
    }

    /// `lv_obj_get_id()` (`LV_USE_OBJ_ID`): the object id of `obj` (0 for
    /// `NULL`).
    #[cfg(feature = "obj-id")]
    #[cfg_attr(docsrs, doc(cfg(feature = "obj-id")))]
    pub fn obj_get_id(&self, obj: Obj) -> usize {
        let Some(d) = self.obj(obj) else {
            check_arg_failed("lv_obj_get_id: obj != NULL");
            return 0;
        };
        d.id
    }

    /// `lv_obj_find_by_id()` (`LV_USE_OBJ_ID`): the first descendant of
    /// `obj` (the active screen of the default display for `None`) whose id
    /// equals `id` by `lv_obj_id_compare()`, depth first.
    #[cfg(feature = "obj-id")]
    #[cfg_attr(docsrs, doc(cfg(feature = "obj-id")))]
    #[deprecated(note = "IDs are used only to print the widget trees. To find a widget use obj_find_by_name")]
    pub fn obj_find_by_id(&self, obj: Option<Obj>, id: usize) -> Option<Obj> {
        if id == 0 {
            check_arg_failed("lv_obj_find_by_id: id != NULL");
            return None;
        }
        #[cfg(feature = "log")]
        log::warn!("IDs are used only to print the widget trees. To find a widget use obj_name");
        let obj = match obj {
            Some(obj) => obj,
            /*`lv_display_get_screen_active(lv_display_get_default())`*/
            None => self
                .display_get_default()
                .and_then(|d| self.display_get_screen_active(Some(d)))?,
        };
        self.obj_find_by_id_core(obj, id)
    }

    /// `obj_find_by_id()` (static)
    #[cfg(feature = "obj-id")]
    fn obj_find_by_id_core(&self, obj: Obj, id: usize) -> Option<Obj> {
        let child_cnt = self.obj_get_child_count(obj);
        for i in 0..child_cnt {
            let child = self.obj_get_child(obj, i as i32)?;
            if crate::core::obj_id::obj_id_compare(self.obj(child)?.id, id) == 0 {
                return Some(child);
            }
            let found = self.obj_find_by_id_core(child, id);
            if found.is_some() {
                return found;
            }
        }
        None
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

/**********************
 *   STATIC FUNCTIONS
 **********************/

/// The [`SpecAttr::user_flags`] bits of `f` (`LAYOUT_1` .. `USER_4`).
#[inline]
const fn user_flag_bits(f: ObjFlags) -> u8 {
    (f.bits() >> USER_FLAGS_SHIFT) as u8
}

impl Ui {
    /// `LV_CHECK_OBJ(obj, MY_CLASS, ...)`: every class derives from
    /// `lv_obj_class`, so only the `obj != NULL` part (a live handle) can
    /// fail.
    #[inline]
    fn check_obj(&self, obj: Obj) -> bool {
        if self.objs.contains(obj.0.0) {
            true
        } else {
            check_arg_failed("obj != NULL");
            false
        }
    }

    /// The bit field of `obj` (`obj->hidden` etc.), `false` for a stale handle.
    #[inline]
    fn raw_flag(&self, obj: Obj, flag: ObjFlags) -> bool {
        self.obj(obj).is_some_and(|d| d.flags.contains(flag))
    }

    /// `obj->state`, empty for a stale handle.
    #[inline]
    fn raw_state(&self, obj: Obj) -> State {
        self.obj(obj).map_or(State::DEFAULT, |d| d.state)
    }

    /// Write `obj-><flag> = en`.
    #[inline]
    fn write_flag(&mut self, obj: Obj, flag: ObjFlags, en: bool) {
        if let Some(d) = self.obj_mut(obj) {
            d.flags.set(flag, en);
        }
    }

    /// Body of the trivial `lv_obj_set_<flag>()` setters.
    fn set_flag_bit(&mut self, obj: Obj, flag: ObjFlags, en: bool) {
        if !self.check_obj(obj) {
            return;
        }
        self.write_flag(obj, flag, en);
    }

    /// Body of the `lv_obj_is_<flag>()` getters.
    fn get_flag_bit(&self, obj: Obj, flag: ObjFlags) -> bool {
        if !self.check_obj(obj) {
            return false;
        }
        self.raw_flag(obj, flag)
    }

    /// Body of `lv_obj_set_ignore_layout()` / `lv_obj_set_floating()`.
    fn set_layout_affecting_flag(&mut self, obj: Obj, flag: ObjFlags, en: bool) {
        if !self.check_obj(obj) {
            return;
        }

        if self.raw_flag(obj, flag) == en {
            return;
        }

        let was_on_layout = self.obj_is_layout_positioned(obj);

        self.write_flag(obj, flag, en);

        if was_on_layout != self.obj_is_layout_positioned(obj) {
            self.mark_parent_and_self_layout_dirty(obj);
        }
    }

    /// `lv_obj_mark_layout_as_dirty(lv_obj_get_parent(obj));
    /// lv_obj_mark_layout_as_dirty(obj);` (a screen has no parent to mark).
    fn mark_parent_and_self_layout_dirty(&mut self, obj: Obj) {
        let parent = self.obj(obj).and_then(|d| d.parent).map(Obj::from);
        if let Some(parent) = parent {
            self.obj_mark_layout_as_dirty(parent);
        }
        self.obj_mark_layout_as_dirty(obj);
    }

    /// The per-flag setter calls of `lv_obj_add_flag()` /
    /// `lv_obj_remove_flag()`, in upstream order.
    fn apply_flag_setters(&mut self, obj: Obj, f: ObjFlags, en: bool) {
        if f.intersects(ObjFlags::HIDDEN) {
            self.obj_set_hidden(obj, en);
        }
        if f.intersects(ObjFlags::CLICKABLE) {
            self.obj_set_clickable(obj, en);
        }
        if f.intersects(ObjFlags::CLICK_FOCUSABLE) {
            self.obj_set_click_focusable(obj, en);
        }
        if f.intersects(ObjFlags::CHECKABLE) {
            self.obj_set_checkable(obj, en);
        }
        if f.intersects(ObjFlags::SCROLLABLE) {
            self.obj_set_scrollable(obj, en);
        }
        if f.intersects(ObjFlags::SCROLL_ELASTIC) {
            self.obj_set_scroll_elastic(obj, en);
        }
        if f.intersects(ObjFlags::SCROLL_MOMENTUM) {
            self.obj_set_scroll_momentum(obj, en);
        }
        if f.intersects(ObjFlags::SCROLL_ONE) {
            self.obj_set_scroll_one(obj, en);
        }
        if f.intersects(ObjFlags::SCROLL_CHAIN_HOR) {
            self.obj_set_scroll_chain_hor(obj, en);
        }
        if f.intersects(ObjFlags::SCROLL_CHAIN_VER) {
            self.obj_set_scroll_chain_ver(obj, en);
        }
        if f.intersects(ObjFlags::SCROLL_ON_FOCUS) {
            self.obj_set_scroll_on_focus(obj, en);
        }
        if f.intersects(ObjFlags::SCROLL_WITH_ARROW) {
            self.obj_set_scroll_with_arrow(obj, en);
        }
        if f.intersects(ObjFlags::SNAPPABLE) {
            self.obj_set_snappable(obj, en);
        }
        if f.intersects(ObjFlags::PRESS_LOCK) {
            self.obj_set_press_lock(obj, en);
        }
        if f.intersects(ObjFlags::EVENT_BUBBLE) {
            self.obj_set_event_bubble(obj, en);
        }
        if f.intersects(ObjFlags::GESTURE_BUBBLE) {
            self.obj_set_gesture_bubble(obj, en);
        }
        if f.intersects(ObjFlags::ADV_HITTEST) {
            self.obj_set_adv_hittest(obj, en);
        }
        if f.intersects(ObjFlags::IGNORE_LAYOUT) {
            self.obj_set_ignore_layout(obj, en);
        }
        if f.intersects(ObjFlags::FLOATING) {
            self.obj_set_floating(obj, en);
        }
        if f.intersects(ObjFlags::SEND_DRAW_TASK_EVENTS) {
            self.obj_set_send_draw_task_events(obj, en);
        }
        if f.intersects(ObjFlags::OVERFLOW_VISIBLE) {
            self.obj_set_overflow_visible(obj, en);
        }
        if f.intersects(ObjFlags::EVENT_TRICKLE) {
            self.obj_set_event_trickle(obj, en);
        }
        if f.intersects(ObjFlags::STATE_TRICKLE) {
            self.obj_set_state_trickle(obj, en);
        }
    }

    /// `lv_obj_get_child_count()` read straight from `spec_attr`.
    #[inline]
    fn child_count(&self, obj: Obj) -> u32 {
        self.obj(obj)
            .and_then(|d| d.spec.as_deref())
            .map_or(0, |s| s.children.len() as u32)
    }

    /// `lv_obj_get_style_<prop>_internal()` for `bool` properties.
    #[inline]
    fn style_bool(&self, obj: Obj, part: Part, prop: StyleProp) -> bool {
        self.style_num(obj, part, prop) != 0
    }

    /// `obj->coords` grown by `transform_width` / `transform_height` of
    /// `LV_PART_MAIN`.
    fn transformed_coords(&self, obj: Obj) -> Area {
        let w = self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_WIDTH);
        let h = self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_HEIGHT);
        let mut coords = self.obj(obj).map(|d| d.coords).unwrap_or_default();
        coords.increase(w, h);
        coords
    }

    /// `update_obj_state()`: set the state (fully overwrite) of an object.
    /// If specified in the styles, transition animations are started from
    /// the previous state to the current.
    fn update_obj_state(&mut self, obj: Obj, new_state: State) {
        let Some(d) = self.obj(obj) else {
            return;
        };

        if d.state == new_state {
            return;
        }

        let prev_state = d.state;

        let (cmp_res, changed_part) = self.obj_style_state_compare(obj, prev_state, new_state);
        /*If there is no difference in styles there is nothing else to do*/
        if cmp_res == StyleStateCmp::Same {
            if let Some(d) = self.obj_mut(obj) {
                d.state = new_state;
            }
            self.obj_send_event(obj, EventCode::STATE_CHANGED, EventParam::State(prev_state));
            return;
        }

        /*Invalidate the object in their current state*/
        self.obj_invalidate(obj);

        if let Some(d) = self.obj_mut(obj) {
            d.state = new_state;
        }
        self.obj_update_layer_type(obj);
        self.obj_update_blur_status(obj);

        /*Skip transitions if the widget is not rendered yet. */
        if !self.obj(obj).is_some_and(|d| d.rendered) {
            self.obj_invalidate(obj);
            if cmp_res == StyleStateCmp::DiffDrawPad {
                self.obj_refresh_ext_draw_size(obj);
            }

            self.obj_send_event(obj, EventCode::STATE_CHANGED, EventParam::State(prev_state));
            return;
        }

        self.obj_transition_states(obj, prev_state, new_state);

        if cmp_res == StyleStateCmp::DiffRedraw && changed_part == Part::SCROLLBAR {
            /*A redraw-only change confined to the scrollbar: refresh just that part.
             *lv_obj_refresh_style() skips the child-subtree refresh for
             *LV_PART_SCROLLBAR, avoiding a full-subtree style cascade. This is safe
             *because the scrollbar is drawn by lv_obj and has no styled children.*/
            self.obj_refresh_style(obj, Part::SCROLLBAR, StyleProp::ANY);
        } else if cmp_res == StyleStateCmp::DiffRedraw {
            /*Invalidation is not enough, e.g. layer type needs to be updated too*/
            self.obj_refresh_style(obj, Part::ANY, StyleProp::ANY);
        } else if cmp_res == StyleStateCmp::DiffLayout {
            self.obj_refresh_style(obj, Part::ANY, StyleProp::ANY);
        } else if cmp_res == StyleStateCmp::DiffDrawPad {
            self.obj_invalidate(obj);
            self.obj_refresh_ext_draw_size(obj);
        }

        self.obj_send_event(obj, EventCode::STATE_CHANGED, EventParam::State(prev_state));
    }

    /// `lv_style_get_prop_internal(obj_style->style, prop, &v)`
    fn obj_style_get_prop(&self, obj_style: &ObjStyle, prop: StyleProp) -> Option<StyleValue> {
        match &obj_style.src {
            ObjStyleSrc::Local(s) | ObjStyleSrc::Trans(s) => s.get_prop(prop).cloned(),
            ObjStyleSrc::Shared(id) => self.styles.get(id.0)?.get_prop(prop).cloned(),
            ObjStyleSrc::Const(c) => c.get_prop(prop),
        }
    }

    /// `obj_transition_states()`
    fn obj_transition_states(&mut self, obj: Obj, prev_state: State, new_state: State) {
        let mut ts: Vec<ObjStyleTransitionDsc> = Vec::with_capacity(STYLE_TRANSITION_MAX);
        if let Some(d) = self.obj(obj) {
            let style_cnt = d.styles.len();
            let mut i = 0;
            while i < style_cnt && ts.len() < STYLE_TRANSITION_MAX {
                let obj_style = &d.styles[i];
                i += 1;
                let state_act = obj_style.selector.state();
                let part_act = obj_style.selector.part();
                if state_act.bits() & !new_state.bits() != 0 {
                    continue; /*Skip unrelated styles*/
                }
                if obj_style.is_trans() {
                    continue;
                }

                let Some(StyleValue::Transition(tr)) =
                    self.obj_style_get_prop(obj_style, StyleProp::TRANSITION)
                else {
                    continue;
                };

                /*Add the props to the set if not added yet or added but with smaller weight*/
                let props: &[StyleProp] = &tr.props;
                let mut j = 0;
                while j < props.len() && props[j] != StyleProp::INV && ts.len() < STYLE_TRANSITION_MAX {
                    let prop = props[j];
                    j += 1;
                    let mut found = false;
                    for t in ts.iter() {
                        let state_ts = t.selector.state();
                        let part_ts = t.selector.part();
                        if t.prop == prop && part_ts == part_act && state_ts.bits() >= state_act.bits() {
                            found = true;
                            break;
                        }
                    }

                    /*If not found  add it*/
                    if !found {
                        ts.push(ObjStyleTransitionDsc {
                            path: tr.path,
                            user_data: tr.user_data.clone(),
                            selector: obj_style.selector,
                            time: tr.time as u16,
                            delay: tr.delay as u16,
                            prop,
                        });
                    }
                }
            }
        }

        for t in &ts {
            let part_act = t.selector.part();
            self.obj_style_create_transition(obj, part_act, prev_state, new_state, t);
        }
    }

    /// `lv_obj_children_add_state()`: apply the state to the children of
    /// the object.
    fn obj_children_add_state(&mut self, obj: Obj, state: State) {
        let child_count = self.child_count(obj);

        for i in 0..child_count {
            if let Some(child) = self.child_at(obj, i) {
                self.obj_add_state(child, state);
            }
        }
    }

    /// `lv_obj_children_remove_state()`: remove the state from the children
    /// of the object.
    fn obj_children_remove_state(&mut self, obj: Obj, state: State) {
        let child_count = self.child_count(obj);

        for i in 0..child_count {
            if let Some(child) = self.child_at(obj, i) {
                self.obj_remove_state(child, state);
            }
        }
    }

    /// `set_state()`
    #[inline]
    fn set_state_internal(&mut self, obj: Obj, state: State, value: bool) {
        if value {
            self.obj_add_state(obj, state);
        } else {
            self.obj_remove_state(obj, state);
        }
    }

    /// `has_state()`: `!!(obj->state & state)`.
    #[inline]
    fn has_state(&self, obj: Obj, state: State) -> bool {
        self.raw_state(obj).intersects(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::draw::{DrawDsc, DrawTaskType};

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

    #[test]
    fn user_flag_bits_follow_spec_attr_layout() {
        // lv_obj.c: LAYOUT_1 -> user_flags bit 0 ... USER_4 -> bit 7.
        assert_eq!(user_flag_bits(ObjFlags::LAYOUT_1), 1 << 0);
        assert_eq!(user_flag_bits(ObjFlags::LAYOUT_2), 1 << 1);
        assert_eq!(user_flag_bits(ObjFlags::WIDGET_1), 1 << 2);
        assert_eq!(user_flag_bits(ObjFlags::WIDGET_2), 1 << 3);
        assert_eq!(user_flag_bits(ObjFlags::USER_1), 1 << 4);
        assert_eq!(user_flag_bits(ObjFlags::USER_4), 1 << 7);
        assert_eq!(user_flag_bits(ObjFlags::STATE_TRICKLE), 0);
        assert_eq!(OBJ_FLAG_BITS_MASK, (1 << 23) - 1);
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

    static TEST_DERIVED_CLASS: ObjClass = ObjClass {
        base: Some(&OBJ_CLASS),
        name: "test_derived",
        new_state: || Box::new(()),
        constructor: None,
        destructor: None,
        event_cb: None,
        width_def: 10,
        height_def: 10,
        editable: ObjClassEditable::Inherit,
        group_def: ObjClassGroupDef::Inherit,
        theme_inheritable: true,
    };

    static TEST_OTHER_CLASS: ObjClass = ObjClass {
        base: Some(&OBJ_CLASS),
        name: "test_other",
        new_state: || Box::new(()),
        constructor: None,
        destructor: None,
        event_cb: None,
        width_def: 10,
        height_def: 10,
        editable: ObjClassEditable::Inherit,
        group_def: ObjClassGroupDef::Inherit,
        theme_inheritable: true,
    };

    /// A zeroed object like `lv_obj_class_create_obj()` allocates, without
    /// running any constructor.
    fn raw_obj(ui: &mut Ui, class: &'static ObjClass, parent: Option<Obj>) -> Obj {
        let data = ObjData {
            ext_data: Default::default(),
            class,
            parent: parent.map(|p| p.id()),
            spec: None,
            styles: Vec::new(),
            style_main_prop_is_set: 0,
            style_other_prop_is_set: 0,
            user_data: None,
            coords: Area::new(0, 0, 0, 0),
            flags: ObjFlags::empty(),
            radio_button: false,
            state: State::DEFAULT,
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
            #[cfg(feature = "obj-id")]
            id: 0,
            widget: (class.new_state)(),
        };
        let obj = Obj(ObjId(ui.objs.insert(data)));
        if let Some(p) = parent {
            ui.obj_allocate_spec_attr(p).unwrap().children.push(obj.id());
        }
        obj
    }

    #[test]
    fn constructor_sets_screen_attributes() {
        let mut ui = Ui::new(UiConfig::default());
        let scr = raw_obj(&mut ui, &OBJ_CLASS, None);
        obj_constructor(&OBJ_CLASS, &mut ui, scr);
        assert!(ui.obj_is_clickable(scr));
        assert!(ui.obj_is_snappable(scr));
        assert!(ui.obj_is_click_focusable(scr));
        assert!(ui.obj_is_scrollable(scr));
        assert!(ui.obj_is_scroll_elastic(scr));
        assert!(ui.obj_is_scroll_momentum(scr));
        assert!(ui.obj_is_scroll_with_arrow(scr));
        // Only objects with a parent get these.
        assert!(!ui.obj_is_press_lock(scr));
        assert!(!ui.obj_is_scroll_chain_hor(scr));
        assert!(!ui.obj_is_scroll_chain_ver(scr));
        assert!(!ui.obj_is_gesture_bubble(scr));
        assert!(!ui.obj_is_hidden(scr));
        assert!(!ui.obj_is_checkable(scr));
        assert!(ui.obj(scr).unwrap().spec.is_none());
    }

    #[test]
    fn simple_flag_setters_and_getters() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = raw_obj(&mut ui, &OBJ_CLASS, None);

        macro_rules! check {
            ($set:ident, $is:ident, $flag:ident) => {
                ui.$set(obj, true);
                assert!(ui.$is(obj));
                assert!(ui.obj_has_flag(obj, ObjFlags::$flag));
                ui.$set(obj, false);
                assert!(!ui.$is(obj));
                assert!(!ui.obj_has_flag_any(obj, ObjFlags::$flag));
            };
        }
        check!(obj_set_clickable, obj_is_clickable, CLICKABLE);
        check!(obj_set_click_focusable, obj_is_click_focusable, CLICK_FOCUSABLE);
        check!(obj_set_checkable, obj_is_checkable, CHECKABLE);
        check!(obj_set_scroll_elastic, obj_is_scroll_elastic, SCROLL_ELASTIC);
        check!(obj_set_scroll_momentum, obj_is_scroll_momentum, SCROLL_MOMENTUM);
        check!(obj_set_scroll_one, obj_is_scroll_one, SCROLL_ONE);
        check!(
            obj_set_scroll_chain_hor,
            obj_is_scroll_chain_hor,
            SCROLL_CHAIN_HOR
        );
        check!(
            obj_set_scroll_chain_ver,
            obj_is_scroll_chain_ver,
            SCROLL_CHAIN_VER
        );
        check!(obj_set_scroll_on_focus, obj_is_scroll_on_focus, SCROLL_ON_FOCUS);
        check!(
            obj_set_scroll_with_arrow,
            obj_is_scroll_with_arrow,
            SCROLL_WITH_ARROW
        );
        check!(obj_set_snappable, obj_is_snappable, SNAPPABLE);
        check!(obj_set_press_lock, obj_is_press_lock, PRESS_LOCK);
        check!(obj_set_event_bubble, obj_is_event_bubble, EVENT_BUBBLE);
        check!(obj_set_gesture_bubble, obj_is_gesture_bubble, GESTURE_BUBBLE);
        check!(obj_set_adv_hittest, obj_is_adv_hittest, ADV_HITTEST);
        check!(
            obj_set_send_draw_task_events,
            obj_is_send_draw_task_events,
            SEND_DRAW_TASK_EVENTS
        );
        check!(
            obj_set_overflow_visible,
            obj_is_overflow_visible,
            OVERFLOW_VISIBLE
        );
        check!(obj_set_event_trickle, obj_is_event_trickle, EVENT_TRICKLE);
        check!(obj_set_state_trickle, obj_is_state_trickle, STATE_TRICKLE);

        ui.obj_set_scroll_chain(obj, true);
        assert!(ui.obj_has_flag(obj, ObjFlags::SCROLL_CHAIN));
        ui.obj_set_scroll_chain(obj, false);
        assert!(!ui.obj_has_flag_any(obj, ObjFlags::SCROLL_CHAIN));

        ui.obj_set_radio_button(obj, true);
        assert!(ui.obj_is_radio_button(obj));
        ui.obj_set_radio_button(obj, false);
        assert!(!ui.obj_is_radio_button(obj));
    }

    #[test]
    fn add_and_remove_flag_dispatch_to_setters() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = raw_obj(&mut ui, &OBJ_CLASS, None);

        ui.obj_add_flag(obj, ObjFlags::CLICKABLE | ObjFlags::EVENT_BUBBLE);
        assert!(ui.obj_is_clickable(obj));
        assert!(ui.obj_is_event_bubble(obj));
        assert!(ui.obj_has_flag(obj, ObjFlags::CLICKABLE | ObjFlags::EVENT_BUBBLE));
        // Low flags never allocate spec_attr.
        assert!(ui.obj(obj).unwrap().spec.is_none());

        ui.obj_add_flag(obj, ObjFlags::USER_2 | ObjFlags::WIDGET_1);
        assert_eq!(
            ui.obj(obj).unwrap().spec.as_ref().unwrap().user_flags,
            (1 << 5) | (1 << 2)
        );
        assert!(ui.obj_has_flag(obj, ObjFlags::USER_2));
        assert!(ui.obj_has_flag(obj, ObjFlags::USER_2 | ObjFlags::CLICKABLE));
        assert!(!ui.obj_has_flag(obj, ObjFlags::USER_2 | ObjFlags::USER_3));
        assert!(ui.obj_has_flag_any(obj, ObjFlags::USER_2 | ObjFlags::USER_3));
        assert!(ui.obj_get_user_flag(obj, 1));
        assert!(!ui.obj_get_user_flag(obj, 0));

        ui.obj_remove_flag(obj, ObjFlags::USER_2 | ObjFlags::CLICKABLE);
        assert!(!ui.obj_is_clickable(obj));
        assert!(ui.obj_is_event_bubble(obj));
        assert!(!ui.obj_has_flag_any(obj, ObjFlags::USER_2));
        assert!(ui.obj_has_flag(obj, ObjFlags::WIDGET_1));

        ui.obj_set_flag(obj, ObjFlags::CHECKABLE, true);
        assert!(ui.obj_is_checkable(obj));
        ui.obj_set_flag(obj, ObjFlags::CHECKABLE, false);
        assert!(!ui.obj_is_checkable(obj));
    }

    #[test]
    fn high_flags_without_spec_attr_are_unset() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = raw_obj(&mut ui, &OBJ_CLASS, None);
        // `f >= LV_OBJ_FLAG_LAYOUT_1 && spec_attr == NULL` -> false.
        assert!(!ui.obj_has_flag(obj, ObjFlags::LAYOUT_1));
        assert!(!ui.obj_has_flag(obj, ObjFlags::USER_1));
        assert!(!ui.obj_has_flag_any(obj, ObjFlags::USER_1 | ObjFlags::WIDGET_2));
        assert!(!ui.obj_is_flex_in_new_track(obj));
        assert!(!ui.obj_get_user_flag(obj, 3));
        // An empty flag set is "all set" (`lv_obj_has_flag(obj, 0)`).
        assert!(ui.obj_has_flag(obj, ObjFlags::empty()));
        assert!(!ui.obj_has_flag_any(obj, ObjFlags::empty()));
        // has_flag_any still finds low flags without spec_attr.
        ui.obj_set_clickable(obj, true);
        assert!(ui.obj_has_flag_any(obj, ObjFlags::CLICKABLE | ObjFlags::USER_1));
        assert!(!ui.obj_has_flag(obj, ObjFlags::CLICKABLE | ObjFlags::USER_1));
    }

    #[test]
    fn user_flags_set_and_get() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = raw_obj(&mut ui, &OBJ_CLASS, None);
        for bit in 0..4 {
            ui.obj_set_user_flag(obj, bit, true);
            assert!(ui.obj_get_user_flag(obj, bit));
        }
        assert_eq!(ui.obj(obj).unwrap().spec.as_ref().unwrap().user_flags, 0xF0);
        assert!(ui.obj_has_flag(
            obj,
            ObjFlags::USER_1 | ObjFlags::USER_2 | ObjFlags::USER_3 | ObjFlags::USER_4
        ));
        ui.obj_set_user_flag(obj, 2, false);
        assert!(!ui.obj_get_user_flag(obj, 2));
        assert!(!ui.obj_has_flag(obj, ObjFlags::USER_3));
        assert_eq!(ui.obj(obj).unwrap().spec.as_ref().unwrap().user_flags, 0xB0);
    }

    #[test]
    fn spec_attr_defaults() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = raw_obj(&mut ui, &OBJ_CLASS, None);
        let spec = ui.obj_allocate_spec_attr(obj).unwrap();
        assert_eq!(spec.scroll_dir, Dir::ALL);
        assert_eq!(spec.scrollbar_mode, ScrollbarMode::Auto);
        assert_eq!(spec.user_flags, 0);
        assert!(spec.group.is_none());
        spec.user_flags = 0x11;
        // A second call returns the existing attributes.
        assert_eq!(ui.obj_allocate_spec_attr(obj).unwrap().user_flags, 0x11);
        assert!(ui.obj_get_group(obj).is_none());
    }

    #[test]
    fn class_checks() {
        let mut ui = Ui::new(UiConfig::default());
        let base = raw_obj(&mut ui, &OBJ_CLASS, None);
        let derived = raw_obj(&mut ui, &TEST_DERIVED_CLASS, Some(base));

        assert!(ui.obj_check_type(base, &OBJ_CLASS));
        assert!(!ui.obj_check_type(base, &TEST_DERIVED_CLASS));
        assert!(ui.obj_check_type(derived, &TEST_DERIVED_CLASS));
        assert!(!ui.obj_check_type(derived, &OBJ_CLASS));

        assert!(ui.obj_has_class(derived, &TEST_DERIVED_CLASS));
        assert!(ui.obj_has_class(derived, &OBJ_CLASS));
        assert!(!ui.obj_has_class(derived, &TEST_OTHER_CLASS));
        assert!(!ui.obj_has_class(base, &TEST_DERIVED_CLASS));

        assert!(core::ptr::eq(ui.obj_get_class(derived), &TEST_DERIVED_CLASS));
        assert!(core::ptr::eq(ui.obj_get_class(base), &OBJ_CLASS));
        assert!(ui.obj_is_valid(derived));
    }

    #[test]
    fn widget_tree_needs_a_display() {
        let mut ui = Ui::new(UiConfig::default());
        let scr = raw_obj(&mut ui, &OBJ_CLASS, None);
        let child = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        // No display owns `scr`.
        assert!(!ui.obj_is_in_widget_tree(scr));
        assert!(!ui.obj_is_in_widget_tree(child));
    }

    #[test]
    fn state_getters_read_the_state_bits() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = raw_obj(&mut ui, &OBJ_CLASS, None);
        ui.obj_mut(obj).unwrap().state = State::CHECKED | State::USER_3;
        assert_eq!(ui.obj_get_state(obj), State::CHECKED | State::USER_3);
        assert!(ui.obj_is_checked(obj));
        assert!(ui.obj_is_state_user_3(obj));
        assert!(!ui.obj_is_pressed(obj));
        assert!(ui.obj_has_state(obj, State::CHECKED | State::PRESSED));
        assert!(!ui.obj_has_state(obj, State::PRESSED));
        // Setting a state that is already set (or clearing an unset one) is
        // a no-op that never reaches update_obj_state().
        ui.obj_add_state(obj, State::CHECKED);
        ui.obj_remove_state(obj, State::PRESSED);
        ui.obj_set_checked(obj, true);
        ui.obj_set_pressed(obj, false);
        assert_eq!(ui.obj_get_state(obj), State::CHECKED | State::USER_3);
    }

    #[test]
    fn user_data_roundtrip() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = raw_obj(&mut ui, &OBJ_CLASS, None);
        assert!(ui.obj_get_user_data(obj).is_none());
        ui.obj_set_user_data(obj, Some(Box::new(5u32)));
        assert_eq!(
            ui.obj_get_user_data(obj).and_then(|d| d.downcast_ref::<u32>()),
            Some(&5)
        );
        *ui.obj_get_user_data_mut(obj)
            .unwrap()
            .downcast_mut::<u32>()
            .unwrap() = 9;
        assert_eq!(
            ui.obj_get_user_data(obj).and_then(|d| d.downcast_ref::<u32>()),
            Some(&9)
        );
        ui.obj_set_user_data(obj, None);
        assert!(ui.obj_get_user_data(obj).is_none());
    }

    #[test]
    fn children_helpers_read_spec_attr() {
        let mut ui = Ui::new(UiConfig::default());
        let scr = raw_obj(&mut ui, &OBJ_CLASS, None);
        assert_eq!(ui.child_count(scr), 0);
        assert!(ui.child_at(scr, 0).is_none());
        let a = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let b = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        assert_eq!(ui.child_count(scr), 2);
        assert_eq!(ui.child_at(scr, 0), Some(a));
        assert_eq!(ui.child_at(scr, 1), Some(b));
        assert!(ui.child_at(scr, 2).is_none());
    }

    #[test]
    fn transitions_without_styles_do_nothing() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = raw_obj(&mut ui, &OBJ_CLASS, None);
        ui.obj_transition_states(obj, State::DEFAULT, State::PRESSED);
        assert!(ui.style_trans.is_empty());
    }

    #[test]
    fn add_child_appends_to_spec_attr() {
        let mut ui = Ui::new(UiConfig::default());
        let scr = raw_obj(&mut ui, &OBJ_CLASS, None);
        let a = raw_obj(&mut ui, &OBJ_CLASS, None);
        let b = raw_obj(&mut ui, &OBJ_CLASS, None);

        /*`lv_obj_class_create_obj()` allocates the parent's spec_attr first*/
        assert_eq!(obj_add_child(&mut ui, scr, a), LvResult::Invalid);
        ui.obj_allocate_spec_attr(scr);
        assert_eq!(obj_add_child(&mut ui, scr, a), LvResult::Ok);
        assert_eq!(obj_add_child(&mut ui, scr, b), LvResult::Ok);
        assert_eq!(ui.child_count(scr), 2);
        assert_eq!(ui.child_at(scr, 0), Some(a));
        assert_eq!(ui.child_at(scr, 1), Some(b));
    }

    #[test]
    fn remove_child_shifts_from_the_index_and_frees_the_last() {
        let mut ui = Ui::new(UiConfig::default());
        let scr = raw_obj(&mut ui, &OBJ_CLASS, None);
        let a = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let b = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let c = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        assert_eq!(obj_get_index_raw(&ui, b), 1);
        assert_eq!(obj_get_index_raw(&ui, scr), -1);

        obj_remove_child(&mut ui, scr, b);
        assert_eq!(ui.child_count(scr), 2);
        assert_eq!(ui.child_at(scr, 0), Some(a));
        assert_eq!(ui.child_at(scr, 1), Some(c));

        obj_remove_child(&mut ui, scr, a);
        assert_eq!(ui.child_count(scr), 1);
        assert_eq!(ui.child_at(scr, 0), Some(c));

        /*"No more children": the array is freed, spec_attr stays*/
        obj_remove_child(&mut ui, scr, c);
        assert_eq!(ui.child_count(scr), 0);
        let spec = ui.obj(scr).unwrap().spec.as_deref().unwrap();
        assert_eq!(spec.children.capacity(), 0);
    }

    #[test]
    fn remove_delete_cb_ignores_none() {
        let mut ui = Ui::new(UiConfig::default());
        let obj = raw_obj(&mut ui, &OBJ_CLASS, None);
        ui.obj_remove_delete_cb(None);
        assert!(ui.obj_is_valid(obj));
    }

    fn blur_dsc(blur_radius: i32) -> BlurDsc {
        BlurDsc {
            base: crate::draw::DrawDscBase::new(),
            blur_radius,
            corner_radius: 3,
            quality: crate::misc::style::BlurQuality::Auto,
        }
    }

    #[test]
    fn obj_blur_runs_the_full_task_finalization() {
        let mut ui = Ui::new(UiConfig::default());
        let layer = ui.draw.layer_create(
            None,
            crate::misc::color::ColorFormat::Argb8888,
            &Area::new(0, 0, 99, 99),
        );
        let coords = Area::new(10, 10, 29, 29);

        /*`blur_radius <= 0`: no task*/
        ui.draw_blur(layer, &blur_dsc(0), &coords);
        assert!(ui.draw.layer(layer).unwrap().tasks.is_empty());

        /*Keep the task in the layer: with a running task the finalization
         *only evaluates*/
        ui.draw.task_running = true;
        ui.draw_blur(layer, &blur_dsc(10), &coords);
        ui.draw.task_running = false;
        {
            let tasks = &ui.draw.layer(layer).unwrap().tasks;
            assert_eq!(tasks.len(), 1);
            let task = &tasks[0];
            assert_eq!(task.ty, DrawTaskType::Blur);
            assert_eq!(task.area, coords);
            /*The draw units' evaluation ran (`DrawCtx::draw_blur` skips it)*/
            assert_eq!(task.preference_score, 100);
            match &task.dsc {
                DrawDsc::Blur(d) => {
                    assert_eq!(d.blur_radius, 10);
                    assert_eq!(d.corner_radius, 3);
                    assert_eq!(d.base.layer, Some(layer));
                }
                _ => panic!("not a blur descriptor"),
            }
        }

        /*Not taken by any draw unit: failed, then removed by the dispatch that
         *ends the finalization (`DrawCtx::draw_blur` would leave it waiting)*/
        ui.draw_blur(layer, &blur_dsc(10), &coords);
        let tasks = &ui.draw.layer(layer).unwrap().tasks;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].state, crate::draw::DrawTaskState::Waiting);
    }
}

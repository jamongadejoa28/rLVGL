//! Small helpers shared by the example ports: C `static` storage per [`Ui`],
//! `static lv_style_t` slots and the `LV_SYMBOL_*` strings the examples use.

use core::any::Any;
use core::marker::PhantomData;

use alloc::rc::Rc;

use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::misc::area::PointPrecise;
use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Event, Obj, Style, Ui};

/// `lv_screen_active()`. The examples, like their C originals, need a default
/// display with an active screen.
pub(crate) fn screen_active(ui: &Ui) -> Obj {
    ui.screen_active()
        .expect("lv_screen_active(): the examples need a default display")
}

/// The `void * user_data` of an event callback registered with an object.
pub(crate) fn user_obj(e: &Event<'_>) -> Option<Obj> {
    e.user_data().and_then(|d| d.downcast_ref::<Obj>()).copied()
}

/// `(void *) obj` as event / timer user data.
pub(crate) fn obj_data(obj: Obj) -> Option<Rc<dyn Any>> {
    Some(Rc::new(obj))
}

/// `lv_point_precise_t {x, y}` from integer coordinates, for `static` point arrays.
#[allow(dead_code)]
pub(crate) const fn pp(x: i32, y: i32) -> PointPrecise {
    PointPrecise::new(x as ValuePrecise, y as ValuePrecise)
}

/// `static const char * map[]` of a button matrix. A `NULL` terminator is
/// the end of the slice; an `""` terminator is kept as an entry.
#[allow(dead_code)]
pub(crate) fn btnm_map(entries: &[&'static str]) -> ferrovgl_widgets::ButtonmatrixMap {
    use ferrovgl_core::misc::style::{SharedRef, SharedStr};
    SharedRef::Shared(
        entries
            .iter()
            .map(|s| SharedRef::Static(*s))
            .collect::<Rc<[SharedStr]>>(),
    )
}

/// Storage of a group of `static lv_style_t` variables of one example. The
/// marker type `M` keeps every example's styles apart.
struct StaticStyles<M, const N: usize> {
    ids: Option<[StyleId; N]>,
    _marker: PhantomData<fn() -> M>,
}

impl<M, const N: usize> Default for StaticStyles<M, N> {
    fn default() -> Self {
        Self {
            ids: None,
            _marker: PhantomData,
        }
    }
}

/// The C pattern
/// `static lv_style_t s; static bool inited = false; if(!inited) { lv_style_init(&s); ...; inited = true; }`:
/// `init` builds the styles on the first call for this [`Ui`]; later calls
/// return the same styles untouched.
pub(crate) fn styles_once<M: 'static, const N: usize>(
    ui: &mut Ui,
    init: impl FnOnce() -> [Style; N],
) -> [StyleId; N] {
    if let Some(ids) = ui.widget_static::<StaticStyles<M, N>>().and_then(|s| s.ids) {
        return ids;
    }
    let ids = init().map(|style| ui.style_create(style));
    ui.widget_static_mut::<StaticStyles<M, N>>().ids = Some(ids);
    ids
}

/// The styles of `M` created by [`styles_once`] / [`styles_reinit`], if any
/// (for callbacks that read a file-scope `static lv_style_t`).
#[allow(dead_code)]
pub(crate) fn static_styles<M: 'static, const N: usize>(ui: &Ui) -> Option<[StyleId; N]> {
    ui.widget_static::<StaticStyles<M, N>>().and_then(|s| s.ids)
}

/// The C pattern `static lv_style_t s; lv_style_init(&s); lv_style_set_...(&s, ...);`
/// without an `inited` guard: every call re-initializes the same static
/// styles (objects created by an earlier call keep referencing them), without
/// reporting the change, exactly like `lv_style_init()` on a used style.
pub(crate) fn styles_reinit<M: 'static, const N: usize>(
    ui: &mut Ui,
    init: impl FnOnce() -> [Style; N],
) -> [StyleId; N] {
    let styles = init();
    if let Some(ids) = ui.widget_static::<StaticStyles<M, N>>().and_then(|s| s.ids) {
        for (id, style) in ids.iter().zip(styles) {
            if let Some(dst) = ui.style_mut(*id) {
                *dst = style;
            }
        }
        return ids;
    }
    let ids = styles.map(|style| ui.style_create(style));
    ui.widget_static_mut::<StaticStyles<M, N>>().ids = Some(ids);
    ids
}

/// The `#else` branch of the `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` examples
/// (`lv_example_style_gradient_{buttons,conic,radial}`): a circularly
/// scrolling label reporting the missing feature.
#[cfg(not(feature = "complex-gradients"))]
pub(crate) fn complex_gradients_not_enabled(ui: &mut Ui) {
    use ferrovgl_core::misc::types::coord;
    use ferrovgl_widgets::{Label, LabelLongMode};

    let label = Label::create(ui, screen_active(ui));
    ui.obj_set_width(label.obj(), coord::pct(80));
    label.set_text(ui, "LV_USE_DRAW_SW_COMPLEX_GRADIENTS is not enabled");
    label.set_long_mode(ui, LabelLongMode::ScrollCircular);
    ui.obj_center(label.obj());
}

/// `LV_SYMBOL_*` strings (`include/lvgl/font/lv_symbol_def.h`) used by the examples.
#[allow(dead_code)]
pub(crate) mod symbol {
    /// `LV_SYMBOL_OK`
    pub(crate) const OK: &str = "\u{F00C}";
    /// `LV_SYMBOL_CLOSE`
    pub(crate) const CLOSE: &str = "\u{F00D}";
    /// `LV_SYMBOL_LEFT`
    pub(crate) const LEFT: &str = "\u{F053}";
    /// `LV_SYMBOL_RIGHT`
    pub(crate) const RIGHT: &str = "\u{F054}";
    /// `LV_SYMBOL_DIRECTORY`
    pub(crate) const DIRECTORY: &str = "\u{F07B}";
    /// `LV_SYMBOL_SAVE`
    pub(crate) const SAVE: &str = "\u{F0C7}";
    /// `LV_SYMBOL_GPS`
    pub(crate) const GPS: &str = "\u{F124}";
    /// `LV_SYMBOL_FILE`
    pub(crate) const FILE: &str = "\u{F158}";
    /// `LV_SYMBOL_BATTERY_FULL`
    pub(crate) const BATTERY_FULL: &str = "\u{F240}";
    /// `LV_SYMBOL_USB`
    pub(crate) const USB: &str = "\u{F287}";
    /// `LV_SYMBOL_BLUETOOTH`
    pub(crate) const BLUETOOTH: &str = "\u{F293}";
    /// `LV_SYMBOL_EDIT`
    pub(crate) const EDIT: &str = "\u{F304}";
    /// `LV_SYMBOL_BACKSPACE`
    pub(crate) const BACKSPACE: &str = "\u{F55A}";
    /// `LV_SYMBOL_NEW_LINE`
    pub(crate) const NEW_LINE: &str = "\u{F8A2}";
}

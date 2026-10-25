//! Ported from LVGL v9.6.0: `src/widgets/checkbox/lv_checkbox.c`.
//!
//! Also covers `include/lvgl/widgets/lv_checkbox.h` and
//! `src/widgets/checkbox/lv_checkbox_private.h`.
//!
//! Like the label widget, the `lv_checkbox_set_text(obj, NULL)` form is
//! [`Checkbox::refresh_text`]. The parity configuration enables
//! `LV_USE_ARABIC_PERSIAN_CHARS` (the `arabic-persian-chars` feature): the text
//! is copied through `lv_text_ap_proc()`. `LV_USE_OBJ_PROPERTY` is disabled in the
//! parity configuration, so the property table is not provided.

use alloc::boxed::Box;
use alloc::rc::Rc;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::draw::RectDsc;
use ferrovgl_core::draw::label::LabelDsc;
use ferrovgl_core::font;
use ferrovgl_core::misc::event::LvResult;
use ferrovgl_core::misc::style::{SharedRef, SharedStr};
use ferrovgl_core::misc::text::{self, BaseDir, TextAttributes, TextFlags};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Area, Event, EventCode, Obj, ObjClass, Part, Point, Ui};

/// The constructor's text with `LV_WIDGETS_HAS_DEFAULT_VALUE` enabled.
const CHECKBOX_DEFAULT_TEXT: &str = "Check box";

/// `lv_checkbox_t` without its `lv_obj_t` header. Zeroed like
/// `lv_malloc_zeroed`; the constructor sets the initial text.
#[derive(Clone, Debug, Default)]
pub(crate) struct CheckboxState {
    /// `txt` (`NULL` = `None`): static or owned text.
    pub(crate) txt: Option<SharedStr>,
    /// `static_txt : 1`
    pub(crate) static_txt: bool,
}

ferrovgl_core::impl_widget_state!(CheckboxState);

/// `lv_checkbox_class`.
pub static CHECKBOX_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_checkbox",
    new_state: || Box::new(CheckboxState::default()),
    constructor: Some(constructor),
    destructor: Some(destructor),
    event_cb: Some(checkbox_event),
    width_def: coord::SIZE_CONTENT,
    height_def: coord::SIZE_CONTENT,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::True,
    theme_inheritable: false,
};

/// A copyable checkbox handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Checkbox(Obj);

impl Checkbox {
    /// `lv_checkbox_create()`. `None` creates a checkbox screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&CHECKBOX_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    #[must_use]
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the checkbox class.
    #[must_use]
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        check_obj(ui, obj).then_some(Self(obj))
    }

    /// `lv_checkbox_set_text()`: copy the text, stopping at the first NUL like
    /// `lv_strcpy`.
    pub fn set_text(self, ui: &mut Ui, text: &str) {
        set_text_internal(ui, self.0, Some(text));
    }

    /// The `lv_checkbox_set_text(obj, NULL)` form: keep the text and only
    /// refresh the self size and redraw the widget.
    pub fn refresh_text(self, ui: &mut Ui) {
        set_text_internal(ui, self.0, None);
    }

    /// `lv_checkbox_set_text_static()`: use the text without copying it.
    pub fn set_text_static(self, ui: &mut Ui, text: &'static str) {
        let Some(cb) = state_mut(ui, self.0) else {
            return;
        };
        // `if(!cb->static_txt) lv_free(cb->txt);` is the drop of the old text.
        cb.txt = Some(SharedRef::Static(text));
        cb.static_txt = true;

        ui.obj_refresh_self_size(self.0);
        ui.obj_invalidate(self.0);
    }

    /// `lv_checkbox_get_text()`. A missing text (`NULL`) is returned as `""`.
    #[must_use]
    pub fn text(self, ui: &Ui) -> &str {
        state(ui, self.0).and_then(|cb| cb.txt.as_deref()).unwrap_or("")
    }
}

impl From<Checkbox> for Obj {
    fn from(checkbox: Checkbox) -> Self {
        checkbox.obj()
    }
}

/// `LV_CHECK_OBJ(obj, MY_CLASS, ...)`: a valid object of the checkbox class.
fn check_obj(ui: &Ui, obj: Obj) -> bool {
    ui.obj_is_valid(obj) && ui.obj_has_class(obj, &CHECKBOX_CLASS)
}

fn state(ui: &Ui, obj: Obj) -> Option<&CheckboxState> {
    if !check_obj(ui, obj) {
        return None;
    }
    ui.obj(obj)?.widget.get::<CheckboxState>()
}

fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut CheckboxState> {
    if !check_obj(ui, obj) {
        return None;
    }
    ui.obj_mut(obj)?.widget.get_mut::<CheckboxState>()
}

/// Body of `lv_checkbox_set_text()`; `None` is the `NULL` text argument.
fn set_text_internal(ui: &mut Ui, obj: Obj, txt: Option<&str>) {
    // LV_CHECK_OBJ(obj, MY_CLASS, return);
    let Some(cb) = state_mut(ui, obj) else {
        return;
    };

    if let Some(txt) = txt {
        // `lv_text_ap_proc()` (`LV_USE_ARABIC_PERSIAN_CHARS`) / `lv_strcpy` stop at the
        // first NUL.
        #[cfg(feature = "arabic-persian-chars")]
        let processed = ferrovgl_core::misc::text_ap::ap_proc(txt);
        #[cfg(feature = "arabic-persian-chars")]
        let txt: &str = &processed;
        #[cfg(not(feature = "arabic-persian-chars"))]
        let txt = txt.split('\0').next().unwrap_or("");
        cb.txt = Some(SharedRef::Shared(Rc::from(txt)));
        cb.static_txt = false;
    }

    ui.obj_refresh_self_size(obj);
    ui.obj_invalidate(obj);
}

/// `lv_checkbox_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    if let Some(cb) = state_mut(ui, obj) {
        // LV_WIDGETS_HAS_DEFAULT_VALUE
        cb.txt = Some(SharedRef::Static(CHECKBOX_DEFAULT_TEXT));
        cb.static_txt = true;
    }

    ui.obj_set_clickable(obj, true);
    ui.obj_set_checkable(obj, true);
    ui.obj_set_scroll_on_focus(obj, true);
    ui.obj_set_scrollable(obj, false);
}

/// `lv_checkbox_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    if let Some(cb) = state_mut(ui, obj)
        && !cb.static_txt
    {
        cb.txt = None;
    }
}

/// The text attributes both `LV_EVENT_GET_SELF_SIZE` and the draw use.
fn text_attributes(ui: &Ui, obj: Obj) -> TextAttributes {
    TextAttributes {
        line_space: ui.obj_get_style_text_line_space(obj, Part::MAIN),
        letter_space: ui.obj_get_style_text_letter_space(obj, Part::MAIN),
        max_width: coord::MAX,
        text_flags: TextFlags::NONE,
    }
}

/// `lv_checkbox_event()`
fn checkbox_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    /*Call the ancestor's event handler*/
    if ui.obj_event_base(&CHECKBOX_CLASS, event) != LvResult::Ok {
        return;
    }

    let Some(obj) = event.current_target_obj() else {
        return;
    };

    match event.code {
        EventCode::GET_SELF_SIZE => {
            let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN) else {
                return;
            };
            let font_h = font::get_line_height(font.as_font());
            let attributes = text_attributes(ui, obj);

            let txt_size = text::get_size_attributes(Checkbox(obj).text(ui), font.as_font(), &attributes);

            let bg_colp = ui.obj_get_style_pad_column(obj, Part::MAIN);
            let marker_leftp = ui.obj_get_style_pad_left(obj, Part::INDICATOR);
            let marker_rightp = ui.obj_get_style_pad_right(obj, Part::INDICATOR);
            let marker_topp = ui.obj_get_style_pad_top(obj, Part::INDICATOR);
            let marker_bottomp = ui.obj_get_style_pad_bottom(obj, Part::INDICATOR);
            let marker_size = Point::new(
                font_h + marker_leftp + marker_rightp,
                font_h + marker_topp + marker_bottomp,
            );

            if let Some(p) = event.self_size_info() {
                p.x = marker_size.x + txt_size.x + bg_colp;
                p.y = marker_size.y.max(txt_size.y);
            }
        }
        EventCode::REFR_EXT_DRAW_SIZE => {
            let m = ui.obj_calculate_ext_draw_size(obj, Part::INDICATOR);
            event.set_ext_draw_size(m);
        }
        EventCode::DRAW_MAIN => checkbox_draw(ui, obj, event),
        _ => {}
    }
}

/// `lv_checkbox_draw()`
fn checkbox_draw(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(layer) = event.layer() else {
        return;
    };
    let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN) else {
        return;
    };
    let font_h = font::get_line_height(font.as_font());

    let is_rtl = BaseDir::Rtl == ui.obj_get_style_base_dir(obj, Part::MAIN);

    let bg_border = ui.obj_get_style_border_width(obj, Part::MAIN);
    let bg_topp = ui.obj_get_style_pad_top(obj, Part::MAIN) + bg_border;
    // Upstream adds the border width only to the left padding.
    let bg_p = if is_rtl {
        ui.obj_get_style_pad_right(obj, Part::MAIN)
    } else {
        ui.obj_get_style_pad_left(obj, Part::MAIN) + bg_border
    };
    let bg_colp = ui.obj_get_style_pad_column(obj, Part::MAIN);

    let marker_leftp = ui.obj_get_style_pad_left(obj, Part::INDICATOR);
    let marker_rightp = ui.obj_get_style_pad_right(obj, Part::INDICATOR);
    let marker_topp = ui.obj_get_style_pad_top(obj, Part::INDICATOR);
    let marker_bottomp = ui.obj_get_style_pad_bottom(obj, Part::INDICATOR);

    let transf_w = ui.obj_get_style_transform_width(obj, Part::INDICATOR);
    let transf_h = ui.obj_get_style_transform_height(obj, Part::INDICATOR);

    let mut indic_dsc = RectDsc::new();
    indic_dsc.base.layer = Some(layer);
    ui.obj_init_draw_rect_dsc(obj, Part::INDICATOR, &mut indic_dsc);

    let coords = ui.obj_get_coords(obj);
    let mut marker_area = Area::default();
    if is_rtl {
        marker_area.x2 = coords.x2 - bg_p;
        marker_area.x1 = marker_area.x2 - font_h - marker_leftp - marker_rightp + 1;
    } else {
        marker_area.x1 = coords.x1 + bg_p;
        marker_area.x2 = marker_area.x1 + font_h + marker_leftp + marker_rightp - 1;
    }
    marker_area.y1 = coords.y1 + bg_topp;
    marker_area.y2 = marker_area.y1 + font_h + marker_topp + marker_bottomp - 1;

    let mut marker_area_transf = marker_area;
    marker_area_transf.increase(transf_w, transf_h);

    ui.draw_rect(layer, &indic_dsc, &marker_area_transf);

    // The draw may run user callbacks (`LV_EVENT_DRAW_TASK_ADDED`).
    if !ui.obj_is_valid(obj) {
        return;
    }

    let attributes = text_attributes(ui, obj);
    let txt = state(ui, obj).and_then(|cb| cb.txt.clone());
    let txt_str: &str = txt.as_deref().unwrap_or("");
    let txt_size = text::get_size_attributes(txt_str, font.as_font(), &attributes);

    let mut txt_dsc = LabelDsc::new();
    txt_dsc.base.layer = Some(layer);
    ui.obj_init_draw_label_dsc(obj, Part::MAIN, &mut txt_dsc);
    txt_dsc.text = match &txt {
        Some(SharedRef::Shared(rc)) => rc.clone(),
        Some(SharedRef::Static(s)) => Rc::from(*s),
        None => Rc::from(""),
    };

    // `obj->coords` is read again here: a draw task handler may have moved the
    // object during `lv_draw_rect()`.
    let coords = ui.obj_get_coords(obj);
    let y_ofs = (marker_area.height() - font_h) / 2;
    let mut txt_area = Area::default();
    if is_rtl {
        txt_area.x2 = marker_area.x1 - bg_colp;
        txt_area.x1 = txt_area.x2 - txt_size.x;
    } else {
        txt_area.x1 = marker_area.x2 + bg_colp;
        txt_area.x2 = txt_area.x1 + txt_size.x;
    }
    txt_area.y1 = coords.y1 + bg_topp + y_ofs;
    txt_area.y2 = txt_area.y1 + txt_size.y;

    ui.draw_label(layer, &txt_dsc, &txt_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::{FontRef, State, UiConfig};

    fn setup() -> (Ui, Obj) {
        let mut ui = Ui::new(UiConfig {
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        });
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        (ui, screen)
    }

    #[test]
    fn constructor_sets_flags_and_default_text() {
        let (mut ui, screen) = setup();
        let group = ui.group_create();
        ui.group_set_default(Some(group));
        let cb = Checkbox::create(&mut ui, screen);
        assert!(ui.obj_check_type(cb.obj(), &CHECKBOX_CLASS));
        assert!(ui.obj_is_clickable(cb.obj()));
        assert!(ui.obj_is_checkable(cb.obj()));
        assert!(ui.obj_is_scroll_on_focus(cb.obj()));
        assert!(!ui.obj_is_scrollable(cb.obj()));
        assert_eq!(ui.group_get_focused(group), Some(cb.obj()));
        assert_eq!(cb.text(&ui), CHECKBOX_DEFAULT_TEXT);
        assert!(state(&ui, cb.obj()).unwrap().static_txt);
        assert_eq!(Checkbox::from_obj(&ui, cb.obj()), Some(cb));
        assert_eq!(Checkbox::from_obj(&ui, screen), None);
        ui.obj_delete(cb.obj());
        assert_eq!(Checkbox::from_obj(&ui, cb.obj()), None);
        assert_eq!(cb.text(&ui), "");
    }

    #[test]
    fn text_storage_follows_static_flag() {
        let (mut ui, screen) = setup();
        let cb = Checkbox::create(&mut ui, screen);
        cb.set_text(&mut ui, "Hello\0World");
        assert_eq!(cb.text(&ui), "Hello");
        assert!(!state(&ui, cb.obj()).unwrap().static_txt);
        cb.refresh_text(&mut ui);
        assert_eq!(cb.text(&ui), "Hello");
        assert!(!state(&ui, cb.obj()).unwrap().static_txt);
        static TEXT: &str = "Static";
        cb.set_text_static(&mut ui, TEXT);
        assert_eq!(cb.text(&ui).as_ptr(), TEXT.as_ptr());
        assert!(state(&ui, cb.obj()).unwrap().static_txt);
        cb.refresh_text(&mut ui);
        assert_eq!(cb.text(&ui).as_ptr(), TEXT.as_ptr());
        // The API rejects other classes like LV_CHECK_OBJ.
        Checkbox(screen).set_text(&mut ui, "x");
        assert_eq!(Checkbox(screen).text(&ui), "");
    }

    #[test]
    fn self_size_matches_marker_and_text() {
        let (mut ui, screen) = setup();
        let cb = Checkbox::create(&mut ui, screen);
        ui.obj_update_layout(cb.obj());
        let font = ui.obj_get_style_text_font(cb.obj(), Part::MAIN).unwrap();
        let font_h = font::get_line_height(font.as_font());
        let attributes = text_attributes(&ui, cb.obj());
        let txt = text::get_size_attributes(CHECKBOX_DEFAULT_TEXT, font.as_font(), &attributes);
        // No theme: every padding is zero.
        assert_eq!(ui.obj_get_width(cb.obj()), font_h + txt.x);
        assert_eq!(ui.obj_get_height(cb.obj()), font_h.max(txt.y));
        ui.obj_add_state(cb.obj(), State::CHECKED);
        assert!(ui.obj_has_state(cb.obj(), State::CHECKED));
    }
}

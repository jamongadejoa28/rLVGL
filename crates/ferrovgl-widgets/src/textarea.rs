//! Ported from LVGL v9.6.0: `src/widgets/textarea/lv_textarea.c`.
//!
//! Also covers `include/lvgl/widgets/lv_textarea.h` and
//! `src/widgets/textarea/lv_textarea_private.h`. The text area is a base
//! object with a label child (`lv_textarea_t::label`); the parity
//! configuration enables `LV_LABEL_TEXT_SELECTION`, so the selection fields
//! are always present. The file-scope `ta_insert_replace` static is a
//! `TextareaStatics` stored with [`Ui::widget_static_mut`].
//! `LV_USE_OBJ_PROPERTY` is disabled in the parity configuration, so the
//! property table is not provided. `LV_BIG_ENDIAN_SYSTEM` is 0, so the
//! byte-swapping branch of `add_char()` is not ported.
//!
//! Adaptations to Rust's ownership and `&str` model (documented at each site):
//!
//! - Strings stop at their first NUL byte like the C strings they replace.
//! - `update_cursor_position_on_click()` writes the label's selection through
//!   [`Label::set_text_selection_start`]/[`Label::set_text_selection_end`]
//!   instead of the `lv_label_t` fields; those also invalidate the label,
//!   which lies inside the text area that is invalidated right after.
//! - A packed character whose bytes are not valid UTF-8 still goes through
//!   `LV_EVENT_INSERT` (with a lossy copy as the event text) and the insert
//!   replace handling, but `add_char()` rejects it where C would call
//!   `lv_label_ins_text()` with the raw bytes.
//! - Where C would keep using a text area that a user callback deleted (after
//!   `LV_EVENT_INSERT`, `LV_EVENT_VALUE_CHANGED` or a layout update) the port
//!   stops instead.
//! - Allocation failure paths (`LV_ASSERT_MALLOC`) do not exist.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::{String, ToString};

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::draw::RectDsc;
use ferrovgl_core::draw::label::LabelDsc;
use ferrovgl_core::font;
use ferrovgl_core::group::{
    KEY_BACKSPACE, KEY_DEL, KEY_DOWN, KEY_END, KEY_ENTER, KEY_HOME, KEY_LEFT, KEY_RIGHT, KEY_UP,
};
use ferrovgl_core::misc::anim::{ANIM_OFF, ANIM_ON, ANIM_REPEAT_INFINITE, AnimExec, AnimTarget};
use ferrovgl_core::misc::color::OPA_MIN;
use ferrovgl_core::misc::event::EventCallback;
use ferrovgl_core::misc::style::SharedStr;
use ferrovgl_core::misc::text::{self, TextAlign, TextFlags};
use ferrovgl_core::{
    Align, Anim, AnimPath, Area, Event, EventCode, EventParam, IndevType, LvResult, Obj, ObjClass, Part,
    Point, StyleProp, StyleSelector, Ui, coord,
};

use crate::label::{LABEL_TEXT_SELECTION_OFF, Label};

/// `LV_TEXTAREA_CURSOR_LAST`: put the cursor after the last character.
pub const TEXTAREA_CURSOR_LAST: i32 = 0x7FFF;

/// `LV_PART_TEXTAREA_PLACEHOLDER` (= `LV_PART_CUSTOM_FIRST`): the
/// placeholder text part.
pub const PART_TEXTAREA_PLACEHOLDER: Part = Part::CUSTOM_FIRST;

/// `LV_TEXTAREA_DEF_CURSOR_BLINK_TIME` (ms). The cursor blink time comes
/// from the `anim_duration` style of `LV_PART_CURSOR`; upstream defines but
/// never reads this value either.
#[allow(dead_code)]
const TEXTAREA_DEF_CURSOR_BLINK_TIME: u32 = 400;
/// `LV_TEXTAREA_DEF_PWD_SHOW_TIME` (ms).
const TEXTAREA_DEF_PWD_SHOW_TIME: u32 = 1500;
/// `LV_TEXTAREA_PWD_BULLET_UNICODE`
const TEXTAREA_PWD_BULLET_UNICODE: u32 = 0x2022;
/// `IGNORE_KERNING`
const IGNORE_KERNING: u32 = 0;
/// `LV_SYMBOL_BULLET`
const SYMBOL_BULLET: &str = "\u{2022}";

/// Identity of `cursor_blink_anim_cb` for `lv_anim_delete()`.
const ANIM_TAG_CURSOR_BLINK: u32 = 0x4C54_4143;
/// Identity of `pwd_char_hider_anim` for `lv_anim_delete()`.
const ANIM_TAG_PWD_HIDER: u32 = 0x4C54_4150;

/// The file-scope statics of `lv_textarea.c`, one instance per [`Ui`]
/// (`ui.widget_static_mut::<TextareaStatics>()`).
#[derive(Clone, Debug, Default)]
pub(crate) struct TextareaStatics {
    /// `ta_insert_replace` (`NULL` = `None`): the replacement set by
    /// `lv_textarea_set_insert_replace()` during `LV_EVENT_INSERT`.
    pub(crate) ta_insert_replace: Option<SharedStr>,
}

/// The `cursor` struct of `lv_textarea_t`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TextareaCursor {
    /// `valid_x`: used when stepping up/down to a shorter line.
    pub(crate) valid_x: i32,
    /// `pos`: the cursor position (0: before the 1st letter, ...).
    pub(crate) pos: u32,
    /// `area`: cursor area relative to the text area.
    pub(crate) area: Area,
    /// `txt_byte_pos`: byte index of the letter after (on) the cursor.
    pub(crate) txt_byte_pos: u32,
    /// `show : 1`: the cursor is visible now.
    pub(crate) show: bool,
    /// `click_pos : 1`: position the cursor by clicking the text area.
    pub(crate) click_pos: bool,
}

/// `lv_textarea_t` without its `lv_obj_t` header. Zeroed like
/// `lv_malloc_zeroed`; the constructor sets the initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct TextareaState {
    /// `label`: the label of the text area.
    pub(crate) label: Option<Obj>,
    /// `placeholder_txt` (`NULL` = `None`): only visible if the text is empty.
    pub(crate) placeholder_txt: Option<String>,
    /// `placeholder_txt_size`: size of the placeholder text.
    pub(crate) placeholder_txt_size: Point,
    /// `pwd_tmp` (`NULL` = `None`): the original text in password mode.
    pub(crate) pwd_tmp: Option<String>,
    /// `pwd_bullet` (`NULL` = `None`): the replacement shown in password mode.
    pub(crate) pwd_bullet: Option<String>,
    /// `accepted_chars` (`NULL` = `None`: accept all); static or owned, see
    /// `static_accepted_chars`.
    pub(crate) accepted_chars: Option<SharedStr>,
    /// `max_length`: the max. number of characters, 0: no limit.
    pub(crate) max_length: u32,
    /// `pwd_show_time`: ms to show characters in password mode.
    pub(crate) pwd_show_time: u32,
    /// `cursor`
    pub(crate) cursor: TextareaCursor,
    /// `sel_start` (`LV_LABEL_TEXT_SELECTION`)
    pub(crate) sel_start: u32,
    /// `sel_end` (`LV_LABEL_TEXT_SELECTION`)
    pub(crate) sel_end: u32,
    /// `text_sel_in_prog : 1`: the user is selecting.
    pub(crate) text_sel_in_prog: bool,
    /// `text_sel_en : 1`: text can be selected.
    pub(crate) text_sel_en: bool,
    /// `pwd_mode : 1`: replace characters with bullets.
    pub(crate) pwd_mode: bool,
    /// `one_line : 1`: one line mode (ignore line breaks).
    pub(crate) one_line: bool,
    /// `static_accepted_chars : 1`: only a pointer is saved in
    /// `accepted_chars`.
    pub(crate) static_accepted_chars: bool,
}

ferrovgl_core::impl_widget_state!(TextareaState);

/// `lv_textarea_class`.
pub static TEXTAREA_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_textarea",
    new_state: || Box::new(TextareaState::default()),
    constructor: Some(constructor),
    destructor: Some(destructor),
    event_cb: Some(textarea_event),
    width_def: DPI_DEF * 2,
    height_def: DPI_DEF,
    editable: ObjClassEditable::True,
    group_def: ObjClassGroupDef::True,
    theme_inheritable: false,
};

/// A copyable text area handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Textarea(Obj);

impl Textarea {
    /// `lv_textarea_create()`. `None` creates a text area screen on the
    /// default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&TEXTAREA_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the text area class.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &TEXTAREA_CLASS)).then_some(Self(obj))
    }

    /*======================
     * Add/remove functions
     *=====================*/

    /// `lv_textarea_add_char()`: insert a character (a UTF-8 encoded letter
    /// as from `lv_text_encoded_conv_wc`) at the cursor.
    pub fn add_char(self, ui: &mut Ui, c: u32) {
        let obj = self.0;
        if state(ui, obj).is_none() {
            return; /*LV_CHECK_OBJ*/
        }
        let res = add_char(ui, obj, c);
        if res != LvResult::Ok {
            return;
        }
        let pos = self.cursor_pos(ui);
        self.set_cursor_pos(ui, pos.wrapping_add(1) as i32);
        if state(ui, obj).is_none() {
            return; /*A layout callback deleted the text area*/
        }
        let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
    }

    /// `lv_textarea_add_text()`: insert a text at the cursor.
    pub fn add_text(self, ui: &mut Ui, txt: &str) {
        let obj = self.0;
        let txt = c_str(txt);
        let Some(ta) = state(ui, obj) else { return };

        if ta.pwd_mode {
            pwd_char_hider(ui, obj); /*Make sure all the current text contains only '*'*/
        }

        /*Add the character one-by-one if not all characters are accepted or there is character limit.*/
        if self.accepted_chars(ui).is_some() || self.max_length(ui) != 0 {
            if add_text(ui, obj, txt) {
                let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
            }
            return;
        }

        let res = insert_handler(ui, obj, txt);
        if res != LvResult::Ok {
            return;
        }
        let Some(label) = label_of(ui, obj) else { return };

        /*If the textarea is empty, invalidate it to hide the placeholder*/
        if state(ui, obj).is_some_and(|ta| ta.placeholder_txt.is_some()) {
            let txt_act = label.text(ui);
            if txt_act.is_empty() {
                ui.obj_invalidate(obj);
            }
        }

        /*Insert the text*/
        let cursor_pos = self.cursor_pos(ui);
        label.insert_text(ui, cursor_pos, txt);
        self.clear_selection(ui);

        let Some(ta) = state_mut(ui, obj) else { return };
        if ta.pwd_mode {
            let cursor_pos = ta.cursor.pos;
            if let Some(pwd_tmp) = ta.pwd_tmp.as_mut() {
                text::ins(pwd_tmp, cursor_pos, txt);
            }

            /*Auto hide characters*/
            auto_hide_characters(ui, obj);
        }

        /*Move the cursor after the new text*/
        let pos = self
            .cursor_pos(ui)
            .wrapping_add(text::get_encoded_length(txt.as_bytes()));
        self.set_cursor_pos(ui, pos as i32);
        if state(ui, obj).is_none() {
            return;
        }

        let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
    }

    /// `lv_textarea_delete_char()`: delete the character left of the cursor.
    pub fn delete_char(self, ui: &mut Ui) {
        let obj = self.0;
        let Some(ta) = state(ui, obj) else { return };
        let cur_pos = ta.cursor.pos;

        if cur_pos == 0 {
            return;
        }

        let del_buf = [KEY_DEL as u8];
        let del_buf = core::str::from_utf8(&del_buf).unwrap_or("");

        let res = insert_handler(ui, obj, del_buf);
        if res != LvResult::Ok {
            return;
        }
        let Some(label) = label_of(ui, obj) else { return };

        let mut label_txt = label.text(ui).to_string();

        /*Delete a character*/
        let cursor_pos = self.cursor_pos(ui);
        text::cut(&mut label_txt, cursor_pos.wrapping_sub(1), 1);

        /*Refresh the label*/
        label.set_text(ui, &label_txt);
        self.clear_selection(ui);

        /*If the textarea became empty, invalidate it to hide the placeholder*/
        if state(ui, obj).is_some_and(|ta| ta.placeholder_txt.is_some()) {
            let txt = label.text(ui);
            if txt.is_empty() {
                ui.obj_invalidate(obj);
            }
        }

        let Some(ta) = state_mut(ui, obj) else { return };
        if ta.pwd_mode {
            let cursor_pos = ta.cursor.pos;
            if let Some(pwd_tmp) = ta.pwd_tmp.as_mut() {
                text::cut(pwd_tmp, cursor_pos.wrapping_sub(1), 1);
            }
        }

        /*Move the cursor to the place of the deleted character*/
        let pos = self.cursor_pos(ui).wrapping_sub(1);
        self.set_cursor_pos(ui, pos as i32);
        if state(ui, obj).is_none() {
            return;
        }

        let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
    }

    /// `lv_textarea_delete_char_forward()`: delete the character right of the
    /// cursor.
    pub fn delete_char_forward(self, ui: &mut Ui) {
        if state(ui, self.0).is_none() {
            return; /*LV_CHECK_OBJ*/
        }
        let cp = self.cursor_pos(ui);
        self.set_cursor_pos(ui, cp.wrapping_add(1) as i32);
        if state(ui, self.0).is_some() && cp != self.cursor_pos(ui) {
            self.delete_char(ui);
        }
    }

    /*=====================
     * Setter functions
     *====================*/

    /// `lv_textarea_set_text()`
    pub fn set_text(self, ui: &mut Ui, txt: &str) {
        let obj = self.0;
        let txt = c_str(txt);
        let Some(label) = label_of(ui, obj) else { return };

        /*Clear the existing selection*/
        self.clear_selection(ui);

        /*Add the character one-by-one if not all characters are accepted or there is character limit.*/
        let text_changed = if self.accepted_chars(ui).is_some() || self.max_length(ui) != 0 {
            label.set_text(ui, "");
            self.set_cursor_pos(ui, TEXTAREA_CURSOR_LAST);
            let Some(ta) = state_mut(ui, obj) else { return };
            if ta.pwd_mode
                && let Some(pwd_tmp) = ta.pwd_tmp.as_mut()
            {
                pwd_tmp.clear(); /*Clear the password too*/
            }
            add_text(ui, obj, txt)
        } else {
            label.set_text(ui, txt);
            self.set_cursor_pos(ui, TEXTAREA_CURSOR_LAST);
            true
        };

        let Some(ta) = state(ui, obj) else { return };
        /*If the textarea is empty, invalidate it to hide the placeholder*/
        if ta.placeholder_txt.is_some() {
            let txt_act = label.text(ui);
            if txt_act.is_empty() {
                ui.obj_invalidate(obj);
            }
        }

        let Some(ta) = state_mut(ui, obj) else { return };
        if ta.pwd_mode {
            ta.pwd_tmp = Some(txt.to_string());

            pwd_char_hider(ui, obj);
        }

        if text_changed {
            let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
        }
    }

    /// `lv_textarea_set_placeholder_text()`: `None` removes the placeholder.
    pub fn set_placeholder_text(self, ui: &mut Ui, txt: Option<&str>) {
        let obj = self.0;
        let txt = txt.map(c_str);
        let Some(ta) = state_mut(ui, obj) else { return };
        let has_text = txt.is_some_and(|txt| !txt.is_empty());

        if !has_text {
            if ta.placeholder_txt.is_some() {
                ta.placeholder_txt = None;
            }
        } else if let Some(txt) = txt {
            ta.placeholder_txt = Some(txt.to_string());

            calc_placeholder_text_size(ui, obj);
        }

        ui.obj_invalidate(obj);
    }

    /// `lv_textarea_set_cursor_pos()`: character index; < 0 counts from the
    /// end, [`TEXTAREA_CURSOR_LAST`] goes after the last character.
    pub fn set_cursor_pos(self, ui: &mut Ui, pos: i32) {
        let obj = self.0;
        if state(ui, obj).is_none() {
            return; /*LV_CHECK_OBJ*/
        }
        set_cursor_pos_internal(ui, obj, pos);

        /*Position the label to make the cursor visible*/
        ui.obj_update_layout(obj);
        if state(ui, obj).is_none() {
            return;
        }

        scroll_to_cursor_pos(ui, obj, pos);
    }

    /// `lv_textarea_set_cursor_click_pos()`: position the cursor by clicking.
    pub fn set_cursor_click_pos(self, ui: &mut Ui, en: bool) {
        if let Some(ta) = state_mut(ui, self.0) {
            ta.cursor.click_pos = en;
        }
    }

    /// `lv_textarea_set_password_mode()`
    pub fn set_password_mode(self, ui: &mut Ui, en: bool) {
        let obj = self.0;
        let Some(ta) = state(ui, obj) else { return };
        if ta.pwd_mode == en {
            return;
        }
        let Some(label) = label_of(ui, obj) else { return };

        /*Pwd mode is now enabled*/
        if en {
            let txt = label.text(ui).to_string();
            let Some(ta) = state_mut(ui, obj) else { return };
            ta.pwd_tmp = Some(txt);

            ta.pwd_mode = true;
            pwd_char_hider(ui, obj);

            self.clear_selection(ui);
        }
        /*Pwd mode is now disabled*/
        else {
            let Some(ta) = state_mut(ui, obj) else { return };
            ta.pwd_mode = false;
            self.clear_selection(ui);
            let pwd_tmp = state_mut(ui, obj).and_then(|ta| ta.pwd_tmp.take());
            label.set_text(ui, pwd_tmp.as_deref().unwrap_or(""));
        }

        refr_cursor_area(ui, obj);
    }

    /// `lv_textarea_set_password_bullet()`: `None` uses the default bullet.
    pub fn set_password_bullet(self, ui: &mut Ui, bullet: Option<&str>) {
        let obj = self.0;
        let bullet = bullet.map(c_str);
        let Some(ta) = state_mut(ui, obj) else { return };

        if bullet.is_none() && ta.pwd_bullet.is_some() {
            ta.pwd_bullet = None;
        }

        if let Some(bullet) = bullet {
            ta.pwd_bullet = Some(bullet.to_string());
        }

        pwd_char_hider(ui, obj);
    }

    /// `lv_textarea_set_one_line()`
    pub fn set_one_line(self, ui: &mut Ui, en: bool) {
        let obj = self.0;
        let Some(ta) = state_mut(ui, obj) else { return };
        if ta.one_line == en {
            return;
        }

        ta.one_line = en;
        let Some(label) = ta.label else { return };
        let width = if en { coord::SIZE_CONTENT } else { coord::pct(100) };
        let min_width_value = if en { coord::pct(100) } else { 0 };

        ui.obj_set_width(label, width);
        ui.obj_set_style_min_width(label, min_width_value, StyleSelector::DEFAULT);

        if en {
            ui.obj_set_height(obj, coord::SIZE_CONTENT);
        } else {
            ui.obj_remove_local_style_prop(obj, StyleProp::HEIGHT, Part::MAIN.into());
        }

        ui.obj_scroll_to(obj, 0, 0, ANIM_OFF);

        /* update placeholder text size */
        calc_placeholder_text_size(ui, obj);
    }

    /// `lv_textarea_set_accepted_chars()`: a copy is saved; `None` accepts
    /// every character.
    pub fn set_accepted_chars(self, ui: &mut Ui, list: Option<&str>) {
        let Some(ta) = state_mut(ui, self.0) else { return };

        let copied_list = list.map(|list| SharedStr::Shared(Rc::from(c_str(list))));

        ta.static_accepted_chars = false;
        ta.accepted_chars = copied_list;
    }

    /// `lv_textarea_set_accepted_chars_static()`: the list is referenced;
    /// `None` accepts every character.
    pub fn set_accepted_chars_static(self, ui: &mut Ui, list: Option<&'static str>) {
        let Some(ta) = state_mut(ui, self.0) else { return };

        ta.static_accepted_chars = true;
        ta.accepted_chars = list.map(SharedStr::Static);
    }

    /// `lv_textarea_set_max_length()`: max. number of characters that can be
    /// added (`lv_textarea_set_text` ignores it).
    pub fn set_max_length(self, ui: &mut Ui, num: u32) {
        if let Some(ta) = state_mut(ui, self.0) {
            ta.max_length = num;
        }
    }

    /// `lv_textarea_set_insert_replace()`: in `LV_EVENT_INSERT`, replace the
    /// text to insert. `None` or `""` drops it.
    pub fn set_insert_replace(self, ui: &mut Ui, txt: Option<SharedStr>) {
        if state(ui, self.0).is_none() {
            return; /*LV_CHECK_OBJ*/
        }
        ui.widget_static_mut::<TextareaStatics>().ta_insert_replace = txt;
    }

    /// `lv_textarea_set_text_selection()`: enable/disable selection mode.
    pub fn set_text_selection(self, ui: &mut Ui, en: bool) {
        let Some(ta) = state_mut(ui, self.0) else { return };

        ta.text_sel_en = en;

        if !en {
            self.clear_selection(ui);
        }
    }

    /// `lv_textarea_set_password_show_time()`: ms before a character becomes
    /// a bullet, 0: hide immediately.
    pub fn set_password_show_time(self, ui: &mut Ui, time: u32) {
        let Some(ta) = state_mut(ui, self.0) else { return };
        ta.pwd_show_time = time;
        pwd_char_hider(ui, self.0);
    }

    /// `lv_textarea_set_align()`: set the label's text alignment.
    #[deprecated(note = "use the text_align style property instead")]
    pub fn set_align(self, ui: &mut Ui, align: TextAlign) {
        if state(ui, self.0).is_none() {
            return; /*LV_CHECK_OBJ*/
        }
        ui.obj_set_style_text_align(self.0, align, StyleSelector::DEFAULT);
    }

    /*=====================
     * Getter functions
     *====================*/

    /// `lv_textarea_get_text()`: the real text, also in password mode.
    pub fn text(self, ui: &Ui) -> &str {
        let Some(ta) = state(ui, self.0) else { return "" };

        if !ta.pwd_mode {
            label_of(ui, self.0).map_or("", |label| label.text(ui))
        } else {
            ta.pwd_tmp.as_deref().unwrap_or("")
        }
    }

    /// `lv_textarea_get_placeholder_text()`
    pub fn placeholder_text(self, ui: &Ui) -> &str {
        state(ui, self.0)
            .and_then(|ta| ta.placeholder_txt.as_deref())
            .unwrap_or("")
    }

    /// `lv_textarea_get_label()`
    pub fn label(self, ui: &Ui) -> Option<Obj> {
        state(ui, self.0).and_then(|ta| ta.label)
    }

    /// `lv_textarea_get_cursor_pos()`: the cursor position in characters.
    pub fn cursor_pos(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(0, |ta| ta.cursor.pos)
    }

    /// `lv_textarea_get_cursor_click_pos()`
    pub fn cursor_click_pos(self, ui: &Ui) -> bool {
        state(ui, self.0).is_some_and(|ta| ta.cursor.click_pos)
    }

    /// `lv_textarea_get_password_mode()`
    pub fn password_mode(self, ui: &Ui) -> bool {
        state(ui, self.0).is_some_and(|ta| ta.pwd_mode)
    }

    /// `lv_textarea_get_password_bullet()`
    pub fn password_bullet(self, ui: &Ui) -> &str {
        let Some(ta) = state(ui, self.0) else { return "" };

        if let Some(bullet) = ta.pwd_bullet.as_deref() {
            return bullet;
        }

        /*If the textarea's font has the bullet character use it else fallback to "*"*/
        if let Some(bullet_font) = ui.obj_get_style_text_font(self.0, Part::MAIN)
            && font::get_glyph_dsc(bullet_font.as_font(), TEXTAREA_PWD_BULLET_UNICODE, 0)
                .1
                .is_some()
        {
            return SYMBOL_BULLET;
        }

        "*"
    }

    /// `lv_textarea_get_one_line()`
    pub fn one_line(self, ui: &Ui) -> bool {
        state(ui, self.0).is_some_and(|ta| ta.one_line)
    }

    /// `lv_textarea_get_accepted_chars()`: `None` is `NULL`.
    pub fn accepted_chars(self, ui: &Ui) -> Option<&str> {
        state(ui, self.0).and_then(|ta| ta.accepted_chars.as_deref())
    }

    /// `lv_textarea_get_max_length()`
    pub fn max_length(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(0, |ta| ta.max_length)
    }

    /// `lv_textarea_text_is_selected()`
    pub fn text_is_selected(self, ui: &Ui) -> bool {
        let Some(label) = label_of(ui, self.0) else {
            return false;
        };

        label.text_selection_start(ui) != LABEL_TEXT_SELECTION_OFF
            || label.text_selection_end(ui) != LABEL_TEXT_SELECTION_OFF
    }

    /// `lv_textarea_get_text_selection()`: whether selection mode is enabled.
    pub fn text_selection(self, ui: &Ui) -> bool {
        state(ui, self.0).is_some_and(|ta| ta.text_sel_en)
    }

    /// `lv_textarea_get_password_show_time()`
    pub fn password_show_time(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(0, |ta| ta.pwd_show_time)
    }

    /// `lv_textarea_get_current_char()`: the character at the cursor or 0.
    pub fn current_char(self, ui: &Ui) -> u32 {
        let Some(ta) = state(ui, self.0) else { return 0 };

        let txt = self.text(ui).as_bytes();
        let mut pos = ta.cursor.pos;
        if text::get_encoded_length(txt) >= pos && pos > 0 {
            text::encoded_prev(txt, Some(&mut pos))
        } else {
            0
        }
    }

    /*=====================
     * Other functions
     *====================*/

    /// `lv_textarea_clear_selection()`
    pub fn clear_selection(self, ui: &mut Ui) {
        let Some(label) = label_of(ui, self.0) else { return };

        if label.text_selection_start(ui) != LABEL_TEXT_SELECTION_OFF
            || label.text_selection_end(ui) != LABEL_TEXT_SELECTION_OFF
        {
            label.set_text_selection_start(ui, LABEL_TEXT_SELECTION_OFF);
            label.set_text_selection_end(ui, LABEL_TEXT_SELECTION_OFF);
        }
    }

    /// `lv_textarea_cursor_right()`: move the cursor one character right.
    pub fn cursor_right(self, ui: &mut Ui) {
        if state(ui, self.0).is_none() {
            return; /*LV_CHECK_OBJ*/
        }
        let mut cp = self.cursor_pos(ui);
        cp = cp.wrapping_add(1);
        self.set_cursor_pos(ui, cp as i32);
    }

    /// `lv_textarea_cursor_left()`: move the cursor one character left.
    pub fn cursor_left(self, ui: &mut Ui) {
        if state(ui, self.0).is_none() {
            return; /*LV_CHECK_OBJ*/
        }
        let mut cp = self.cursor_pos(ui);
        if cp > 0 {
            cp -= 1;
            self.set_cursor_pos(ui, cp as i32);
        }
    }

    /// `lv_textarea_cursor_down()`: move the cursor one line down.
    pub fn cursor_down(self, ui: &mut Ui) {
        let obj = self.0;
        let Some(label) = label_of(ui, obj) else { return };

        /*Get the position of the current letter*/
        let mut pos = label.letter_pos(ui, self.cursor_pos(ui));

        /*Increment the y with one line and keep the valid x*/

        let line_space = ui.obj_get_style_text_line_space(obj, Part::MAIN);
        let font_h = ui
            .obj_get_style_text_font(obj, Part::MAIN)
            .map_or(0, |font| font::get_line_height(font.as_font()));
        let Some(valid_x) = state(ui, obj).map(|ta| ta.cursor.valid_x) else {
            return;
        };
        pos.y = pos
            .y
            .wrapping_add(font_h.wrapping_add(line_space).wrapping_add(1));
        pos.x = valid_x;

        /*Do not go below the last line*/
        if pos.y < ui.obj_get_height(label.obj()) {
            /*Get the letter index on the new cursor position and set it*/
            let new_cur_pos = label.letter_on(ui, pos, true);

            let cur_valid_x_tmp = valid_x; /*Cursor position set overwrites the valid position*/
            self.set_cursor_pos(ui, new_cur_pos as i32);
            if let Some(ta) = state_mut(ui, obj) {
                ta.cursor.valid_x = cur_valid_x_tmp;
            }
        }
    }

    /// `lv_textarea_cursor_up()`: move the cursor one line up.
    pub fn cursor_up(self, ui: &mut Ui) {
        let obj = self.0;
        let Some(label) = label_of(ui, obj) else { return };

        /*Get the position of the current letter*/
        let mut pos = label.letter_pos(ui, self.cursor_pos(ui));

        /*Decrement the y with one line and keep the valid x*/
        let line_space = ui.obj_get_style_text_line_space(obj, Part::MAIN);
        let font_h = ui
            .obj_get_style_text_font(obj, Part::MAIN)
            .map_or(0, |font| font::get_line_height(font.as_font()));
        let Some(valid_x) = state(ui, obj).map(|ta| ta.cursor.valid_x) else {
            return;
        };
        pos.y = pos
            .y
            .wrapping_sub(font_h.wrapping_add(line_space).wrapping_sub(1));
        pos.x = valid_x;

        /*Get the letter index on the new cursor position and set it*/
        let new_cur_pos = label.letter_on(ui, pos, true);
        let cur_valid_x_tmp = valid_x; /*Cursor position set overwrites the valid position*/
        self.set_cursor_pos(ui, new_cur_pos as i32);
        if let Some(ta) = state_mut(ui, obj) {
            ta.cursor.valid_x = cur_valid_x_tmp;
        }
    }
}

impl From<Textarea> for Obj {
    fn from(textarea: Textarea) -> Self {
        textarea.obj()
    }
}

/**********************
 *   STATIC FUNCTIONS
 **********************/

fn state(ui: &Ui, obj: Obj) -> Option<&TextareaState> {
    ui.obj(obj)?.widget.get::<TextareaState>()
}

fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut TextareaState> {
    ui.obj_mut(obj)?.widget.get_mut::<TextareaState>()
}

/// `ta->label` as a label handle.
fn label_of(ui: &Ui, obj: Obj) -> Option<Label> {
    state(ui, obj)?.label.and_then(|label| Label::from_obj(ui, label))
}

/// The part of `txt` that a C string with the same bytes would hold.
fn c_str(txt: &str) -> &str {
    txt.split('\0').next().unwrap_or("")
}

/// `&txt[i]` of a NUL-terminated C string: the bytes from `i` on (empty past
/// the end).
fn tail(txt: &[u8], i: u32) -> &[u8] {
    txt.get(i as usize..).unwrap_or(&[])
}

/// `ta->cursor.area` moved by the label's top-left corner (`area_tmp` in C).
fn cursor_area_abs(ui: &Ui, ta: &TextareaState) -> Area {
    let label_coords = ta.label.map_or(Area::default(), |label| ui.obj_get_coords(label));
    let mut area_tmp = ta.cursor.area;
    area_tmp.x1 += label_coords.x1;
    area_tmp.y1 += label_coords.y1;
    area_tmp.x2 += label_coords.x1;
    area_tmp.y2 += label_coords.y1;
    area_tmp
}

/// `lv_textarea_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    let Some(ta) = state_mut(ui, obj) else { return };

    ta.pwd_mode = false;
    ta.pwd_tmp = None;
    ta.pwd_bullet = None;
    ta.pwd_show_time = TEXTAREA_DEF_PWD_SHOW_TIME;
    ta.accepted_chars = None;
    ta.static_accepted_chars = true;
    ta.max_length = 0;
    ta.cursor.show = true;
    /*It will be set to zero later (with zero value lv_textarea_set_cursor_pos(obj, 0); wouldn't do anything as there is no difference)*/
    ta.cursor.pos = 1;
    ta.cursor.click_pos = true;
    ta.cursor.valid_x = 0;
    ta.one_line = false;
    ta.text_sel_en = false;
    ta.label = None;
    ta.placeholder_txt = None;

    let label = Label::create(ui, obj);
    if let Some(ta) = state_mut(ui, obj) {
        ta.label = Some(label.obj());
    }
    ui.obj_set_width(label.obj(), coord::pct(100));
    ui.obj_align(label.obj(), Align::TopLeft, 0, 0); /*Label handles text alignment*/
    label.set_text(ui, "");
    let cb: EventCallback = Rc::new(label_event_cb);
    ui.obj_add_event_cb(label.obj(), cb.clone(), EventCode::STYLE_CHANGED, None);
    ui.obj_add_event_cb(label.obj(), cb, EventCode::SIZE_CHANGED, None);
    ui.obj_set_scroll_on_focus(obj, true);
    ui.obj_set_scroll_with_arrow(obj, false);

    Textarea(obj).set_cursor_pos(ui, 0);

    if state(ui, obj).is_some() {
        start_cursor_blink(ui, obj);
    }
}

/// `lv_textarea_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    let Some(ta) = state_mut(ui, obj) else { return };
    if ta.pwd_tmp.is_some() {
        ta.pwd_tmp = None;
    }
    if ta.pwd_bullet.is_some() {
        ta.pwd_bullet = None;
    }
    if ta.placeholder_txt.is_some() {
        ta.placeholder_txt = None;
    }
    ta.accepted_chars = None;
}

/// `lv_textarea_event()`
fn textarea_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    /*Call the ancestor's event handler*/
    let res = ui.obj_event_base(&TEXTAREA_CLASS, event);
    if res != LvResult::Ok {
        return;
    }

    let code = event.code();
    let Some(obj) = event.current_target_obj() else {
        return;
    };
    if state(ui, obj).is_none() {
        return;
    }
    let ta = Textarea(obj);

    if code == EventCode::FOCUSED {
        start_cursor_blink(ui, obj);
    } else if code == EventCode::KEY {
        let c = event.key(); /*uint32_t because can be UTF-8*/
        if c == KEY_RIGHT {
            ta.cursor_right(ui);
        } else if c == KEY_LEFT {
            ta.cursor_left(ui);
        } else if c == KEY_UP {
            ta.cursor_up(ui);
        } else if c == KEY_DOWN {
            ta.cursor_down(ui);
        } else if c == KEY_BACKSPACE {
            ta.delete_char(ui);
        } else if c == KEY_DEL {
            ta.delete_char_forward(ui);
        } else if c == KEY_HOME {
            ta.set_cursor_pos(ui, 0);
        } else if c == KEY_END {
            ta.set_cursor_pos(ui, TEXTAREA_CURSOR_LAST);
        } else if c == KEY_ENTER && ta.one_line(ui) {
            let _ = ui.obj_send_event(obj, EventCode::READY, EventParam::None);
        } else {
            ta.add_char(ui, c);
        }
    } else if code == EventCode::PRESSED
        || code == EventCode::PRESSING
        || code == EventCode::PRESS_LOST
        || code == EventCode::RELEASED
    {
        update_cursor_position_on_click(ui, obj, code);
    } else if code == EventCode::DRAW_MAIN {
        draw_placeholder(ui, obj, event);
    } else if code == EventCode::DRAW_POST {
        draw_cursor(ui, obj, event);
    } else if code == EventCode::SIZE_CHANGED {
        let pos = ta.cursor_pos(ui);
        scroll_to_cursor_pos(ui, obj, pos as i32);
    } else if code == EventCode::STYLE_CHANGED {
        refr_cursor_area(ui, obj);
    }
}

/// `label_event_cb()`
fn label_event_cb(ui: &mut Ui, event: &mut Event<'_>) {
    let code = event.code();
    let Some(label) = event.current_target_obj() else {
        return;
    };
    let Some(ta) = ui.obj_get_parent(label) else {
        return;
    };

    if code == EventCode::STYLE_CHANGED || code == EventCode::SIZE_CHANGED {
        if let Some(label) = Label::from_obj(ui, label) {
            label.refresh_text(ui);
        }
        if state(ui, ta).is_none() {
            return;
        }
        if code == EventCode::SIZE_CHANGED {
            /*Stop pending scroll that was aimed at the old label*/
            ui.obj_stop_scroll_anim(ta);
            ui.obj_readjust_scroll(ta, ANIM_ON);
        }
        if state(ui, ta).is_none() {
            return;
        }
        refr_cursor_area(ui, ta);
        start_cursor_blink(ui, ta);
    }
}

/// The exec callback `cursor_blink_anim_cb` as an animation exec.
fn cursor_blink_exec() -> AnimExec {
    AnimExec::Custom {
        tag: ANIM_TAG_CURSOR_BLINK,
        f: Rc::new(|ui, target, show| {
            if let AnimTarget::Obj(obj) = target {
                cursor_blink_anim_cb(ui, obj, show);
            }
        }),
    }
}

/// `cursor_blink_anim_cb()`: blink the cursor.
/// `show`: 1: show the cursor, 0: hide it.
fn cursor_blink_anim_cb(ui: &mut Ui, obj: Obj, show: i32) {
    let Some(ta) = state(ui, obj) else { return };
    if show != i32::from(ta.cursor.show) {
        let area_tmp = cursor_area_abs(ui, ta);
        if let Some(ta) = state_mut(ui, obj) {
            ta.cursor.show = show != 0;
        }
        ui.obj_invalidate_area(obj, &area_tmp);
    }
}

/// `pwd_char_hider_anim`: a dummy exec callback to animate char hiding in
/// pwd mode. Does nothing, but a function is required in char hiding anim.
/// (`pwd_char_hider` callback does the real job.)
fn pwd_char_hider_exec() -> AnimExec {
    AnimExec::Custom {
        tag: ANIM_TAG_PWD_HIDER,
        f: Rc::new(|_ui, _target, _x| {}),
    }
}

/// `pwd_char_hider_anim_completed()`: called when an animation is ready to
/// convert all characters to '*'.
fn pwd_char_hider_anim_completed(ui: &mut Ui, a: ferrovgl_core::AnimId) {
    let var = ui.anim(a).map(|a| a.var);
    if let Some(AnimTarget::Obj(obj)) = var
        && state(ui, obj).is_some()
    {
        pwd_char_hider(ui, obj);
    }
}

/// `pwd_char_hider()`: hide all characters (convert them to '*').
fn pwd_char_hider(ui: &mut Ui, obj: Obj) {
    let Some(ta) = state(ui, obj) else { return };
    if !ta.pwd_mode {
        return;
    }
    let Some(label) = label_of(ui, obj) else { return };

    /* When ta->label is empty we get 0 back */
    let txt = label.text(ui);
    let enc_len = text::get_encoded_length(txt.as_bytes());
    if enc_len == 0 {
        return;
    }

    let bullet = Textarea(obj).password_bullet(ui);
    let mut txt_tmp = String::with_capacity(enc_len as usize * bullet.len());
    for _ in 0..enc_len {
        txt_tmp.push_str(bullet);
    }

    label.set_text(ui, &txt_tmp);

    auto_hide_characters_cancel(ui, obj);

    refr_cursor_area(ui, obj);
}

/// `char_is_accepted()`: test a unicode character if it is accepted or not.
/// Checks max length and accepted char list.
fn char_is_accepted(ui: &Ui, obj: Obj, c: u32) -> bool {
    let Some(ta) = state(ui, obj) else { return false };

    /*Too many characters?*/
    if ta.max_length > 0 && text::get_encoded_length(Textarea(obj).text(ui).as_bytes()) >= ta.max_length {
        return false;
    }

    let Some(accepted_chars) = ta.accepted_chars.as_deref() else {
        return true;
    };
    let accepted_chars = accepted_chars.as_bytes();
    if accepted_chars.first().is_none_or(|&b| b == 0) {
        return true;
    }
    /*Accepted character?*/
    let mut i: u32 = 0;

    while accepted_chars.get(i as usize).is_some_and(|&b| b != 0) {
        let a = text::encoded_next(accepted_chars, Some(&mut i));
        if a == c {
            return true; /*Accepted*/
        }
    }

    false /*The character wasn't in the list*/
}

/// `start_cursor_blink()`
fn start_cursor_blink(ui: &mut Ui, obj: Obj) {
    let blink_time = ui.obj_get_style_anim_duration(obj, Part::CURSOR);
    if blink_time == 0 {
        ui.anim_delete(AnimTarget::Obj(obj), Some(&cursor_blink_exec()));
        if let Some(ta) = state_mut(ui, obj) {
            ta.cursor.show = true;
        }
    } else {
        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(obj));
        a.set_exec_cb(cursor_blink_exec());
        a.set_duration(blink_time);
        a.set_reverse_duration(blink_time);
        a.set_values(1, 0);
        a.set_path_cb(AnimPath::Step);
        a.set_repeat_count(ANIM_REPEAT_INFINITE);
        ui.anim_start(&a);
    }
}

/// `refr_cursor_area()`
fn refr_cursor_area(ui: &mut Ui, obj: Obj) {
    let Some(label) = label_of(ui, obj) else { return };

    let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN) else {
        return;
    };
    let font = font.as_font();
    let line_space = ui.obj_get_style_text_line_space(obj, Part::MAIN);

    let cur_pos = Textarea(obj).cursor_pos(ui);
    let txt = label.text(ui).as_bytes();

    let mut byte_pos = text::encoded_get_byte_id(txt, cur_pos);
    let mut letter = text::encoded_next(tail(txt, byte_pos), None);

    /* Letter height and width */
    let letter_h = font::get_line_height(font);
    /*Set letter_w (set not 0 on non printable but valid chars)*/
    let mut letter_space = letter;
    if is_valid_but_non_printable_char(letter) {
        letter_space = u32::from(b' ');
    }
    let mut letter_w = i32::from(font::get_glyph_width(font, letter_space, IGNORE_KERNING));

    let mut letter_pos = label.letter_pos(ui, cur_pos);

    let align = ui.obj_calculate_style_text_align(label.obj(), Part::MAIN, label.text(ui));

    let label_coords = ui.obj_get_coords(label.obj());
    let one_line = state(ui, obj).is_some_and(|ta| ta.one_line);

    /*If the cursor is out of the text (most right) draw it to the next line*/
    if ((letter_pos.x + label_coords.x1) + letter_w > label_coords.x2)
        && (!one_line && align != TextAlign::Right)
    {
        letter_pos.x = 0;
        letter_pos.y += letter_h + line_space;

        if letter != 0 {
            byte_pos += u32::from(text::encoded_size(tail(txt, byte_pos)));
            letter = text::encoded_next(tail(txt, byte_pos), None);
        }

        let mut tmp = letter;
        if is_valid_but_non_printable_char(letter) {
            /*If non printable get the letter_w of the space char*/
            tmp = u32::from(b' ');
        }
        letter_w = i32::from(font::get_glyph_width(font, tmp, IGNORE_KERNING));
    }

    /*Save the byte position. It is required to draw `LV_CURSOR_BLOCK`*/
    if let Some(ta) = state_mut(ui, obj) {
        ta.cursor.txt_byte_pos = byte_pos;
    }

    /*Calculate the cursor according to its type*/
    let border_width = ui.obj_get_style_border_width(obj, Part::CURSOR);
    let top = ui.obj_get_style_pad_top(obj, Part::CURSOR) + border_width;
    let bottom = ui.obj_get_style_pad_bottom(obj, Part::CURSOR) + border_width;
    let left = ui.obj_get_style_pad_left(obj, Part::CURSOR) + border_width;
    let right = ui.obj_get_style_pad_right(obj, Part::CURSOR) + border_width;
    let letter_space_w = ui.obj_get_style_text_letter_space(label.obj(), Part::MAIN);

    let cur_area = Area {
        x1: letter_pos.x - left - letter_space_w / 2,
        y1: letter_pos.y - top,
        x2: letter_pos.x + right + letter_w - 1 + (letter_space_w + 1) / 2,
        y2: letter_pos.y + bottom + letter_h - 1,
    };

    /*Save the new area*/
    let Some(ta) = state(ui, obj) else { return };
    let area_tmp = cursor_area_abs(ui, ta);
    ui.obj_invalidate_area(obj, &area_tmp);

    let Some(ta) = state_mut(ui, obj) else { return };
    ta.cursor.area = cur_area;

    let Some(ta) = state(ui, obj) else { return };
    let area_tmp = cursor_area_abs(ui, ta);
    ui.obj_invalidate_area(obj, &area_tmp);
}

/// `update_cursor_position_on_click()`
fn update_cursor_position_on_click(ui: &mut Ui, obj: Obj, code: EventCode) {
    let Some(click_source) = ui.indev_active() else {
        return;
    };

    let Some(ta) = state(ui, obj) else { return };
    if !ta.cursor.click_pos {
        return;
    }

    if ui.indev_get_type(click_source) == IndevType::Keypad
        || ui.indev_get_type(click_source) == IndevType::Encoder
    {
        return;
    }
    let Some(label) = label_of(ui, obj) else { return };

    let label_coords = ui.obj_get_coords(label.obj());

    let point_act = ui.indev_get_point(click_source);
    let _vect_act = ui.indev_get_vect(click_source);

    if point_act.x < 0 || point_act.y < 0 {
        return; /*Ignore event from keypad*/
    }
    let rel_pos = Point::new(point_act.x - label_coords.x1, point_act.y - label_coords.y1);

    let label_width = ui.obj_get_width(label.obj());
    let char_id_at_click: u32;

    let click_outside_label: bool;
    /*Check if the click happened on the left side of the area outside the label*/
    if rel_pos.x < 0 {
        char_id_at_click = 0;
        click_outside_label = true;
    }
    /*Check if the click happened on the right side of the area outside the label*/
    else if rel_pos.x >= label_width {
        char_id_at_click = TEXTAREA_CURSOR_LAST as u32;
        click_outside_label = true;
    } else {
        char_id_at_click = label.letter_on(ui, rel_pos, true);
        click_outside_label = !label.is_char_under_pos(ui, rel_pos);
    }

    let Some(ta) = state_mut(ui, obj) else { return };
    if ta.text_sel_en {
        if !ta.text_sel_in_prog && !click_outside_label && code == EventCode::PRESSED {
            /*Input device just went down. Store the selection start position*/
            ta.sel_start = char_id_at_click;
            ta.sel_end = LABEL_TEXT_SELECTION_OFF;
            ta.text_sel_in_prog = true;
            ui.obj_set_scroll_chain(obj, false);
        } else if ta.text_sel_in_prog && code == EventCode::PRESSING {
            /*Input device may be moving. Store the end position*/
            ta.sel_end = char_id_at_click;
        } else if ta.text_sel_in_prog && (code == EventCode::PRESS_LOST || code == EventCode::RELEASED) {
            /*Input device is released. Check if anything was selected.*/
            ui.obj_set_scroll_chain(obj, true);
        }
    }

    let text_sel_in_prog = state(ui, obj).is_some_and(|ta| ta.text_sel_in_prog);
    if text_sel_in_prog || code == EventCode::PRESSED {
        Textarea(obj).set_cursor_pos(ui, char_id_at_click as i32);
    }

    let Some(ta) = state(ui, obj) else { return };
    if ta.text_sel_in_prog {
        /*If the selected area has changed then update the real values and*/
        let (sel_start, sel_end) = (ta.sel_start, ta.sel_end);
        let label_sel_start = label.text_selection_start(ui);
        let label_sel_end = label.text_selection_end(ui);

        /*Invalidate the text area.*/
        if sel_start > sel_end {
            if label_sel_start != sel_end || label_sel_end != sel_start {
                label.set_text_selection_start(ui, sel_end);
                label.set_text_selection_end(ui, sel_start);
                ui.obj_invalidate(obj);
            }
        } else if sel_start < sel_end {
            if label_sel_start != sel_start || label_sel_end != sel_end {
                label.set_text_selection_start(ui, sel_start);
                label.set_text_selection_end(ui, sel_end);
                ui.obj_invalidate(obj);
            }
        } else if label_sel_start != LABEL_TEXT_SELECTION_OFF || label_sel_end != LABEL_TEXT_SELECTION_OFF {
            label.set_text_selection_start(ui, LABEL_TEXT_SELECTION_OFF);
            label.set_text_selection_end(ui, LABEL_TEXT_SELECTION_OFF);
            ui.obj_invalidate(obj);
        }
        /*Finish selection if necessary*/
        if (code == EventCode::PRESS_LOST || code == EventCode::RELEASED)
            && let Some(ta) = state_mut(ui, obj)
        {
            ta.text_sel_in_prog = false;
        }
    }
}

/// `insert_handler()`: returns `LV_RESULT_OK` when no operation were
/// performed, `LV_RESULT_INVALID` when a user defined text was inserted.
fn insert_handler(ui: &mut Ui, obj: Obj, txt: &str) -> LvResult {
    insert_handler_raw(ui, obj, txt, txt.as_bytes())
}

/// `insert_handler()` for a C string that may not be valid UTF-8: `txt` is
/// the event parameter (a lossy copy when `raw` is not UTF-8) and `raw` the C
/// string bytes the replacement is compared with (`lv_strcmp`).
fn insert_handler_raw(ui: &mut Ui, obj: Obj, txt: &str, raw: &[u8]) -> LvResult {
    ui.widget_static_mut::<TextareaStatics>().ta_insert_replace = None;
    let _ = ui.obj_send_event(obj, EventCode::INSERT, EventParam::Text(txt));
    if state(ui, obj).is_none() {
        return LvResult::Invalid; /*The text area was deleted by the event*/
    }

    let ta_insert_replace = ui
        .widget_static::<TextareaStatics>()
        .and_then(|statics| statics.ta_insert_replace.clone());

    if let Some(ta_insert_replace) = ta_insert_replace {
        let replace = c_str(&ta_insert_replace);
        /* Drop txt if insert replace is set to '\0' */
        if replace.is_empty() {
            return LvResult::Invalid;
        }

        /*Add the replaced text directly it's different from the original*/
        if replace.as_bytes() != raw {
            Textarea(obj).add_text(ui, replace);
            return LvResult::Invalid;
        }
    }

    LvResult::Ok
}

/// `draw_placeholder()`
fn draw_placeholder(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(layer) = event.layer() else { return };
    let Some(label) = label_of(ui, obj) else { return };
    let Some(ta) = state(ui, obj) else { return };
    let txt = label.text(ui);

    /*Draw the place holder*/
    let Some(placeholder_txt) = ta.placeholder_txt.as_deref() else {
        return;
    };
    if txt.is_empty() && !placeholder_txt.is_empty() {
        let mut ph_dsc = LabelDsc::new();
        ph_dsc.base.layer = Some(layer);
        ui.obj_init_draw_label_dsc(obj, PART_TEXTAREA_PLACEHOLDER, &mut ph_dsc);

        if ta.one_line {
            ph_dsc.flag |= TextFlags::EXPAND;
        }

        let left = ui.obj_get_style_pad_left(label.obj(), PART_TEXTAREA_PLACEHOLDER);
        let top = ui.obj_get_style_pad_top(label.obj(), PART_TEXTAREA_PLACEHOLDER);

        let mut ph_coords = ui.obj_get_coords(label.obj());
        ph_coords.move_by(left, top);
        ph_dsc.text = Rc::from(placeholder_txt);
        ph_dsc.text_size = ta.placeholder_txt_size;
        ui.draw_label(layer, &ph_dsc, &ph_coords);
    }
}

/// `draw_cursor()`
fn draw_cursor(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(layer) = event.layer() else { return };
    let Some(label) = label_of(ui, obj) else { return };
    let Some(ta) = state(ui, obj) else { return };

    if !ta.cursor.show {
        return;
    }

    let mut cur_dsc = RectDsc::new();
    cur_dsc.base.layer = Some(layer);
    ui.obj_init_draw_rect_dsc(obj, Part::CURSOR, &mut cur_dsc);

    /*Draw he cursor according to the type*/
    let mut cur_area = cursor_area_abs(ui, ta);
    let txt_byte_pos = ta.cursor.txt_byte_pos;

    ui.draw_rect(layer, &cur_dsc, &cur_area);

    let border_width = ui.obj_get_style_border_width(obj, Part::CURSOR);
    let left = ui.obj_get_style_pad_left(obj, Part::CURSOR) + border_width;
    let top = ui.obj_get_style_pad_top(obj, Part::CURSOR) + border_width;
    /*`letter_buf`: the letter's bytes up to the first NUL. The byte position
     *comes from the last cursor refresh and the port does not read past the
     *text or split a character if the text changed since then.*/
    let txt = label.text(ui).as_bytes();
    let letter = tail(txt, txt_byte_pos);
    let size = usize::from(text::encoded_size(letter)).min(letter.len());
    let letter_buf: Rc<str> = Rc::from(c_str(core::str::from_utf8(&letter[..size]).unwrap_or("")));

    cur_area.x1 += left;
    cur_area.y1 += top;

    /*Draw the letter over the cursor only if
     *the cursor has background or the letter has different color than the original.
     *Else the original letter is drawn twice which makes it look bolder*/
    let label_color = ui.obj_get_style_text_color(label.obj(), Part::MAIN);
    let mut cur_label_dsc = LabelDsc::new();
    cur_label_dsc.base.layer = Some(layer);
    ui.obj_init_draw_label_dsc(obj, Part::CURSOR, &mut cur_label_dsc);
    if cur_dsc.bg_opa > OPA_MIN || cur_label_dsc.color != label_color {
        cur_label_dsc.text = letter_buf;
        cur_label_dsc.text_local = true;
        ui.draw_label(layer, &cur_label_dsc, &cur_area);
    }
}

/// `auto_hide_characters()`
fn auto_hide_characters(ui: &mut Ui, obj: Obj) {
    let Some(ta) = state(ui, obj) else { return };

    if ta.pwd_show_time == 0 {
        pwd_char_hider(ui, obj);
    } else {
        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(obj));
        a.set_exec_cb(pwd_char_hider_exec());
        a.set_duration(ta.pwd_show_time);
        a.set_values(0, 1);
        a.set_path_cb(AnimPath::Step);
        a.set_completed_cb(Some(Rc::new(pwd_char_hider_anim_completed)));
        ui.anim_start(&a);
    }
}

/// `auto_hide_characters_cancel()`
fn auto_hide_characters_cancel(ui: &mut Ui, obj: Obj) {
    ui.anim_delete(AnimTarget::Obj(obj), Some(&pwd_char_hider_exec()));
}

/// `is_valid_but_non_printable_char()`
#[inline]
fn is_valid_but_non_printable_char(letter: u32) -> bool {
    letter == u32::from(b'\0') || letter == u32::from(b'\n') || letter == u32::from(b'\r')
}

/// `lv_textarea_scroll_to_cursor_pos()`
fn scroll_to_cursor_pos(ui: &mut Ui, obj: Obj, pos: i32) {
    let Some(label) = label_of(ui, obj) else { return };

    ui.obj_update_layout(label.obj());
    if state(ui, obj).is_none() {
        return;
    }
    let font_h = ui
        .obj_get_style_text_font(obj, Part::MAIN)
        .map_or(0, |font| font::get_line_height(font.as_font()));
    let cur_pos = label.letter_pos(ui, pos as u32);

    /*The text area needs to have it's final size to see if the cursor is out of the area or not*/

    let h = ui.obj_get_content_height(obj);
    let w = ui.obj_get_content_width(obj);

    /*Check the top, then bottom*/
    if cur_pos.y < ui.obj_get_scroll_top(obj) {
        ui.obj_scroll_to_y(obj, cur_pos.y, ANIM_ON);
    } else if cur_pos.y + font_h - ui.obj_get_scroll_top(obj) > h {
        ui.obj_scroll_to_y(obj, cur_pos.y - h + font_h, ANIM_ON);
    }

    /*Check the left, then right*/
    if cur_pos.x < ui.obj_get_scroll_left(obj) {
        ui.obj_scroll_to_x(obj, cur_pos.x, ANIM_ON);
    } else if cur_pos.x + font_h > ui.obj_get_scroll_left(obj) + w {
        ui.obj_scroll_to_x(obj, cur_pos.x - w + font_h, ANIM_ON);
    }

    let Some(ta) = state_mut(ui, obj) else { return };
    ta.cursor.valid_x = cur_pos.x;

    start_cursor_blink(ui, obj);

    refr_cursor_area(ui, obj);
}

/// `set_cursor_pos_internal()`
fn set_cursor_pos_internal(ui: &mut Ui, obj: Obj, mut pos: i32) {
    let Some(ta) = state(ui, obj) else { return };
    if ta.cursor.pos == pos as u32 {
        return;
    }

    let len = label_of(ui, obj).map_or(0, |label| text::get_encoded_length(label.text(ui).as_bytes()));

    if pos < 0 {
        pos = (len as i32).wrapping_add(pos);
    }

    if pos > len as i32 || pos == TEXTAREA_CURSOR_LAST {
        pos = len as i32;
    }

    if let Some(ta) = state_mut(ui, obj) {
        ta.cursor.pos = pos as u32;
    }
}

/// `add_text()`: add the characters one by one; true if any was added.
fn add_text(ui: &mut Ui, obj: Obj, txt: &str) -> bool {
    let bytes = txt.as_bytes();
    let mut i: u32 = 0;
    let mut text_changed = false;
    while bytes.get(i as usize).is_some_and(|&b| b != 0) {
        let c = text::encoded_next(bytes, Some(&mut i));
        let res = add_char(ui, obj, text::unicode_to_encoded(c));
        if res != LvResult::Ok {
            if state(ui, obj).is_none() {
                return false; /*Deleted by a callback*/
            }
            continue;
        }
        let pos = Textarea(obj).cursor_pos(ui);
        set_cursor_pos_internal(ui, obj, pos.wrapping_add(1) as i32);
        text_changed = true;
    }

    if !text_changed {
        return false;
    }

    /*Move the cursor after the new character*/
    ui.obj_update_layout(obj);
    if state(ui, obj).is_none() {
        return false;
    }
    let pos = Textarea(obj).cursor_pos(ui);
    scroll_to_cursor_pos(ui, obj, pos as i32);
    refr_cursor_area(ui, obj);

    true
}

/// `add_char()`
fn add_char(ui: &mut Ui, obj: Obj, c: u32) -> LvResult {
    let Some(ta) = state(ui, obj) else {
        return LvResult::Invalid; /*LV_CHECK_OBJ: `return false`*/
    };

    if ta.one_line && (c == u32::from(b'\n') || c == u32::from(b'\r')) {
        /*Text area: line break ignored in one-line mode*/
        return LvResult::Invalid;
    }

    /*`u32_buf = {c, 0}` read as a C string*/
    let u32_buf = c.to_ne_bytes();
    let end = u32_buf.iter().position(|&b| b == 0).unwrap_or(u32_buf.len());
    let letter_raw = &u32_buf[..end];
    /*`LV_EVENT_INSERT` carries a `&str`: bytes that are not valid UTF-8 are sent
     *as a lossy copy, compared raw with the replacement, and rejected only where
     *C inserts them into the label (see below)*/
    let letter_txt = String::from_utf8_lossy(letter_raw);
    let letter_buf = core::str::from_utf8(letter_raw).ok();

    let c2 = c;

    let res = insert_handler_raw(ui, obj, &letter_txt, letter_raw);
    if res != LvResult::Ok {
        return res;
    }

    let c_uni = text::encoded_next(&c2.to_ne_bytes(), None);

    if !char_is_accepted(ui, obj, c_uni) {
        /*Character is not accepted by the text area (too long text or not in the accepted list)*/
        return LvResult::Invalid;
    }

    if state(ui, obj).is_some_and(|ta| ta.pwd_mode) {
        pwd_char_hider(ui, obj); /*Make sure all the current text contains only '*'*/
    }
    let Some(label) = label_of(ui, obj) else {
        return LvResult::Invalid;
    };

    /*If the textarea is empty, invalidate it to hide the placeholder*/
    if state(ui, obj).is_some_and(|ta| ta.placeholder_txt.is_some()) {
        let txt = label.text(ui);
        if txt.is_empty() {
            ui.obj_invalidate(obj);
        }
    }

    let Some(letter_buf) = letter_buf else {
        /*Not valid UTF-8: C would `lv_label_ins_text()` the raw bytes here, but a
         *`&str` label text cannot hold them, so the character is rejected*/
        return LvResult::Invalid;
    };
    let cursor_pos = Textarea(obj).cursor_pos(ui);
    label.insert_text(ui, cursor_pos, letter_buf); /*Insert the character*/
    Textarea(obj).clear_selection(ui); /*Clear selection*/

    let Some(ta) = state_mut(ui, obj) else {
        return LvResult::Invalid;
    };
    if ta.pwd_mode {
        let cursor_pos = ta.cursor.pos;
        if let Some(pwd_tmp) = ta.pwd_tmp.as_mut() {
            text::ins(pwd_tmp, cursor_pos, letter_buf);
        }

        /*Auto hide characters*/
        auto_hide_characters(ui, obj);
    }
    LvResult::Ok
}

/// `calc_placeholder_text_size()`
fn calc_placeholder_text_size(ui: &mut Ui, obj: Obj) {
    let Some(ta) = state(ui, obj) else { return };
    let Some(placeholder_txt) = ta.placeholder_txt.as_deref() else {
        return;
    };

    let mut ph_dsc = LabelDsc::new();
    ui.obj_init_draw_label_dsc(obj, PART_TEXTAREA_PLACEHOLDER, &mut ph_dsc);
    ph_dsc.font = ui.obj_get_style_text_font(obj, Part::MAIN);
    if ta.one_line {
        ph_dsc.flag |= TextFlags::EXPAND;
    }
    let Some(font) = ph_dsc.font.as_ref() else { return };

    let size = text::get_size(
        placeholder_txt,
        font.as_font(),
        ph_dsc.letter_space,
        ph_dsc.line_space,
        coord::MAX,
        ph_dsc.flag,
    );
    if let Some(ta) = state_mut(ui, obj) {
        ta.placeholder_txt_size = size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::UiConfig;

    fn setup() -> (Ui, Textarea) {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let ta = Textarea::create(&mut ui, screen);
        (ui, ta)
    }

    #[test]
    fn constructor_matches_c() {
        let (ui, ta) = setup();
        let obj = ta.obj();
        assert!(ui.obj_check_type(obj, &TEXTAREA_CLASS));
        assert_eq!(Textarea::from_obj(&ui, obj), Some(ta));
        assert!(ui.obj_is_scroll_on_focus(obj));
        assert!(!ui.obj_is_scroll_with_arrow(obj));
        let label = ta.label(&ui).unwrap();
        assert_eq!(ui.obj_get_parent(label), Some(obj));
        assert_eq!(ta.cursor_pos(&ui), 0);
        assert!(ta.cursor_click_pos(&ui));
        assert_eq!(ta.password_show_time(&ui), TEXTAREA_DEF_PWD_SHOW_TIME);
        assert_eq!(ta.accepted_chars(&ui), None);
        assert!(state(&ui, obj).unwrap().static_accepted_chars);
        assert_eq!(ta.max_length(&ui), 0);
        assert!(!ta.text_selection(&ui));
        assert_eq!(ta.text(&ui), "");
    }

    #[test]
    fn cursor_pos_clamps_and_counts_from_the_end() {
        let (mut ui, ta) = setup();
        ta.set_text(&mut ui, "h\u{e9}llo");
        assert_eq!(ta.cursor_pos(&ui), 5);
        ta.set_cursor_pos(&mut ui, -2);
        assert_eq!(ta.cursor_pos(&ui), 3);
        ta.set_cursor_pos(&mut ui, 100);
        assert_eq!(ta.cursor_pos(&ui), 5);
        ta.set_cursor_pos(&mut ui, 2);
        assert_eq!(ta.current_char(&ui), 0xE9);
        ta.cursor_left(&mut ui);
        ta.delete_char_forward(&mut ui);
        assert_eq!(ta.text(&ui), "hllo");
    }

    #[test]
    fn add_char_takes_packed_utf8() {
        let (mut ui, ta) = setup();
        ta.add_char(&mut ui, text::unicode_to_encoded(0xC1));
        ta.add_char(&mut ui, u32::from(b'B'));
        assert_eq!(ta.text(&ui), "\u{c1}B");
        // A lone continuation byte cannot be held by a `&str`.
        ta.add_char(&mut ui, 0x80);
        assert_eq!(ta.text(&ui), "\u{c1}B");
        assert_eq!(ta.cursor_pos(&ui), 2);
    }

    #[test]
    fn add_char_sends_insert_before_rejecting_invalid_utf8() {
        let (mut ui, ta) = setup();
        let seen = Rc::new(core::cell::RefCell::new(alloc::vec::Vec::<String>::new()));
        let seen_cb = seen.clone();
        // test_textarea.c `event_handler()`: replace any non-digit with "123".
        let cb: EventCallback = Rc::new(move |ui, e| {
            let txt = match e.param() {
                EventParam::Text(txt) => Some(txt.to_string()),
                _ => None,
            };
            let digit = txt
                .as_ref()
                .and_then(|t| t.as_bytes().first().copied())
                .is_some_and(|b| b.is_ascii_digit());
            seen_cb.borrow_mut().push(txt.unwrap_or_default());
            if !digit {
                let obj = e.target_obj().unwrap();
                Textarea(obj).set_insert_replace(ui, Some(SharedStr::Shared(Rc::from("123"))));
            }
        });
        ui.obj_add_event_cb(ta.obj(), cb, EventCode::INSERT, None);
        // A raw code point (not packed UTF-8): C sends INSERT, then adds "123".
        ta.add_char(&mut ui, 0xE9);
        assert_eq!(ta.text(&ui), "123");
        // The lossy event text keeps a valid prefix: "1" + 0x80 is not replaced
        // and the raw bytes are rejected where C inserts them into the label.
        ta.add_char(&mut ui, 0x8031);
        assert_eq!(ta.text(&ui), "123");
        assert_eq!(ta.cursor_pos(&ui), 3);
        // INSERT order: the raw letter, the replacement (via add_text), the raw letter.
        assert_eq!(*seen.borrow(), ["\u{fffd}", "123", "1\u{fffd}"]);
    }

    #[test]
    fn strings_stop_at_nul_like_c() {
        let (mut ui, ta) = setup();
        ta.set_text(&mut ui, "ab\0cd");
        assert_eq!(ta.text(&ui), "ab");
        ta.set_placeholder_text(&mut ui, Some("\0x"));
        assert_eq!(ta.placeholder_text(&ui), "");
        ta.set_accepted_chars(&mut ui, Some("ab\0c"));
        assert_eq!(ta.accepted_chars(&ui), Some("ab"));
        ta.set_accepted_chars_static(&mut ui, None);
        assert!(state(&ui, ta.obj()).unwrap().static_accepted_chars);
    }

    #[test]
    fn insert_callback_deleting_the_text_area_is_safe() {
        let (mut ui, ta) = setup();
        let cb: EventCallback = Rc::new(|ui, e| {
            if let Some(obj) = e.current_target_obj() {
                ui.obj_delete(obj);
            }
        });
        ui.obj_add_event_cb(ta.obj(), cb, EventCode::INSERT, None);
        ta.add_text(&mut ui, "abc");
        assert!(!ui.obj_is_valid(ta.obj()));
        ta.add_char(&mut ui, u32::from(b'x'));
        ta.delete_char(&mut ui);
        assert_eq!(ta.text(&ui), "");
    }

    #[test]
    fn password_bullet_and_hiding() {
        let (mut ui, ta) = setup();
        ta.set_text(&mut ui, "abc");
        ta.set_password_mode(&mut ui, true);
        let label = Label::from_obj(&ui, ta.label(&ui).unwrap()).unwrap();
        let bullet = ta.password_bullet(&ui).to_string();
        assert_eq!(label.text(&ui), bullet.repeat(3));
        assert_eq!(ta.text(&ui), "abc");
        ta.set_password_bullet(&mut ui, Some("#"));
        assert_eq!(label.text(&ui), "###");
        ta.set_password_mode(&mut ui, false);
        assert_eq!(label.text(&ui), "abc");
    }
}

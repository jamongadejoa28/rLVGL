//! Ported from LVGL v9.6.0: `src/widgets/textarea/lv_textarea.c`.
//!
//! Also covers `include/lvgl/widgets/lv_textarea.h` and
//! `src/widgets/textarea/lv_textarea_private.h`. The text area is a base
//! object with a label child (`lv_textarea_t::label`); the parity
//! configuration enables `LV_LABEL_TEXT_SELECTION`, so the selection fields
//! are always present. The file-scope `ta_insert_replace` static is a
//! `TextareaStatics` stored with [`Ui::widget_static_mut`].
//! `LV_USE_OBJ_PROPERTY` is disabled in the parity configuration, so the
//! property table is not provided.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;
use alloc::string::String;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::misc::style::SharedStr;
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::{Area, Event, Obj, ObjClass, Part, Point, Ui};

/// `LV_TEXTAREA_CURSOR_LAST`: put the cursor after the last character.
pub const TEXTAREA_CURSOR_LAST: i32 = 0x7FFF;

/// `LV_PART_TEXTAREA_PLACEHOLDER` (= `LV_PART_CUSTOM_FIRST`): the
/// placeholder text part.
pub const PART_TEXTAREA_PLACEHOLDER: Part = Part::CUSTOM_FIRST;

/// `LV_TEXTAREA_DEF_CURSOR_BLINK_TIME` (ms).
const TEXTAREA_DEF_CURSOR_BLINK_TIME: u32 = 400;
/// `LV_TEXTAREA_DEF_PWD_SHOW_TIME` (ms).
const TEXTAREA_DEF_PWD_SHOW_TIME: u32 = 1500;
/// `LV_TEXTAREA_PWD_BULLET_UNICODE`
const TEXTAREA_PWD_BULLET_UNICODE: u32 = 0x2022;
/// `IGNORE_KERNING`
const IGNORE_KERNING: u32 = 0;

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

    /// `lv_textarea_add_char()`: insert a character (a UTF-8 encoded letter
    /// as from `lv_text_encoded_conv_wc`) at the cursor.
    pub fn add_char(self, ui: &mut Ui, c: u32) {
        todo!()
    }

    /// `lv_textarea_add_text()`: insert a text at the cursor.
    pub fn add_text(self, ui: &mut Ui, txt: &str) {
        todo!()
    }

    /// `lv_textarea_delete_char()`: delete the character left of the cursor.
    pub fn delete_char(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_textarea_delete_char_forward()`: delete the character right of the
    /// cursor.
    pub fn delete_char_forward(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_textarea_set_text()`
    pub fn set_text(self, ui: &mut Ui, txt: &str) {
        todo!()
    }

    /// `lv_textarea_set_placeholder_text()`: `None` removes the placeholder.
    pub fn set_placeholder_text(self, ui: &mut Ui, txt: Option<&str>) {
        todo!()
    }

    /// `lv_textarea_set_cursor_pos()`: character index; < 0 counts from the
    /// end, [`TEXTAREA_CURSOR_LAST`] goes after the last character.
    pub fn set_cursor_pos(self, ui: &mut Ui, pos: i32) {
        todo!()
    }

    /// `lv_textarea_set_cursor_click_pos()`: position the cursor by clicking.
    pub fn set_cursor_click_pos(self, ui: &mut Ui, en: bool) {
        todo!()
    }

    /// `lv_textarea_set_password_mode()`
    pub fn set_password_mode(self, ui: &mut Ui, en: bool) {
        todo!()
    }

    /// `lv_textarea_set_password_bullet()`: `None` uses the default bullet.
    pub fn set_password_bullet(self, ui: &mut Ui, bullet: Option<&str>) {
        todo!()
    }

    /// `lv_textarea_set_one_line()`
    pub fn set_one_line(self, ui: &mut Ui, en: bool) {
        todo!()
    }

    /// `lv_textarea_set_accepted_chars()`: a copy is saved; `None` accepts
    /// every character.
    pub fn set_accepted_chars(self, ui: &mut Ui, list: Option<&str>) {
        todo!()
    }

    /// `lv_textarea_set_accepted_chars_static()`: the list is referenced;
    /// `None` accepts every character.
    pub fn set_accepted_chars_static(self, ui: &mut Ui, list: Option<&'static str>) {
        todo!()
    }

    /// `lv_textarea_set_max_length()`: max. number of characters that can be
    /// added (`lv_textarea_set_text` ignores it).
    pub fn set_max_length(self, ui: &mut Ui, num: u32) {
        todo!()
    }

    /// `lv_textarea_set_insert_replace()`: in `LV_EVENT_INSERT`, replace the
    /// text to insert. `None` or `""` drops it.
    pub fn set_insert_replace(self, ui: &mut Ui, txt: Option<SharedStr>) {
        todo!()
    }

    /// `lv_textarea_set_text_selection()`: enable/disable selection mode.
    pub fn set_text_selection(self, ui: &mut Ui, en: bool) {
        todo!()
    }

    /// `lv_textarea_set_password_show_time()`: ms before a character becomes
    /// a bullet, 0: hide immediately.
    pub fn set_password_show_time(self, ui: &mut Ui, time: u32) {
        todo!()
    }

    /// `lv_textarea_set_align()`: set the label's text alignment.
    #[deprecated(note = "use the text_align style property instead")]
    pub fn set_align(self, ui: &mut Ui, align: TextAlign) {
        todo!()
    }

    /// `lv_textarea_get_text()`: the real text, also in password mode.
    pub fn text(self, ui: &Ui) -> &str {
        todo!()
    }

    /// `lv_textarea_get_placeholder_text()`
    pub fn placeholder_text(self, ui: &Ui) -> &str {
        todo!()
    }

    /// `lv_textarea_get_label()`
    pub fn label(self, ui: &Ui) -> Option<Obj> {
        todo!()
    }

    /// `lv_textarea_get_cursor_pos()`: the cursor position in characters.
    pub fn cursor_pos(self, ui: &Ui) -> u32 {
        todo!()
    }

    /// `lv_textarea_get_cursor_click_pos()`
    pub fn cursor_click_pos(self, ui: &Ui) -> bool {
        todo!()
    }

    /// `lv_textarea_get_password_mode()`
    pub fn password_mode(self, ui: &Ui) -> bool {
        todo!()
    }

    /// `lv_textarea_get_password_bullet()`
    pub fn password_bullet(self, ui: &Ui) -> &str {
        todo!()
    }

    /// `lv_textarea_get_one_line()`
    pub fn one_line(self, ui: &Ui) -> bool {
        todo!()
    }

    /// `lv_textarea_get_accepted_chars()`: `None` is `NULL`.
    pub fn accepted_chars(self, ui: &Ui) -> Option<&str> {
        todo!()
    }

    /// `lv_textarea_get_max_length()`
    pub fn max_length(self, ui: &Ui) -> u32 {
        todo!()
    }

    /// `lv_textarea_text_is_selected()`
    pub fn text_is_selected(self, ui: &Ui) -> bool {
        todo!()
    }

    /// `lv_textarea_get_text_selection()`: whether selection mode is enabled.
    pub fn text_selection(self, ui: &Ui) -> bool {
        todo!()
    }

    /// `lv_textarea_get_password_show_time()`
    pub fn password_show_time(self, ui: &Ui) -> u32 {
        todo!()
    }

    /// `lv_textarea_get_current_char()`: the character at the cursor or 0.
    pub fn current_char(self, ui: &Ui) -> u32 {
        todo!()
    }

    /// `lv_textarea_clear_selection()`
    pub fn clear_selection(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_textarea_cursor_right()`: move the cursor one character right.
    pub fn cursor_right(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_textarea_cursor_left()`: move the cursor one character left.
    pub fn cursor_left(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_textarea_cursor_down()`: move the cursor one line down.
    pub fn cursor_down(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_textarea_cursor_up()`: move the cursor one line up.
    pub fn cursor_up(self, ui: &mut Ui) {
        todo!()
    }
}

impl From<Textarea> for Obj {
    fn from(textarea: Textarea) -> Self {
        textarea.obj()
    }
}

/// `lv_textarea_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_textarea_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_textarea_event()`
fn textarea_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    todo!()
}

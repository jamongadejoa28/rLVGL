//! Ported from lvgl v9.6.0: `src/widgets/label/lv_label.c` and its public/private headers.
//!
//! Text size is cached by GET_SELF_SIZE; text refresh is deferred until the
//! display's UPDATE_LAYOUT_COMPLETED event, exactly as in upstream. UTF-8 text
//! has owned or static storage, and draw tasks share an Rc text snapshot.
//!
//! Observer binding (`lv_label_bind_text()`) is declared in this crate's
//! `observer.rs`; translation and Arabic shaping require core subsystems
//! that are not implemented yet and are not exposed here. BiDi paragraph
//! processing has the same identity fallback as the current core text renderer.
//!
//! Inserting into dotted text intentionally restores the original text first.
//! Upstream inserts into the visible byte buffer before restoring saved bytes;
//! that order can produce malformed UTF-8. This adaptation preserves Rust's
//! valid `&str` invariant and interprets insertion indices in the original text.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::any::Any;
use core::cell::Cell;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::draw::label::{DRAW_LABEL_NO_TXT_SEL, LabelDsc, LabelHint};
use ferrovgl_core::font::{self, Font};
use ferrovgl_core::misc::anim::{ANIM_REPEAT_INFINITE, Anim, AnimExec, AnimTarget};
use ferrovgl_core::misc::event::{EventCallback, LvResult};
use ferrovgl_core::misc::text::{
    self, LeadingTrim, TEXT_LEN_MAX, TextAlign, TextAttributes, TextCmdState, TextFlags,
};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Area, Event, EventCode, FontRef, Obj, ObjClass, Part, Point, Ui};

/// `LV_LABEL_DOT_NUM`.
pub const LABEL_DOT_NUM: usize = 3;
/// `LV_LABEL_POS_LAST`: insert after the final character.
pub const LABEL_POS_LAST: u32 = 0xFFFF;
/// `LV_LABEL_TEXT_SELECTION_OFF`.
pub const LABEL_TEXT_SELECTION_OFF: u32 = DRAW_LABEL_NO_TXT_SEL;
/// `LV_LABEL_DEFAULT_TEXT` with `LV_WIDGETS_HAS_DEFAULT_VALUE` enabled.
pub const LABEL_DEFAULT_TEXT: &str = "Text";
const DOT_BEGIN_INVALID: u32 = u32::MAX;
const WAIT_CHAR_COUNT: i32 = 3;
const SCROLL_DELAY: u32 = 300;
const HINT_HEIGHT_LIMIT: i32 = 1024;
const ANIM_OFFSET_X: u32 = 0x4c42_4c58;
const ANIM_OFFSET_Y: u32 = 0x4c42_4c59;

/// `lv_label_long_mode_t`, with the upstream discriminants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum LabelLongMode {
    /// Wrap long lines and expand content height.
    #[default]
    Wrap = 0,
    /// Replace overflow on the last visible line with three dots.
    Dots = 1,
    /// Scroll overflowing text back and forth.
    Scroll = 2,
    /// Scroll continuously, drawing a second copy.
    ScrollCircular = 3,
    /// Clip text to the object without wrapping.
    Clip = 4,
}

enum TextStorage {
    None,
    // Keep bytes past the first NUL: dots overwrite bytes temporarily in C.
    Owned(Vec<u8>),
    Static(&'static str),
}

impl TextStorage {
    fn as_str(&self) -> &str {
        let bytes = match self {
            Self::None => &[],
            Self::Owned(bytes) => bytes.as_slice(),
            Self::Static(text) => text.as_bytes(),
        };
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        // Every public input is valid UTF-8; dots start on a character boundary.
        core::str::from_utf8(&bytes[..end]).expect("label text remains valid UTF-8")
    }
    fn owned(text: &str) -> Self {
        let text = text.split('\0').next().unwrap_or("");
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        Self::Owned(bytes)
    }
    fn is_static(&self) -> bool {
        matches!(self, Self::Static(_))
    }
}

struct LabelState {
    text: TextStorage,
    draw_text: Rc<str>,
    dot: [u8; LABEL_DOT_NUM + 1],
    dot_begin: u32,
    max_lines: i32,
    hint: Rc<Cell<LabelHint>>,
    sel_start: u32,
    sel_end: u32,
    size_cache: Point,
    offset: Point,
    long_mode: LabelLongMode,
    recolor: bool,
    expand: bool,
    invalid_size_cache: bool,
    need_refr_text: bool,
    text_size: Point,
    layout_callback: Option<(EventCallback, Rc<dyn Any>)>,
}

impl LabelState {
    fn new() -> Self {
        Self {
            text: TextStorage::None,
            draw_text: Rc::from(""),
            dot: [0; 4],
            dot_begin: DOT_BEGIN_INVALID,
            max_lines: 0,
            hint: Rc::new(Cell::new(LabelHint {
                line_start: -1,
                y: 0,
                coord_y: 0,
            })),
            sel_start: LABEL_TEXT_SELECTION_OFF,
            sel_end: LABEL_TEXT_SELECTION_OFF,
            size_cache: Point::new(0, 0),
            offset: Point::new(0, 0),
            long_mode: LabelLongMode::Wrap,
            recolor: false,
            expand: false,
            invalid_size_cache: false,
            need_refr_text: false,
            text_size: Point::new(0, 0),
            layout_callback: None,
        }
    }
    fn update_draw_text(&mut self) {
        self.draw_text = Rc::from(self.text.as_str());
    }
}

ferrovgl_core::impl_widget_state!(LabelState);

/// `lv_label_class`.
pub static LABEL_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_label",
    new_state: || Box::new(LabelState::new()),
    constructor: Some(constructor),
    destructor: Some(destructor),
    event_cb: Some(label_event),
    width_def: coord::SIZE_CONTENT,
    height_def: coord::SIZE_CONTENT,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::Inherit,
    theme_inheritable: false,
};

/// A copyable label handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Label(Obj);

impl Label {
    /// `lv_label_create()`. `None` creates a label screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&LABEL_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }
    /// The base object for positioning and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }
    /// Check that an existing object contains label state.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        state(ui, obj).map(|_| Self(obj))
    }
    /// `lv_label_set_text()`: copy the text, stopping at the first NUL.
    pub fn set_text(self, ui: &mut Ui, text: &str) {
        revert_dots(ui, self.0);
        if let Some(label) = state_mut(ui, self.0) {
            label.text = TextStorage::owned(text);
            label.update_draw_text();
        }
        mark_need_refr_text(ui, self.0);
    }
    /// The `lv_label_set_text(label, NULL)` form: retain and refresh current text.
    pub fn refresh_text(self, ui: &mut Ui) {
        revert_dots(ui, self.0);
        // In C a static source takes set_text_internal's copy branch, so this
        // form also switches static text to mutable, owned storage.
        if let Some(label) = state_mut(ui, self.0)
            && label.text.is_static()
        {
            label.text = TextStorage::owned(label.text.as_str());
            label.update_draw_text();
        }
        mark_need_refr_text(ui, self.0);
    }
    /// `lv_label_set_text_fmt/vfmt()`, using Rust formatting arguments.
    pub fn set_text_fmt(self, ui: &mut Ui, args: core::fmt::Arguments<'_>) {
        self.set_text(ui, &text::set_text_vfmt(args));
    }
    /// `lv_label_set_text_static()`. Static strings cannot be changed by dots,
    /// insertion or deletion, matching upstream's static-text restriction.
    pub fn set_text_static(self, ui: &mut Ui, text: &'static str) {
        if let Some(label) = state_mut(ui, self.0) {
            label.text = TextStorage::Static(text);
            label.update_draw_text();
        }
        mark_need_refr_text(ui, self.0);
    }
    /// `lv_label_set_long_mode()`; resets both offset animations.
    pub fn set_long_mode(self, ui: &mut Ui, mode: LabelLongMode) {
        ui.anim_delete(AnimTarget::Obj(self.0), Some(&offset_exec(false)));
        ui.anim_delete(AnimTarget::Obj(self.0), Some(&offset_exec(true)));
        if let Some(label) = state_mut(ui, self.0) {
            label.offset = Point::new(0, 0);
            label.expand = matches!(
                mode,
                LabelLongMode::Scroll | LabelLongMode::ScrollCircular | LabelLongMode::Clip
            );
            label.long_mode = mode;
        }
        mark_need_refr_text(ui, self.0);
    }
    /// `lv_label_set_max_lines()`. Nonpositive values mean no limit.
    pub fn set_max_lines(self, ui: &mut Ui, lines: i32) {
        if let Some(label) = state_mut(ui, self.0) {
            label.max_lines = lines;
        }
        mark_need_refr_text(ui, self.0);
    }
    /// `lv_label_set_text_selection_start()`; indices count Unicode characters.
    pub fn set_text_selection_start(self, ui: &mut Ui, index: u32) {
        if let Some(label) = state_mut(ui, self.0) {
            label.sel_start = index;
            ui.obj_invalidate(self.0);
        }
    }
    /// `lv_label_set_text_selection_end()`.
    pub fn set_text_selection_end(self, ui: &mut Ui, index: u32) {
        if let Some(label) = state_mut(ui, self.0) {
            label.sel_end = index;
            ui.obj_invalidate(self.0);
        }
    }
    /// `lv_label_set_recolor()`; interpret inline `#rrggbb text#` commands.
    pub fn set_recolor(self, ui: &mut Ui, enabled: bool) {
        let Some(label) = state_mut(ui, self.0) else {
            return;
        };
        if label.recolor == enabled {
            return;
        }
        label.recolor = enabled;
        mark_need_refr_text(ui, self.0);
    }
    /// `lv_label_get_text()`, including any currently displayed overflow dots.
    pub fn text(self, ui: &Ui) -> &str {
        state(ui, self.0).map_or("", |s| s.text.as_str())
    }
    /// `lv_label_get_long_mode()`.
    pub fn long_mode(self, ui: &Ui) -> LabelLongMode {
        state(ui, self.0).map_or(LabelLongMode::Wrap, |s| s.long_mode)
    }
    /// `lv_label_get_max_lines()`.
    pub fn max_lines(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |s| s.max_lines)
    }
    /// `lv_label_get_recolor()`.
    pub fn recolor(self, ui: &Ui) -> bool {
        state(ui, self.0).is_some_and(|s| s.recolor)
    }
    /// `lv_label_get_text_selection_start()`.
    pub fn text_selection_start(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(LABEL_TEXT_SELECTION_OFF, |s| s.sel_start)
    }
    /// `lv_label_get_text_selection_end()`.
    pub fn text_selection_end(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(LABEL_TEXT_SELECTION_OFF, |s| s.sel_end)
    }
    /// `lv_label_ins_text()`. `LABEL_POS_LAST` appends; static text is unchanged.
    ///
    /// For dotted labels, restore the original text before inserting. This is
    /// an intentional divergence from C's byte-buffer mutation order, which can
    /// split UTF-8 characters when the saved bytes are subsequently restored.
    /// Insertion indices then refer to the restored text, including its suffix.
    pub fn insert_text(self, ui: &mut Ui, mut position: u32, inserted: &str) {
        let Some(label) = state(ui, self.0) else {
            return;
        };
        if label.text.is_static() {
            return;
        }
        if label.dot_begin != DOT_BEGIN_INVALID {
            revert_dots(ui, self.0);
        }
        let Some(label) = state_mut(ui, self.0) else {
            return;
        };
        let mut value = label.text.as_str().to_string();
        if position == LABEL_POS_LAST {
            position = text::get_encoded_length(value.as_bytes());
        }
        text::ins(&mut value, position, inserted);
        label.text = TextStorage::owned(&value);
        self.refresh_text(ui);
    }
    /// `lv_label_cut_text()`. Character indices are UTF-8 aware.
    pub fn cut_text(self, ui: &mut Ui, position: u32, count: u32) {
        let Some(label) = state_mut(ui, self.0) else {
            return;
        };
        if let TextStorage::Owned(bytes) = &mut label.text {
            // lv_text_cut keeps its allocation, including bytes hidden beyond
            // the dots' NUL terminator. Rebuilding from as_str would discard
            // that suffix and could split a UTF-8 character when dots revert.
            let old_len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            let position = text::encoded_get_byte_id(bytes, position) as usize;
            if position < old_len {
                let removed =
                    (text::encoded_get_byte_id(&bytes[position..], count) as usize).min(old_len - position);
                // Upstream includes the NUL in the forward copy.
                bytes.copy_within(position + removed..=old_len, position);
            }
        } else {
            return;
        }
        label.update_draw_text();
        mark_need_refr_text(ui, self.0);
    }
    /// `lv_label_get_letter_pos()`; coordinates are relative to the content area.
    pub fn letter_pos(self, ui: &Ui, character: u32) -> Point {
        letter_pos(ui, self.0, character)
    }
    /// `lv_label_get_letter_on()`. BiDi uses the current core's identity fallback.
    pub fn letter_on(self, ui: &Ui, position: Point, _bidi: bool) -> u32 {
        letter_on(ui, self.0, position)
    }
    /// `lv_label_is_char_under_pos()`.
    pub fn is_char_under_pos(self, ui: &Ui, position: Point) -> bool {
        char_under_pos(ui, self.0, position)
    }
}

impl From<Label> for Obj {
    fn from(label: Label) -> Self {
        label.0
    }
}

fn state(ui: &Ui, obj: Obj) -> Option<&LabelState> {
    ui.obj(obj)?.widget.get::<LabelState>()
}
fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut LabelState> {
    ui.obj_mut(obj)?.widget.get_mut::<LabelState>()
}

fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    ui.obj_set_clickable(obj, false);
    Label(obj).set_long_mode(ui, LabelLongMode::Wrap);
    Label(obj).set_text(ui, LABEL_DEFAULT_TEXT);
}

fn detach_layout_callback(ui: &mut Ui, obj: Obj) {
    let callback = state_mut(ui, obj).and_then(|label| label.layout_callback.take());
    if let Some((cb, user_data)) = callback
        && let Some(display) = ui.obj_get_display(obj)
    {
        ui.display_remove_event_cb_with_user_data(display, &cb, &user_data);
    }
}

fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    if let Some(label) = state_mut(ui, obj) {
        label.text = TextStorage::None;
        label.update_draw_text();
    }
    detach_layout_callback(ui, obj);
}

fn label_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    if ui.obj_event_base(&LABEL_CLASS, event) != LvResult::Ok {
        return;
    }
    let Some(obj) = event.current_target_obj() else {
        return;
    };
    if state(ui, obj).is_none() {
        return;
    }
    match event.code {
        EventCode::STYLE_CHANGED | EventCode::SIZE_CHANGED => mark_need_refr_text(ui, obj),
        EventCode::REFR_EXT_DRAW_SIZE => {
            if let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN) {
                event.set_ext_draw_size(font::get_line_height(font.as_font()) / 4);
            }
        }
        EventCode::GET_SELF_SIZE => {
            if state(ui, obj).is_some_and(|s| s.invalid_size_cache) {
                calculate_self_size(ui, obj);
            }
            if let Some(size) = event.self_size_info()
                && let Some(label) = state(ui, obj)
            {
                size.x = size.x.max(label.size_cache.x);
                size.y = size.y.max(label.size_cache.y);
            }
        }
        EventCode::DRAW_MAIN => draw_main(ui, obj, event),
        _ => {}
    }
}

fn label_flags(ui: &Ui, obj: Obj) -> TextFlags {
    let Some(label) = state(ui, obj) else {
        return TextFlags::NONE;
    };
    let mut flags = TextFlags::NONE;
    if label.recolor {
        flags |= TextFlags::RECOLOR;
    }
    if label.expand {
        flags |= TextFlags::EXPAND;
    }
    if ui.obj_get_style_width(obj, Part::MAIN) == coord::SIZE_CONTENT
        && ui.obj_get_style_max_width(obj, Part::MAIN) == coord::MAX
        && ui.obj(obj).is_some_and(|o| !o.w_layout)
    {
        flags |= TextFlags::FIT;
    }
    flags
}

fn attributes(ui: &Ui, obj: Obj, max_width: i32, flags: TextFlags) -> TextAttributes {
    TextAttributes {
        line_space: ui.obj_get_style_text_line_space(obj, Part::MAIN),
        letter_space: ui.obj_get_style_text_letter_space(obj, Part::MAIN),
        max_width,
        text_flags: flags,
    }
}

fn calculate_self_size(ui: &mut Ui, obj: Obj) {
    let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN) else {
        return;
    };
    let Some(label) = state(ui, obj) else { return };
    let mut flags = TextFlags::NONE;
    if label.recolor {
        flags |= TextFlags::RECOLOR;
    }
    if label.expand {
        flags |= TextFlags::EXPAND;
    }
    let mut width = if ui.obj_get_style_width(obj, Part::MAIN) == coord::SIZE_CONTENT
        && ui.obj(obj).is_some_and(|o| !o.w_layout)
    {
        coord::MAX
    } else {
        ui.obj_get_content_width(obj)
    };
    width = width.min(ui.obj_get_style_max_width(obj, Part::MAIN));
    let attrs = attributes(ui, obj, width, flags);
    let dot_begin = label.dot_begin;
    let max_lines = label.max_lines;
    revert_dots(ui, obj);
    let mut size = text::get_size_attributes(Label(obj).text(ui), font.as_font(), &attrs);
    set_dots(ui, obj, dot_begin);
    if max_lines > 0 {
        size.y = size
            .y
            .min(font::get_line_height(font.as_font()) * max_lines + attrs.line_space * (max_lines - 1));
    }
    let trim = ui.obj_get_style_text_leading_trim(obj, Part::MAIN);
    if trim != LeadingTrim::None {
        size.y -=
            text::font_get_top_trim(font.as_font(), trim) + text::font_get_bottom_trim(font.as_font(), trim);
    }
    size.y = size.y.min(ui.obj_get_style_max_height(obj, Part::MAIN));
    if let Some(label) = state_mut(ui, obj) {
        label.size_cache = size;
        label.invalid_size_cache = false;
    }
}

fn draw_main(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(layer_id) = event.layer() else { return };
    let Some(label) = state(ui, obj) else { return };
    let mut coords = ui.obj_get_content_coords(obj);
    let mode = label.long_mode;
    let offset = label.offset;
    let text_size = label.text_size;
    let mut descriptor = LabelDsc::new();
    descriptor.text = label.draw_text.clone();
    descriptor.text_static = label.text.is_static();
    descriptor.ofs_x = offset.x;
    descriptor.ofs_y = offset.y;
    descriptor.text_size = text_size;
    descriptor.sel_start = label.sel_start;
    descriptor.sel_end = label.sel_end;
    let trim = ui.obj_get_style_text_leading_trim(obj, Part::MAIN);
    if trim != LeadingTrim::None
        && let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN)
    {
        coords.y1 -= text::font_get_top_trim(font.as_font(), trim);
        coords.y2 += text::font_get_bottom_trim(font.as_font(), trim);
    }
    if mode != LabelLongMode::ScrollCircular && coords.height() >= HINT_HEIGHT_LIMIT {
        descriptor.hint = Some(label.hint.clone());
    }
    descriptor.flag = label_flags(ui, obj);
    descriptor.base.layer = Some(layer_id);
    ui.obj_init_draw_label_dsc(obj, Part::MAIN, &mut descriptor);
    descriptor.align = ui.obj_calculate_style_text_align(obj, Part::MAIN, &descriptor.text);
    if descriptor.sel_start != LABEL_TEXT_SELECTION_OFF && descriptor.sel_end != LABEL_TEXT_SELECTION_OFF {
        descriptor.sel_color = ui.obj_get_style_text_color_filtered(obj, Part::SELECTED);
        descriptor.sel_bg_color = ui.obj_get_style_bg_color(obj, Part::SELECTED);
    }
    descriptor.outline_stroke_color = ui.obj_get_style_text_outline_stroke_color(obj, Part::MAIN);
    descriptor.outline_stroke_opa = ui.obj_get_style_text_outline_stroke_opa(obj, Part::MAIN);
    descriptor.outline_stroke_width = ui.obj_get_style_text_outline_stroke_width(obj, Part::MAIN);
    if matches!(mode, LabelLongMode::Scroll | LabelLongMode::ScrollCircular)
        && matches!(descriptor.align, TextAlign::Center | TextAlign::Right)
        && text_size.x > coords.width()
    {
        descriptor.align = TextAlign::Left;
    }
    let Some(layer) = ui.draw.layer(layer_id) else {
        return;
    };
    let original_clip = layer.clip_area;
    let Some(text_clip) = Area::intersect(&coords, &original_clip) else {
        return;
    };
    if mode == LabelLongMode::Wrap {
        let top = ui.obj_get_scroll_top(obj);
        coords.y1 -= top;
        coords.y2 -= top;
        coords.y2 = ui.obj_get_coords(obj).y2;
    }
    if let Some(layer) = ui.draw.layer_mut(layer_id) {
        if matches!(
            mode,
            LabelLongMode::Scroll | LabelLongMode::ScrollCircular | LabelLongMode::Clip
        ) {
            layer.clip_area = text_clip;
        } else if descriptor.base.drop_shadow_opa == 0 {
            layer.clip_area.y2 = text_clip.y2;
        }
    }
    ui.draw_label(layer_id, &descriptor, &coords);
    if let Some(layer) = ui.draw.layer_mut(layer_id) {
        layer.clip_area = text_clip;
    }
    if mode == LabelLongMode::ScrollCircular
        && let Some(font) = descriptor.font.clone()
    {
        if text_size.x > coords.width() {
            descriptor.ofs_x = offset.x
                + text_size.x
                + i32::from(font::get_glyph_width(font.as_font(), 32, 32)) * WAIT_CHAR_COUNT;
            descriptor.ofs_y = offset.y;
            ui.draw_label(layer_id, &descriptor, &coords);
        }
        if text_size.y > coords.height() {
            descriptor.ofs_x = offset.x;
            descriptor.ofs_y = offset.y + text_size.y + font::get_line_height(font.as_font());
            ui.draw_label(layer_id, &descriptor, &coords);
        }
    }
    if let Some(layer) = ui.draw.layer_mut(layer_id) {
        layer.clip_area = original_clip;
    }
}

fn mark_need_refr_text(ui: &mut Ui, obj: Obj) {
    let Some(label) = state_mut(ui, obj) else { return };
    if matches!(label.text, TextStorage::None) {
        return;
    }
    label.invalid_size_cache = true;
    ui.obj_invalidate(obj);
    if ui.obj_is_style_any_height_content(obj) || ui.obj_is_style_any_width_content(obj) {
        ui.obj_mark_layout_as_dirty(obj);
    }
    let Some(label) = state_mut(ui, obj) else { return };
    if label.need_refr_text {
        return;
    }
    label.need_refr_text = true;
    let Some(display) = ui.obj_get_display(obj) else {
        return;
    };
    let callback: EventCallback = Rc::new(move |ui, _event| update_layout_completed(ui, obj));
    let user_data: Rc<dyn Any> = Rc::new(obj);
    if let Some(label) = state_mut(ui, obj) {
        label.layout_callback = Some((callback.clone(), user_data.clone()));
    }
    ui.display_add_event_cb(
        display,
        callback,
        EventCode::UPDATE_LAYOUT_COMPLETED,
        Some(user_data),
    );
}

fn update_layout_completed(ui: &mut Ui, obj: Obj) {
    if state(ui, obj).is_none() {
        return;
    }
    detach_layout_callback(ui, obj);
    let Some(label) = state_mut(ui, obj) else { return };
    if !label.need_refr_text {
        return;
    }
    label.need_refr_text = false;
    refresh_text(ui, obj);
}

fn revert_dots(ui: &mut Ui, obj: Obj) {
    let Some(label) = state_mut(ui, obj) else { return };
    if label.dot_begin != DOT_BEGIN_INVALID
        && let TextStorage::Owned(bytes) = &mut label.text
    {
        for (index, &value) in label.dot.iter().enumerate() {
            if value == 0 {
                break;
            }
            if let Some(byte) = bytes.get_mut(label.dot_begin as usize + index) {
                *byte = value;
            }
        }
    }
    label.dot_begin = DOT_BEGIN_INVALID;
    label.update_draw_text();
}

fn set_dots(ui: &mut Ui, obj: Obj, dot_begin: u32) {
    if dot_begin == DOT_BEGIN_INVALID {
        return;
    }
    let Some(label) = state_mut(ui, obj) else { return };
    debug_assert_eq!(label.dot_begin, DOT_BEGIN_INVALID);
    label.dot_begin = dot_begin;
    let TextStorage::Owned(bytes) = &mut label.text else {
        return;
    };
    let begin = dot_begin as usize;
    for index in 0..LABEL_DOT_NUM + 1 {
        label.dot[index] = bytes.get(begin + index).copied().unwrap_or(0);
    }
    let mut count = 0;
    while count < LABEL_DOT_NUM && bytes.get(begin + count).is_some_and(|&b| b != 0) {
        bytes[begin + count] = b'.';
        count += 1;
    }
    if let Some(byte) = bytes.get_mut(begin + count) {
        *byte = 0;
    }
    label.update_draw_text();
}

fn offset_exec(vertical: bool) -> AnimExec {
    AnimExec::Custom {
        tag: if vertical { ANIM_OFFSET_Y } else { ANIM_OFFSET_X },
        f: Rc::new(move |ui, target, value| {
            if let AnimTarget::Obj(obj) = target
                && let Some(label) = state_mut(ui, obj)
            {
                if vertical {
                    label.offset.y = value;
                } else {
                    label.offset.x = value;
                }
                ui.obj_invalidate(obj);
            }
        }),
    }
}

fn overwrite_anim_properties(dest: &mut Anim, source: &Anim, mode: LabelLongMode) {
    if dest.act_time <= 0 {
        dest.act_time = source.act_time;
    }
    dest.repeat_cnt = source.repeat_cnt;
    dest.repeat_delay = source.repeat_delay;
    dest.completed_cb = source.completed_cb.clone();
    if mode == LabelLongMode::Scroll {
        dest.reverse_delay = source.reverse_delay;
    }
}

fn refresh_text(ui: &mut Ui, obj: Obj) {
    let Some(label) = state(ui, obj) else { return };
    if matches!(label.text, TextStorage::None) {
        return;
    }
    let mut hint = label.hint.get();
    hint.line_start = -1;
    label.hint.set(hint);
    let mode = label.long_mode;
    let coords = ui.obj_get_content_coords(obj);
    let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN) else {
        return;
    };
    let attrs = attributes(ui, obj, coords.width(), label_flags(ui, obj));
    revert_dots(ui, obj);
    let size = text::get_size_attributes(Label(obj).text(ui), font.as_font(), &attrs);
    if let Some(label) = state_mut(ui, obj) {
        label.text_size = size;
    }
    if matches!(mode, LabelLongMode::Scroll | LabelLongMode::ScrollCircular) {
        refresh_scroll(ui, obj, mode, size, coords, &font);
    } else if mode == LabelLongMode::Dots
        && size.y > coords.height()
        && size.y > font::get_line_height(font.as_font())
        && text::get_encoded_length(Label(obj).text(ui).as_bytes()) > LABEL_DOT_NUM as u32
    {
        let line_height = font::get_line_height(font.as_font());
        let mut pos = Point::new(
            coords.width()
                - (i32::from(font::get_glyph_width(font.as_font(), 46, 46)) + attrs.letter_space)
                    * LABEL_DOT_NUM as i32,
            coords.height(),
        );
        let period = line_height + attrs.line_space;
        if period > 0 {
            let overflow = pos.y % period;
            if overflow >= line_height {
                pos.y = pos.y - overflow + line_height;
            } else {
                pos.y = pos.y - overflow - attrs.line_space;
            }
        }
        let char_id = letter_on(ui, obj, pos);
        let value = Label(obj).text(ui);
        let mut byte_id = text::encoded_get_byte_id(value.as_bytes(), char_id);
        while byte_id as usize + LABEL_DOT_NUM > value.len() && byte_id > 0 {
            text::encoded_prev(value.as_bytes(), Some(&mut byte_id));
        }
        set_dots(ui, obj, byte_id);
    }
    ui.obj_invalidate(obj);
}

fn refresh_scroll(ui: &mut Ui, obj: Obj, mode: LabelLongMode, size: Point, coords: Area, font: &FontRef) {
    let template = ui.obj_get_style_anim(obj, Part::MAIN);
    let duration = match ui.obj_get_style_anim_duration(obj, Part::MAIN) {
        0 => Ui::anim_speed_clamped(40, 300, 10000),
        value => value,
    };
    let mut animation = Anim::new();
    animation.var = AnimTarget::Obj(obj);
    animation.repeat_cnt = ANIM_REPEAT_INFINITE;
    if mode == LabelLongMode::Scroll {
        animation.reverse_delay = SCROLL_DELAY;
        animation.repeat_delay = SCROLL_DELAY;
    }
    let horizontal = size.x > coords.width();
    for vertical in [false, true] {
        let needed = if vertical {
            size.y > coords.height() && !horizontal
        } else {
            horizontal
        };
        let exec = offset_exec(vertical);
        if !needed {
            ui.anim_delete(AnimTarget::Obj(obj), Some(&exec));
            if let Some(label) = state_mut(ui, obj) {
                if vertical {
                    label.offset.y = 0;
                } else {
                    label.offset.x = 0;
                }
            }
            continue;
        }
        let end = match (mode, vertical) {
            (LabelLongMode::Scroll, false) => coords.width() - size.x,
            (LabelLongMode::Scroll, true) => coords.height() - size.y - font::get_line_height(font.as_font()),
            (_, false) => {
                -size.x - i32::from(font::get_glyph_width(font.as_font(), 32, 32)) * WAIT_CHAR_COUNT
            }
            (_, true) => -size.y - font::get_line_height(font.as_font()),
        };
        animation.set_values(0, end);
        animation.exec = exec.clone();
        let current = ui
            .anim_get(AnimTarget::Obj(obj), Some(&exec))
            .and_then(|id| ui.anim(id))
            .cloned();
        let mut act_time = current.as_ref().map_or(0, |a| a.act_time);
        if mode == LabelLongMode::Scroll {
            let limit = if vertical {
                animation.duration
            } else {
                Ui::anim_resolve_speed(duration, 0, end) as i32
            };
            act_time = act_time.min(limit);
            animation.act_time = act_time;
            if current.as_ref().is_some_and(|a| a.reverse_play_in_progress) {
                animation.reverse_play_in_progress = true;
                core::mem::swap(&mut animation.start_value, &mut animation.end_value);
            }
            animation.set_duration(duration).set_reverse_duration(duration);
            if let Some(template) = &template {
                overwrite_anim_properties(&mut animation, template, mode);
            }
        } else {
            animation.set_duration(duration);
            if !vertical && act_time < Ui::anim_resolve_speed(duration, 0, end) as i32 {
                animation.act_time = act_time;
            }
            if let Some(template) = &template {
                overwrite_anim_properties(&mut animation, template, mode);
            } else if vertical && act_time < animation.duration {
                animation.act_time = act_time;
            }
        }
        ui.anim_start(&animation);
        if !vertical
            && mode == LabelLongMode::Scroll
            && act_time < 0
            && let Some(label) = state_mut(ui, obj)
        {
            label.offset.x = 0;
        }
    }
}

fn tail(value: &str, index: u32) -> &str {
    value.get(index as usize..).unwrap_or("")
}
fn byte(value: &str, index: u32) -> u8 {
    value.as_bytes().get(index as usize).copied().unwrap_or(0)
}

fn align_x(
    x: &mut i32,
    align: TextAlign,
    value: &str,
    length: u32,
    font: &dyn Font,
    coords: &Area,
    attrs: &TextAttributes,
) {
    if align == TextAlign::Center {
        *x += coords.width() / 2 - text::get_line_width(value, length, font, attrs) / 2;
    } else if align == TextAlign::Right {
        *x += coords.width() - text::get_line_width(value, length, font, attrs);
    }
}

fn letter_pos(ui: &Ui, obj: Obj, character: u32) -> Point {
    let Some(label) = state(ui, obj) else {
        return Point::new(0, 0);
    };
    let value = label.text.as_str();
    let align = ui.obj_calculate_style_text_align(obj, Part::MAIN, value);
    if value.is_empty() {
        return Point::new(
            match align {
                TextAlign::Center => ui.obj_get_content_width(obj) / 2,
                TextAlign::Right => ui.obj_get_content_width(obj),
                _ => 0,
            },
            0,
        );
    }
    let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN) else {
        return Point::new(0, 0);
    };
    let coords = ui.obj_get_content_coords(obj);
    let mut attrs = attributes(ui, obj, coords.width(), label_flags(ui, obj));
    let height = font::get_line_height(font.as_font());
    let byte_id = text::encoded_get_byte_id(value.as_bytes(), character);
    let (mut y, mut start, mut next) = (0, 0, 0);
    while byte(value, next) != 0 {
        if y + height + attrs.line_space + height > coords.height() && label.long_mode == LabelLongMode::Dots
        {
            attrs.text_flags |= TextFlags::BREAK_ALL;
        }
        next += text::get_next_line(tail(value, start), TEXT_LEN_MAX, font.as_font(), None, &mut attrs);
        if byte_id < next || byte(value, next) == 0 {
            break;
        }
        y += height + attrs.line_space;
        start = next;
    }
    if byte_id > 0 && matches!(byte(value, byte_id - 1), b'\n' | b'\r') && byte(value, byte_id) == 0 {
        y += height + attrs.line_space;
        start = byte_id;
    }
    let line = tail(value, start);
    let mut x = text::get_width(line, byte_id.saturating_sub(start), font.as_font(), &attrs);
    if character != start {
        x += attrs.letter_space;
    }
    align_x(
        &mut x,
        align,
        line,
        next.saturating_sub(start),
        font.as_font(),
        &coords,
        &attrs,
    );
    let trim = ui.obj_get_style_text_leading_trim(obj, Part::MAIN);
    if trim != LeadingTrim::None {
        y -= text::font_get_top_trim(font.as_font(), trim);
    }
    Point::new(x, y)
}

fn letter_on(ui: &Ui, obj: Obj, position: Point) -> u32 {
    let Some(label) = state(ui, obj) else { return 0 };
    let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN) else {
        return 0;
    };
    let value = label.text.as_str();
    let coords = ui.obj_get_content_coords(obj);
    let mut pos = Point::new(
        position.x - ui.obj_get_style_pad_left(obj, Part::MAIN),
        position.y - ui.obj_get_style_pad_top(obj, Part::MAIN),
    );
    let trim = ui.obj_get_style_text_leading_trim(obj, Part::MAIN);
    if trim != LeadingTrim::None {
        pos.y += text::font_get_top_trim(font.as_font(), trim);
    }
    let height = font::get_line_height(font.as_font());
    let mut attrs = attributes(ui, obj, coords.width(), label_flags(ui, obj));
    let (mut y, mut start, mut next) = (0, 0, 0);
    while byte(value, start) != 0 {
        if y + height + attrs.line_space + height > coords.height() && label.long_mode == LabelLongMode::Dots
        {
            attrs.text_flags |= TextFlags::BREAK_ALL;
        }
        next += text::get_next_line(tail(value, start), TEXT_LEN_MAX, font.as_font(), None, &mut attrs);
        if pos.y <= y + height {
            let mut previous = next;
            let letter = text::encoded_prev(value.as_bytes(), Some(&mut previous));
            if letter != 10 && byte(value, next) == 0 {
                next += 1;
            }
            break;
        }
        y += height + attrs.line_space;
        start = next;
    }
    let line = tail(value, start);
    let align = ui.obj_calculate_style_text_align(obj, Part::MAIN, value);
    let mut x = 0;
    align_x(
        &mut x,
        align,
        line,
        next.saturating_sub(start),
        font.as_font(),
        &coords,
        &attrs,
    );
    let (mut index, mut active) = (0, 0);
    let mut command = TextCmdState::Wait;
    while next > 0 && index + start < next {
        let (mut letter, mut following) = (0, 0);
        text::encoded_letter_next_2(line, &mut letter, &mut following, &mut index);
        if attrs.text_flags.contains(TextFlags::RECOLOR)
            && text::is_cmd(&mut command, u32::from(byte(line, index)))
        {
            continue;
        }
        let width = i32::from(font::get_glyph_width(font.as_font(), letter, following));
        if pos.x < x + width || index + start == next || byte(value, active + start) == 0 {
            index = active;
            break;
        }
        x += width + attrs.letter_space;
        active = index;
    }
    text::encoded_get_char_id(line.as_bytes(), index) + text::encoded_get_char_id(value.as_bytes(), start)
}

fn char_under_pos(ui: &Ui, obj: Obj, pos: Point) -> bool {
    let Some(label) = state(ui, obj) else { return false };
    let Some(font) = ui.obj_get_style_text_font(obj, Part::MAIN) else {
        return false;
    };
    let value = label.text.as_str();
    let coords = ui.obj_get_content_coords(obj);
    let height = font::get_line_height(font.as_font());
    let mut attrs = attributes(ui, obj, coords.width(), label_flags(ui, obj));
    let (mut y, mut start, mut next) = (0, 0, 0);
    while byte(value, start) != 0 {
        if y + height + attrs.line_space + height > coords.height() && label.long_mode == LabelLongMode::Dots
        {
            attrs.text_flags |= TextFlags::BREAK_ALL;
        }
        next += text::get_next_line(tail(value, start), TEXT_LEN_MAX, font.as_font(), None, &mut attrs);
        if pos.y <= y + height {
            break;
        }
        y += height + attrs.line_space;
        start = next;
    }
    let mut x = 0;
    align_x(
        &mut x,
        ui.obj_calculate_style_text_align(obj, Part::MAIN, value),
        tail(value, start),
        next.saturating_sub(start),
        font.as_font(),
        &coords,
        &attrs,
    );
    let mut command = TextCmdState::Wait;
    let (mut last_x, mut index, mut letter, mut following) = (0, start, 0, 0);
    while next > 0 && index < next {
        text::encoded_letter_next_2(value, &mut letter, &mut following, &mut index);
        if attrs.text_flags.contains(TextFlags::RECOLOR)
            && text::is_cmd(&mut command, u32::from(byte(value, index)))
        {
            continue;
        }
        last_x = x;
        x += i32::from(font::get_glyph_width(font.as_font(), letter, following));
        if pos.x < x {
            break;
        }
        x += attrs.letter_space;
    }
    let max_diff =
        i32::from(font::get_glyph_width(font.as_font(), letter, following)) + attrs.letter_space + 1;
    pos.x >= last_x - attrs.letter_space && pos.x <= last_x + max_diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::{StyleSelector, UiConfig};

    fn setup() -> (Ui, Obj) {
        let mut ui = Ui::new(UiConfig {
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        });
        ui.display_create(800, 480);
        let screen = ui.screen_active().unwrap();
        (ui, screen)
    }

    #[test]
    fn content_size_recolor_and_deferred_refresh() {
        let (mut ui, screen) = setup();
        let label = Label::create(&mut ui, screen);
        assert_eq!(label.text(&ui), "Text");
        assert_eq!(label.long_mode(&ui), LabelLongMode::Wrap);
        assert!(!ui.obj_is_clickable(label.obj()));
        ui.obj_update_layout(screen);
        assert_eq!(ui.obj_get_width(label.obj()), 29);
        assert_eq!(ui.obj_get_height(label.obj()), 16);
        label.set_text(&mut ui, "Write a #ff0000 red# word");
        ui.obj_update_layout(screen);
        let with_commands = ui.obj_get_width(label.obj());
        label.set_recolor(&mut ui, true);
        ui.obj_update_layout(screen);
        assert!(ui.obj_get_width(label.obj()) < with_commands);
        assert!(!state(&ui, label.obj()).unwrap().need_refr_text);
        assert!(state(&ui, label.obj()).unwrap().layout_callback.is_none());
        let count = ui.display_get_event_count(ui.display_get_default().unwrap());
        label.set_text(&mut ui, "a");
        label.set_text(&mut ui, "b");
        assert_eq!(
            ui.display_get_event_count(ui.display_get_default().unwrap()),
            count + 1
        );
        ui.obj_delete(label.obj());
        assert_eq!(
            ui.display_get_event_count(ui.display_get_default().unwrap()),
            count
        );
        ui.deinit();
    }

    #[test]
    fn multiline_positions_match_upstream_label_tests() {
        let (mut ui, screen) = setup();
        let label = Label::create(&mut ui, screen);
        let value = "Lorem ipsum dolor sit amet,\nconsectetur adipiscing elit.\nCras malesuada ultrices magna in rutrum.\n";
        label.set_text(&mut ui, value);
        ui.obj_update_layout(screen);
        assert_eq!(label.letter_pos(&ui, 0), Point::new(0, 0));
        assert_eq!(label.letter_pos(&ui, value.len() as u32 - 1), Point::new(305, 32));
        assert_eq!(label.letter_pos(&ui, value.len() as u32), Point::new(0, 48));
        assert_eq!(label.letter_on(&ui, Point::new(0, 0), false), 0);
        assert!(label.is_char_under_pos(&ui, Point::new(1, 0)));
    }

    #[test]
    fn utf8_insertion_cut_and_static_storage() {
        let (mut ui, screen) = setup();
        let label = Label::create(&mut ui, screen);
        label.set_text(&mut ui, "aé界z");
        label.insert_text(&mut ui, 2, "!🙂");
        assert_eq!(label.text(&ui), "aé!🙂界z");
        label.cut_text(&mut ui, 1, 3);
        assert_eq!(label.text(&ui), "a界z");
        label.insert_text(&mut ui, LABEL_POS_LAST, "!");
        assert_eq!(label.text(&ui), "a界z!");
        label.set_text_static(&mut ui, "static");
        label.insert_text(&mut ui, 0, "ignored");
        label.cut_text(&mut ui, 0, 4);
        assert_eq!(label.text(&ui), "static");
        label.refresh_text(&mut ui);
        label.insert_text(&mut ui, LABEL_POS_LAST, "!");
        assert_eq!(label.text(&ui), "static!");
        label.set_text_fmt(&mut ui, format_args!("{} °C", 23));
        assert_eq!(label.text(&ui), "23 °C");
    }

    #[test]
    fn dots_restore_owned_utf8_text_and_skip_static_text() {
        let (mut ui, screen) = setup();
        let label = Label::create(&mut ui, screen);
        let value = "Hello world and another long line of text";
        label.set_text(&mut ui, value);
        label.set_long_mode(&mut ui, LabelLongMode::Dots);
        ui.obj_set_size(label.obj(), 60, 32);
        ui.obj_update_layout(screen);
        assert!(label.text(&ui).ends_with("..."), "{}", label.text(&ui));
        label.set_long_mode(&mut ui, LabelLongMode::Wrap);
        ui.obj_update_layout(screen);
        assert_eq!(label.text(&ui), value);
        label.set_text(&mut ui, "aé界🙂z");
        set_dots(&mut ui, label.obj(), 1);
        assert_eq!(label.text(&ui), "a...");
        revert_dots(&mut ui, label.obj());
        assert_eq!(label.text(&ui), "aé界🙂z");
        label.set_text_static(&mut ui, value);
        label.set_long_mode(&mut ui, LabelLongMode::Dots);
        ui.obj_update_layout(screen);
        assert_eq!(label.text(&ui), value);
    }

    #[test]
    fn cut_dotted_text_preserves_hidden_utf8_bytes_for_restoration() {
        let (mut ui, screen) = setup();
        let label = Label::create(&mut ui, screen);
        label.set_text(&mut ui, "aé界🙂z");
        // Dots replace the prefix of the multi-byte characters, leaving the
        // rest beyond NUL in the same C allocation.
        set_dots(&mut ui, label.obj(), 1);
        assert_eq!(label.text(&ui), "a...");
        label.cut_text(&mut ui, 0, 1);
        assert_eq!(label.text(&ui), "...");
        label.refresh_text(&mut ui);
        // C lv_text_cut moves only the visible suffix INCLUDING the NUL. Its
        // untouched allocation tail combines with the four saved dot bytes.
        assert_eq!(label.text(&ui), ".é界🙂z");
        ui.obj_update_layout(screen);
        assert_eq!(label.text(&ui), ".é界🙂z");

        label.set_text(&mut ui, "aé界🙂z");
        set_dots(&mut ui, label.obj(), 1);
        label.cut_text(&mut ui, 0, 0);
        label.set_long_mode(&mut ui, LabelLongMode::Wrap);
        ui.obj_update_layout(screen);
        assert_eq!(
            label.text(&ui),
            "aé界🙂z",
            "a zero-length cut preserves the complete hidden suffix"
        );
    }

    #[test]
    fn insertion_into_dotted_text_preserves_utf8_and_original_suffix() {
        let (mut ui, screen) = setup();
        let label = Label::create(&mut ui, screen);
        for (position, inserted, expected) in [
            (0, "🙂", "🙂aé界🙂z"),
            (2, "中", "aé中界🙂z"),
            (LABEL_POS_LAST, "!", "aé界🙂z!"),
            (0, "", "aé界🙂z"),
        ] {
            label.set_text(&mut ui, "aé界🙂z");
            set_dots(&mut ui, label.obj(), 1);
            assert_eq!(label.text(&ui), "a...");
            label.insert_text(&mut ui, position, inserted);
            assert_eq!(label.text(&ui), expected);
            label.refresh_text(&mut ui);
            ui.obj_update_layout(screen);
            assert_eq!(label.text(&ui), expected);
        }

        label.set_text_static(&mut ui, "aé界🙂z");
        set_dots(&mut ui, label.obj(), 1);
        label.insert_text(&mut ui, 0, "🙂");
        assert_eq!(label.text(&ui), "aé界🙂z", "static insertion remains a no-op");
        label.refresh_text(&mut ui);
        label.insert_text(&mut ui, 0, "🙂");
        assert_eq!(label.text(&ui), "🙂aé界🙂z");
    }

    #[test]
    fn max_lines_selection_and_scroll_lifecycle() {
        let (mut ui, screen) = setup();
        let label = Label::create(&mut ui, screen);
        label.set_text(&mut ui, "one\ntwo\nthree\nfour");
        label.set_max_lines(&mut ui, 2);
        ui.obj_set_style_text_line_space(label.obj(), 3, StyleSelector::DEFAULT);
        ui.obj_update_layout(screen);
        assert_eq!(ui.obj_get_height(label.obj()), 35);
        label.set_text_selection_start(&mut ui, 1);
        label.set_text_selection_end(&mut ui, 6);
        assert_eq!(
            (label.text_selection_start(&ui), label.text_selection_end(&ui)),
            (1, 6)
        );
        label.set_text(&mut ui, "This text is much longer than its label");
        label.set_long_mode(&mut ui, LabelLongMode::Scroll);
        ui.obj_set_size(label.obj(), 50, 20);
        ui.obj_update_layout(screen);
        let anim = ui
            .anim_get(AnimTarget::Obj(label.obj()), Some(&offset_exec(false)))
            .unwrap();
        assert_eq!(ui.anim(anim).unwrap().repeat_cnt, ANIM_REPEAT_INFINITE);
        ui.tick_inc(100);
        ui.anim_refr_now();
        assert!(state(&ui, label.obj()).unwrap().offset.x < 0);
        label.set_long_mode(&mut ui, LabelLongMode::Clip);
        assert!(
            ui.anim_get(AnimTarget::Obj(label.obj()), Some(&offset_exec(false)))
                .is_none()
        );
        assert_eq!(state(&ui, label.obj()).unwrap().offset, Point::new(0, 0));
        label.set_long_mode(&mut ui, LabelLongMode::ScrollCircular);
        ui.obj_update_layout(screen);
        assert!(
            ui.anim_get(AnimTarget::Obj(label.obj()), Some(&offset_exec(false)))
                .is_some()
        );
        ui.obj_delete(label.obj());
        assert!(
            ui.anim_get(AnimTarget::Obj(label.obj()), Some(&offset_exec(false)))
                .is_none()
        );
    }
}

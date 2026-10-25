//! DOM `KeyboardEvent` to LVGL key mapping.
//!
//! The browser has no SDL, so this follows upstream `lv_sdl_keyboard.c`,
//! whose `SDL_KEYDOWN` handler maps control keys with
//! `keycode_to_ctrl_key()` and whose `SDL_TEXTINPUT` handler queues the
//! typed UTF-8 text. A DOM `keydown` carries both: `KeyboardEvent.code`
//! names the physical key (for the keypad `+`/`-` remaps) and
//! `KeyboardEvent.key` the produced text.

use ferrovgl_core::group::{
    KEY_BACKSPACE, KEY_DEL, KEY_DOWN, KEY_END, KEY_ENTER, KEY_ESC, KEY_HOME, KEY_LEFT, KEY_NEXT, KEY_PREV,
    KEY_RIGHT, KEY_UP,
};

/// Modifier state of a keyboard event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// `KeyboardEvent.shiftKey`
    pub shift: bool,
    /// `KeyboardEvent.ctrlKey`
    pub ctrl: bool,
    /// `KeyboardEvent.altKey`
    pub alt: bool,
    /// `KeyboardEvent.metaKey`
    pub meta: bool,
}

/// The LVGL keys one DOM `keydown` produces, in delivery order.
///
/// SDL reports `SDL_KEYDOWN` before the matching `SDL_TEXTINPUT`, so
/// [`Self::control`] is sent before [`Self::text`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyMapping {
    /// The `keycode_to_ctrl_key()` result, if the key is a control key.
    pub control: Option<u32>,
    /// The typed character as packed UTF-8 (see [`pack_utf8`]).
    pub text: Option<u32>,
}

impl KeyMapping {
    /// Whether the key produced anything; the page should then call
    /// `preventDefault()` so Tab, arrows and Space do not move the browser.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.control.is_some() || self.text.is_some()
    }

    /// Iterates the keys in delivery order.
    pub fn keys(&self) -> impl Iterator<Item = u32> {
        self.control.into_iter().chain(self.text)
    }
}

/// Packs a character like `sdl_keyboard_read()`: `lv_memcpy(&data->key,
/// buf, utf8_len)` copies the UTF-8 bytes into a zeroed native `uint32_t`.
#[must_use]
pub fn pack_utf8(c: char) -> u32 {
    let mut bytes = [0u8; 4];
    c.encode_utf8(&mut bytes);
    u32::from_ne_bytes(bytes)
}

/// `keycode_to_ctrl_key()` for a DOM key: `key` is `KeyboardEvent.key` and
/// `code` is `KeyboardEvent.code`.
///
/// Upstream sends `LV_KEY_NEXT` for Tab regardless of Shift; like the
/// SDL backend of this workspace, Shift+Tab sends `LV_KEY_PREV` instead.
#[must_use]
pub fn control_key(key: &str, code: &str, mods: Modifiers) -> Option<u32> {
    // SDLK_KP_PLUS / SDLK_KP_MINUS / SDLK_KP_ENTER are physical keypad keys.
    match code {
        "NumpadAdd" => return Some(KEY_RIGHT),
        "NumpadSubtract" => return Some(KEY_LEFT),
        "NumpadEnter" => return Some(KEY_ENTER),
        _ => {}
    }
    Some(match key {
        "ArrowRight" | "Right" => KEY_RIGHT,
        "ArrowLeft" | "Left" => KEY_LEFT,
        "ArrowUp" | "Up" => KEY_UP,
        "ArrowDown" | "Down" => KEY_DOWN,
        "Escape" | "Esc" => KEY_ESC,
        "Backspace" => KEY_BACKSPACE,
        "Delete" | "Del" => KEY_DEL,
        "Enter" => KEY_ENTER,
        "Tab" if mods.shift => KEY_PREV,
        "Tab" | "PageDown" => KEY_NEXT,
        "PageUp" => KEY_PREV,
        "Home" => KEY_HOME,
        "End" => KEY_END,
        _ => return None,
    })
}

/// The `SDL_TEXTINPUT` part: a `key` that is exactly one character and is
/// not a shortcut (Ctrl/Meta without AltGr) produces that character.
#[must_use]
pub fn text_key(key: &str, mods: Modifiers) -> Option<u32> {
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() || c.is_control() {
        return None;
    }
    // AltGr is reported as Ctrl+Alt on Windows layouts and does type text.
    let shortcut = (mods.ctrl && !mods.alt) || mods.meta;
    if shortcut {
        return None;
    }
    Some(pack_utf8(c))
}

/// Maps one DOM `keydown` to LVGL keys. Events with
/// `KeyboardEvent.isComposing` (IME composition) should not be passed in.
#[must_use]
pub fn map_key(key: &str, code: &str, mods: Modifiers) -> KeyMapping {
    let control = control_key(key, code, mods);
    // SDL emits no text input for Enter, Tab, Escape or Backspace, but the
    // keypad +/- produce both the remapped arrow and the character.
    let text = text_key(key, mods);
    KeyMapping { control, text }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };

    #[test]
    fn control_keys() {
        for (key, want) in [
            ("ArrowRight", KEY_RIGHT),
            ("ArrowLeft", KEY_LEFT),
            ("ArrowUp", KEY_UP),
            ("ArrowDown", KEY_DOWN),
            ("Enter", KEY_ENTER),
            ("Escape", KEY_ESC),
            ("Backspace", KEY_BACKSPACE),
            ("Delete", KEY_DEL),
            ("Tab", KEY_NEXT),
            ("PageDown", KEY_NEXT),
            ("PageUp", KEY_PREV),
            ("Home", KEY_HOME),
            ("End", KEY_END),
        ] {
            let m = map_key(key, "", NONE);
            assert_eq!(
                m,
                KeyMapping {
                    control: Some(want),
                    text: None
                },
                "{key}"
            );
            assert!(m.is_handled());
        }
        let shift = Modifiers { shift: true, ..NONE };
        assert_eq!(map_key("Tab", "Tab", shift).control, Some(KEY_PREV));
        assert_eq!(map_key("Enter", "NumpadEnter", NONE).control, Some(KEY_ENTER));
        assert!(!map_key("Shift", "ShiftLeft", shift).is_handled());
        assert!(!map_key("F5", "F5", NONE).is_handled());
        assert!(!map_key("Dead", "Quote", NONE).is_handled());
    }

    #[test]
    fn keypad_plus_minus_send_arrow_then_text() {
        let m = map_key("+", "NumpadAdd", NONE);
        assert_eq!(m.keys().collect::<Vec<_>>(), [KEY_RIGHT, u32::from(b'+')]);
        let m = map_key("-", "NumpadSubtract", NONE);
        assert_eq!(m.keys().collect::<Vec<_>>(), [KEY_LEFT, u32::from(b'-')]);
        // The main-row '+' is just text.
        assert_eq!(
            map_key("+", "Equal", NONE).keys().collect::<Vec<_>>(),
            [u32::from(b'+')]
        );
    }

    #[test]
    fn printable_characters_pack_utf8() {
        assert_eq!(map_key("a", "KeyA", NONE).text, Some(u32::from(b'a')));
        assert_eq!(map_key(" ", "Space", NONE).text, Some(0x20));
        let shift = Modifiers { shift: true, ..NONE };
        assert_eq!(map_key("A", "KeyA", shift).text, Some(u32::from(b'A')));
        // 'é' is C3 A9 in UTF-8, packed into the low bytes in memory order.
        assert_eq!(pack_utf8('é'), u32::from_ne_bytes([0xC3, 0xA9, 0, 0]));
        assert_eq!(pack_utf8('€'), u32::from_ne_bytes([0xE2, 0x82, 0xAC, 0]));
        assert_eq!(pack_utf8('😀'), u32::from_ne_bytes([0xF0, 0x9F, 0x98, 0x80]));
        assert_eq!(map_key("€", "KeyE", NONE).text, Some(pack_utf8('€')));
    }

    #[test]
    fn shortcuts_do_not_type() {
        let ctrl = Modifiers { ctrl: true, ..NONE };
        let meta = Modifiers { meta: true, ..NONE };
        let altgr = Modifiers {
            ctrl: true,
            alt: true,
            ..NONE
        };
        assert_eq!(map_key("c", "KeyC", ctrl).text, None);
        assert_eq!(map_key("v", "KeyV", meta).text, None);
        assert_eq!(map_key("@", "KeyQ", altgr).text, Some(u32::from(b'@')));
        // Control keys still work with modifiers.
        assert_eq!(map_key("ArrowUp", "ArrowUp", ctrl).control, Some(KEY_UP));
    }
}

//! Shared helpers of the example tests.

#![allow(dead_code)]

use ferrovgl_core::{Obj, Ui};
use ferrovgl_examples::Example;
use ferrovgl_test::TestDisplay;

/// Frames rendered after an example is built, and their period in ms
/// (`LV_DEF_REFR_PERIOD`).
pub const FRAMES: u32 = 10;
/// Period of one frame.
pub const FRAME_MS: u32 = 33;

/// Number of objects in the tree under `obj`, `obj` excluded.
pub fn descendants(ui: &Ui, obj: Obj) -> u32 {
    let mut count = 0;
    for i in 0..ui.obj_get_child_count(obj) as i32 {
        if let Some(child) = ui.obj_get_child(obj, i) {
            count += 1 + descendants(ui, child);
        }
    }
    count
}

/// Build `example` on a fresh upstream-configured display with the default
/// theme, like `tests/main.py` builds the examples.
pub fn build(example: &Example) -> TestDisplay {
    let mut display = TestDisplay::with_default_theme().expect("test display");
    (example.run)(&mut display.ui);
    display
}

/// Render [`FRAMES`] frames of [`FRAME_MS`] each.
pub fn render_frames(display: &mut TestDisplay) {
    for _ in 0..FRAMES {
        display.wait(FRAME_MS);
    }
}

/// 64-bit FNV-1a hash of the visible RGB bytes of the current frame.
pub fn frame_hash(display: &mut TestDisplay) -> u64 {
    let image = display.snapshot().expect("snapshot");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(&image.width().to_le_bytes());
    feed(&image.height().to_le_bytes());
    feed(image.pixels());
    hash
}

/// Class names and texts of the objects under a root, in creation (depth
/// first) order: the "key properties" the smoke tests compare.
#[derive(Debug, Default)]
pub struct Tree {
    /// `(class name, count)` of every descendant.
    pub classes: std::collections::BTreeMap<&'static str, u32>,
    /// Texts of the labels, checkboxes (`cb:` prefix) and text areas (`ta:`
    /// prefix), in depth-first order.
    pub texts: Vec<String>,
}

impl Tree {
    /// Collect the descendants of `root`.
    pub fn of(ui: &Ui, root: Obj) -> Self {
        let mut tree = Self::default();
        tree.walk(ui, root);
        tree
    }

    fn walk(&mut self, ui: &Ui, obj: Obj) {
        use ferrovgl_widgets::{Checkbox, Label, Textarea};
        for i in 0..ui.obj_get_child_count(obj) as i32 {
            let Some(child) = ui.obj_get_child(obj, i) else {
                continue;
            };
            *self.classes.entry(ui.obj_get_class(child).name).or_default() += 1;
            if let Some(label) = Label::from_obj(ui, child) {
                self.texts.push(label.text(ui).to_owned());
            }
            if let Some(cb) = Checkbox::from_obj(ui, child) {
                self.texts.push(format!("cb:{}", cb.text(ui)));
            }
            if let Some(ta) = Textarea::from_obj(ui, child) {
                self.texts.push(format!("ta:{}", ta.text(ui)));
            }
            self.walk(ui, child);
        }
    }

    /// The tree of the active screen of `display`.
    pub fn of_screen(display: &TestDisplay) -> Self {
        Self::of(&display.ui, display.screen().expect("active screen"))
    }

    /// Whether some label / checkbox / text area shows `text`.
    pub fn has_text(&self, text: &str) -> bool {
        self.texts.iter().any(|t| t == text)
    }
}

/// The first descendant of `root` (depth first) that is a label showing
/// `text`, or a checkbox / text area whose text is `text`.
pub fn find_text(ui: &Ui, root: Obj, text: &str) -> Option<Obj> {
    use ferrovgl_widgets::{Checkbox, Label};
    for i in 0..ui.obj_get_child_count(root) as i32 {
        let child = ui.obj_get_child(root, i)?;
        let matches = Label::from_obj(ui, child).is_some_and(|l| l.text(ui) == text)
            || Checkbox::from_obj(ui, child).is_some_and(|c| c.text(ui) == text);
        if matches {
            return Some(child);
        }
        if let Some(found) = find_text(ui, child, text) {
            return Some(found);
        }
    }
    None
}

/// The first descendant of `root` (depth first) of the class named `class`.
pub fn find_class(ui: &Ui, root: Obj, class: &str) -> Option<Obj> {
    for i in 0..ui.obj_get_child_count(root) as i32 {
        let child = ui.obj_get_child(root, i)?;
        if ui.obj_get_class(child).name == class {
            return Some(child);
        }
        if let Some(found) = find_class(ui, child, class) {
            return Some(found);
        }
    }
    None
}

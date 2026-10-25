//! Observer bindings of the widgets (`LV_USE_OBSERVER`).
//!
//! Ported from LVGL v9.6.0: the `#if LV_USE_OBSERVER` parts of
//! `src/widgets/arc/lv_arc.c`, `src/widgets/bar/lv_bar.c`,
//! `src/widgets/image/lv_image.c`, `src/widgets/label/lv_label.c` and
//! `src/widgets/slider/lv_slider.c` (`lv_<widget>_bind_*()` and their static
//! observer / event callbacks).
//!
//! They are gathered here, next to the core subject/observer API
//! (`ferrovgl_core::core::observer`), so that the observer port does not edit
//! the widget modules; each binding is an `impl` block of its widget handle.
//!
//! SKELETON: the public API is declared; the bodies are `todo!()` until the
//! observer module is ported.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use ferrovgl_core::Ui;
#[allow(unused_imports)]
use ferrovgl_core::core::observer::{ObserverId, SubjectId};

#[cfg(feature = "arc")]
impl crate::arc::Arc {
    /// `lv_arc_bind_value()`: bind an integer subject to the arc's value.
    pub fn bind_value(self, ui: &mut Ui, subject: SubjectId) -> Option<ObserverId> {
        todo!()
    }
}

#[cfg(feature = "bar")]
impl crate::bar::Bar {
    /// `lv_bar_bind_value()`: bind an integer or float subject to the bar's
    /// value.
    pub fn bind_value(self, ui: &mut Ui, subject: SubjectId) -> Option<ObserverId> {
        crate::bar::bar_bind_value(ui, self.obj(), subject)
    }
}

#[cfg(feature = "slider")]
impl crate::slider::Slider {
    /// `lv_slider_bind_value()`: bind an integer or float subject to the
    /// slider's value.
    pub fn bind_value(self, ui: &mut Ui, subject: SubjectId) -> Option<ObserverId> {
        crate::slider::slider_bind_value(ui, self.obj(), subject)
    }
}

#[cfg(feature = "label")]
impl crate::label::Label {
    /// `lv_label_bind_text()`: bind an integer, string or pointer subject to
    /// the label's text. `fmt` is an optional printf-like format with one
    /// specifier (e.g. `"%d °C"`); with `None` strings are shown as they
    /// are, integers as `%d` and floats as `%0.1f`.
    pub fn bind_text(self, ui: &mut Ui, subject: SubjectId, fmt: Option<&'static str>) -> Option<ObserverId> {
        todo!()
    }
}

#[cfg(feature = "image")]
impl crate::image::Image {
    /// `lv_image_bind_src()`: bind a pointer subject to the image's source.
    pub fn bind_src(self, ui: &mut Ui, subject: SubjectId) -> Option<ObserverId> {
        todo!()
    }
}

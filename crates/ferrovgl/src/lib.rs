//! ferrovgl: a pure-Rust reimplementation of LVGL v9.6.0.
//!
//! This umbrella crate re-exports the workspace crates behind feature flags.
//! Start with [`prelude`].
//!
//! ferrovgl is not affiliated with LVGL LLC. LVGL is a trademark of LVGL LLC.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub use ferrovgl_core as core;
#[cfg(feature = "draw-sw")]
#[cfg_attr(docsrs, doc(cfg(feature = "draw-sw")))]
pub use ferrovgl_draw_sw as draw_sw;
#[cfg(feature = "embedded")]
#[cfg_attr(docsrs, doc(cfg(feature = "embedded")))]
pub use ferrovgl_embedded as embedded;
#[cfg(feature = "fonts")]
#[cfg_attr(docsrs, doc(cfg(feature = "fonts")))]
pub use ferrovgl_fonts as fonts;
#[cfg(feature = "sdl")]
#[cfg_attr(docsrs, doc(cfg(feature = "sdl")))]
pub use ferrovgl_sdl as sdl;
#[cfg(feature = "widgets")]
#[cfg_attr(docsrs, doc(cfg(feature = "widgets")))]
pub use ferrovgl_widgets as widgets;

/// What a typical application needs, for `use ferrovgl::prelude::*;`.
///
/// The context and handles ([`Ui`](ferrovgl_core::Ui), [`Obj`](ferrovgl_core::Obj),
/// the `*Id` handles), events, styles and selectors, the geometry and colour
/// values, the layout and scroll enums used with the style setters, the
/// animation description types, the coordinate helpers (`pct`,
/// `SIZE_CONTENT`, `grid_fr`, ...) and, with the `widgets` feature, every
/// widget handle and its mode enums. Renderer, font and backend setup
/// (`ferrovgl::draw_sw::init`, `ferrovgl::fonts::MONTSERRAT_14`,
/// `ferrovgl::sdl::SdlDisplay`) stay behind their module paths.
///
/// ```
/// use ferrovgl::prelude::*;
///
/// let mut ui = Ui::new(UiConfig::default());
/// let _display: DisplayId = ui.display_create(320, 240);
/// let screen: Obj = ui.screen_active().expect("a display has a screen");
/// let panel = ui.obj_create(Some(screen));
/// ui.obj_set_size(panel, pct(50), SIZE_CONTENT);
/// ui.obj_set_flex_flow(panel, FlexFlow::Column);
/// ui.obj_set_style_bg_color(panel, Color::hex(0x1565c0), StyleSelector::DEFAULT);
/// assert_eq!(ui.obj_get_parent(panel), Some(screen));
/// ```
pub mod prelude {
    pub use ferrovgl_core::core::obj::{ScrollSnap, ScrollbarMode};
    pub use ferrovgl_core::core::obj_style::StyleId;
    pub use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
    pub use ferrovgl_core::layouts::grid::{GRID_CONTENT, GRID_TEMPLATE_LAST, GridAlign, grid_fr};
    pub use ferrovgl_core::misc::anim::{ANIM_OFF, ANIM_ON, ANIM_REPEAT_INFINITE, AnimExec, AnimTarget};
    pub use ferrovgl_core::misc::color::{OPA_COVER, OPA_TRANSP};
    pub use ferrovgl_core::misc::palette::Palette;
    pub use ferrovgl_core::misc::style::ImageSrc;
    pub use ferrovgl_core::misc::text::TextAlign;
    pub use ferrovgl_core::misc::types::coord::{SIZE_CONTENT, pct};
    pub use ferrovgl_core::{
        Align, Anim, AnimId, AnimPath, Area, Color, Color32, ColorFormat, Dir, DisplayId, Event, EventCode,
        EventParam, FontRef, GroupId, IndevId, LvResult, Obj, ObjFlags, Opa, Part, Point, RenderMode, State,
        Style, StyleProp, StyleSelector, StyleValue, TimerId, Ui, UiConfig, coord,
    };
    #[cfg(feature = "widgets")]
    pub use ferrovgl_widgets::{
        Arc, ArcMode, Bar, BarMode, BarOrientation, Button, Buttonmatrix, ButtonmatrixCtrl, Checkbox, Image,
        ImageAlign, Label, LabelLongMode, Led, Line, Slider, SliderMode, SliderOrientation, Spinner, Switch,
        SwitchOrientation, Textarea,
    };
}

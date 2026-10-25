//! Ported from LVGL v9.6.0: `tests/src/test_cases/draw/test_render_to_*.c`
//! (`al88`, `argb8888`, `argb8888_premultiplied`, `l8`, `rgb565`,
//! `rgb565_swapped`, `rgb888`, `xrgb8888`) and the scene builder they call,
//! `demos/render/lv_demo_render.c` (`lv_demo_render()`).
//!
//! Every test file sets the display colour format and renders each scene of
//! `lv_demo_render.c` at `opa_values = {0xff, 0x80}`, comparing
//! `draw/render/<format>/demo_render_<scene>_opa_<opa>.png`. The parity
//! configuration has `LV_BIN_DECODER_RAM_LOAD == 0`, so upstream does not
//! compare `LV_DEMO_RENDER_SCENE_IMAGE_NORMAL_3` / `IMAGE_RECOLOR_3`, and
//! `LV_USE_DRAW_SW_COMPLEX_GRADIENTS == 1`, so the three gradient scenes exist.
//! Seven files skip those two scenes before calling `lv_demo_render()`;
//! `test_render_to_l8.c` calls `lv_demo_render()` first and skips only the
//! screenshot. No frame is rendered between that build and the next scene's
//! `lv_obj_clean()`, so no compared image depends on it and the `_3` scenes
//! are not built here for any format.
//!
//! Each `#[test]` here is one (format, scene) pair rendered at both opacities
//! on a fresh default-theme display. Upstream renders all scenes of a format on
//! one display; `lv_demo_render()` starts every scene with
//! `lv_obj_clean(lv_screen_active())` and `lv_obj_remove_style_all()` on the
//! screen, so no widget or style state crosses scenes. The canvas of the
//! `blend_mode` scene is a child of the screen and is deleted by that clean.
//!
//! The canvas widget is not ported. `blend_mode_cb()` uses a canvas only to
//! draw four small image buffers (`lv_canvas_fill_bg()`,
//! `lv_canvas_init_layer()`, `lv_draw_label()`, `lv_canvas_finish_layer()`) and
//! to show the last one at the screen's top-left corner. Those steps are
//! reproduced here on a `DrawBuf` with the same header, and the canvas object
//! is an Image widget showing that buffer (`lv_canvas_t` is an `lv_image_t`
//! whose source is its draw buffer).
//!
//! Image assets are byte-exact extractions of the C arrays in
//! `demos/render/assets/` at LVGL v9.6.0; see the comments on each static.

use std::rc::Rc;

use ferrovgl_core::draw::DrawBuf;
use ferrovgl_core::draw::draw_buf::{ImageDsc, ImageHeader};
use ferrovgl_core::draw::grad::{GradDir, GradDsc, GradExtend, GradParams};
use ferrovgl_core::draw::label::LabelDsc;
use ferrovgl_core::draw::triangle::TriangleDsc;
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign};
use ferrovgl_core::misc::area::PointPrecise;
use ferrovgl_core::misc::style::{BlendMode, BorderSide, ImageSrc, SharedRef, TracksRef};
use ferrovgl_core::misc::text::TextDecor;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Area, Color, ColorFormat, EventCode, FontRef, Obj, Part, Point, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Arc, Image, Label, Line};

const SEL: StyleSelector = StyleSelector::DEFAULT;

const COL_CNT: i32 = 8;
const DEF_WIDTH: i32 = 55;
const DEF_HEIGHT: i32 = 30;

/// `LV_OPA_20`
const OPA_20: u8 = 51;
/// `LV_OPA_50`
const OPA_50: u8 = 127;

/// `lv_demo_render_scene_t` entries ported here, in `scenes[]` order.
#[derive(Clone, Copy, Debug)]
enum Scene {
    Fill,
    Border,
    BoxShadow,
    Text,
    ImageNormal1,
    ImageRecolor1,
    ImageNormal2,
    ImageRecolor2,
    Line,
    ArcNormal,
    ArcImage,
    Triangle,
    LayerNormal,
    BlendMode,
    LinearGradient,
    RadialGradient,
    ConicalGradient,
}

impl Scene {
    /// `scenes[id].name`
    fn name(self) -> &'static str {
        match self {
            Scene::Fill => "fill",
            Scene::Border => "border",
            Scene::BoxShadow => "box_shadow",
            Scene::Text => "text",
            Scene::ImageNormal1 => "image_normal_1",
            Scene::ImageRecolor1 => "image_recolor_1",
            Scene::ImageNormal2 => "image_normal_2",
            Scene::ImageRecolor2 => "image_recolor_2",
            Scene::Line => "line",
            Scene::ArcNormal => "arc_normal",
            Scene::ArcImage => "arc_image",
            Scene::Triangle => "triangle",
            Scene::LayerNormal => "layer_normal",
            Scene::BlendMode => "blend_mode",
            Scene::LinearGradient => "linear_gradient",
            Scene::RadialGradient => "radial_gradient",
            Scene::ConicalGradient => "conical_gradient",
        }
    }
}

/// The scene builder state: the UI and `opa_saved`.
struct Ctx<'a> {
    ui: &'a mut Ui,
    opa: u8,
}

/// `add_to_cell()`
fn add_to_cell(ui: &mut Ui, obj: Obj, col: i32, row: i32) {
    ui.obj_set_grid_cell(obj, GridAlign::Center, col, 1, GridAlign::Center, row, 1);
}

/* ---------------------------------------------------------------- fill */

fn fill_obj_create(c: &mut Ctx<'_>, parent: Obj, col: i32, row: i32) -> Obj {
    let colors = [
        Color::hex3(0x000),
        Color::hex3(0xfff),
        Color::hex3(0xf00),
        Color::hex3(0x0f0),
        Color::hex3(0x00f),
        Color::hex3(0xff0),
        Color::hex3(0x0ff),
        Color::hex3(0xf0f),
    ];
    let ui = &mut *c.ui;
    let obj = ui.obj_create(Some(parent));
    ui.obj_remove_style_all(obj);
    ui.obj_set_style_opa(obj, c.opa, SEL);
    ui.obj_set_style_bg_opa(obj, 255, SEL);
    ui.obj_set_style_bg_color(obj, colors[col as usize], SEL);
    ui.obj_set_size(obj, DEF_WIDTH, DEF_HEIGHT);
    add_to_cell(ui, obj, col, row);
    obj
}

fn fill_cb(c: &mut Ctx<'_>, parent: Obj) {
    for i in 0..COL_CNT {
        fill_obj_create(c, parent, i, 0);
    }
    for i in 0..COL_CNT {
        let obj = fill_obj_create(c, parent, i, 1);
        c.ui.obj_set_style_radius(obj, 10, SEL);
    }
    for i in 0..COL_CNT {
        let obj = fill_obj_create(c, parent, i, 2);
        c.ui.obj_set_style_radius(obj, 100, SEL);
    }
    for (row, dir, transp) in [
        (3, GradDir::Hor, false),
        (4, GradDir::Ver, false),
        (5, GradDir::Hor, true),
        (6, GradDir::Ver, true),
    ] {
        for i in 0..COL_CNT {
            let obj = fill_obj_create(c, parent, i, row);
            let ui = &mut *c.ui;
            ui.obj_set_style_radius(obj, 10, SEL);
            ui.obj_set_style_bg_grad_dir(obj, dir, SEL);
            ui.obj_set_style_bg_grad_color(obj, Color::hex3(0x888), SEL);
            if transp {
                ui.obj_set_style_bg_grad_opa(obj, 0, SEL);
            }
            ui.obj_set_style_bg_grad_stop(obj, 200, SEL);
        }
    }
}

/* -------------------------------------------------------------- border */

fn border_obj_create(c: &mut Ctx<'_>, parent: Obj, col: i32, row: i32) -> Obj {
    let ui = &mut *c.ui;
    let obj = ui.obj_create(Some(parent));
    ui.obj_remove_style_all(obj);
    ui.obj_set_style_border_color(obj, Color::hex3(0x000), SEL);
    ui.obj_set_style_border_width(obj, 3, SEL);
    ui.obj_set_style_opa(obj, c.opa, SEL);
    ui.obj_set_size(obj, DEF_WIDTH, DEF_HEIGHT);
    add_to_cell(ui, obj, col, row);
    obj
}

fn border_cb(c: &mut Ctx<'_>, parent: Obj) {
    let sides = [
        BorderSide::NONE,
        BorderSide::FULL,
        BorderSide::LEFT,
        BorderSide::RIGHT,
        BorderSide::TOP,
        BorderSide::BOTTOM,
        BorderSide::TOP | BorderSide::RIGHT,
        BorderSide::RIGHT | BorderSide::BOTTOM,
        BorderSide::LEFT | BorderSide::BOTTOM,
        BorderSide::LEFT | BorderSide::TOP,
        BorderSide::LEFT | BorderSide::RIGHT,
        BorderSide::TOP | BorderSide::BOTTOM,
        BorderSide::LEFT | BorderSide::RIGHT | BorderSide::BOTTOM,
        BorderSide::LEFT | BorderSide::BOTTOM | BorderSide::TOP,
        BorderSide::LEFT | BorderSide::RIGHT | BorderSide::TOP,
        BorderSide::BOTTOM | BorderSide::RIGHT | BorderSide::TOP,
    ];
    // (row, radius, side offset, colour, border width override)
    for (row, radius, ofs, color, width) in [
        (0, 0, 0, 0xf00, None),
        (1, 0, 8, 0xf00, None),
        (2, 10, 0, 0x0f0, None),
        (3, 10, 8, 0x0f0, None),
        (4, 100, 0, 0x00f, None),
        (5, 100, 8, 0x00f, None),
        (6, 100, 0, 0x888, Some(10)),
        (7, 100, 8, 0x888, Some(10)),
    ] {
        for i in 0..COL_CNT {
            let obj = border_obj_create(c, parent, i, row);
            let ui = &mut *c.ui;
            ui.obj_set_style_radius(obj, radius, SEL);
            ui.obj_set_style_border_side(obj, sides[(i + ofs) as usize], SEL);
            ui.obj_set_style_border_color(obj, Color::hex3(color), SEL);
            if let Some(w) = width {
                ui.obj_set_style_border_width(obj, w, SEL);
            }
        }
    }
}

/* ---------------------------------------------------------- box shadow */

fn box_shadow_obj_create(c: &mut Ctx<'_>, parent: Obj, col: i32, row: i32) -> Obj {
    let ui = &mut *c.ui;
    let obj = ui.obj_create(Some(parent));
    ui.obj_remove_style_all(obj);
    ui.obj_set_style_bg_opa(obj, OPA_20, SEL);
    ui.obj_set_style_bg_color(obj, Color::BLACK, SEL);
    ui.obj_set_style_shadow_color(obj, Color::hex3(0xf00), SEL);
    ui.obj_set_style_opa(obj, c.opa, SEL);
    ui.obj_set_size(obj, DEF_WIDTH - 20, DEF_HEIGHT - 5);
    add_to_cell(ui, obj, col, row);
    obj
}

fn box_shadow_cb(c: &mut Ctx<'_>, parent: Obj) {
    c.ui.obj_set_grid_dsc_array(
        parent,
        TracksRef::Static(&[68, 68, 68, 68, 68, 68, 68, GRID_TEMPLATE_LAST]),
        TracksRef::Static(&[45, 45, 45, 45, 45, 45, GRID_TEMPLATE_LAST]),
    );
    let ofs = [(0, 0), (5, 5), (5, -5), (-5, 5), (-5, -5), (10, 0), (0, 10)];
    for (row, radius, spread) in [(0, 0, None), (1, 5, None), (2, 100, None), (3, 5, Some(3))] {
        for (i, (x, y)) in ofs.into_iter().enumerate() {
            let obj = box_shadow_obj_create(c, parent, i as i32, row);
            let ui = &mut *c.ui;
            ui.obj_set_style_radius(obj, radius, SEL);
            ui.obj_set_style_shadow_width(obj, 10, SEL);
            if let Some(s) = spread {
                ui.obj_set_style_shadow_spread(obj, s, SEL);
            }
            ui.obj_set_style_shadow_offset_x(obj, x, SEL);
            ui.obj_set_style_shadow_offset_y(obj, y, SEL);
        }
    }
}

/* ---------------------------------------------------------------- text */

fn text_obj_create(c: &mut Ctx<'_>, parent: Obj, col: i32, row: i32) -> Label {
    let ui = &mut *c.ui;
    let label = Label::create(ui, parent);
    let obj = label.obj();
    ui.obj_remove_style_all(obj);
    label.set_text(ui, "Hello LVGL! It should be a placeholder: ű. Looks good?");
    ui.obj_set_style_opa(obj, c.opa, SEL);
    add_to_cell(ui, obj, col, row);
    label
}

fn text_cb(c: &mut Ctx<'_>, parent: Obj) {
    text_obj_create(c, parent, 3, 0);

    let obj = text_obj_create(c, parent, 3, 1);
    c.ui.obj_set_style_text_color(obj.obj(), Color::hex3(0xff0), SEL);

    let obj = text_obj_create(c, parent, 3, 2);
    obj.set_text_selection_start(c.ui, 12);
    obj.set_text_selection_end(c.ui, 21);
    c.ui.obj_set_style_bg_color(obj.obj(), Color::hex3(0x0ff), Part::SELECTED.into());

    let obj = text_obj_create(c, parent, 3, 3);
    c.ui.obj_set_style_text_decor(obj.obj(), TextDecor::UNDERLINE, SEL);

    let obj = text_obj_create(c, parent, 3, 4);
    c.ui.obj_set_style_text_decor(obj.obj(), TextDecor::STRIKETHROUGH, SEL);

    let obj = text_obj_create(c, parent, 3, 5);
    c.ui.obj_set_style_text_decor(obj.obj(), TextDecor::UNDERLINE | TextDecor::STRIKETHROUGH, SEL);
}

/* --------------------------------------------------------------- image */

// Byte-exact extractions of the `img_render_lvgl_logo_*_map` arrays of LVGL
// v9.6.0 `demos/render/assets/img_render_lvgl_logo_<format>.c` (every
// hexadecimal byte of the initialiser, in order). The headers repeat the C
// `lv_image_dsc_t` initialisers (all with `LV_IMAGE_HEADER_MAGIC`).

/// `img_render_lvgl_logo_argb8888` (3600 bytes, SHA-256
/// 013d00793d2833295875e435008580874c644f9a228deb13669ed373f4178581).
static LOGO_ARGB8888: ImageDsc = ImageDsc {
    header: ImageHeader::new(ColorFormat::Argb8888, 30, 30, 120),
    data_size: 3600,
    data: include_bytes!("assets/render-to-formats/lvgl_logo.argb8888"),
};

/// `img_render_lvgl_logo_argb8888_premultiplied` (3600 bytes, SHA-256
/// 89b0461bc99ee7e54051b11761bce690ce3e94bc4504de4367fda42dadffde01).
static LOGO_ARGB8888_PREMULTIPLIED: ImageDsc = ImageDsc {
    header: ImageHeader::new(ColorFormat::Argb8888Premultiplied, 30, 30, 120),
    data_size: 3600,
    data: include_bytes!("assets/render-to-formats/lvgl_logo.argb8888_premultiplied"),
};

/// `img_render_lvgl_logo_xrgb8888` (3600 bytes, SHA-256
/// e161144cc0b48080f62599a7589d7da98c658983f3396fe20dd59353eaca8ed2).
static LOGO_XRGB8888: ImageDsc = ImageDsc {
    header: ImageHeader::new(ColorFormat::Xrgb8888, 30, 30, 120),
    data_size: 3600,
    data: include_bytes!("assets/render-to-formats/lvgl_logo.xrgb8888"),
};

/// `img_render_lvgl_logo_rgb888` (2700 bytes, SHA-256
/// 644ac1bcab46bacb2ff8688b469cb237d15106e15e88a592013babcd97a01446).
static LOGO_RGB888: ImageDsc = ImageDsc {
    header: ImageHeader::new(ColorFormat::Rgb888, 30, 30, 90),
    data_size: 2700,
    data: include_bytes!("assets/render-to-formats/lvgl_logo.rgb888"),
};

/// `img_render_lvgl_logo_rgb565` (1800 bytes, SHA-256
/// b9f57a43e4083680ca990b31a978c25a6c9b69a26ca5501c3bb61b646eebbad0).
static LOGO_RGB565: ImageDsc = ImageDsc {
    header: ImageHeader::new(ColorFormat::Rgb565, 30, 30, 60),
    data_size: 1800,
    data: include_bytes!("assets/render-to-formats/lvgl_logo.rgb565"),
};

/// `img_render_lvgl_logo_rgb565_swapped` (1800 bytes, SHA-256
/// d800d516c31b2e1f116d2c076b4e238f4082dbb5f20d8e8929ebf0f307fb2510).
static LOGO_RGB565_SWAPPED: ImageDsc = ImageDsc {
    header: ImageHeader::new(ColorFormat::Rgb565Swapped, 30, 30, 60),
    data_size: 1800,
    data: include_bytes!("assets/render-to-formats/lvgl_logo.rgb565_swapped"),
};

/// `img_render_lvgl_logo_l8` (900 bytes, SHA-256
/// d74019d36f87d9520f533cb5f716b6e646e2e2ba633485752e61adf8a91e334f).
static LOGO_L8: ImageDsc = ImageDsc {
    header: ImageHeader::new(ColorFormat::L8, 30, 30, 30),
    data_size: 900,
    data: include_bytes!("assets/render-to-formats/lvgl_logo.l8"),
};

fn image_obj_create(c: &mut Ctx<'_>, parent: Obj, col: i32, row: i32, recolor: bool) -> Image {
    let ui = &mut *c.ui;
    let image = Image::create(ui, parent);
    let obj = image.obj();
    ui.obj_remove_style_all(obj);
    if recolor {
        ui.obj_set_style_image_recolor_opa(obj, OPA_50, SEL);
        ui.obj_set_style_image_recolor(obj, Color::hex3(0x0f0), SEL);
    }
    ui.obj_set_style_opa(obj, c.opa, SEL);
    add_to_cell(ui, obj, col, row);
    image
}

fn image_core_cb(c: &mut Ctx<'_>, parent: Obj, recolor: bool, start_at: usize) {
    // `srcs[]` / `names[]` up to index 7; indices 8 and 9 (RGB565A8, I1) are
    // only used by the `_3` scenes, which the parity configuration skips.
    let srcs = [
        ImageSrc::Static(&LOGO_ARGB8888),
        ImageSrc::Static(&LOGO_ARGB8888_PREMULTIPLIED),
        ImageSrc::Static(&LOGO_XRGB8888),
        ImageSrc::Static(&LOGO_RGB888),
        ImageSrc::Static(&LOGO_RGB565),
        ImageSrc::Static(&LOGO_RGB565_SWAPPED),
        ImageSrc::Static(&LOGO_L8),
        ImageSrc::None,
    ];
    let names = [
        "ARGB\n8888",
        "ARGB\n8888\nPM",
        "XRGB\n8888",
        "RGB\n888",
        "RGB\n565",
        "RGB\n565\nSWAP",
        "L8",
        "",
    ];
    /* `sizeof(srcs) / sizeof(void *)` of the full upstream array is 10. */
    let stop_at = start_at + (10 - start_at).min(4);
    for i in start_at..stop_at {
        let row = (i - start_at) as i32;
        let label = Label::create(c.ui, parent);
        label.set_text(c.ui, names[i]);
        add_to_cell(c.ui, label.obj(), 0, row * 2);

        let src = &srcs[i];
        /* Column 1 only sets the source and keeps the default pivot. */
        let image = image_obj_create(c, parent, 1, row * 2, recolor);
        image.set_src(c.ui, src.clone());

        for (col, rotation, scale, scale_x, scale_y) in [
            (2, Some(300), None, None, None),
            (3, None, Some(400), None, None),
            (4, None, None, Some(400), None),
            (5, None, None, None, Some(400)),
            (6, Some(300), Some(400), None, None),
        ] {
            let image = image_obj_create(c, parent, col, row * 2, recolor);
            image.set_src(c.ui, src.clone());
            if let Some(r) = rotation {
                image.set_rotation(c.ui, r);
            }
            if let Some(s) = scale {
                image.set_scale(c.ui, s);
            }
            if let Some(s) = scale_x {
                image.set_scale_x(c.ui, s);
            }
            if let Some(s) = scale_y {
                image.set_scale_y(c.ui, s);
            }
            image.set_pivot(c.ui, 0, 0);
        }

        /* Column 7 sets the vertical scale before the rotation. */
        let image = image_obj_create(c, parent, 7, row * 2, recolor);
        image.set_src(c.ui, src.clone());
        image.set_scale_y(c.ui, 400);
        image.set_rotation(c.ui, 300);
        image.set_pivot(c.ui, 0, 0);
    }
}

/* ---------------------------------------------------------------- line */

const fn pp(x: i32, y: i32) -> PointPrecise {
    PointPrecise::new(x as _, y as _)
}

/// `line_cb()`'s `static lv_point_precise_t points[][2]`.
static LINE_POINTS: [[PointPrecise; 2]; 8] = [
    [pp(5, DEF_HEIGHT / 2), pp(DEF_WIDTH - 5, DEF_HEIGHT / 2)],
    [pp(5, DEF_HEIGHT / 2), pp(DEF_WIDTH - 5, DEF_HEIGHT / 2 + 1)],
    [pp(5, DEF_HEIGHT / 2), pp(DEF_WIDTH - 5, DEF_HEIGHT / 2 - 1)],
    [pp(DEF_WIDTH / 2, 5), pp(DEF_WIDTH / 2, DEF_HEIGHT - 5)],
    [pp(DEF_WIDTH / 2, 5), pp(DEF_WIDTH / 2 + 1, DEF_HEIGHT - 5)],
    [pp(DEF_WIDTH / 2, 5), pp(DEF_WIDTH / 2 - 1, DEF_HEIGHT - 5)],
    [pp(5, 5), pp(DEF_WIDTH - 5, DEF_HEIGHT - 5)],
    [pp(DEF_WIDTH - 5, 5), pp(5, DEF_HEIGHT - 5)],
];

fn line_obj_create(c: &mut Ctx<'_>, parent: Obj, col: i32, row: i32, p: &'static [PointPrecise]) -> Obj {
    let ui = &mut *c.ui;
    let line = Line::create(ui, parent);
    let obj = line.obj();
    ui.obj_remove_style_all(obj);
    line.set_points(ui, Some(SharedRef::Static(p)), 2);
    ui.obj_set_size(obj, DEF_WIDTH, DEF_HEIGHT);
    ui.obj_set_style_line_color(obj, Color::hex3(0xff0), SEL);
    ui.obj_set_style_opa(obj, c.opa, SEL);
    add_to_cell(ui, obj, col, row);
    obj
}

fn line_cb(c: &mut Ctx<'_>, parent: Obj) {
    let widths = [1, 3, 5, 10];
    for r in 0..2 {
        for (w, width) in widths.into_iter().enumerate() {
            for i in 0..COL_CNT {
                let obj = line_obj_create(c, parent, i, w as i32 + 4 * r, &LINE_POINTS[i as usize]);
                c.ui.obj_set_style_line_width(obj, width, SEL);
                c.ui.obj_set_style_line_rounded(obj, r != 0, SEL);
            }
        }
    }
}

/* ----------------------------------------------------------------- arc */

// `img_render_arc_bg`: the same extraction already used by `parity_arc.rs`,
// LVGL v9.6.0 `demos/render/assets/img_render_arc_bg.c::img_render_arc_bg_map`
// (40,000 bytes, SHA-256
// 3e3067a4649d8d1758fff6a05fedf602b18f1228e3dd72dc0288d886b16394bf). The C
// descriptor sets no magic.
static ARC_BG: ImageDsc = ImageDsc {
    header: ImageHeader {
        magic: 0,
        ..ImageHeader::new(ColorFormat::Argb8888, 100, 100, 400)
    },
    data_size: 40000,
    data: include_bytes!("assets/arc_bg.argb8888"),
};

fn arc_obj_create(c: &mut Ctx<'_>, parent: Obj, col: i32, row: i32, w: i32, start: i32, end: i32) -> Obj {
    let ui = &mut *c.ui;
    let arc = Arc::create(ui, parent);
    let obj = arc.obj();
    ui.obj_remove_style_all(obj);
    ui.obj_set_style_arc_width(obj, w, SEL);
    ui.obj_set_style_arc_color(obj, Color::WHITE, SEL);
    ui.obj_set_style_opa(obj, c.opa, SEL);
    arc.set_bg_angles(ui, start as _, end as _);
    ui.obj_set_size(obj, DEF_HEIGHT, DEF_HEIGHT);
    ui.obj_set_style_line_color(obj, Color::hex3(0xff0), SEL);
    add_to_cell(ui, obj, col, row);
    obj
}

fn arc_core_cb(c: &mut Ctx<'_>, parent: Obj, img_src: ImageSrc) {
    let angles = [
        (355, 5),
        (85, 95),
        (175, 185),
        (265, 275),
        (30, 330),
        (120, 60),
        (0, 180),
        (0, 360),
    ];
    let widths = [1, 5, 10, 100];
    for r in 0..2 {
        for (w, width) in widths.into_iter().enumerate() {
            for (i, (start, end)) in angles.into_iter().enumerate() {
                let obj = arc_obj_create(c, parent, i as i32, w as i32 + 4 * r, width, start, end);
                c.ui.obj_set_style_arc_rounded(obj, r != 0, SEL);
                c.ui.obj_set_style_arc_image_src(obj, img_src.clone(), SEL);
            }
        }
    }
}

/* ------------------------------------------------------------ triangle */

/// `triangle_draw_event_cb()`
fn triangle_draw_event_cb(
    ui: &mut Ui,
    obj: Obj,
    layer: ferrovgl_core::draw::LayerId,
    p_rel: [(i32, i32); 3],
) {
    let mut dsc = TriangleDsc::new();

    let coords = ui.obj_get_coords(obj);
    for (p, (x, y)) in dsc.p.iter_mut().zip(p_rel) {
        p.x = (x + coords.x1 + 8) as _;
        p.y = (y + coords.y1 + 2) as _;
    }

    let opa = ui.obj_get_style_opa(obj, Part::MAIN);
    /* LV_OPA_MIX2(a, b): ((uint16_t)(a) * (b)) >> 8 */
    let mix2 = |a: u8, b: u8| ((u16::from(a) * u16::from(b)) >> 8) as u8;
    dsc.grad.dir = ui.obj_get_style_bg_grad_dir(obj, Part::MAIN);
    dsc.grad.stops[0].color = ui.obj_get_style_bg_color(obj, Part::MAIN);
    dsc.grad.stops[0].frac = ui.obj_get_style_bg_main_stop(obj, Part::MAIN) as u8;
    dsc.grad.stops[0].opa = mix2(ui.obj_get_style_bg_main_opa(obj, Part::MAIN), opa);
    dsc.grad.stops[1].color = ui.obj_get_style_bg_grad_color(obj, Part::MAIN);
    dsc.grad.stops[1].frac = ui.obj_get_style_bg_grad_stop(obj, Part::MAIN) as u8;
    dsc.grad.stops[1].opa = mix2(ui.obj_get_style_bg_grad_opa(obj, Part::MAIN), opa);
    dsc.grad.stops_count = 2;

    dsc.color = dsc.grad.stops[0].color;
    dsc.opa = dsc.grad.stops[0].opa;

    ui.draw_triangle(layer, &dsc);
}

fn triangle_obj_create(c: &mut Ctx<'_>, parent: Obj, col: i32, row: i32, p: [(i32, i32); 3]) -> Obj {
    let ui = &mut *c.ui;
    let arc = Arc::create(ui, parent);
    let obj = arc.obj();
    ui.obj_remove_style_all(obj);
    ui.obj_set_size(obj, DEF_WIDTH, DEF_HEIGHT);
    ui.obj_set_style_bg_color(obj, Color::hex3(0xff0), SEL);
    ui.obj_set_style_opa(obj, c.opa, SEL);
    ui.obj_add_event_cb(
        obj,
        Rc::new(move |ui, e| {
            let target = e.target_obj().unwrap();
            let layer = e.layer().unwrap();
            triangle_draw_event_cb(ui, target, layer, p);
        }),
        EventCode::DRAW_MAIN,
        None,
    );
    add_to_cell(ui, obj, col, row);
    obj
}

fn triangle_cb(c: &mut Ctx<'_>, parent: Obj) {
    let points: [[(i32, i32); 3]; 16] = [
        /*Right angle triangles*/
        [(0, 0), (0, 26), (26, 26)],
        [(0, 26), (26, 0), (26, 26)],
        [(0, 0), (26, 0), (26, 26)],
        [(0, 0), (0, 26), (26, 0)],
        /*One side vertical or horizontal triangles*/
        [(0, 0), (26, 0), (13, 26)],
        [(0, 26), (26, 26), (13, 0)],
        [(0, 0), (0, 26), (26, 13)],
        [(0, 13), (26, 0), (26, 26)],
        /*Thin triangles*/
        [(0, 0), (26, 26), (13, 18)],
        [(0, 0), (26, 26), (13, 8)],
        [(26, 0), (0, 26), (13, 18)],
        [(26, 0), (0, 26), (13, 8)],
        /*General triangles with various point orders*/
        [(0, 1), (26, 6), (13, 26)],
        [(0, 1), (13, 26), (26, 6)],
        [(26, 6), (0, 1), (13, 26)],
        [(13, 26), (26, 6), (0, 1)],
    ];

    for (i, p) in points.into_iter().enumerate() {
        let i = i as i32;
        triangle_obj_create(c, parent, i % 8, i / 8, p);
    }
    for (row0, dir, count, transp) in [
        (2, GradDir::Ver, 16, false),
        (4, GradDir::Hor, 16, false),
        (6, GradDir::Ver, 8, true),
        (7, GradDir::Hor, 8, true),
    ] {
        for (i, p) in points.into_iter().take(count).enumerate() {
            let i = i as i32;
            let obj = triangle_obj_create(c, parent, i % 8, row0 + i / 8, p);
            let ui = &mut *c.ui;
            ui.obj_set_style_bg_grad_color(obj, Color::hex3(0xf00), SEL);
            ui.obj_set_style_bg_grad_dir(obj, dir, SEL);
            if transp {
                ui.obj_set_style_bg_grad_opa(obj, 0, SEL);
            }
        }
    }
}

/* --------------------------------------------------------------- layer */

fn layer_obj_create(c: &mut Ctx<'_>, parent: Obj, col: i32, row: i32, blend_mode: BlendMode) -> Obj {
    let ui = &mut *c.ui;
    let obj = ui.obj_create(Some(parent));
    ui.obj_remove_style_all(obj);
    ui.obj_set_size(obj, DEF_WIDTH - 10, DEF_HEIGHT);
    ui.obj_set_style_bg_opa(obj, 255, SEL);
    ui.obj_set_style_bg_color(obj, Color::hex3(0xff0), SEL);
    ui.obj_set_style_bg_grad_color(obj, Color::hex3(0xf00), SEL);
    ui.obj_set_style_bg_grad_dir(obj, GradDir::Ver, SEL);
    ui.obj_set_style_border_width(obj, 3, SEL);
    ui.obj_set_style_border_color(obj, Color::hex3(0x000), SEL);
    ui.obj_set_style_transform_pivot_x(obj, 0, SEL);
    ui.obj_set_style_transform_pivot_y(obj, 0, SEL);
    ui.obj_set_style_blend_mode(obj, blend_mode, SEL);
    ui.obj_set_style_opa(obj, c.opa, SEL);
    add_to_cell(ui, obj, col, row);

    let label = Label::create(ui, obj);
    label.set_text(ui, "ABC");
    ui.obj_center(label.obj());

    obj
}

fn layer_core_cb(c: &mut Ctx<'_>, parent: Obj, blend_mode: BlendMode) {
    for i in 0..2 {
        let row = 4 * i;
        let radius = 8 * i;

        let obj = layer_obj_create(c, parent, 0, row, blend_mode);
        c.ui.obj_set_style_radius(obj, radius, SEL);

        let obj = layer_obj_create(c, parent, 1, row, blend_mode);
        c.ui.obj_set_style_transform_rotation(obj, 300, SEL);
        c.ui.obj_set_style_translate_x(obj, 10, SEL);
        c.ui.obj_set_style_radius(obj, radius, SEL);

        let obj = layer_obj_create(c, parent, 2, row, blend_mode);
        c.ui.obj_set_style_transform_scale(obj, 400, SEL);
        c.ui.obj_set_style_radius(obj, radius, SEL);

        let obj = layer_obj_create(c, parent, 4, row, blend_mode);
        c.ui.obj_set_style_transform_rotation(obj, 300, SEL);
        c.ui.obj_set_style_transform_scale(obj, 400, SEL);
        c.ui.obj_set_style_radius(obj, radius, SEL);

        let obj = layer_obj_create(c, parent, 5, row, blend_mode);
        c.ui.obj_set_style_transform_scale_x(obj, 400, SEL);
        c.ui.obj_set_style_radius(obj, radius, SEL);

        let obj = layer_obj_create(c, parent, 7, row, blend_mode);
        c.ui.obj_set_style_transform_scale_y(obj, 400, SEL);
        c.ui.obj_set_style_radius(obj, radius, SEL);

        let obj = layer_obj_create(c, parent, 0, row + 2, blend_mode);
        c.ui.obj_set_style_transform_rotation(obj, 300, SEL);
        c.ui.obj_set_style_transform_scale_x(obj, 400, SEL);
        c.ui.obj_set_style_translate_x(obj, 10, SEL);
        c.ui.obj_set_style_radius(obj, radius, SEL);

        let obj = layer_obj_create(c, parent, 2, row + 2, blend_mode);
        c.ui.obj_set_style_transform_rotation(obj, 300, SEL);
        c.ui.obj_set_style_transform_scale_y(obj, 400, SEL);
        c.ui.obj_set_style_radius(obj, radius, SEL);

        let obj = layer_obj_create(c, parent, 5, row + 2, blend_mode);
        c.ui.obj_set_style_opa_layered(obj, OPA_50, SEL);
        c.ui.obj_set_style_translate_y(obj, 10, SEL);
        c.ui.obj_set_style_radius(obj, radius, SEL);

        let obj = layer_obj_create(c, parent, 7, row + 2, blend_mode);
        c.ui.obj_set_style_opa_layered(obj, OPA_50, SEL);
        c.ui.obj_set_style_transform_rotation(obj, 300, SEL);
        c.ui.obj_set_style_translate_y(obj, 10, SEL);
        c.ui.obj_set_style_radius(obj, radius, SEL);
    }
}

/* ---------------------------------------------------------- blend mode */

/// `lv_canvas_fill_bg()` on a buffer of the formats `blend_mode_cb()` uses.
fn canvas_fill_bg(buf: &mut DrawBuf, color: Color, opa: u8) {
    let (w, h, stride, cf) = (
        usize::from(buf.header.w),
        usize::from(buf.header.h),
        usize::from(buf.header.stride),
        buf.header.cf,
    );
    let data = buf.data_mut().unwrap();
    for y in 0..h {
        let row = &mut data[y * stride..];
        for x in 0..w {
            match cf {
                ColorFormat::Rgb565 => {
                    /* lv_color_to_u16() */
                    let c16 = ((u16::from(color.red) & 0xF8) << 8)
                        | ((u16::from(color.green) & 0xFC) << 3)
                        | (u16::from(color.blue) >> 3);
                    row[x * 2..x * 2 + 2].copy_from_slice(&c16.to_le_bytes());
                }
                ColorFormat::Xrgb8888 | ColorFormat::Argb8888 => {
                    /* lv_color_to_u32() */
                    let mut c32 = 0xff00_0000
                        | (u32::from(color.red) << 16)
                        | (u32::from(color.green) << 8)
                        | u32::from(color.blue);
                    if cf == ColorFormat::Argb8888 {
                        c32 &= 0x00ff_ffff;
                        c32 |= u32::from(opa) << 24;
                    }
                    row[x * 4..x * 4 + 4].copy_from_slice(&c32.to_le_bytes());
                }
                ColorFormat::Rgb888 => {
                    row[x * 3..x * 3 + 3].copy_from_slice(&[color.blue, color.green, color.red]);
                }
                _ => unreachable!("blend_mode_cb() only uses these canvas formats"),
            }
        }
    }
}

/// `create_blend_mode_image_buffer()`: `lv_canvas_fill_bg()`, then
/// `lv_canvas_init_layer()` / `lv_draw_label()` x5 / `lv_canvas_finish_layer()`.
fn create_blend_mode_image_buffer(ui: &mut Ui, mut buf: DrawBuf) -> DrawBuf {
    canvas_fill_bg(&mut buf, Color::hex3(0x844), 255);

    /* lv_canvas_init_layer() */
    let cf = buf.header.cf;
    let canvas_area = Area::new(0, 0, i32::from(buf.header.w) - 1, i32::from(buf.header.h) - 1);
    let layer = ui.draw_layer_create(None, cf, &canvas_area);
    ui.draw.layer_mut(layer).unwrap().draw_buf = Some(buf);

    let mut dsc = LabelDsc::new();
    /* lv_draw_label_dsc_init(): font = LV_FONT_DEFAULT */
    dsc.font = Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14));
    let mut coords = Area::new(0, 0, 100, 60);
    for (color, text, x1, y1) in [
        (0xff0000, "R", 0, None),
        (0x00ff00, "G", 11, None),
        (0x0000ff, "B", 23, None),
        (0xffffff, "W", 4, Some(14)),
        (0x000000, "K", 20, None),
    ] {
        dsc.color = Color::hex(color);
        dsc.text = Rc::from(text);
        if let Some(y1) = y1 {
            coords.y1 = y1;
        }
        coords.x1 = x1;
        ui.draw_label(layer, &dsc, &coords);
    }

    /* lv_canvas_finish_layer() */
    for _ in 0..64 {
        if ui.draw.layer(layer).unwrap().tasks.is_empty() {
            break;
        }
        ui.draw_dispatch();
    }
    assert!(
        ui.draw.layer(layer).unwrap().tasks.is_empty(),
        "canvas draw queue stalled"
    );
    let buf = ui.draw.layer_mut(layer).unwrap().draw_buf.take().unwrap();
    ui.draw_layer_delete(layer);
    buf
}

fn create_blend_mode_obj(
    c: &mut Ctx<'_>,
    parent: Obj,
    col: i32,
    row: i32,
    src: &ImageSrc,
    blend_mode: BlendMode,
) -> Image {
    let ui = &mut *c.ui;
    let image = Image::create(ui, parent);
    image.set_src(ui, src.clone());
    image.set_blend_mode(ui, blend_mode);
    ui.obj_set_style_image_opa(image.obj(), c.opa, SEL);
    ui.obj_set_style_image_recolor(image.obj(), Color::hex(0x00ff00), SEL);
    add_to_cell(ui, image.obj(), col, row);
    image
}

fn blend_mode_cb(c: &mut Ctx<'_>, parent: Obj) {
    c.ui.obj_set_grid_dsc_array(
        parent,
        TracksRef::Static(&[53, 53, 53, 53, 53, 53, 53, 53, 53, GRID_TEMPLATE_LAST]),
        TracksRef::Static(&[32, 40, 40, 40, 40, 40, 40, GRID_TEMPLATE_LAST]),
    );

    /*Make the parent darker for additive blending*/
    c.ui.obj_set_style_bg_color(parent, Color::hex(0x808080), SEL);

    /* LV_DRAW_BUF_DEFINE_STATIC(buf_*, 36, 30, cf) + LV_DRAW_BUF_INIT_STATIC.
     * buf_argb8888_premul is defined but never drawn (`cf < 4`). */
    let cfs = [
        ColorFormat::Rgb565,
        ColorFormat::Rgb888,
        ColorFormat::Xrgb8888,
        ColorFormat::Argb8888,
    ];

    /*The canvas will stay in the top left corner to show the original image*/
    let screen = c.ui.display_get_screen_active(None).unwrap();
    let canvas = Image::create(c.ui, screen);

    let cf_txt = ["RGB565", "RGB888.", "XRGB8888", "ARGB8888"];
    let mode_txt = ["Add.", "Sub.", "Mul."];
    let mode_values = [BlendMode::Additive, BlendMode::Subtractive, BlendMode::Multiply];

    for (m, txt) in mode_txt.into_iter().enumerate() {
        let mode_label = Label::create(c.ui, parent);
        mode_label.set_text(c.ui, txt);
        c.ui.obj_set_grid_cell(
            mode_label.obj(),
            GridAlign::Center,
            0,
            1,
            GridAlign::Center,
            1 + m as i32 * 2,
            2,
        );
    }

    let mut canvas_src = ImageSrc::None;
    for (cf, (format, txt)) in cfs.into_iter().zip(cf_txt).enumerate() {
        let cf = cf as i32;
        let cf_label = Label::create(c.ui, parent);
        cf_label.set_text(c.ui, txt);
        c.ui.obj_set_grid_cell(
            cf_label.obj(),
            GridAlign::Center,
            1 + cf * 2,
            2,
            GridAlign::Center,
            0,
            1,
        );

        let buf = DrawBuf::create(36, 30, format, 0).unwrap();
        let buf = create_blend_mode_image_buffer(c.ui, buf);
        let src = ImageSrc::DrawBuf(Rc::new(buf));
        /* lv_canvas_set_draw_buf(canvas, cf_bufs[cf]) */
        Image::set_src(canvas, c.ui, src.clone());
        canvas_src = src.clone();

        for (m, mode) in mode_values.into_iter().enumerate() {
            let m = m as i32;
            create_blend_mode_obj(c, parent, 1 + cf * 2, 1 + m * 2, &src, mode);

            let img = create_blend_mode_obj(c, parent, 2 + cf * 2, 1 + m * 2, &src, mode);
            img.set_rotation(c.ui, 200);

            let img = create_blend_mode_obj(c, parent, 1 + cf * 2, 2 + m * 2, &src, mode);
            c.ui.obj_set_style_image_recolor_opa(img.obj(), OPA_50, SEL);

            let img = create_blend_mode_obj(c, parent, 2 + cf * 2, 2 + m * 2, &src, mode);
            img.set_rotation(c.ui, 200);
            c.ui.obj_set_style_image_recolor_opa(img.obj(), OPA_50, SEL);
        }
    }

    /*Show the recolored image to show the original image*/
    let img_recolored = Image::create(c.ui, parent);
    img_recolored.set_src(c.ui, canvas_src);
    let obj = img_recolored.obj();
    c.ui.obj_set_style_image_recolor(obj, Color::hex(0x00ff00), SEL);
    c.ui.obj_set_style_image_recolor_opa(obj, OPA_50, SEL);
    c.ui.obj_set_y(obj, 30);
    c.ui.obj_set_ignore_layout(obj, true);
}

/* ------------------------------------------------- complex gradients */

/// The grid, the row/column labels and the 3 x 4 objects shared by
/// `linear_gradient_cb()`, `radial_gradient_cb()` and `conical_gradient_cb()`.
/// `params(y)` returns the gradient's `dir` and `params` for row `y`.
fn gradient_scene(c: &mut Ctx<'_>, parent: Obj, params: impl Fn(usize) -> (GradDir, GradParams)) {
    c.ui.obj_set_grid_dsc_array(
        parent,
        TracksRef::Static(&[53, 53, 53, 53, 53, 53, 53, 53, 53, GRID_TEMPLATE_LAST]),
        TracksRef::Static(&[32, 40, 40, 40, 40, 40, 40, GRID_TEMPLATE_LAST]),
    );

    let opa_txt = ["no opa", "no opa round", "stop opa", "stop opa round"];
    let radius_values = [0, 20, 0, 20];
    let opa_map_values = [false, false, true, true];

    let offs_txt = ["pad", "repeat", "reflect"];
    let extend_values = [GradExtend::Pad, GradExtend::Repeat, GradExtend::Reflect];

    for (y, txt) in offs_txt.into_iter().enumerate() {
        let offs_label = Label::create(c.ui, parent);
        offs_label.set_text(c.ui, txt);
        c.ui.obj_set_grid_cell(
            offs_label.obj(),
            GridAlign::Center,
            0,
            1,
            GridAlign::Center,
            1 + y as i32 * 2,
            2,
        );
    }

    for (x, txt) in opa_txt.into_iter().enumerate() {
        let op_label = Label::create(c.ui, parent);
        op_label.set_text(c.ui, txt);
        c.ui.obj_set_grid_cell(
            op_label.obj(),
            GridAlign::Center,
            1 + x as i32 * 2,
            2,
            GridAlign::Center,
            0,
            1,
        );

        for (y, extend) in extend_values.into_iter().enumerate() {
            let grad_color = [Color::make(0xd5, 0x03, 0x47), Color::make(0x00, 0x00, 0x00)];
            let grad_opa = [255, 0];

            /* `static lv_grad_dsc_t grad_values[3][4]`: zero-initialised. */
            let mut grad = GradDsc::new();
            /*init gradient color map*/
            grad.init_stops(
                &grad_color,
                opa_map_values[x].then_some(&grad_opa[..]),
                None,
                grad_color.len(),
            );
            /*init gradient parameters*/
            let (dir, p) = params(y);
            grad.dir = dir;
            grad.params = p;
            grad.extend = extend;

            /*create rectangle*/
            let ui = &mut *c.ui;
            let obj = ui.obj_create(Some(parent));
            ui.obj_remove_style_all(obj);
            ui.obj_set_size(obj, 70, 50);
            ui.obj_set_style_radius(obj, radius_values[x], SEL);
            ui.obj_set_style_bg_opa(obj, 255, SEL);
            ui.obj_set_style_opa(obj, c.opa, SEL);

            /*set gradient as background*/
            ui.obj_set_style_bg_grad(obj, SharedRef::Shared(Rc::new(grad)), SEL);

            add_to_cell(ui, obj, 1 + x as i32 * 2, 1 + y as i32 * 2);
        }
    }
}

fn linear_gradient_cb(c: &mut Ctx<'_>, parent: Obj) {
    let x1_values = [coord::pct(100), coord::pct(15), coord::pct(30)];
    let y1_values = [coord::pct(100), coord::pct(30), coord::pct(15)];
    gradient_scene(c, parent, |y| {
        (
            GradDir::Linear,
            GradParams::Linear {
                start: Point::new(0, 0),
                end: Point::new(x1_values[y], y1_values[y]),
            },
        )
    });
}

fn radial_gradient_cb(c: &mut Ctx<'_>, parent: Obj) {
    gradient_scene(c, parent, |y| {
        let offs = y as i32 * 5;
        let r0 = 0;
        let focal = Point::new(coord::pct(50), coord::pct(50));
        let end = Point::new(focal.x + offs, focal.y + offs);
        (
            GradDir::Radial,
            GradParams::Radial {
                focal,
                focal_extent: Point::new(focal.x + r0, focal.y),
                end,
                end_extent: Point::new(end.x, coord::pct(85)),
            },
        )
    });
}

fn conical_gradient_cb(c: &mut Ctx<'_>, parent: Obj) {
    gradient_scene(c, parent, |_| {
        (
            GradDir::Conical,
            GradParams::Conical {
                center: Point::new(coord::pct(50), coord::pct(50)),
                start_angle: 10,
                end_angle: 100,
            },
        )
    });
}

/* ------------------------------------------------------ lv_demo_render */

/// `lv_demo_render(id, opa)`
fn demo_render(ui: &mut Ui, scene: Scene, opa: u8) {
    let scr = ui.display_get_screen_active(None).unwrap();
    ui.obj_clean(scr);
    ui.obj_remove_style_all(scr);
    ui.obj_set_style_bg_opa(scr, 255, SEL);
    ui.obj_set_style_text_color(scr, Color::BLACK, SEL);
    ui.obj_set_style_bg_color(scr, Color::WHITE, SEL);

    let main_parent = ui.obj_create(Some(scr));
    ui.obj_remove_style_all(main_parent);
    ui.obj_set_style_bg_opa(main_parent, 255, SEL);
    ui.obj_set_style_bg_color(main_parent, Color::hex3(0xaaf), SEL);
    ui.obj_set_size(main_parent, 480, 272);

    ui.obj_set_grid_dsc_array(
        main_parent,
        TracksRef::Static(&[60, 60, 60, 60, 60, 60, 60, 60, GRID_TEMPLATE_LAST]),
        TracksRef::Static(&[34, 34, 34, 34, 34, 34, 34, 34, GRID_TEMPLATE_LAST]),
    );

    let c = &mut Ctx { ui, opa };
    match scene {
        Scene::Fill => fill_cb(c, main_parent),
        Scene::Border => border_cb(c, main_parent),
        Scene::BoxShadow => box_shadow_cb(c, main_parent),
        Scene::Text => text_cb(c, main_parent),
        Scene::ImageNormal1 => image_core_cb(c, main_parent, false, 0),
        Scene::ImageRecolor1 => image_core_cb(c, main_parent, true, 0),
        Scene::ImageNormal2 => image_core_cb(c, main_parent, false, 4),
        Scene::ImageRecolor2 => image_core_cb(c, main_parent, true, 4),
        Scene::Line => line_cb(c, main_parent),
        Scene::ArcNormal => arc_core_cb(c, main_parent, ImageSrc::None),
        Scene::ArcImage => arc_core_cb(c, main_parent, ImageSrc::Static(&ARC_BG)),
        Scene::Triangle => triangle_cb(c, main_parent),
        Scene::LayerNormal => layer_core_cb(c, main_parent, BlendMode::Normal),
        Scene::BlendMode => blend_mode_cb(c, main_parent),
        Scene::LinearGradient => linear_gradient_cb(c, main_parent),
        Scene::RadialGradient => radial_gradient_cb(c, main_parent),
        Scene::ConicalGradient => conical_gradient_cb(c, main_parent),
    }
}

/// One iteration of the upstream scene loop for `scene` at both
/// `opa_values`: `lv_demo_render(i, opa)` + `TEST_ASSERT_EQUAL_SCREENSHOT`.
fn render_to(dir: &str, cf: ColorFormat, scene: Scene) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let disp = display.display();
    /* lv_display_set_color_format(lv_display_get_default(), cf) */
    display.ui.display_set_color_format(disp, cf);
    let mut failures = Vec::new();
    for opa in [0xff, 0x80] {
        demo_render(&mut display.ui, scene, opa);
        let name = format!("draw/render/{dir}/demo_render_{}_opa_{opa}.png", scene.name());
        if let Err(e) = display.screenshot_compare(&name) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// One module per `test_render_to_<format>.c`, one test per scene.
macro_rules! render_tests {
    ($($module:ident => $cf:expr;)*) => {
        $(
            mod $module {
                use super::*;
                render_tests!(@scenes $module, $cf, [
                    fill: Fill,
                    border: Border,
                    box_shadow: BoxShadow,
                    text: Text,
                    image_normal_1: ImageNormal1,
                    image_recolor_1: ImageRecolor1,
                    image_normal_2: ImageNormal2,
                    image_recolor_2: ImageRecolor2,
                    line: Line,
                    arc_normal: ArcNormal,
                    arc_image: ArcImage,
                    triangle: Triangle,
                    layer_normal: LayerNormal,
                    blend_mode: BlendMode,
                    linear_gradient: LinearGradient,
                    radial_gradient: RadialGradient,
                    conical_gradient: ConicalGradient,
                ]);
            }
        )*
    };
    (@scenes $module:ident, $cf:expr, [$($test:ident: $scene:ident,)*]) => {
        $(
            #[test]
            #[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
            fn $test() {
                render_to(stringify!($module), $cf, Scene::$scene);
            }
        )*
    };
}

render_tests! {
    xrgb8888 => ColorFormat::Xrgb8888;
    argb8888 => ColorFormat::Argb8888;
    argb8888_premultiplied => ColorFormat::Argb8888Premultiplied;
    rgb888 => ColorFormat::Rgb888;
    rgb565 => ColorFormat::Rgb565;
    rgb565_swapped => ColorFormat::Rgb565Swapped;
    l8 => ColorFormat::L8;
    al88 => ColorFormat::Al88;
}

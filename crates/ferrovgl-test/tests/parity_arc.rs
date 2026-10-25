//! LVGL v9.6.0 demos/render/lv_demo_render.c arc scenes, reproduced through
//! normal object/grid/DRAW_MAIN tasks. The arc widget API itself is not ported;
//! its background draw command is emitted from an ordinary object's callback.

use std::rc::Rc;

use ferrovgl_core::draw::ArcDsc;
use ferrovgl_core::draw::draw_buf::{ImageDsc, ImageHeader};
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign};
use ferrovgl_core::misc::style::{ImageSrc, Part, TracksRef};
use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Color, ColorFormat, EventCode, Point, StyleSelector};
use ferrovgl_test::TestDisplay;

fn scene(opa: u8, image: ImageSrc) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let ui = &mut display.ui;
    let screen = ui.display_get_screen_active(None).unwrap();
    let selector = StyleSelector::DEFAULT;
    ui.obj_clean(screen);
    ui.obj_remove_style_all(screen);
    ui.obj_set_style_bg_opa(screen, 255, selector);
    ui.obj_set_style_text_color(screen, Color::BLACK, selector);
    ui.obj_set_style_bg_color(screen, Color::WHITE, selector);
    let parent = ui.obj_create(Some(screen));
    ui.obj_remove_style_all(parent);
    ui.obj_set_style_bg_opa(parent, 255, selector);
    ui.obj_set_style_bg_color(parent, Color::make(170, 170, 255), selector);
    ui.obj_set_size(parent, 480, 272);
    ui.obj_set_grid_dsc_array(
        parent,
        TracksRef::Static(&[60, 60, 60, 60, 60, 60, 60, 60, GRID_TEMPLATE_LAST]),
        TracksRef::Static(&[34, 34, 34, 34, 34, 34, 34, 34, GRID_TEMPLATE_LAST]),
    );
    for rounded in [false, true] {
        for (row, width) in [1, 5, 10, 100].into_iter().enumerate() {
            for (col, (start, end)) in [
                (355, 5),
                (85, 95),
                (175, 185),
                (265, 275),
                (30, 330),
                (120, 60),
                (0, 180),
                (0, 360),
            ]
            .into_iter()
            .enumerate()
            {
                let obj = ui.obj_create(Some(parent));
                ui.obj_remove_style_all(obj);
                ui.obj_set_size(obj, 30, 30);
                ui.obj_set_style_arc_width(obj, width, selector);
                ui.obj_set_style_arc_color(obj, Color::WHITE, selector);
                ui.obj_set_style_arc_rounded(obj, rounded, selector);
                ui.obj_set_style_arc_image_src(obj, image.clone(), selector);
                ui.obj_set_style_opa(obj, opa, selector);
                ui.obj_set_style_line_color(obj, Color::make(255, 255, 0), selector);
                ui.obj_set_grid_cell(
                    obj,
                    GridAlign::Center,
                    col as i32,
                    1,
                    GridAlign::Center,
                    row as i32 + if rounded { 4 } else { 0 },
                    1,
                );
                ui.obj_add_event_cb(
                    obj,
                    Rc::new(move |ui, event| {
                        let layer = event.layer().unwrap();
                        let coords = ui.obj_get_coords(obj);
                        let mut dsc = ArcDsc::new();
                        dsc.base.layer = Some(layer);
                        ui.obj_init_draw_arc_dsc(obj, Part::MAIN, &mut dsc);
                        dsc.center = Point::new(coords.x1 + 15, coords.y1 + 15);
                        dsc.radius = 15;
                        dsc.start_angle = start as ValuePrecise;
                        dsc.end_angle = end as ValuePrecise;
                        ui.draw_arc(layer, &dsc);
                    }),
                    EventCode::DRAW_MAIN,
                    None,
                );
            }
        }
    }
    let kind = if image.is_none() { "normal" } else { "image" };
    display
        .screenshot_compare(format!(
            "draw/render/xrgb8888/demo_render_arc_{kind}_opa_{opa}.png"
        ))
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn arc_normal_and_rounded_opaque() {
    scene(255, ImageSrc::None);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn arc_normal_and_rounded_translucent() {
    scene(128, ImageSrc::None);
}

// Exact byte-array extraction from LVGL v9.6.0 commit
// 80ca777e37a2b176770726a02e07a6fb79ef0b39,
// demos/render/assets/img_render_arc_bg.c::img_render_arc_bg_map (40,000 bytes).
// SHA-256: 3e3067a4649d8d1758fff6a05fedf602b18f1228e3dd72dc0288d886b16394bf.
// Header matches the source descriptor, including legacy magic.
static ARC_BACKGROUND: ImageDsc = ImageDsc {
    header: ImageHeader {
        magic: 0,
        ..ImageHeader::new(ColorFormat::Argb8888, 100, 100, 400)
    },
    data_size: 40000,
    data: include_bytes!("assets/arc_bg.argb8888"),
};

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn arc_image_opaque() {
    scene(255, ImageSrc::Static(&ARC_BACKGROUND));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn arc_image_translucent() {
    scene(128, ImageSrc::Static(&ARC_BACKGROUND));
}

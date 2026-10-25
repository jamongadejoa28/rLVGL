//! Image widget integration through public object, event, draw and snapshot APIs.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ferrovgl_core::draw::image::image_buf_get_transformed_area;
use ferrovgl_core::draw::{DrawBuf, DrawDsc};
use ferrovgl_core::misc::event::{CoverCheckInfo, CoverRes, EventParam, HitTestInfo};
use ferrovgl_core::misc::style::{BlendMode, ImageSrc, SharedRef};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Area, Color, ColorFormat, EventCode, FontRef, Point, StyleSelector};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Image, ImageAlign};

const SELECTOR: StyleSelector = StyleSelector::DEFAULT;

fn source(w: u32, h: u32, cf: ColorFormat) -> Rc<DrawBuf> {
    let mut buf = DrawBuf::create(w, h, cf, 0).unwrap();
    if cf == ColorFormat::Argb8888 || cf == ColorFormat::Xrgb8888 {
        for p in buf.data_mut().unwrap().as_chunks_mut::<4>().0 {
            *p = [0x56, 0x34, 0x12, 255];
        }
    } else {
        buf.data_mut().unwrap().fill(255);
    }
    Rc::new(buf)
}

#[test]
fn defaults_source_content_size_and_owned_lifetime() {
    let mut display = TestDisplay::with_resolution(64, 48).unwrap();
    let screen = display.screen();
    let image = Image::create(&mut display.ui, screen);
    let ui = &mut display.ui;
    assert!(!ui.obj_is_clickable(image.obj()));
    assert!(ui.obj_is_adv_hittest(image.obj()));
    assert_eq!(image.rotation(ui), 0);
    assert_eq!(image.scale(ui), 256);
    assert_eq!(image.inner_align(ui), ImageAlign::Center);
    assert!(image.antialias(ui));
    let buffer = source(12, 8, ColorFormat::Argb8888);
    let weak = Rc::downgrade(&buffer);
    image.set_src(ui, ImageSrc::DrawBuf(buffer));
    ui.obj_update_layout(image.obj());
    assert_eq!(
        (ui.obj_get_width(image.obj()), ui.obj_get_height(image.obj())),
        (12, 8)
    );
    assert_eq!(image.pivot(ui), Point::new(5, 3));
    assert!(Image::from_obj(ui, image.obj()).is_some());
    image.set_src(ui, ImageSrc::None);
    ui.obj_update_layout(image.obj());
    assert_eq!((image.src_width(ui), image.src_height(ui)), (0, 0));
    assert!(weak.upgrade().is_none());
    ui.obj_delete(image.obj());
    image.set_rotation(ui, 450);
    image.set_src(ui, ImageSrc::None);
    assert!(Image::from_obj(ui, image.obj()).is_none());
}

#[test]
fn unsupported_file_and_invalid_metadata_clear_the_previous_image() {
    let mut display = TestDisplay::with_resolution(64, 48).unwrap();
    let screen = display.screen();
    let ui = &mut display.ui;
    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::DrawBuf(source(12, 8, ColorFormat::Argb8888)));
    image.set_src(ui, ImageSrc::Path(SharedRef::Static("A:/missing.png")));
    assert!(image.src(ui).is_none());
    assert_eq!(image.src_width(ui), 0);
    let mut invalid = DrawBuf::create(2, 2, ColorFormat::Argb8888, 0).unwrap();
    invalid.header.magic = 1; // 0 is a valid legacy image-header magic.
    image.set_src(ui, ImageSrc::DrawBuf(Rc::new(invalid)));
    assert!(image.src(ui).is_none());
}

#[test]
fn rotation_percentage_pivot_and_scale_follow_source_geometry() {
    let mut display = TestDisplay::with_resolution(64, 48).unwrap();
    let screen = display.screen();
    let ui = &mut display.ui;
    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::DrawBuf(source(20, 10, ColorFormat::Argb8888)));
    image.set_rotation(ui, -450);
    assert_eq!(image.rotation(ui), 3150);
    image.set_rotation(ui, 4500);
    assert_eq!(image.rotation(ui), 900);
    image.set_pivot(ui, coord::pct(100), coord::pct(0));
    assert_eq!(image.pivot(ui), Point::new(19, 0));
    assert_eq!(
        (image.transformed_width(ui), image.transformed_height(ui)),
        (10, 20)
    );
    assert!(ui.obj_get_ext_draw_size(image.obj()) > 0);
    image.set_rotation(ui, 0);
    image.set_scale(ui, 0);
    assert_eq!((image.scale_x(ui), image.scale_y(ui)), (1, 1));
    image.set_scale_x(ui, 512);
    image.set_scale_y(ui, 256);
    assert_eq!((image.scale_x(ui), image.scale_y(ui)), (512, 256));
    image.set_blend_mode(ui, BlendMode::Additive);
    image.set_antialias(ui, false);
    assert_eq!(image.blend_mode(ui), BlendMode::Additive);
    assert!(!image.antialias(ui));
}

#[test]
fn automatic_alignment_controls_rotation_pivot_and_scales() {
    for (align, expected) in [
        (ImageAlign::Stretch, (512, 768)),
        (ImageAlign::Contain, (512, 512)),
        (ImageAlign::ContainDownscale, (256, 256)),
        (ImageAlign::Cover, (768, 768)),
        (ImageAlign::Tile, (256, 256)),
    ] {
        let mut display = TestDisplay::with_resolution(128, 64).unwrap();
        let screen = display.screen();
        let ui = &mut display.ui;
        let image = Image::create(ui, screen);
        image.set_src(ui, ImageSrc::DrawBuf(source(20, 10, ColorFormat::Argb8888)));
        ui.obj_set_size(image.obj(), 40, 30);
        image.set_rotation(ui, 450);
        image.set_inner_align(ui, align);
        image.set_rotation(ui, 500);
        image.set_pivot(ui, 30, 40);
        image.set_scale_x(ui, 999);
        image.set_scale_y(ui, 998);
        assert_eq!(image.rotation(ui), 0, "{align:?}");
        assert_eq!(image.pivot(ui), Point::new(0, 0));
        assert_eq!((image.scale_x(ui), image.scale_y(ui)), expected);
        image.set_inner_align(ui, ImageAlign::Center);
        assert_eq!(
            (image.scale_x(ui), image.scale_y(ui)),
            expected,
            "C preserves automatic scale when exiting this mode"
        );
        image.set_scale(ui, 256);
        assert_eq!(image.scale(ui), 256);
    }
}

#[test]
fn resizing_an_automatic_image_refreshes_its_scale() {
    let mut display = TestDisplay::with_resolution(64, 48).unwrap();
    let screen = display.screen();
    let ui = &mut display.ui;
    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::DrawBuf(source(20, 10, ColorFormat::Argb8888)));
    image.set_inner_align(ui, ImageAlign::Stretch);
    ui.obj_set_size(image.obj(), 60, 20);
    ui.obj_update_layout(image.obj());
    assert_eq!((image.scale_x(ui), image.scale_y(ui)), (768, 512));
}

#[test]
fn tiled_offset_renders_and_bitmap_mask_is_forwarded_to_image_tasks() {
    let mut display = TestDisplay::with_resolution(32, 24).unwrap();
    let screen = display.screen();
    let ui = &mut display.ui;
    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::DrawBuf(source(4, 4, ColorFormat::Argb8888)));
    ui.obj_set_size(image.obj(), 12, 8);
    image.set_inner_align(ui, ImageAlign::Tile);
    image.set_offset_x(ui, -2000);
    image.set_offset_y(ui, 2000);
    let buf = ui.snapshot_take(image.obj(), ColorFormat::Argb8888).unwrap();
    assert!(
        buf.data()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0x56, 0x34, 0x12, 255])
    );

    image.set_inner_align(ui, ImageAlign::Center);
    image.set_offset_x(ui, 0);
    image.set_offset_y(ui, 0);
    ui.obj_set_size(image.obj(), 4, 4);
    let mut mask = DrawBuf::create(4, 4, ColorFormat::A8, 0).unwrap();
    mask.data_mut().unwrap()[..4].fill(255);
    let mask = ImageSrc::DrawBuf(Rc::new(mask));
    image.set_bitmap_map_src(ui, mask.clone());
    let forwarded = Rc::new(Cell::new(false));
    let flag = forwarded.clone();
    ui.obj_set_send_draw_task_events(image.obj(), true);
    ui.obj_add_event_cb(
        image.obj(),
        Rc::new(move |ui, event| {
            let task = ui.draw.task(event.draw_task().unwrap()).unwrap();
            if let DrawDsc::Image(dsc) = &task.dsc {
                assert_eq!(dsc.bitmap_mask_src, mask);
                flag.set(true);
            }
        }),
        EventCode::DRAW_TASK_ADDED,
        None,
    );
    let buf = ui.snapshot_take(image.obj(), ColorFormat::Argb8888).unwrap();
    assert!(forwarded.get());
    assert_eq!(&buf.data()[..4], &[0x56, 0x34, 0x12, 255]);
    // Pinned lv_draw_sw_img.c applies bitmap masks in lv_draw_sw_layer only.
    // Direct image tasks retain the mask descriptor without applying it.
    assert_eq!(
        &buf.data()[usize::from(buf.header.stride)..][..4],
        &[0x56, 0x34, 0x12, 255]
    );
    image.set_bitmap_map_src(ui, ImageSrc::None);
    assert!(image.bitmap_map_src(ui).is_none());
}

#[test]
fn cover_and_hit_test_events_use_image_transform_and_alpha() {
    let mut display = TestDisplay::with_resolution(64, 48).unwrap();
    let screen = display.screen();
    let ui = &mut display.ui;
    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::DrawBuf(source(20, 10, ColorFormat::Xrgb8888)));
    ui.obj_set_style_bg_opa(image.obj(), 255, SELECTOR);
    ui.obj_update_layout(image.obj());
    let mut cover = CoverCheckInfo {
        res: CoverRes::Cover,
        area: Area::new(0, 0, 5, 5),
    };
    ui.obj_send_event(
        image.obj(),
        EventCode::COVER_CHECK,
        EventParam::CoverCheck(&mut cover),
    );
    assert_eq!(cover.res, CoverRes::Cover);
    image.set_rotation(ui, 900);
    ui.obj_send_event(
        image.obj(),
        EventCode::COVER_CHECK,
        EventParam::CoverCheck(&mut cover),
    );
    assert_eq!(cover.res, CoverRes::NotCover);
    let mut hit = HitTestInfo {
        point: Point::new(9, -3),
        res: false,
    };
    ui.obj_send_event(image.obj(), EventCode::HIT_TEST, EventParam::HitTest(&mut hit));
    assert!(hit.res, "rotated bounds extend above the untransformed image");
    hit.point = Point::new(0, 0);
    ui.obj_send_event(image.obj(), EventCode::HIT_TEST, EventParam::HitTest(&mut hit));
    assert!(!hit.res);
}

#[test]
fn symbol_source_remeasures_on_font_change_and_draws_glyphs() {
    let mut display = TestDisplay::with_resolution(64, 48).unwrap();
    let screen = display.screen();
    let ui = &mut display.ui;
    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::Symbol(SharedRef::Static("\u{f03e}")));
    let small_h = image.src_height(ui);
    ui.obj_set_style_text_font(
        image.obj(),
        FontRef::Static(&ferrovgl_fonts::MONTSERRAT_28),
        SELECTOR,
    );
    assert!(image.src_height(ui) > small_h);
    ui.obj_set_style_text_color(image.obj(), Color::WHITE, SELECTOR);
    let buf = ui.snapshot_take(image.obj(), ColorFormat::Argb8888).unwrap();
    assert!(buf.data().as_chunks::<4>().0.iter().any(|p| p[3] != 0));
    assert!(buf.data().as_chunks::<4>().0.iter().any(|p| p[3] == 0));
}

#[test]
fn queued_image_task_retains_source_after_widget_deletion() {
    let mut display = TestDisplay::with_resolution(32, 24).unwrap();
    let screen = display.screen();
    let ui = &mut display.ui;
    let image = Image::create(ui, screen);
    let buf = source(4, 4, ColorFormat::Argb8888);
    let weak = Rc::downgrade(&buf);
    image.set_src(ui, ImageSrc::DrawBuf(buf));
    ui.obj_update_layout(image.obj());
    let layer = ui.draw_layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 3, 3));
    ui.draw.task_running = true;
    ui.obj_send_event(image.obj(), EventCode::DRAW_MAIN, EventParam::Layer(layer));
    ui.draw.task_running = false;
    assert_eq!(ui.draw.layer(layer).unwrap().tasks.len(), 1);
    ui.obj_delete(image.obj());
    assert!(weak.upgrade().is_some());
    ui.draw_dispatch();
    ui.draw_dispatch();
    assert_eq!(
        &ui.draw.layer(layer).unwrap().draw_buf.as_ref().unwrap().data()[..4],
        &[0x56, 0x34, 0x12, 255]
    );
    assert!(weak.upgrade().is_none());
    ui.draw_layer_delete(layer);
}

#[test]
fn transformation_stops_when_layout_callback_deletes_image() {
    let mut display = TestDisplay::with_resolution(64, 48).unwrap();
    let screen = display.screen();
    let ui = &mut display.ui;
    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::DrawBuf(source(20, 10, ColorFormat::Argb8888)));
    ui.obj_add_event_cb(
        image.obj(),
        Rc::new(move |ui, _| ui.obj_delete(image.obj())),
        EventCode::GET_SELF_SIZE,
        None,
    );
    image.set_rotation(ui, 450);
    assert!(!ui.obj_is_valid(image.obj()));
}

#[test]
fn ext_draw_callback_deletion_restores_invalidation_or_accepts_deleted_display() {
    for delete_display in [false, true] {
        let mut display = TestDisplay::with_resolution(64, 48).unwrap();
        let screen = display.screen();
        let disp = display.display();
        let ui = &mut display.ui;
        let image = Image::create(ui, screen);
        image.set_src(ui, ImageSrc::DrawBuf(source(20, 10, ColorFormat::Argb8888)));
        ui.obj_update_layout(image.obj());
        let calls = Rc::new(Cell::new(0));
        let count = calls.clone();
        ui.obj_add_event_cb(
            image.obj(),
            Rc::new(move |ui, _| {
                count.set(count.get() + 1);
                if delete_display {
                    ui.display_delete(disp);
                } else {
                    ui.obj_delete(image.obj());
                }
            }),
            EventCode::REFR_EXT_DRAW_SIZE,
            None,
        );
        image.set_scale(ui, 512);
        assert_eq!(calls.get(), 1);
        assert!(!ui.obj_is_valid(image.obj()));
        if !delete_display {
            assert!(ui.display_is_invalidation_enabled(Some(disp)));
        }
    }
}

#[test]
fn transform_invalidation_preserves_the_c_setters_cached_pivot_contract() {
    for operation in 0..3 {
        let mut display = TestDisplay::with_resolution(128, 128).unwrap();
        let screen = display.screen();
        let disp = display.display();
        let image = Image::create(&mut display.ui, screen);
        image.set_src(
            &mut display.ui,
            ImageSrc::DrawBuf(source(20, 10, ColorFormat::Argb8888)),
        );
        display.ui.obj_set_pos(image.obj(), 40, 40);
        if operation == 2 {
            image.set_rotation(&mut display.ui, 450);
        }
        display.snapshot().unwrap();
        let ui = &mut display.ui;
        let initial_pivot = image.pivot(ui);
        let changed = Rc::new(Cell::new(false));
        let flag = changed.clone();
        ui.obj_add_event_cb(
            image.obj(),
            Rc::new(move |ui, _| {
                if !flag.replace(true) {
                    image.set_pivot(ui, 0, 0);
                }
            }),
            EventCode::REFR_EXT_DRAW_SIZE,
            None,
        );
        let areas = Rc::new(RefCell::new(Vec::new()));
        let recorded = areas.clone();
        ui.display_add_event_cb(
            disp,
            Rc::new(move |_, event| {
                if let EventParam::AreaMut(area) = &event.param {
                    recorded.borrow_mut().push(**area);
                }
            }),
            EventCode::INVALIDATE_AREA,
            None,
        );
        match operation {
            0 => image.set_rotation(ui, 450),
            1 => image.set_scale(ui, 512),
            _ => image.set_pivot(ui, coord::pct(100), coord::pct(100)),
        }
        assert!(changed.get());
        assert_eq!(image.pivot(ui), Point::new(0, 0));
        let mut expected = Area::default();
        // lv_image_set_rotation/scale_update reuse their initial pivot_px;
        // lv_image_set_pivot resolves the pivot again after the callback.
        image_buf_get_transformed_area(
            &mut expected,
            20,
            10,
            image.rotation(ui),
            image.scale_x(ui) as u16,
            image.scale_y(ui) as u16,
            if operation == 2 {
                Point::new(0, 0)
            } else {
                initial_pivot
            },
        );
        expected.move_by(40, 40);
        if operation == 1 {
            expected.increase(1, 1);
        }
        // obj_invalidate_area clips to the object's current extended bounds
        // before the display sees INVALIDATE_AREA, including C's scale padding.
        let mut visible = ui.obj_get_coords(image.obj());
        let ext = ui.obj_get_ext_draw_size(image.obj());
        visible.increase(ext, ext);
        expected = Area::intersect(&expected, &visible).unwrap();
        assert_eq!(areas.borrow().last(), Some(&expected), "operation {operation}");
    }
}

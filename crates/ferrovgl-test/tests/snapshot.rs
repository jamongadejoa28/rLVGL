//! Public snapshot API: allocation, rendering, queue isolation and callback safety.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ferrovgl_core::draw::draw_buf::{BufStorage, DrawBuf, ImageHeader};
use ferrovgl_core::draw::{
    DRAW_UNIT_IDLE, DrawCtx, DrawImageDsc, DrawTask, DrawTaskState, DrawUnit, FillDsc, LayerId,
};
use ferrovgl_core::misc::event::EventParam;
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::{Area, Color, ColorFormat, EventCode, LvResult, Obj, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;

const SELECTOR: StyleSelector = StyleSelector::DEFAULT;

fn rectangle(ui: &mut Ui, parent: Obj, x: i32, y: i32, w: i32, h: i32, color: Color) -> Obj {
    let obj = ui.obj_create(Some(parent));
    ui.obj_remove_style_all(obj);
    ui.obj_set_pos(obj, x, y);
    ui.obj_set_size(obj, w, h);
    ui.obj_set_style_bg_opa(obj, 255, SELECTOR);
    ui.obj_set_style_bg_color(obj, color, SELECTOR);
    obj
}

fn pixel(buf: &DrawBuf, x: usize, y: usize) -> [u8; 4] {
    let offset = y * usize::from(buf.header.stride) + x * 4;
    buf.data()[offset..offset + 4].try_into().unwrap()
}

#[test]
fn allocation_updates_layout_and_includes_extended_draw_area() {
    let mut display = TestDisplay::with_resolution(64, 48).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 9, 7, 24, 18, Color::WHITE);
    ui.obj_set_style_shadow_width(obj, 5, SELECTOR);
    ui.obj_set_style_shadow_spread(obj, 3, SELECTOR);
    let buf = ui.snapshot_create_draw_buf(obj, ColorFormat::Argb8888).unwrap();
    let ext = ui.obj_get_ext_draw_size(obj);
    assert!(ext > 0);
    assert_eq!(i32::from(buf.header.w), 24 + 2 * ext);
    assert_eq!(i32::from(buf.header.h), 18 + 2 * ext);
    let snapshot = ui.snapshot_take(obj, ColorFormat::Argb8888).unwrap();
    assert_eq!(pixel(&snapshot, ext as usize + 2, ext as usize + 2), [255; 4]);
    assert_eq!(pixel(&snapshot, 0, 0)[3], 0);
}

#[test]
fn reuse_changes_format_and_size_without_reallocating() {
    let mut display = TestDisplay::with_resolution(32, 24).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 11, 8, 12, 10, Color::hex(0xff0000));
    let mut buf = DrawBuf::create(32, 24, ColorFormat::Argb8888, 0).unwrap();
    let original = buf.data().as_ptr();
    let capacity = buf.data_size;
    assert_eq!(
        ui.snapshot_take_to_draw_buf(obj, ColorFormat::Rgb565, &mut buf),
        LvResult::Ok
    );
    assert_eq!(buf.header.cf, ColorFormat::Rgb565);
    assert_eq!((buf.header.w, buf.header.h), (12, 10));
    assert_eq!(buf.data()[..2], [0, 248]);
    ui.obj_set_size(obj, 8, 6);
    assert_eq!(
        ui.snapshot_take_to_draw_buf(obj, ColorFormat::Argb8888, &mut buf),
        LvResult::Ok
    );
    assert_eq!(pixel(&buf, 0, 0), [0, 0, 255, 255]);
    assert_eq!((buf.header.w, buf.header.h), (8, 6));
    assert_eq!(buf.data().as_ptr(), original);
    assert_eq!(buf.data_size, capacity);
}

#[test]
fn invalid_shapes_formats_and_buffers_leave_header_unchanged() {
    let mut display = TestDisplay::with_resolution(32, 24).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 0, 0, 10, 10, Color::WHITE);
    let mut buf = DrawBuf::create(2, 2, ColorFormat::Argb8888, 0).unwrap();
    let original = buf.header;
    assert_eq!(ui.snapshot_reshape_draw_buf(obj, &mut buf), LvResult::Invalid);
    assert_eq!(buf.header, original);
    assert_eq!(
        ui.snapshot_take_to_draw_buf(obj, ColorFormat::Unknown, &mut buf),
        LvResult::Invalid
    );
    assert!(ui.snapshot_take(obj, ColorFormat::Rgb565A8).is_none());
    for (w, h) in [(0, 0), (0, 10), (10, 0), (20_000, 1)] {
        ui.obj_set_size(obj, w, h);
        assert!(ui.snapshot_create_draw_buf(obj, ColorFormat::Argb8888).is_none());
    }
    ui.obj_set_size(obj, 1, 1);
    let mut read_only =
        DrawBuf::init(1, 1, ColorFormat::Argb8888, 4, BufStorage::Static(&[0; 8]), 8).unwrap();
    assert_eq!(
        ui.snapshot_reshape_draw_buf(obj, &mut read_only),
        LvResult::Invalid
    );
    let mut malformed = DrawBuf {
        header: ImageHeader::default(),
        data_size: u32::MAX,
        storage: BufStorage::Owned(vec![0u64; 1].into_boxed_slice()),
        data_offset: usize::MAX,
    };
    assert_eq!(
        ui.snapshot_reshape_draw_buf(obj, &mut malformed),
        LvResult::Invalid
    );
    malformed.data_offset = 0;
    ui.obj_set_size(obj, 5, 5);
    assert_eq!(
        ui.snapshot_reshape_draw_buf(obj, &mut malformed),
        LvResult::Invalid
    );
    ui.obj_delete(obj);
    assert!(ui.snapshot_take(obj, ColorFormat::Argb8888).is_none());
    assert_eq!(ui.snapshot_reshape_draw_buf(obj, &mut buf), LvResult::Invalid);
}

#[test]
fn transparent_reuse_clears_previous_pixels() {
    let mut display = TestDisplay::with_resolution(32, 24).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 3, 4, 16, 16, Color::WHITE);
    let mut buf = ui.snapshot_take(obj, ColorFormat::Argb8888).unwrap();
    assert_eq!(pixel(&buf, 0, 0)[3], 255);
    ui.obj_set_style_radius(obj, 8, SELECTOR);
    assert_eq!(
        ui.snapshot_take_to_draw_buf(obj, ColorFormat::Argb8888, &mut buf),
        LvResult::Ok
    );
    assert_eq!(pixel(&buf, 0, 0), [0; 4]);
    assert_eq!(pixel(&buf, 8, 8), [255; 4]);
}

#[test]
fn covering_child_skips_older_objects_but_draws_younger_siblings_and_parent_post() {
    let mut display = TestDisplay::with_resolution(40, 32).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 7, 5, 20, 20, Color::WHITE);
    let old = rectangle(ui, obj, 0, 0, 20, 20, Color::hex(0xff0000));
    rectangle(ui, obj, 0, 0, 20, 20, Color::hex(0x0000ff));
    rectangle(ui, obj, 10, 0, 10, 20, Color::hex(0x00ff00));
    rectangle(ui, screen, 0, 0, 40, 32, Color::hex(0xff00ff));
    let draws = Rc::new(Cell::new(0));
    let posts = Rc::new(Cell::new(0));
    for target in [obj, old] {
        let count = draws.clone();
        ui.obj_add_event_cb(
            target,
            Rc::new(move |_, _| count.set(count.get() + 1)),
            EventCode::DRAW_MAIN,
            None,
        );
    }
    let count = posts.clone();
    ui.obj_add_event_cb(
        obj,
        Rc::new(move |_, _| count.set(count.get() + 1)),
        EventCode::DRAW_POST,
        None,
    );
    let buf = ui.snapshot_take(obj, ColorFormat::Argb8888).unwrap();
    assert_eq!(pixel(&buf, 3, 3), [255, 0, 0, 255]);
    assert_eq!(pixel(&buf, 13, 3), [0, 255, 0, 255]);
    assert_eq!(draws.get(), 0);
    assert_eq!(posts.get(), 1);
}

#[test]
fn previous_refresh_display_and_pending_queue_are_restored() {
    let mut display = TestDisplay::with_resolution(32, 24).unwrap();
    let disp = display.display();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 0, 0, 8, 8, Color::WHITE);
    ui.disp_refresh = Some(disp);
    let old_layer = ui.display(disp).unwrap().layer_head.unwrap();
    ui.draw.task_running = true;
    ui.draw_fill(old_layer, &FillDsc::new(), &Area::new(0, 0, 3, 3));
    ui.draw.task_running = false;
    let old_state = ui.draw.layer(old_layer).unwrap().tasks[0].state;
    let layer_count = ui.draw.layers.len();
    let bytes = ui.display(disp).unwrap().buf_1.as_ref().unwrap().data().to_vec();
    assert!(ui.snapshot_take(obj, ColorFormat::Argb8888).is_some());
    assert_eq!(ui.disp_refresh, Some(disp));
    assert_eq!(ui.display(disp).unwrap().layer_head, Some(old_layer));
    assert_eq!(ui.draw.layer(old_layer).unwrap().tasks.len(), 1);
    assert_eq!(ui.draw.layer(old_layer).unwrap().tasks[0].state, old_state);
    assert_eq!(ui.draw.layers.len(), layer_count);
    assert_eq!(ui.display(disp).unwrap().buf_1.as_ref().unwrap().data(), bytes);
    ui.draw_dispatch();
    ui.draw_dispatch();
    assert!(ui.draw.layer(old_layer).unwrap().tasks.is_empty());
}

#[test]
fn draw_callbacks_can_delete_object_display_or_deinitialize_ui() {
    for action in 0..3 {
        let mut display = TestDisplay::with_resolution(32, 24).unwrap();
        let disp = display.display();
        let screen = display.screen().unwrap();
        let ui = &mut display.ui;
        let obj = rectangle(ui, screen, 0, 0, 8, 8, Color::WHITE);
        let mut buf = ui.snapshot_create_draw_buf(obj, ColorFormat::Argb8888).unwrap();
        let original = buf.data().as_ptr();
        ui.obj_add_event_cb(
            obj,
            Rc::new(move |ui, _| match action {
                0 => ui.obj_delete(obj),
                1 => ui.display_delete(disp),
                _ => ui.deinit(),
            }),
            EventCode::DRAW_MAIN,
            None,
        );
        assert_eq!(
            ui.snapshot_take_to_draw_buf(obj, ColorFormat::Argb8888, &mut buf),
            LvResult::Invalid
        );
        assert_eq!(
            buf.data().as_ptr(),
            original,
            "action {action} must return caller storage"
        );
        assert_eq!(ui.draw.used_memory_for_layers, 0);
        assert!(ui.draw.layers.iter().all(|(_, layer)| layer.tasks.is_empty()));
        assert!(ui.disp_refresh.is_none());
    }
}

#[test]
fn layout_and_cover_callbacks_can_delete_the_snapshot_target() {
    for code in [EventCode::SIZE_CHANGED, EventCode::COVER_CHECK] {
        let mut display = TestDisplay::with_resolution(32, 24).unwrap();
        let screen = display.screen().unwrap();
        let ui = &mut display.ui;
        let obj = rectangle(ui, screen, 0, 0, 8, 8, Color::WHITE);
        ui.obj_update_layout(obj);
        ui.obj_add_event_cb(obj, Rc::new(move |ui, _| ui.obj_delete(obj)), code, None);
        if code == EventCode::SIZE_CHANGED {
            ui.obj_set_size(obj, 10, 10);
        }
        assert!(ui.snapshot_take(obj, ColorFormat::Argb8888).is_none());
        assert!(!ui.obj_is_valid(obj));
    }
}

#[test]
fn nested_snapshot_restores_outer_render_chain() {
    let mut display = TestDisplay::with_resolution(32, 24).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let outer = rectangle(ui, screen, 0, 0, 8, 8, Color::hex(0xff0000));
    let inner = rectangle(ui, screen, 12, 0, 8, 8, Color::hex(0x0000ff));
    let called = Rc::new(Cell::new(false));
    let flag = called.clone();
    ui.obj_add_event_cb(
        outer,
        Rc::new(move |ui, _| {
            let buf = ui.snapshot_take(inner, ColorFormat::Argb8888).unwrap();
            assert_eq!(pixel(&buf, 3, 3), [255, 0, 0, 255]);
            flag.set(true);
        }),
        EventCode::DRAW_MAIN,
        None,
    );
    let layer_count = ui.draw.layers.len();
    let buf = ui.snapshot_take(outer, ColorFormat::Argb8888).unwrap();
    assert_eq!(pixel(&buf, 3, 3), [0, 0, 255, 255]);
    assert!(called.get());
    assert_eq!(ui.draw.layers.len(), layer_count);
}

#[test]
fn independent_root_created_by_a_callback_keeps_its_pending_child_composite() {
    let mut display = TestDisplay::with_resolution(32, 24).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 0, 0, 8, 8, Color::WHITE);
    let created = Rc::new(Cell::new(None));
    let result = created.clone();
    ui.obj_add_event_cb(
        obj,
        Rc::new(move |ui, _| {
            let area = Area::new(0, 0, 3, 3);
            let root = ui.draw_layer_create(None, ColorFormat::Argb8888, &area);
            let child = ui.draw_layer_create(Some(root), ColorFormat::Argb8888, &area);
            let mut fill = FillDsc::new();
            fill.color = Color::hex(0x123456);
            ui.draw_fill(child, &fill, &area);
            let mut composite = DrawImageDsc::new();
            composite.src = ImageSrc::Layer(child);
            // This dispatch renders the child and makes its parent's layer
            // composite ready. The independent root is revisited next time
            // its owner dispatches, after snapshot has already completed.
            ui.draw_layer(root, &composite, &area);
            result.set(Some((root, child)));
        }),
        EventCode::DRAW_MAIN,
        None,
    );
    let original_layers = ui.draw.layers.len();
    assert!(ui.snapshot_take(obj, ColorFormat::Argb8888).is_some());
    let (root, child) = created.get().unwrap();
    assert_eq!(ui.draw.layers.len(), original_layers + 2);
    let independent = ui.draw.layer(root).expect("callback-created root must survive");
    assert!(independent.parent.is_none());
    assert_eq!(independent.tasks.len(), 1);
    assert_eq!(independent.tasks[0].state, DrawTaskState::Waiting);
    assert_eq!(ui.draw.layer(child).unwrap().parent, Some(root));

    for _ in 0..4 {
        ui.draw_dispatch();
    }
    assert!(ui.draw.layer(root).unwrap().tasks.is_empty());
    assert!(ui.draw.layer(child).is_none());
    assert_eq!(
        pixel(ui.draw.layer(root).unwrap().draw_buf.as_ref().unwrap(), 0, 0),
        [0x56, 0x34, 0x12, 255]
    );
    ui.draw_layer_delete(root);
    assert_eq!(ui.draw.layers.len(), original_layers);
    assert_eq!(ui.draw.used_memory_for_layers, 0);
}

#[test]
fn software_destination_formats_render_real_pixels() {
    let mut display = TestDisplay::with_resolution(16, 16).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 0, 0, 8, 8, Color::WHITE);
    for (cf, expected) in [
        (ColorFormat::A8, &[255][..]),
        (ColorFormat::L8, &[255][..]),
        (ColorFormat::Rgb565, &[255, 255][..]),
        (ColorFormat::Rgb888, &[255, 255, 255][..]),
        (ColorFormat::Xrgb8888, &[255, 255, 255][..]),
        (ColorFormat::Argb8888, &[255, 255, 255, 255][..]),
    ] {
        let buf = ui.snapshot_take(obj, cf).unwrap();
        assert_eq!(&buf.data()[..expected.len()], expected, "{cf:?}");
        assert_eq!(buf.header.cf, cf);
    }
    // The API accepts the same packed ARGB formats as C, but this SW backend
    // has no destination kernels for them. Allocation is checked separately;
    // successful rendering in these formats is deliberately not asserted.
    for cf in [
        ColorFormat::Argb8565,
        ColorFormat::Argb2222,
        ColorFormat::Argb4444,
        ColorFormat::Argb1555,
        ColorFormat::I1,
    ] {
        let buf = ui.snapshot_create_draw_buf(obj, cf).unwrap();
        assert_eq!(buf.header.cf, cf);
        assert!(buf.header.stride > 0);
    }
}

struct StalledUnit {
    dispatches: Rc<Cell<u32>>,
    events: Rc<RefCell<Vec<EventCode>>>,
}

impl DrawUnit for StalledUnit {
    fn name(&self) -> &'static str {
        "snapshot-stall"
    }
    fn evaluate(&mut self, task: &mut DrawTask) -> i32 {
        task.preferred_draw_unit_id = 1;
        0
    }
    fn dispatch(&mut self, _ctx: &mut DrawCtx, _layer: LayerId) -> i32 {
        self.dispatches.set(self.dispatches.get() + 1);
        DRAW_UNIT_IDLE
    }
    fn on_event(&mut self, code: EventCode, _param: &mut EventParam<'_>) {
        self.events.borrow_mut().push(code);
    }
}

#[test]
fn stalled_snapshot_releases_nested_layers_and_returns_reusable_storage() {
    let mut display = TestDisplay::with_resolution(32, 24).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 0, 0, 16, 16, Color::WHITE);
    let child = rectangle(ui, obj, 0, 0, 8, 8, Color::BLACK);
    ui.obj_set_style_opa_layered(child, 128, SELECTOR);
    let mut buf = ui.snapshot_create_draw_buf(obj, ColorFormat::Argb8888).unwrap();
    let original = buf.data().as_ptr();
    let layer_count = ui.draw.layers.len();
    let dispatches = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    ui.draw_units.clear();
    ui.draw_create_unit(Box::new(StalledUnit {
        dispatches: dispatches.clone(),
        events: events.clone(),
    }));
    assert_eq!(
        ui.snapshot_take_to_draw_buf(obj, ColorFormat::Argb8888, &mut buf),
        LvResult::Invalid
    );
    assert!(dispatches.get() > 512);
    assert_eq!(buf.data().as_ptr(), original);
    assert_eq!(ui.draw.layers.len(), layer_count);
    assert_eq!(ui.draw.used_memory_for_layers, 0);
    let events = events.borrow();
    assert_eq!(
        events.iter().filter(|&&e| e == EventCode::CHILD_CREATED).count(),
        2
    );
    assert_eq!(
        events.iter().filter(|&&e| e == EventCode::CHILD_DELETED).count(),
        2
    );
    assert!(!events.contains(&EventCode::SCREEN_LOAD_START));
}

#[test]
fn transformed_descendant_larger_than_display_completes_without_leaking() {
    let mut display = TestDisplay::with_resolution(10, 10).unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj = rectangle(ui, screen, 0, 0, 300, 300, Color::WHITE);
    let child = rectangle(ui, obj, 20, 20, 200, 200, Color::hex(0x00ff00));
    ui.obj_set_style_transform_rotation(child, 450, SELECTOR);
    let layer_count = ui.draw.layers.len();
    for _ in 0..3 {
        let buf = ui.snapshot_take(obj, ColorFormat::Argb8888).unwrap();
        assert_eq!((buf.header.w, buf.header.h), (300, 300));
        assert!(buf.data().as_chunks::<4>().0.contains(&[0, 255, 0, 255]));
        assert_eq!(ui.draw.layers.len(), layer_count);
        assert_eq!(ui.draw.used_memory_for_layers, 0);
    }
}

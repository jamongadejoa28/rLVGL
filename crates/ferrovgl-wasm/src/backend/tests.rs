use super::*;

use ferrovgl_core::group::{KEY_BACKSPACE, KEY_ENTER, KEY_NEXT};
use ferrovgl_core::{Color, EventCode, ObjFlags, Point, State, StyleSelector};
use ferrovgl_widgets::{Button, Textarea};

use crate::gallery;
use crate::keys::pack_utf8;

/// An RGBA "canvas" that records every `putImageData`.
#[derive(Default)]
struct Canvas {
    w: usize,
    pixels: Vec<u8>,
    puts: Vec<(i32, i32, u32, u32)>,
}

impl Canvas {
    fn px(&self, x: usize, y: usize) -> [u8; 4] {
        let o = (y * self.w + x) * 4;
        self.pixels[o..o + 4].try_into().unwrap()
    }
}

struct Recorder(Rc<RefCell<Canvas>>);

impl Surface for Recorder {
    fn put_rgba(&mut self, x: i32, y: i32, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
        let mut c = self.0.borrow_mut();
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        let cw = c.w;
        for row in 0..h as usize {
            let dst = ((y as usize + row) * cw + x as usize) * 4;
            let src = row * w as usize * 4;
            c.pixels[dst..dst + w as usize * 4].copy_from_slice(&rgba[src..src + w as usize * 4]);
        }
        c.puts.push((x, y, w, h));
        Ok(())
    }
}

fn backend(config: BackendConfig) -> (Backend, Rc<RefCell<Canvas>>) {
    let canvas = Rc::new(RefCell::new(Canvas {
        w: config.width as usize,
        pixels: vec![0; (config.width * config.height * 4) as usize],
        puts: Vec::new(),
    }));
    let b = Backend::new(config, Box::new(Recorder(canvas.clone()))).unwrap();
    (b, canvas)
}

fn plain(width: u32, height: u32, cf: ColorFormat, mode: RenderMode) -> BackendConfig {
    BackendConfig {
        width,
        height,
        color_format: cf,
        render_mode: mode,
        theme_init: None,
        ..BackendConfig::default()
    }
}

/// Runs frames 40 ms apart (more than `LV_DEF_REFR_PERIOD`) until `t` ms.
fn run(b: &mut Backend, from: &mut f64, until: f64) {
    while *from <= until {
        assert!(b.frame(*from, []).unwrap());
        *from += 40.0;
    }
}

const MODES: [RenderMode; 3] = [RenderMode::Partial, RenderMode::Direct, RenderMode::Full];

#[test]
fn config_validation() {
    let surface = || -> Box<dyn Surface> { Box::new(Recorder(Rc::default())) };
    for (w, h) in [(0, 1), (1, 0), (65536, 1), (16384, 1)] {
        assert!(Backend::new(plain(w, h, ColorFormat::Xrgb8888, RenderMode::Partial), surface()).is_err());
    }
    assert!(Backend::new(plain(4, 4, ColorFormat::I1, RenderMode::Partial), surface()).is_err());
}

#[test]
fn first_frame_fills_the_canvas_in_every_mode_and_format() {
    for mode in MODES {
        for cf in [
            ColorFormat::Xrgb8888,
            ColorFormat::Argb8888,
            ColorFormat::Rgb888,
            ColorFormat::Rgb565,
        ] {
            let mut config = plain(37, 23, cf, mode);
            config.partial_rows = 5;
            let (mut b, canvas) = backend(config);
            let screen = b.screen().unwrap();
            b.ui.obj_set_style_bg_color(screen, Color::hex(0x1084FF), StyleSelector::DEFAULT);
            b.ui.obj_set_style_bg_opa(screen, 255, StyleSelector::DEFAULT);
            let mut t = 0.0;
            run(&mut b, &mut t, 40.0);
            let c = canvas.borrow();
            let want = if cf == ColorFormat::Rgb565 {
                // 0x1084FF -> RGB565 (2, 33, 31) -> expanded.
                [((2 * 2106) >> 8) as u8, ((33 * 1037) >> 8) as u8, 255, 255]
            } else {
                [0x10, 0x84, 0xFF, 0xFF]
            };
            for y in 0..23 {
                for x in 0..37 {
                    assert_eq!(c.px(x, y), want, "{mode:?} {cf:?} ({x}, {y})");
                }
            }
            match mode {
                // 23 rows in 5-row chunks.
                RenderMode::Partial => {
                    assert_eq!(c.puts.len(), 5, "{cf:?}");
                    assert!(c.puts.iter().all(|&(x, _, w, h)| x == 0 && w == 37 && h <= 5));
                }
                _ => assert_eq!(c.puts, [(0, 0, 37, 23)], "{mode:?} {cf:?}"),
            }
            assert_eq!(b.flushed_areas(), c.puts.len() as u64);
        }
    }
}

#[test]
fn invalidated_area_only_is_put() {
    for mode in MODES {
        let (mut b, canvas) = backend(plain(64, 40, ColorFormat::Xrgb8888, mode));
        let screen = b.screen().unwrap();
        let sel = StyleSelector::DEFAULT;
        b.ui.obj_set_style_bg_color(screen, Color::hex(0x000000), sel);
        b.ui.obj_set_style_bg_opa(screen, 255, sel);
        let mut t = 0.0;
        run(&mut b, &mut t, 40.0);
        let obj = b.ui.obj_create(Some(screen));
        b.ui.obj_set_pos(obj, 20, 10);
        b.ui.obj_set_size(obj, 8, 6);
        b.ui.obj_set_style_bg_color(obj, Color::hex(0xFF0000), sel);
        b.ui.obj_set_style_bg_opa(obj, 255, sel);
        canvas.borrow_mut().puts.clear();
        run(&mut b, &mut t, 80.0);
        let c = canvas.borrow();
        if mode == RenderMode::Full {
            assert_eq!(c.puts, [(0, 0, 64, 40)]);
        } else {
            // `lv_obj_set_size()` invalidates the object at its creation
            // position (0, 0) before it moves to (20, 10).
            assert_eq!(c.puts, [(0, 0, 8, 6), (20, 10, 8, 6)], "{mode:?}");
        }
        assert_eq!(c.px(20, 10), [255, 0, 0, 255]);
        assert_eq!(c.px(27, 15), [255, 0, 0, 255]);
        assert_eq!(c.px(19, 10), [0, 0, 0, 255]);
        assert_eq!(c.px(28, 16), [0, 0, 0, 255]);
        // Nothing changes: nothing is flushed.
        drop(c);
        canvas.borrow_mut().puts.clear();
        run(&mut b, &mut t, 160.0);
        assert!(canvas.borrow().puts.is_empty());
    }
}

#[test]
fn double_buffered_direct_mode_keeps_both_buffers_in_sync() {
    let mut config = plain(16, 8, ColorFormat::Xrgb8888, RenderMode::Direct);
    config.double_buffered = true;
    let (mut b, canvas) = backend(config);
    let screen = b.screen().unwrap();
    let sel = StyleSelector::DEFAULT;
    b.ui.obj_set_style_bg_color(screen, Color::hex(0x204060), sel);
    b.ui.obj_set_style_bg_opa(screen, 255, sel);
    let mut t = 0.0;
    run(&mut b, &mut t, 40.0);
    for (i, x) in [2, 9].into_iter().enumerate() {
        let obj = b.ui.obj_create(Some(screen));
        b.ui.obj_set_pos(obj, x, 2);
        b.ui.obj_set_size(obj, 3, 3);
        b.ui.obj_set_style_bg_color(obj, Color::hex(0xFFFFFF), sel);
        b.ui.obj_set_style_bg_opa(obj, 255, sel);
        run(&mut b, &mut t, 80.0 + 40.0 * i as f64);
    }
    let c = canvas.borrow();
    assert_eq!(c.px(2, 2), [255, 255, 255, 255]);
    assert_eq!(c.px(10, 3), [255, 255, 255, 255]);
    assert_eq!(c.px(0, 0), [0x20, 0x40, 0x60, 255]);
}

#[test]
fn gallery_renders_identically_in_every_mode() {
    let mut results = Vec::new();
    for mode in MODES {
        let config = BackendConfig {
            render_mode: mode,
            ..BackendConfig::default()
        };
        let (mut b, canvas) = backend(config);
        gallery::widget_gallery(&mut b.ui);
        // Stop the spinner so the frames compare: render once at t = 0.
        b.refresh_now().unwrap();
        let screen = b.screen().unwrap();
        assert_eq!(b.ui.obj_get_child_count(screen), gallery::WIDGETS.len() as u32);
        // Button, slider, 2 switches, 2 checkboxes, button matrix and text
        // area (arc and the others inherit `LV_OBJ_CLASS_GROUP_DEF_FALSE`).
        assert_eq!(b.ui.group_get_obj_count(b.group()), 8);
        results.push(canvas.borrow().pixels.clone());
        b.ui.deinit();
    }
    assert!(results[0].as_chunks::<4>().0.iter().any(|p| p != &[0, 0, 0, 0]));
    assert!(results[0] == results[1], "partial and direct differ");
    assert!(results[0] == results[2], "partial and full differ");
}

#[test]
fn pointer_click_and_coordinates() {
    let (mut b, _canvas) = backend(plain(120, 60, ColorFormat::Xrgb8888, RenderMode::Partial));
    let screen = b.screen().unwrap();
    let button = Button::create(&mut b.ui, screen).obj();
    b.ui.obj_set_pos(button, 10, 10);
    b.ui.obj_set_size(button, 40, 30);
    let clicks = Rc::new(std::cell::Cell::new(0));
    let c = clicks.clone();
    b.ui.obj_add_event_cb(
        button,
        Rc::new(move |_, _| c.set(c.get() + 1)),
        EventCode::CLICKED,
        None,
    );
    let mut t = 0.0;
    run(&mut b, &mut t, 40.0);
    // Down and up inside the button: a click.
    b.frame(t, [InputEvent::PointerDown(Point::new(20, 20))]).unwrap();
    t += 40.0;
    b.frame(t, [InputEvent::PointerUp]).unwrap();
    assert_eq!(clicks.get(), 1);
    // Down inside, move out, leave: `LV_OBJ_FLAG_PRESS_LOCK` keeps the
    // button pressed, so the leave release still clicks it.
    t += 40.0;
    b.frame(
        t,
        [
            InputEvent::PointerDown(Point::new(20, 20)),
            InputEvent::PointerMove(Point::new(100, 50)),
        ],
    )
    .unwrap();
    assert_eq!(clicks.get(), 1);
    t += 40.0;
    b.frame(t, [InputEvent::PointerLeave]).unwrap();
    assert_eq!(clicks.get(), 2);
    // Moving without a press does not click.
    t += 40.0;
    b.frame(
        t,
        [InputEvent::PointerMove(Point::new(20, 20)), InputEvent::PointerUp],
    )
    .unwrap();
    assert_eq!(clicks.get(), 2);
    // The middle button is the encoder's: it clicks the focused object of
    // the default group (the button) rather than the object under the pointer.
    t += 40.0;
    b.frame(
        t,
        [
            InputEvent::PointerMove(Point::new(20, 20)),
            InputEvent::EncoderButton(true),
            InputEvent::EncoderButton(false),
        ],
    )
    .unwrap();
    assert_eq!(clicks.get(), 3);
    assert_eq!(b.ui.group_get_focused(b.group()), Some(button));
}

#[test]
fn keypad_and_encoder_use_the_default_group() {
    let (mut b, _canvas) = backend(plain(160, 80, ColorFormat::Xrgb8888, RenderMode::Partial));
    let screen = b.screen().unwrap();
    // Created after the backend: the widgets join the default group.
    let first = Button::create(&mut b.ui, screen).obj();
    let ta = Textarea::create(&mut b.ui, screen);
    for (obj, x) in [(first, 0), (ta.obj(), 60)] {
        b.ui.obj_set_pos(obj, x, 0);
        b.ui.obj_set_size(obj, 50, 40);
        b.ui.obj_remove_flag(obj, ObjFlags::SCROLLABLE);
    }
    assert_eq!(b.ui.group_get_obj_count(b.group()), 2);
    let clicks = Rc::new(std::cell::Cell::new(0));
    let c = clicks.clone();
    b.ui.obj_add_event_cb(
        first,
        Rc::new(move |_, _| c.set(c.get() + 1)),
        EventCode::CLICKED,
        None,
    );
    let mut t = 0.0;
    run(&mut b, &mut t, 40.0);
    assert_eq!(b.ui.group_get_focused(b.group()), Some(first));
    b.handle_input(InputEvent::Key(KEY_ENTER));
    assert_eq!(clicks.get(), 1);
    b.handle_input(InputEvent::Key(KEY_NEXT));
    assert_eq!(b.ui.group_get_focused(b.group()), Some(ta.obj()));
    for c in ['h', 'é', '!'] {
        b.handle_input(InputEvent::Key(pack_utf8(c)));
    }
    b.handle_input(InputEvent::Key(KEY_BACKSPACE));
    assert_eq!(ta.text(&b.ui), "hé");
    // One wheel step back to the button; the encoder press clicks it.
    b.frame(t, [InputEvent::Wheel(-1)]).unwrap();
    assert_eq!(b.ui.group_get_focused(b.group()), Some(first));
    b.handle_input(InputEvent::EncoderButton(true));
    b.handle_input(InputEvent::EncoderButton(false));
    assert_eq!(clicks.get(), 2);
    assert!(b.ui.obj_has_state(first, State::FOCUSED));
    // Blur releases a held encoder button (which completes that click),
    // so no press stays stuck.
    b.handle_input(InputEvent::EncoderButton(true));
    b.handle_input(InputEvent::Blur);
    assert_eq!(clicks.get(), 3);
    b.handle_input(InputEvent::Blur);
    assert_eq!(clicks.get(), 3);
}

#[test]
fn ticks_follow_frame_timestamps() {
    let (mut b, _canvas) = backend(plain(8, 8, ColorFormat::Xrgb8888, RenderMode::Partial));
    let start = b.ui.tick_get();
    b.frame(1000.25, []).unwrap();
    assert_eq!(b.ui.tick_get(), start);
    b.frame(1016.9, []).unwrap();
    assert_eq!(b.ui.tick_get(), start + 16);
    b.frame(1033.5, []).unwrap();
    assert_eq!(b.ui.tick_get(), start + 33);
}

#[test]
fn display_changes_are_followed_or_rejected() {
    // The application switches render mode at runtime: followed.
    let (mut b, canvas) = backend(plain(20, 10, ColorFormat::Xrgb8888, RenderMode::Direct));
    let display = b.display();
    let screen = b.screen().unwrap();
    b.ui.obj_set_style_bg_color(screen, Color::hex(0x00FF00), StyleSelector::DEFAULT);
    b.ui.obj_set_style_bg_opa(screen, 255, StyleSelector::DEFAULT);
    b.ui.display_set_render_mode(display, RenderMode::Partial);
    let mut t = 0.0;
    run(&mut b, &mut t, 40.0);
    assert_eq!(b.flush_config().render_mode, RenderMode::Partial);
    assert_eq!(canvas.borrow().px(19, 9), [0, 255, 0, 255]);

    // Rotation cannot be shown: the frame reports an error.
    b.ui.display_set_rotation(display, Rotation::R90);
    let _ = b.ui.obj_invalidate(screen);
    let err = (0..3)
        .find_map(|_| {
            t += 40.0;
            b.frame(t, []).err()
        })
        .unwrap();
    assert!(err.contains("rotation"), "{err}");

    // Deleting the display from a timer ends the loop.
    let (mut b, _canvas) = backend(plain(8, 8, ColorFormat::Xrgb8888, RenderMode::Partial));
    let display = b.display();
    let timer =
        b.ui.timer_create(Rc::new(move |ui, _| ui.display_delete(display)), 0, None);
    b.ui.timer_set_repeat_count(timer, 1);
    assert!(!b.frame(0.0, []).unwrap());
    assert!(!b.frame(10.0, [InputEvent::PointerUp]).unwrap());
    b.refresh_now().unwrap();
}

#[test]
fn scenes() {
    assert_eq!(gallery::scene_names()[0], gallery::GALLERY);
    assert_eq!(
        gallery::scene_names().len(),
        1 + ferrovgl_examples::EXAMPLES.len()
    );
    assert!(gallery::find_scene("gallery").is_some());
    assert!(gallery::find_scene("lv_example_does_not_exist").is_none());
    for example in ferrovgl_examples::EXAMPLES {
        assert!(gallery::find_scene(example.name).is_some());
        let (mut b, _canvas) = backend(BackendConfig::default());
        (example.run)(&mut b.ui);
        b.refresh_now().unwrap();
        b.ui.deinit();
    }
}

/// `FERROVGL_WASM_DUMP=out.ppm cargo test -p ferrovgl-wasm dump_gallery`
/// writes what the demo page shows after 400 ms, for a visual check.
#[test]
fn dump_gallery() {
    let Some(path) = std::env::var_os("FERROVGL_WASM_DUMP") else {
        return;
    };
    let (mut b, canvas) = backend(BackendConfig::default());
    gallery::widget_gallery(&mut b.ui);
    let mut t = 0.0;
    run(&mut b, &mut t, 400.0);
    let c = canvas.borrow();
    let mut out = format!("P6 {} 480 255\n", c.w).into_bytes();
    for p in c.pixels.as_chunks::<4>().0 {
        out.extend_from_slice(&p[..3]);
    }
    std::fs::write(path, out).unwrap();
}

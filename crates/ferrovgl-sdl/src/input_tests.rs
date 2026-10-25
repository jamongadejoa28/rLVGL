use super::*;
use ferrovgl_core::{EventCode, ObjFlags, State};
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::{MouseButton, MouseWheelDirection};

#[test]
fn dummy_sdl_events_drive_pointer_keypad_encoder_and_callback_teardown() {
    if std::env::var_os("FERROVGL_SDL_INPUT_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .env("SDL_VIDEODRIVER", "dummy")
            .env("FERROVGL_SDL_INPUT_CHILD", "1")
            .args([
                "--exact",
                "input_tests::dummy_sdl_events_drive_pointer_keypad_encoder_and_callback_teardown",
                "--nocapture",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let mut window = SdlDisplay::new("SDL input", 120, 60, UiConfig::default(), None).unwrap();
    let screen = window.screen().unwrap();
    let a = window.ui.obj_create(Some(screen));
    let b = window.ui.obj_create(Some(screen));
    for (obj, x) in [(a, 5), (b, 65)] {
        window.ui.obj_set_pos(obj, x, 5);
        window.ui.obj_set_size(obj, 50, 40);
        window.ui.obj_remove_flag(obj, ObjFlags::SCROLLABLE);
    }
    let group = window.ui.group_create();
    window.ui.group_add_obj(group, a);
    window.ui.group_add_obj(group, b);
    window.set_group(Some(group));
    let clicked = Rc::new(RefCell::new(0));
    let clicks = clicked.clone();
    window.ui.obj_add_event_cb(
        a,
        Rc::new(move |_, _| *clicks.borrow_mut() += 1),
        EventCode::CLICKED,
        None,
    );
    assert!(window.step().unwrap());
    let window_id = window.presenter.borrow().canvas.window().id();
    let events = window.sdl.event().unwrap();
    events
        .push_event(Event::MouseButtonDown {
            timestamp: 0,
            window_id,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: 20,
            y: 20,
        })
        .unwrap();
    assert!(window.step().unwrap());
    assert!(window.ui.obj_has_state(a, State::PRESSED));
    events
        .push_event(Event::MouseButtonUp {
            timestamp: 0,
            window_id,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: 20,
            y: 20,
        })
        .unwrap();
    assert!(window.step().unwrap());
    assert_eq!(*clicked.borrow(), 1);
    assert!(!window.ui.obj_has_state(a, State::PRESSED));

    // Other windows' events must not affect this UI.
    events
        .push_event(Event::MouseButtonDown {
            timestamp: 0,
            window_id: window_id + 100,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: 20,
            y: 20,
        })
        .unwrap();
    window.step().unwrap();
    assert!(!window.ui.obj_has_state(a, State::PRESSED));
    for (modifiers, expected) in [(Mod::NOMOD, b), (Mod::LSHIFTMOD, a)] {
        events
            .push_event(Event::KeyDown {
                timestamp: 0,
                window_id,
                keycode: Some(Keycode::Tab),
                scancode: None,
                keymod: modifiers,
                repeat: false,
            })
            .unwrap();
        window.step().unwrap();
        assert_eq!(window.ui.group_get_focused(group), Some(expected));
    }
    events
        .push_event(Event::MouseWheel {
            timestamp: 0,
            window_id,
            which: 0,
            x: 0,
            y: -1,
            direction: MouseWheelDirection::Normal,
            precise_x: 0.0,
            precise_y: -1.0,
            mouse_x: 20,
            mouse_y: 20,
        })
        .unwrap();
    window.step().unwrap();
    assert_eq!(window.ui.group_get_focused(group), Some(b));
    let keys = Rc::new(RefCell::new(Vec::new()));
    let recorded = keys.clone();
    window.ui.obj_add_event_cb(
        b,
        Rc::new(move |_, e| recorded.borrow_mut().push(e.key())),
        EventCode::KEY,
        None,
    );
    // sdl2 0.37 cannot serialize TextInput in push_event; use the same
    // adapter directly for this event while all other checks use SDL's queue.
    window.inputs.handle(
        &mut window.ui,
        &Event::TextInput {
            timestamp: 0,
            window_id,
            text: "A한".into(),
        },
        window_id,
    );
    let mut korean = [0; 4];
    '한'.encode_utf8(&mut korean);
    assert_eq!(*keys.borrow(), [u32::from(b'A'), u32::from_ne_bytes(korean)]);

    // Losing the window releases a press rather than leaving PRESSED stuck.
    events
        .push_event(Event::MouseButtonDown {
            timestamp: 0,
            window_id,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: 20,
            y: 20,
        })
        .unwrap();
    window.step().unwrap();
    events
        .push_event(Event::Window {
            timestamp: 0,
            window_id,
            win_event: WindowEvent::FocusLost,
        })
        .unwrap();
    window.step().unwrap();
    assert!(!window.ui.obj_has_state(a, State::PRESSED));

    // Input callbacks may deinitialize everything while the event pump runs.
    window
        .ui
        .obj_add_event_cb(a, Rc::new(|ui, _| ui.deinit()), EventCode::PRESSED, None);
    events
        .push_event(Event::MouseButtonDown {
            timestamp: 0,
            window_id,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: 20,
            y: 20,
        })
        .unwrap();
    assert!(!window.step().unwrap());
    assert!(window.ui.indevs.is_empty());
}

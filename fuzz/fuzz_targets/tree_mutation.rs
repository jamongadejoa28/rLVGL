#![no_main]

use ferrovgl_core::{Obj, ObjId, Ui, UiConfig};
use libfuzzer_sys::fuzz_target;

fn ancestor(ui: &Ui, parent: Obj, mut child: Obj) -> bool {
    for _ in 0..=ui.objs.len() {
        if child == parent {
            return true;
        }
        let Some(next) = ui.obj_get_parent(child) else {
            return false;
        };
        child = next;
    }
    panic!("cycle in object tree");
}

fn check_tree(ui: &Ui) {
    for (key, object) in ui.objs.iter() {
        let handle = Obj(ObjId(key));
        if let Some(parent) = object.parent {
            assert!(ui.obj_is_valid(Obj(parent)));
            assert_eq!(
                ui.obj_children(Obj(parent))
                    .iter()
                    .filter(|&&id| id == handle.id())
                    .count(),
                1
            );
        }
        for (index, &child) in ui.obj_children(handle).iter().enumerate() {
            assert!(ui.obj_is_valid(Obj(child)));
            assert_eq!(ui.obj_get_parent(Obj(child)), Some(handle));
            assert_eq!(ui.obj_get_child(handle, index as i32), Some(Obj(child)));
            assert!(!ancestor(ui, Obj(child), handle));
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let mut ui = Ui::new(UiConfig::default());
    let display = ui.display_create(160, 120);
    let root = ui.display_get_screen_active(Some(display)).unwrap();
    let mut objects = vec![root];
    // These are valid object operations; cycles and swapping ancestor pairs
    // are excluded because the corresponding C caller must not request them.
    for op in data.chunks_exact(4).take(256) {
        objects.retain(|&obj| ui.obj_is_valid(obj));
        let a = objects[op[1] as usize % objects.len()];
        let b = objects[op[2] as usize % objects.len()];
        match op[0] % 8 {
            0 | 1 if objects.len() < 64 => objects.push(ui.obj_create(Some(a))),
            2 if a != root => ui.obj_delete(a),
            3 if a != root && !ancestor(&ui, a, b) => ui.obj_set_parent(a, b),
            4 if a != root && b != root && !ancestor(&ui, a, b) && !ancestor(&ui, b, a) => ui.obj_swap(a, b),
            5 if a != root => ui.obj_move_to_index(a, i32::from(op[3] as i8)),
            6 => ui.obj_clean(a),
            7 if a != root => {
                ui.obj_delete_async(a);
                ui.tick_inc(1);
                ui.timer_handler();
            }
            _ => {}
        }
        check_tree(&ui);
    }
    ui.deinit();
    assert!(ui.objs.is_empty());
});

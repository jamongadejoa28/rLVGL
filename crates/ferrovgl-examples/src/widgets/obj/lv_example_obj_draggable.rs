//! Port of `examples/widgets/obj/lv_example_obj_draggable.c`.

use alloc::rc::Rc;

use ferrovgl_core::{Event, EventCode, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `static void drag_event_handler(lv_event_t * e)`
fn drag_event_handler(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(obj) = e.target_obj() else { return };

    let Some(indev) = ui.indev_active() else { return };

    let vect = ui.indev_get_vect(indev);

    let x = ui.obj_get_x_aligned(obj) + vect.x;
    let y = ui.obj_get_y_aligned(obj) + vect.y;
    ui.obj_set_pos(obj, x, y);
}

/// `lv_example_obj_draggable()`: make an object draggable.
pub fn lv_example_obj_draggable(ui: &mut Ui) {
    let obj = ui.obj_create(Some(screen_active(ui)));
    ui.obj_set_size(obj, 150, 100);
    ui.obj_add_event_cb(obj, Rc::new(drag_event_handler), EventCode::PRESSING, None);

    let label = Label::create(ui, obj);
    label.set_text(ui, "Drag me");
    ui.obj_center(label.obj());
}

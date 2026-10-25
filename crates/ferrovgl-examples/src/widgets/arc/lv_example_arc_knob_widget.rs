//! Port of `examples/widgets/arc/lv_example_arc_knob_widget.c`.

use alloc::rc::Rc;

use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Event, EventCode, EventParam, Ui};
use ferrovgl_widgets::{Arc, Label};

use crate::util::{obj_data, screen_active};

/// `static lv_obj_t * arc; static lv_obj_t * label;`
#[derive(Default)]
struct Statics {
    arc: Option<Arc>,
    label: Option<Label>,
}

/// `static void value_changed_event_cb(lv_event_t * e)`
fn value_changed_event_cb(ui: &mut Ui, _e: &mut Event<'_>) {
    let s = ui.widget_static_mut::<Statics>();
    let (Some(arc), Some(label)) = (s.arc, s.label) else {
        return;
    };

    let value = arc.value(ui);
    label.set_text_fmt(ui, format_args!("{value}%"));

    /*Rotate the label to the current position of the arc*/
    arc.rotate_obj_to_angle(ui, label.obj(), 25);
}

/// `lv_example_arc_knob_widget()`: a label following the arc knob.
pub fn lv_example_arc_knob_widget(ui: &mut Ui) {
    let label = Label::create(ui, screen_active(ui));
    ui.widget_static_mut::<Statics>().label = Some(label);

    /*Create an Arc*/
    let arc = Arc::create(ui, screen_active(ui));
    ui.widget_static_mut::<Statics>().arc = Some(arc);
    ui.obj_set_size(arc.obj(), 150, 150);
    arc.set_rotation(ui, 135);
    arc.set_bg_angles(ui, 0 as ValuePrecise, 270 as ValuePrecise);
    arc.set_value(ui, 10);
    ui.obj_center(arc.obj());
    ui.obj_add_event_cb(
        arc.obj(),
        Rc::new(value_changed_event_cb),
        EventCode::VALUE_CHANGED,
        obj_data(label.obj()),
    );

    /*Manually update the label for the first time*/
    ui.obj_send_event(arc.obj(), EventCode::VALUE_CHANGED, EventParam::None);
}

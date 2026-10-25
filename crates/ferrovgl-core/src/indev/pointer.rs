//! Pointer/button processing from `src/indev/lv_indev.c`.
use super::*;
use crate::misc::anim::{ANIM_REPEAT_INFINITE, Anim, AnimExec, AnimTarget};
use crate::misc::style::{Part, State};

impl Ui {
    pub(super) fn indev_pointer_proc(&mut self, indev: IndevId, data: &mut IndevReadData) {
        let Some(d) = self.indev_mut(indev) else {
            return;
        };
        d.pointer.last_raw_point = data.point;
        let Some(disp) = d.disp.filter(|&id| self.display(id).is_some()) else {
            return;
        };
        if self.indev_get_ccw(indev) {
            self.display_rotate_point_ccw(disp, &mut data.point);
        } else {
            self.display_rotate_point(disp, &mut data.point);
        }
        let d = self.indev(indev).unwrap();
        if d.pointer.last_point != data.point
            && let Some(cursor) = d.cursor.filter(|&o| self.obj_is_valid(o))
        {
            self.obj_set_pos(cursor, data.point.x, data.point.y);
            if self.indev_reset_check(indev) {
                return;
            }
        }
        let Some(d) = self.indev_mut(indev) else {
            return;
        };
        d.pointer.act_point = data.point;
        d.pointer.diff = i32::from(data.enc_diff);
        self.indev_proc_pointer_diff(indev);
        if self.indev_reset_check(indev) {
            return;
        }
        if data.state == IndevState::Pressed {
            self.indev_proc_press(indev);
        } else {
            self.indev_proc_release(indev);
        }
        if let Some(d) = self.indev_mut(indev) {
            d.prev_state = d.state;
            d.pointer.last_point = d.pointer.act_point;
        }
    }

    pub(super) fn indev_button_proc(&mut self, indev: IndevId, data: &IndevReadData) {
        let Some(d) = self.indev(indev) else {
            return;
        };
        // C indexes an unbounded array. Rust rejects an out-of-range button.
        let Some(&point) = d.btn_points.get(data.btn_id as usize) else {
            return;
        };
        if data.state == IndevState::Pressed && d.pointer.last_point != point {
            self.indev_proc_release(indev);
        }
        if self.indev_reset_check(indev) {
            return;
        }
        self.indev_mut(indev).unwrap().pointer.act_point = point;
        if data.state == IndevState::Pressed {
            self.indev_proc_press(indev);
        } else {
            self.indev_proc_release(indev);
        }
        if let Some(d) = self.indev_mut(indev) {
            d.prev_state = d.state;
        }
        if self.indev_reset_check(indev) {
            return;
        }
        if let Some(d) = self.indev_mut(indev) {
            d.pointer.last_point = d.pointer.act_point;
        }
    }

    fn indev_pointer_search(&mut self, disp: DisplayId, point: Point) -> Option<Obj> {
        let d = self.display(disp)?;
        let layers = [d.sys_layer, d.top_layer, d.act_scr, d.bottom_layer];
        for layer in layers.into_iter().flatten() {
            if let Some(obj) = self.indev_search_obj(Obj(layer), point) {
                self.indev_obj_active = Some(obj);
                return Some(obj);
            }
        }
        self.indev_obj_active = None;
        None
    }

    pub(super) fn indev_object_event(&mut self, indev: IndevId, obj: Obj, code: EventCode) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }
        let result = self.obj_send_event(obj, code, EventParam::Indev(indev));
        !self.indev_reset_check(indev) && result == LvResult::Ok
    }

    fn indev_proc_press(&mut self, indev: IndevId) {
        let Some(d) = self.indev(indev) else {
            return;
        };
        let pointer = d.pointer.clone();
        let (wait, disp, prev_state) = (d.wait_until_release, d.disp, d.prev_state);
        self.indev_obj_active = pointer.act_obj.filter(|&o| self.obj_is_valid(o));
        if wait {
            return;
        }
        let Some(disp) = disp else {
            return;
        };
        let search = self.indev_obj_active.is_none()
            || (pointer.scroll_obj.is_none()
                && self
                    .indev_obj_active
                    .is_some_and(|o| !self.obj_has_flag(o, ObjFlags::PRESS_LOCK)));
        if search {
            self.indev_obj_active = self.indev_pointer_search(disp, pointer.act_point);
            if self.indev_reset_check(indev) {
                return;
            }
        }
        if search && pointer.scroll_obj.is_some() {
            if let Some(anim) = self.indev(indev).and_then(|d| d.scroll_throw_anim) {
                self.anim_custom_delete(anim, None);
                if let Some(d) = self.indev_mut(indev) {
                    d.scroll_throw_anim = None;
                }
            }
            self.indev_scroll_throw_handler(indev);
            if self.indev_reset_check(indev) {
                return;
            }
        }
        let active = self.indev_obj_active;
        if active != pointer.act_obj {
            let Some(d) = self.indev_mut(indev) else {
                return;
            };
            if pointer.act_obj.is_none() {
                d.pointer.last_point = d.pointer.act_point;
            }
            d.pointer.pressed = prev_state == IndevState::Released;
            if let Some(last) = pointer.last_hovered
                && Some(last) != active
            {
                if !self.indev_object_event(indev, last, EventCode::HOVER_LEAVE) {
                    return;
                }
                self.indev_send_event(indev, EventCode::HOVER_LEAVE, EventParam::Obj(last));
                if self.indev_reset_check(indev) {
                    return;
                }
                self.indev_mut(indev).unwrap().pointer.last_hovered = active;
            }
            if let Some(previous) = pointer.act_obj
                && !self.indev_object_event(indev, previous, EventCode::PRESS_LOST)
            {
                return;
            }
            self.indev_mut(indev).unwrap().pointer.act_obj = active;
            if let Some(obj) = active {
                let d = self.indev_mut(indev).unwrap();
                d.pr_timestamp = d.timestamp;
                d.long_pr_sent = false;
                d.pointer.scroll_sum = Point::default();
                d.pointer.scroll_dir = Dir::NONE;
                d.pointer.scroll_obj = None;
                d.pointer.gesture_dir = Dir::NONE;
                d.pointer.gesture_sent = false;
                d.pointer.gesture_sum = Point::default();
                d.pointer.press_moved = false;
                d.pointer.vect = Point::default();
                if prev_state != IndevState::Pressed && !self.obj_has_state(obj, State::DISABLED) {
                    if self.indev(indev).unwrap().pointer.last_hovered != active
                        && !self.indev_input_event(indev, EventCode::HOVER_OVER)
                    {
                        return;
                    }
                    if !self.indev_input_event(indev, EventCode::PRESSED) {
                        return;
                    }
                }
                if self.indev(indev).is_none_or(|d| d.wait_until_release) {
                    return;
                }
                self.indev_click_focus(indev);
                if self.indev_reset_check(indev) {
                    return;
                }
            }
        }
        let Some(d) = self.indev_mut(indev) else {
            return;
        };
        d.pointer.vect = Point::new(
            d.pointer.act_point.x - d.pointer.last_point.x,
            d.pointer.act_point.y - d.pointer.last_point.y,
        );
        let index = usize::from(d.pointer.vect_hist_index);
        d.pointer.vect_hist[index] = d.pointer.vect;
        d.pointer.vect_hist_timestamp[index] = d.timestamp;
        d.pointer.vect_hist_index = ((index + 1) % INDEV_VECT_HIST_SIZE) as u8;
        let mut throw = Point::default();
        for (vector, timestamp) in d
            .pointer
            .vect_hist
            .iter()
            .zip(d.pointer.vect_hist_timestamp.iter())
        {
            let elapsed = d.timestamp.wrapping_sub(*timestamp) as i32;
            throw.x += scroll_throw_decay(vector.x, elapsed);
            throw.y += scroll_throw_decay(vector.y, elapsed);
        }
        d.pointer.scroll_throw_vect = throw;
        d.pointer.scroll_throw_vect_ori = throw;
        if d.pointer.vect.x.abs() > i32::from(d.scroll_limit)
            || d.pointer.vect.y.abs() > i32::from(d.scroll_limit)
        {
            d.pointer.press_moved = true;
        }
        let Some(obj) = self.indev_obj_active else {
            return;
        };
        let enabled = !self.obj_has_state(obj, State::DISABLED);
        if enabled && !self.indev_input_event(indev, EventCode::PRESSING) {
            return;
        }
        if self.indev(indev).is_none_or(|d| d.wait_until_release) {
            return;
        }
        if let Some(scroll) = self.indev(indev).and_then(|d| d.pointer.scroll_obj) {
            self.obj_stop_scroll_anim(scroll);
        }
        self.indev_scroll_handler(indev);
        if self.indev_reset_check(indev) {
            return;
        }
        self.indev_gesture(indev);
        if self.indev_reset_check(indev) {
            return;
        }
        let d = self.indev(indev).unwrap();
        if d.mode == IndevMode::Event
            && let Some(timer) = d.read_timer
            && self.timer_get_paused(timer)
        {
            self.timer_resume(timer);
        }
        let d = self.indev(indev).unwrap();
        if d.pointer.scroll_obj.is_none()
            && !d.long_pr_sent
            && d.timestamp.wrapping_sub(d.pr_timestamp) >= u32::from(d.long_press_time)
        {
            if enabled && !self.indev_input_event(indev, EventCode::LONG_PRESSED) {
                return;
            }
            let Some(d) = self.indev_mut(indev) else {
                return;
            };
            d.long_pr_sent = true;
            d.longpr_rep_timestamp = d.timestamp;
        }
        let Some(d) = self.indev(indev) else {
            return;
        };
        if d.pointer.scroll_obj.is_none()
            && d.long_pr_sent
            && d.timestamp.wrapping_sub(d.longpr_rep_timestamp) >= u32::from(d.long_press_repeat_time)
        {
            if enabled && !self.indev_input_event(indev, EventCode::LONG_PRESSED_REPEAT) {
                return;
            }
            if let Some(d) = self.indev_mut(indev) {
                d.longpr_rep_timestamp = d.timestamp;
            }
        }
    }

    fn indev_proc_release(&mut self, indev: IndevId) {
        let Some(d) = self.indev(indev) else {
            return;
        };
        let pointer = d.pointer.clone();
        if d.wait_until_release || pointer.last_point != pointer.act_point {
            // Upstream release hover search uses the default display.
            let hovered = self
                .display_get_default()
                .and_then(|disp| self.indev_pointer_search(disp, pointer.act_point));
            if self.indev_reset_check(indev) {
                return;
            }
            if pointer.last_hovered != hovered {
                for (obj, code) in [
                    (hovered, EventCode::HOVER_OVER),
                    (pointer.last_hovered, EventCode::HOVER_LEAVE),
                ] {
                    if let Some(obj) = obj {
                        if !self.indev_object_event(indev, obj, code) {
                            return;
                        }
                        self.indev_send_event(indev, code, EventParam::Obj(obj));
                        if self.indev_reset_check(indev) {
                            return;
                        }
                    }
                }
                self.indev_mut(indev).unwrap().pointer.last_hovered = hovered;
            }
        }
        if self.indev(indev).is_some_and(|d| d.wait_until_release) {
            if let Some(obj) = self.indev(indev).and_then(|d| d.pointer.act_obj) {
                self.indev_object_event(indev, obj, EventCode::PRESS_LOST);
            }
            if self.indev_reset_check(indev) {
                if let Some(d) = self.indev_mut(indev) {
                    d.wait_until_release = false;
                }
                return;
            }
            let d = self.indev_mut(indev).unwrap();
            d.pointer.act_obj = None;
            d.pr_timestamp = 0;
            d.longpr_rep_timestamp = 0;
            d.wait_until_release = false;
        }
        let d = self.indev(indev).unwrap();
        let (active, scroll, mode, timer) = (d.pointer.act_obj, d.pointer.scroll_obj, d.mode, d.read_timer);
        self.indev_obj_active = active;
        if mode == IndevMode::Event
            && let Some(timer) = timer
            && !self.timer_get_paused(timer)
        {
            self.timer_pause(timer);
        }
        if let Some(obj) = active {
            let enabled = self.obj_is_valid(obj) && !self.obj_has_state(obj, State::DISABLED);
            if enabled && !self.indev_input_event(indev, EventCode::RELEASED) {
                return;
            }
            if enabled {
                if let Some(scroll) = scroll {
                    if !self.indev_object_event(indev, scroll, EventCode::SCROLL_THROW_BEGIN) {
                        return;
                    }
                } else if self.indev(indev).is_some_and(|d| d.pointer.pressed) {
                    if !self.indev(indev).unwrap().long_pr_sent && !self.indev_proc_short_click(indev) {
                        return;
                    }
                    if !self.indev_input_event(indev, EventCode::CLICKED) {
                        return;
                    }
                }
            }
            let d = self.indev_mut(indev).unwrap();
            d.pointer.act_obj = None;
            d.pr_timestamp = 0;
            d.longpr_rep_timestamp = 0;
            if let Some(scroll) = scroll {
                let (mut vector, mut original) =
                    (d.pointer.scroll_throw_vect, d.pointer.scroll_throw_vect_ori);
                self.indev_transform_release_vector(scroll, &mut vector);
                self.indev_transform_release_vector(scroll, &mut original);
                let d = self.indev_mut(indev).unwrap();
                d.pointer.scroll_throw_vect = vector;
                d.pointer.scroll_throw_vect_ori = original;
            }
        }
        if scroll.is_some() && self.indev(indev).is_some_and(|d| d.scroll_throw_anim.is_none()) {
            self.indev_start_throw(indev);
        }
    }

    fn indev_proc_pointer_diff(&mut self, indev: IndevId) {
        let Some(d) = self.indev(indev) else {
            return;
        };
        let Some(obj) = d.pointer.last_pressed.filter(|&o| self.obj_is_valid(o)) else {
            return;
        };
        let diff = d.pointer.diff;
        if diff == 0 {
            return;
        }
        self.indev_obj_active = Some(obj);
        if self.obj_is_editable(obj) {
            let diff = self.indev_scaled_diff(indev, obj, diff);
            self.indev_dispatch(indev, EventCode::ROTARY, EventParam::RotaryDiff(diff));
        } else {
            let d = self.indev_mut(indev).unwrap();
            d.pointer.vect.y = if diff > 0 {
                i32::from(d.scroll_limit)
            } else {
                -i32::from(d.scroll_limit)
            };
            d.pointer.act_obj = Some(obj);
            let Some(scroll) = self.indev_find_scroll_obj(indev) else {
                return;
            };
            let diff = self.indev_scaled_diff(indev, scroll, diff);
            let d = self.indev_mut(indev).unwrap();
            d.pointer.scroll_throw_vect.y = diff;
            d.pointer.scroll_throw_vect_ori.y = diff;
            self.indev_scroll_handler(indev);
        }
    }

    fn indev_scaled_diff(&self, indev: IndevId, obj: Obj, diff: i32) -> i32 {
        let sensitivity = self.indev(indev).map_or(0, |d| d.rotary_sensitivity) as u32;
        let object = self.obj_get_style_rotary_sensitivity(obj, Part::MAIN);
        ((diff as u32)
            .wrapping_mul(sensitivity)
            .wrapping_mul(object)
            .wrapping_add(32768) as i32)
            >> 16
    }

    fn indev_click_focus(&mut self, indev: IndevId) {
        let Some(active) = self.indev_obj_active else {
            return;
        };
        if !self.obj_has_flag(active, ObjFlags::CLICK_FOCUSABLE) {
            return;
        }
        let last = self
            .indev(indev)
            .and_then(|d| d.pointer.last_pressed)
            .filter(|&o| self.obj_is_valid(o));
        let group = self.obj_get_group(active);
        let previous_group = last.and_then(|o| self.obj_get_group(o));
        if group == previous_group {
            if group.is_some() {
                self.group_focus_obj(active);
                if self.indev_reset_check(indev) {
                    return;
                }
            } else if last != Some(active) {
                if let Some(last) = last
                    && !self.indev_object_event(indev, last, EventCode::DEFOCUSED)
                {
                    return;
                }
                if !self.indev_object_event(indev, active, EventCode::FOCUSED) {
                    return;
                }
            }
        } else {
            if let Some(last) = last {
                let code = if previous_group.is_some() {
                    EventCode::LEAVE
                } else {
                    EventCode::DEFOCUSED
                };
                if !self.indev_object_event(indev, last, code) {
                    return;
                }
            }
            if group.is_some() {
                self.group_focus_obj(active);
                if self.indev_reset_check(indev) {
                    return;
                }
            } else if !self.indev_object_event(indev, active, EventCode::FOCUSED) {
                return;
            }
        }
        let active = self.indev_obj_active;
        if let Some(d) = self.indev_mut(indev) {
            d.pointer.last_pressed = active;
        }
    }

    fn indev_gesture(&mut self, indev: IndevId) {
        let Some(d) = self.indev(indev) else {
            return;
        };
        if d.pointer.scroll_obj.is_some() || d.pointer.gesture_sent {
            return;
        }
        let mut obj = d.pointer.act_obj;
        while let Some(current) = obj {
            if !self.obj_has_flag(current, ObjFlags::GESTURE_BUBBLE) {
                break;
            }
            obj = self.obj_get_parent(current);
        }
        let Some(obj) = obj else {
            return;
        };
        let d = self.indev_mut(indev).unwrap();
        if d.pointer.vect.x.abs() < i32::from(d.gesture_min_velocity)
            && d.pointer.vect.y.abs() < i32::from(d.gesture_min_velocity)
        {
            d.pointer.gesture_sum = Point::default();
        }
        d.pointer.gesture_sum.x += d.pointer.vect.x;
        d.pointer.gesture_sum.y += d.pointer.vect.y;
        let sum = d.pointer.gesture_sum;
        if sum.x.abs() <= i32::from(d.gesture_min_distance)
            && sum.y.abs() <= i32::from(d.gesture_min_distance)
        {
            return;
        }
        d.pointer.gesture_sent = true;
        let (direction, code) = if sum.x.abs() > sum.y.abs() {
            if sum.x > 0 {
                (Dir::RIGHT, EventCode::GESTURE_RIGHT)
            } else {
                (Dir::LEFT, EventCode::GESTURE_LEFT)
            }
        } else if sum.y > 0 {
            (Dir::BOTTOM, EventCode::GESTURE_DOWN)
        } else {
            (Dir::TOP, EventCode::GESTURE_UP)
        };
        d.pointer.gesture_dir = direction;
        if !self.indev_object_event(indev, obj, EventCode::GESTURE) {
            return;
        }
        if !self.indev_object_event(indev, obj, code) {
            return;
        }
        self.indev_send_event(indev, EventCode::GESTURE, EventParam::Obj(obj));
        if self.indev_reset_check(indev) {
            return;
        }
        self.indev_send_event(indev, code, EventParam::Obj(obj));
    }

    fn indev_start_throw(&mut self, indev: IndevId) {
        let mut animation = Anim::new();
        animation.set_var(AnimTarget::Indev(indev));
        animation
            .set_duration(1024)
            .set_values(0, 1024)
            .set_repeat_count(ANIM_REPEAT_INFINITE);
        animation.set_exec_cb(AnimExec::Custom {
            tag: 0x494e_4456,
            f: Rc::new(|ui, target, _| {
                let AnimTarget::Indev(indev) = target else {
                    return;
                };
                ui.indev_scroll_throw_handler(indev);
                if let Some(d) = ui.indev(indev)
                    && (d.pointer.scroll_dir == Dir::NONE || d.pointer.scroll_obj.is_none())
                    && let Some(anim) = d.scroll_throw_anim
                {
                    ui.anim_custom_delete(anim, None);
                }
            }),
        });
        let reset: crate::misc::anim::AnimCallback = Rc::new(|ui, anim| {
            if let Some(AnimTarget::Indev(indev)) = ui.anim(anim).map(|a| a.var)
                && let Some(d) = ui.indev_mut(indev)
            {
                d.pointer.scroll_throw_vect = Point::default();
                d.scroll_throw_anim = None;
            }
        });
        animation
            .set_completed_cb(Some(reset.clone()))
            .set_deleted_cb(Some(reset));
        let anim = self.anim_start(&animation);
        if let Some(d) = self.indev_mut(indev) {
            d.scroll_throw_anim = Some(anim);
        } else {
            self.anim_custom_delete(anim, None);
        }
    }
}

fn scroll_throw_decay(value: i32, elapsed: i32) -> i32 {
    if elapsed <= 0 {
        value
    } else if elapsed >= 99 {
        0
    } else {
        value * (512 - (512 * elapsed) / 99) / 512
    }
}

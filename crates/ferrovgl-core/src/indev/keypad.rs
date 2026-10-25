//! Keypad and encoder processing from `src/indev/lv_indev.c`.
use super::*;
use crate::group::{KEY_ENTER, KEY_ESC, KEY_LEFT, KEY_NEXT, KEY_PREV, KEY_RIGHT};
use crate::misc::style::State;

impl Ui {
    fn indev_key_wait(&mut self, indev: IndevId, state: IndevState) -> bool {
        let Some(d) = self.indev_mut(indev) else {
            return true;
        };
        if state == IndevState::Pressed && d.wait_until_release {
            return true;
        }
        if d.wait_until_release {
            d.wait_until_release = false;
            d.pr_timestamp = 0;
            d.long_pr_sent = false;
            d.keypad.last_state = IndevState::Released;
        }
        false
    }

    fn indev_group_data(&mut self, indev: IndevId, group: GroupId, key: u32) -> bool {
        if self.group(group).is_none() {
            return false;
        }
        let result = self.group_send_data(group, key);
        !self.indev_reset_check(indev) && result == LvResult::Ok && self.group(group).is_some()
    }

    fn indev_group_move(&mut self, indev: IndevId, group: GroupId, next: bool) -> bool {
        if self.group(group).is_none() {
            return false;
        }
        if next {
            self.group_focus_next(group);
        } else {
            self.group_focus_prev(group);
        }
        !self.indev_reset_check(indev) && self.group(group).is_some()
    }

    pub(super) fn indev_keypad_proc(&mut self, indev: IndevId, data: &mut IndevReadData) {
        if self.indev_key_wait(indev, data.state) {
            return;
        }
        data.key = self.indev_remap_key(indev, data.key);
        if self.indev_reset_check(indev) {
            return;
        }
        let d = self.indev_mut(indev).unwrap();
        let previous_key = d.keypad.last_key;
        let previous_state = d.keypad.last_state;
        d.keypad.last_key = data.key;
        d.keypad.last_state = data.state;
        if (previous_key != data.key || previous_state != data.state)
            && (self.indev_send_event(indev, EventCode::KEY, EventParam::None) == LvResult::Invalid
                || self.indev_reset_check(indev))
        {
            return;
        }
        let group = self.indev(indev).and_then(|d| d.group);
        let active = group.and_then(|g| self.group_get_focused(g));
        self.indev_obj_active = active;
        if group.is_some() && active.is_none() {
            return;
        }
        let enabled = active.is_none_or(|o| !self.obj_has_state(o, State::DISABLED));
        if data.state == IndevState::Pressed && previous_state == IndevState::Released {
            let d = self.indev_mut(indev).unwrap();
            d.pr_timestamp = d.timestamp;
            if let Some(group) = group {
                if matches!(data.key, KEY_NEXT | KEY_PREV) {
                    self.group_set_editing(group, false);
                    if self.indev_reset_check(indev)
                        || !self.indev_group_move(indev, group, data.key == KEY_NEXT)
                    {
                        return;
                    }
                } else if enabled {
                    if !self.indev_group_data(indev, group, data.key) {
                        return;
                    }
                    if data.key == KEY_ENTER && !self.indev_input_event(indev, EventCode::PRESSED) {
                        return;
                    }
                    if data.key == KEY_ESC && !self.indev_input_event(indev, EventCode::CANCEL) {
                        return;
                    }
                }
            } else if !self.indev_input_event(indev, EventCode::PRESSED) {
                return;
            }
        } else if enabled && data.state == IndevState::Pressed && previous_state == IndevState::Pressed {
            if (group.is_none() || data.key == KEY_ENTER)
                && !self.indev_input_event(indev, EventCode::PRESSING)
            {
                return;
            }
            let d = self.indev(indev).unwrap();
            if !d.long_pr_sent && d.timestamp.wrapping_sub(d.pr_timestamp) >= u32::from(d.long_press_time) {
                self.indev_mut(indev).unwrap().long_pr_sent = true;
                if group.is_none() || data.key == KEY_ENTER {
                    let d = self.indev_mut(indev).unwrap();
                    d.longpr_rep_timestamp = d.timestamp;
                    if !self.indev_input_event(indev, EventCode::LONG_PRESSED) {
                        return;
                    }
                }
            } else if d.long_pr_sent
                && d.timestamp.wrapping_sub(d.longpr_rep_timestamp) >= u32::from(d.long_press_repeat_time)
            {
                let d = self.indev_mut(indev).unwrap();
                d.longpr_rep_timestamp = d.timestamp;
                if group.is_none() || data.key == KEY_ENTER {
                    if !self.indev_input_event(indev, EventCode::LONG_PRESSED_REPEAT) {
                        return;
                    }
                } else if let Some(group) = group {
                    if matches!(data.key, KEY_NEXT | KEY_PREV) {
                        self.group_set_editing(group, false);
                        if self.indev_reset_check(indev)
                            || !self.indev_group_move(indev, group, data.key == KEY_NEXT)
                        {
                            return;
                        }
                    } else if !self.indev_group_data(indev, group, data.key) {
                        return;
                    }
                }
            }
        } else if enabled && data.state == IndevState::Released && previous_state == IndevState::Pressed {
            data.key = previous_key;
            if group.is_none() || data.key == KEY_ENTER {
                if !self.indev_input_event(indev, EventCode::RELEASED) {
                    return;
                }
                if !self.indev(indev).unwrap().long_pr_sent && !self.indev_proc_short_click(indev) {
                    return;
                }
                if !self.indev_input_event(indev, EventCode::CLICKED) {
                    return;
                }
            }
            let d = self.indev_mut(indev).unwrap();
            d.pr_timestamp = 0;
            d.long_pr_sent = false;
        }
        self.indev_obj_active = None;
    }

    pub(super) fn indev_encoder_proc(&mut self, indev: IndevId, data: &mut IndevReadData) {
        if self.indev_key_wait(indev, data.state) {
            return;
        }
        let d = self.indev_mut(indev).unwrap();
        let previous_state = d.keypad.last_state;
        d.keypad.last_state = data.state;
        d.keypad.last_key = data.key;
        let Some(group) = d.group else {
            return;
        };
        let Some(active) = self.group_get_focused(group) else {
            return;
        };
        self.indev_obj_active = Some(active);
        if data.state != IndevState::Released {
            data.enc_diff = 0;
        }
        let enabled = !self.obj_has_state(active, State::DISABLED);
        if data.state == IndevState::Pressed && previous_state == IndevState::Released {
            let d = self.indev_mut(indev).unwrap();
            d.pr_timestamp = d.timestamp;
            match data.key {
                KEY_ENTER => {
                    let editable =
                        self.obj_is_editable(active) || self.obj_has_flag(active, ObjFlags::SCROLLABLE);
                    if (self.group_get_editing(group) || !editable)
                        && enabled
                        && !self.indev_input_event(indev, EventCode::PRESSED)
                    {
                        return;
                    }
                }
                KEY_LEFT => data.enc_diff = data.enc_diff.wrapping_sub(1),
                KEY_RIGHT => data.enc_diff = data.enc_diff.wrapping_add(1),
                KEY_ESC => {
                    if !self.indev_group_data(indev, group, KEY_ESC) {
                        return;
                    }
                    if enabled && !self.indev_input_event(indev, EventCode::CANCEL) {
                        return;
                    }
                }
                key => {
                    if !self.indev_group_data(indev, group, key) {
                        return;
                    }
                }
            }
        } else if data.state == IndevState::Pressed && previous_state == IndevState::Pressed {
            let d = self.indev(indev).unwrap();
            if !d.long_pr_sent && d.timestamp.wrapping_sub(d.pr_timestamp) >= u32::from(d.long_press_time) {
                let d = self.indev_mut(indev).unwrap();
                d.long_pr_sent = true;
                d.longpr_rep_timestamp = d.timestamp;
                if data.key == KEY_ENTER {
                    self.indev_send_event(indev, EventCode::LONG_PRESSED, EventParam::Obj(active));
                    if self.indev_reset_check(indev) || !self.obj_is_valid(active) {
                        return;
                    }
                    let editable =
                        self.obj_is_editable(active) || self.obj_has_flag(active, ObjFlags::SCROLLABLE);
                    if editable {
                        if self.group_get_obj_count(group) > 1 {
                            self.group_set_editing(group, !self.group_get_editing(group));
                            if self.indev_reset_check(indev) || !self.obj_is_valid(active) {
                                return;
                            }
                            self.obj_remove_state(active, State::PRESSED);
                        }
                    } else if enabled && !self.indev_object_event(indev, active, EventCode::LONG_PRESSED) {
                        return;
                    }
                }
                if let Some(d) = self.indev_mut(indev) {
                    d.long_pr_sent = true;
                }
            } else if d.long_pr_sent
                && d.timestamp.wrapping_sub(d.longpr_rep_timestamp) >= u32::from(d.long_press_repeat_time)
            {
                let d = self.indev_mut(indev).unwrap();
                d.longpr_rep_timestamp = d.timestamp;
                match data.key {
                    KEY_ENTER => {
                        if enabled && !self.indev_input_event(indev, EventCode::LONG_PRESSED_REPEAT) {
                            return;
                        }
                    }
                    KEY_LEFT => data.enc_diff = data.enc_diff.wrapping_sub(1),
                    KEY_RIGHT => data.enc_diff = data.enc_diff.wrapping_add(1),
                    key => {
                        if !self.indev_group_data(indev, group, key) {
                            return;
                        }
                    }
                }
            }
        } else if data.state == IndevState::Released && previous_state == IndevState::Pressed {
            if data.key == KEY_ENTER {
                let editable =
                    self.obj_is_editable(active) || self.obj_has_flag(active, ObjFlags::SCROLLABLE);
                if !editable {
                    if enabled {
                        if !self.indev_input_event(indev, EventCode::RELEASED) {
                            return;
                        }
                        if !self.indev(indev).unwrap().long_pr_sent && !self.indev_proc_short_click(indev) {
                            return;
                        }
                        if !self.indev_input_event(indev, EventCode::CLICKED) {
                            return;
                        }
                    }
                } else if self.group_get_editing(group) {
                    if !self.indev(indev).unwrap().long_pr_sent || self.group_get_obj_count(group) <= 1 {
                        if enabled
                            && (!self.indev_input_event(indev, EventCode::RELEASED)
                                || !self.indev_proc_short_click(indev)
                                || !self.indev_input_event(indev, EventCode::CLICKED))
                        {
                            return;
                        }
                        if !self.indev_group_data(indev, group, KEY_ENTER) {
                            return;
                        }
                    } else {
                        self.obj_remove_state(active, State::PRESSED);
                    }
                } else if !self.indev(indev).unwrap().long_pr_sent {
                    self.group_set_editing(group, true);
                }
            }
            if let Some(d) = self.indev_mut(indev) {
                d.pr_timestamp = 0;
                d.long_pr_sent = false;
            }
        }
        self.indev_obj_active = None;
        if self.indev_reset_check(indev) || self.group(group).is_none() {
            return;
        }
        let editing = self.group_get_editing(group);
        for _ in 0..i32::from(data.enc_diff).abs() {
            if editing {
                let key = if data.enc_diff < 0 { KEY_LEFT } else { KEY_RIGHT };
                if !self.indev_group_data(indev, group, key) {
                    return;
                }
            } else if !self.indev_group_move(indev, group, data.enc_diff > 0) {
                return;
            }
        }
    }
}

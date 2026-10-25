//! Ported from lvgl v9.6.0: `include/lvgl/layouts/lv_grid.h`, `src/layouts/grid/lv_grid.c`.

use super::LAYOUT_GRID;
use crate::core::Ui;
use crate::core::obj::Obj;
use crate::misc::area::Point;
use crate::misc::event::{EventCode, EventParam};
use crate::misc::style::{Part, StyleProp, StyleSelector, StyleValue, TracksRef};
use crate::misc::text::BaseDir;
use crate::misc::types::coord;
use alloc::vec;
use alloc::vec::Vec;

/// `LV_GRID_CONTENT`
pub const GRID_CONTENT: i32 = coord::MAX - 101;
/// `LV_GRID_TEMPLATE_LAST`
pub const GRID_TEMPLATE_LAST: i32 = coord::MAX;

/// `LV_GRID_FR(x)` / `lv_grid_fr()`
#[inline]
#[must_use]
pub const fn grid_fr(x: u8) -> i32 {
    coord::MAX - 100 + x as i32
}

/// `lv_grid_align_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GridAlign {
    /// `LV_GRID_ALIGN_START`
    #[default]
    Start = 0,
    /// `LV_GRID_ALIGN_CENTER`
    Center = 1,
    /// `LV_GRID_ALIGN_END`
    End = 2,
    /// `LV_GRID_ALIGN_STRETCH`
    Stretch = 3,
    /// `LV_GRID_ALIGN_SPACE_EVENLY`
    SpaceEvenly = 4,
    /// `LV_GRID_ALIGN_SPACE_AROUND`
    SpaceAround = 5,
    /// `LV_GRID_ALIGN_SPACE_BETWEEN`
    SpaceBetween = 6,
}

impl GridAlign {
    /// Convert a raw style number (`Start` if unknown).
    #[must_use]
    pub const fn from_u8(v: u8) -> GridAlign {
        match v {
            1 => GridAlign::Center,
            2 => GridAlign::End,
            3 => GridAlign::Stretch,
            4 => GridAlign::SpaceEvenly,
            5 => GridAlign::SpaceAround,
            6 => GridAlign::SpaceBetween,
            _ => GridAlign::Start,
        }
    }
}

/// `grid_update()`: the layout callback registered as `LV_LAYOUT_GRID`.
pub(crate) fn grid_update(ui: &mut Ui, obj: Obj) {
    let Some(grid) = calc(ui, obj) else { return };
    let coords = ui.obj_get_coords(obj);
    let origin = Point::new(
        ui.obj_get_style_space_left(obj, Part::MAIN) + coords.x1 - ui.obj_get_scroll_x(obj),
        ui.obj_get_style_space_top(obj, Part::MAIN) + coords.y1 - ui.obj_get_scroll_y(obj),
    );
    let mut index = 0;
    while ui.obj_is_valid(obj) && index < ui.obj_get_child_count(obj) {
        if let Some(item) = ui.obj_get_child(obj, index as i32) {
            item_repos(ui, item, &grid, origin);
        }
        index += 1;
    }
    if !ui.obj_is_valid(obj) {
        return;
    }
    if ui.obj_get_style_width(obj, Part::MAIN) == coord::SIZE_CONTENT
        || ui.obj_get_style_height(obj, Part::MAIN) == coord::SIZE_CONTENT
    {
        ui.obj_refr_size(obj);
    }
    if ui.obj_is_valid(obj) {
        ui.obj_send_event(obj, EventCode::LAYOUT_CHANGED, EventParam::None);
    }
}

/// LVGL's grid registers a NULL minimum-size callback. Returning `false`
/// delegates content/minimum sizing to the ordinary child-bounds calculation.
pub(crate) fn grid_get_min_size(_ui: &mut Ui, _obj: Obj, _req_size: &mut i32, _width: bool) -> bool {
    false
}

impl Ui {
    /// `lv_obj_set_grid_dsc_array()`
    /// A `None` descriptor inherits the corresponding tracks from the parent
    /// over this object's grid-cell span (a subgrid).
    pub fn obj_set_grid_dsc_array(
        &mut self,
        obj: Obj,
        col_dsc: impl Into<Option<TracksRef>>,
        row_dsc: impl Into<Option<TracksRef>>,
    ) {
        if !self.obj_is_valid(obj) {
            return;
        }
        self.obj_set_local_style_prop(
            obj,
            StyleProp::GRID_COLUMN_DSC_ARRAY,
            col_dsc.into().map_or(StyleValue::Null, StyleValue::Tracks),
            StyleSelector::DEFAULT,
        );
        self.obj_set_local_style_prop(
            obj,
            StyleProp::GRID_ROW_DSC_ARRAY,
            row_dsc.into().map_or(StyleValue::Null, StyleValue::Tracks),
            StyleSelector::DEFAULT,
        );
        self.obj_set_style_layout(obj, LAYOUT_GRID as u16, StyleSelector::DEFAULT);
    }

    /// `lv_obj_set_grid_align()`
    pub fn obj_set_grid_align(&mut self, obj: Obj, column_align: GridAlign, row_align: GridAlign) {
        if !self.obj_is_valid(obj) {
            return;
        }
        if column_align == GridAlign::Stretch || row_align == GridAlign::Stretch {
            crate::misc::check_arg_failed("LV_GRID_ALIGN_STRETCH is not supported for grid tracks");
            return;
        }
        self.obj_set_style_grid_column_align(obj, column_align, StyleSelector::DEFAULT);
        self.obj_set_style_grid_row_align(obj, row_align, StyleSelector::DEFAULT);
    }

    /// `lv_obj_set_grid_cell()`
    pub fn obj_set_grid_cell(
        &mut self,
        obj: Obj,
        column_align: GridAlign,
        col_pos: i32,
        col_span: i32,
        row_align: GridAlign,
        row_pos: i32,
        row_span: i32,
    ) {
        if !self.obj_is_valid(obj) {
            return;
        }
        let selector = StyleSelector::DEFAULT;
        self.obj_set_style_grid_cell_column_pos(obj, col_pos, selector);
        self.obj_set_style_grid_cell_row_pos(obj, row_pos, selector);
        self.obj_set_style_grid_cell_x_align(obj, column_align, selector);
        self.obj_set_style_grid_cell_column_span(obj, col_span, selector);
        self.obj_set_style_grid_cell_row_span(obj, row_span, selector);
        self.obj_set_style_grid_cell_y_align(obj, row_align, selector);
        if let Some(parent) = self.obj_get_parent(obj) {
            self.obj_mark_layout_as_dirty(parent);
        }
    }
}

struct GridCalc {
    x: Vec<i32>,
    y: Vec<i32>,
    w: Vec<i32>,
    h: Vec<i32>,
}

fn calc(ui: &Ui, obj: Obj) -> Option<GridCalc> {
    ui.obj_get_child(obj, 0)?;
    // Upstream evaluates rows before columns.
    let h = calc_tracks(ui, obj, false)?;
    let w = calc_tracks(ui, obj, true)?;
    let data = ui.obj(obj)?;
    let mut grid = GridCalc {
        x: vec![0; w.len()],
        y: vec![0; h.len()],
        w,
        h,
    };
    grid_align(
        ui.obj_get_content_width(obj),
        ui.obj_get_style_width(obj, Part::MAIN) == coord::SIZE_CONTENT && !data.w_layout,
        ui.obj_get_style_grid_column_align(obj, Part::MAIN),
        ui.obj_get_style_pad_column(obj, Part::MAIN),
        &grid.w,
        &mut grid.x,
        ui.obj_get_style_base_dir(obj, Part::MAIN) == BaseDir::Rtl,
    );
    grid_align(
        ui.obj_get_content_height(obj),
        ui.obj_get_style_height(obj, Part::MAIN) == coord::SIZE_CONTENT && !data.h_layout,
        ui.obj_get_style_grid_row_align(obj, Part::MAIN),
        ui.obj_get_style_pad_row(obj, Part::MAIN),
        &grid.h,
        &mut grid.y,
        false,
    );
    Some(grid)
}

fn descriptor(ui: &Ui, obj: Obj, width: bool) -> Option<TracksRef> {
    if width {
        ui.obj_get_style_grid_column_dsc_array(obj, Part::MAIN)
    } else {
        ui.obj_get_style_grid_row_dsc_array(obj, Part::MAIN)
    }
}

fn cell_pos(ui: &Ui, obj: Obj, width: bool) -> i32 {
    if width {
        ui.obj_get_style_grid_cell_column_pos(obj, Part::MAIN)
    } else {
        ui.obj_get_style_grid_cell_row_pos(obj, Part::MAIN)
    }
}

fn cell_span(ui: &Ui, obj: Obj, width: bool) -> i32 {
    if width {
        ui.obj_get_style_grid_cell_column_span(obj, Part::MAIN)
    } else {
        ui.obj_get_style_grid_cell_row_span(obj, Part::MAIN)
    }
}

fn skip_item(ui: &Ui, obj: Obj) -> bool {
    ui.obj_is_ignore_layout(obj) || ui.obj_is_hidden(obj) || ui.obj_is_floating(obj)
}

/// The symmetric `calc_cols`/`calc_rows` calculations share this axis helper.
fn calc_tracks(ui: &Ui, obj: Obj, width: bool) -> Option<Vec<i32>> {
    let own = descriptor(ui, obj, width);
    let (template, start, end) = if let Some(template) = own {
        let count = template.iter().position(|&value| value == GRID_TEMPLATE_LAST)?;
        (template, 0, count)
    } else {
        let parent = ui.obj_get_parent(obj)?;
        let template = descriptor(ui, parent, width)?;
        let count = template.iter().position(|&value| value == GRID_TEMPLATE_LAST)?;
        let start = usize::try_from(cell_pos(ui, obj, width)).ok()?;
        let span = usize::try_from(cell_span(ui, obj, width)).ok()?;
        let end = start.checked_add(span)?;
        // C assumes a valid descriptor and subgrid range. Reject malformed
        // slices instead of reading past their allocation or indexing -1.
        if end > count {
            return None;
        }
        (template, start, end)
    };
    let tracks = &template[start..end];
    if tracks.is_empty() {
        return None;
    }
    let mut sizes = vec![0; tracks.len()];
    for (index, &track) in tracks.iter().enumerate() {
        if track != GRID_CONTENT {
            continue;
        }
        let mut size = coord::MIN;
        for child_index in 0..ui.obj_get_child_count(obj) {
            let Some(item) = ui.obj_get_child(obj, child_index as i32) else {
                continue;
            };
            if skip_item(ui, item)
                || cell_span(ui, item, width) != 1
                || cell_pos(ui, item, width) != index as i32
            {
                continue;
            }
            size = size.max(if width {
                ui.obj_get_width(item)
            } else {
                ui.obj_get_height(item)
            });
        }
        sizes[index] = size.max(0);
    }
    let mut fractions = 0;
    let mut grid_size = 0;
    for (index, &track) in tracks.iter().enumerate() {
        if track >= grid_fr(0) {
            fractions += track - grid_fr(0);
        } else if track == GRID_CONTENT {
            grid_size += sizes[index];
        } else {
            sizes[index] = track;
            grid_size += track;
        }
    }
    let (content, gap) = if width {
        (
            ui.obj_get_content_width(obj),
            ui.obj_get_style_pad_column(obj, Part::MAIN),
        )
    } else {
        (
            ui.obj_get_content_height(obj),
            ui.obj_get_style_pad_row(obj, Part::MAIN),
        )
    };
    let mut remaining = (content - gap * (tracks.len() as i32 - 1) - grid_size).max(0);
    for (index, &track) in tracks.iter().enumerate() {
        if fractions == 0 {
            break;
        }
        if track >= grid_fr(0) {
            let fraction = track - grid_fr(0);
            sizes[index] = (remaining * fraction + fractions / 2) / fractions;
            fractions -= fraction;
            remaining -= sizes[index];
        }
    }
    Some(sizes)
}

fn item_repos(ui: &mut Ui, item: Obj, grid: &GridCalc, origin: Point) {
    if skip_item(ui, item) {
        return;
    }
    let Some(parent) = ui.obj_get_parent(item) else {
        return;
    };
    let reverse = ui.obj_get_style_base_dir(parent, Part::MAIN) == BaseDir::Rtl;
    let mut col = cell_pos(ui, item, true).clamp(0, grid.w.len() as i32 - 1) as usize;
    let row = cell_pos(ui, item, false).clamp(0, grid.h.len() as i32 - 1) as usize;
    let cols = (cell_span(ui, item, true).max(1) as usize).min(grid.w.len() - col);
    let rows = (cell_span(ui, item, false).max(1) as usize).min(grid.h.len() - row);
    let cell_width = if reverse && cols > 1 {
        grid.x[col] + grid.w[col] - grid.x[col + cols - 1]
    } else {
        grid.x[col + cols - 1] + grid.w[col + cols - 1] - grid.x[col]
    };
    let cell_height = grid.y[row + rows - 1] + grid.h[row + rows - 1] - grid.y[row];
    let mut col_align = ui.obj_get_style_grid_cell_x_align(item, Part::MAIN);
    let row_align = ui.obj_get_style_grid_cell_y_align(item, Part::MAIN);
    if ui.obj_get_style_base_dir(item, Part::MAIN) == BaseDir::Rtl {
        col_align = match col_align {
            GridAlign::Start => GridAlign::End,
            GridAlign::End => GridAlign::Start,
            value => value,
        };
    }
    let Some(old_coords) = ui.obj(item).map(|data| data.coords) else {
        return;
    };
    let (mut width, mut height) = (old_coords.width(), old_coords.height());
    if reverse && cols > 1 {
        col += cols - 1;
    }
    let left = ui.obj_get_style_margin_left(item, Part::MAIN);
    let right = ui.obj_get_style_margin_right(item, Part::MAIN);
    let top = ui.obj_get_style_margin_top(item, Part::MAIN);
    let bottom = ui.obj_get_style_margin_bottom(item, Part::MAIN);
    let mut x = match col_align {
        GridAlign::Stretch => {
            width = cell_width - left - right;
            grid.x[col] + left
        }
        GridAlign::Center => grid.x[col] + (cell_width - width) / 2 + (left - right) / 2,
        GridAlign::End => grid.x[col] + cell_width - width - right,
        _ => grid.x[col] + left,
    };
    let mut y = match row_align {
        GridAlign::Stretch => {
            height = cell_height - top - bottom;
            grid.y[row] + top
        }
        GridAlign::Center => grid.y[row] + (cell_height - height) / 2 + (top - bottom) / 2,
        GridAlign::End => grid.y[row] + cell_height - height - bottom,
        _ => grid.y[row] + top,
    };
    if let Some(data) = ui.obj_mut(item) {
        data.w_layout = col_align == GridAlign::Stretch;
        data.h_layout = row_align == GridAlign::Stretch;
    }
    if old_coords.width() != width || old_coords.height() != height {
        ui.obj_invalidate(item);
        if let Some(data) = ui.obj_mut(item) {
            data.coords.set_width(width);
            data.coords.set_height(height);
        }
        ui.obj_invalidate(item);
        ui.obj_send_event(item, EventCode::SIZE_CHANGED, EventParam::Area(old_coords));
        if !ui.obj_is_valid(item) {
            return;
        }
        if let Some(parent) = ui.obj_get_parent(item) {
            ui.obj_send_event(parent, EventCode::CHILD_CHANGED, EventParam::Obj(item));
        }
    }
    if !ui.obj_is_valid(item) {
        return;
    }
    let mut tx = ui.obj_get_style_translate_x(item, Part::MAIN);
    let mut ty = ui.obj_get_style_translate_y(item, Part::MAIN);
    if coord::is_pct(tx) {
        tx = ui.obj_get_width(item) * coord::get_pct(tx) / 100;
    }
    if coord::is_pct(ty) {
        ty = ui.obj_get_height(item) * coord::get_pct(ty) / 100;
    }
    x += tx;
    y += ty;
    let coords = ui.obj_get_coords(item);
    let dx = origin.x + x - coords.x1;
    let dy = origin.y + y - coords.y1;
    if dx != 0 || dy != 0 {
        ui.obj_invalidate(item);
        if let Some(data) = ui.obj_mut(item) {
            data.coords.move_by(dx, dy);
        }
        ui.obj_invalidate(item);
        ui.obj_move_children_by(item, dx, dy, false);
    }
}

fn grid_align(
    content: i32,
    auto: bool,
    mut align: GridAlign,
    mut gap: i32,
    sizes: &[i32],
    positions: &mut [i32],
    reverse: bool,
) -> i32 {
    if auto {
        positions[0] = 0;
    } else {
        if matches!(
            align,
            GridAlign::SpaceAround | GridAlign::SpaceBetween | GridAlign::SpaceEvenly
        ) {
            gap = 0;
            if sizes.len() == 1 {
                align = GridAlign::Center;
            }
        }
        let size: i32 = sizes.iter().map(|size| size + gap).sum::<i32>() - gap;
        positions[0] = match align {
            GridAlign::Start | GridAlign::Stretch => 0,
            GridAlign::Center => (content - size) / 2,
            GridAlign::End => content - size,
            GridAlign::SpaceBetween => {
                gap = (content - size) / (sizes.len() as i32 - 1);
                0
            }
            GridAlign::SpaceAround => {
                gap = (content - size) / sizes.len() as i32;
                gap / 2
            }
            GridAlign::SpaceEvenly => {
                gap = (content - size) / (sizes.len() as i32 + 1);
                gap
            }
        };
    }
    for index in 0..sizes.len() - 1 {
        positions[index + 1] = positions[index] + sizes[index] + gap;
    }
    let total = positions[sizes.len() - 1] + sizes[sizes.len() - 1] - positions[0];
    if reverse {
        for (position, size) in positions.iter_mut().zip(sizes) {
            *position = content - *position - size;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Area, UiConfig};

    fn setup(width: i32, height: i32) -> (Ui, Obj) {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(width, height);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        (ui, screen)
    }

    fn tracks(values: &[i32]) -> TracksRef {
        TracksRef::Shared(alloc::rc::Rc::from(values))
    }

    fn item(ui: &mut Ui, parent: Obj, width: i32, height: i32, col: i32, row: i32) -> Obj {
        let item = ui.obj_create(Some(parent));
        ui.obj_set_size(item, width, height);
        ui.obj_set_grid_cell(item, GridAlign::Start, col, 1, GridAlign::Start, row, 1);
        item
    }

    #[test]
    fn grid_consts() {
        assert_eq!(grid_fr(1), coord::MAX - 99);
        assert!(GRID_CONTENT < grid_fr(0));
        assert_eq!(GRID_TEMPLATE_LAST, coord::MAX);
        assert_eq!(GridAlign::SpaceBetween as u8, 6);
    }

    #[test]
    fn upstream_fractional_tracks_round_remaining_space_without_holes() {
        // test_grid_fr.c: fixed first tracks and unequal FR columns cover the
        // complete 800x480 screen, including the rounding remainder.
        let (mut ui, screen) = setup(800, 480);
        ui.obj_set_grid_dsc_array(
            screen,
            tracks(&[
                27,
                grid_fr(1),
                grid_fr(2),
                grid_fr(3),
                grid_fr(4),
                GRID_TEMPLATE_LAST,
            ]),
            tracks(&[
                9,
                grid_fr(1),
                grid_fr(1),
                grid_fr(1),
                grid_fr(1),
                GRID_TEMPLATE_LAST,
            ]),
        );
        let mut children = Vec::new();
        for col in 0..5 {
            for row in 0..5 {
                let child = item(&mut ui, screen, 1, 1, col, row);
                ui.obj_set_grid_cell(child, GridAlign::Stretch, col, 1, GridAlign::Stretch, row, 1);
                children.push(child);
            }
        }
        ui.obj_update_layout(screen);
        let expected_w = [27, 77, 155, 232, 309];
        let expected_h = [9, 118, 118, 118, 117];
        let mut x = 0;
        for (col, width) in expected_w.into_iter().enumerate() {
            let mut y = 0;
            for (row, height) in expected_h.into_iter().enumerate() {
                let child = children[col * 5 + row];
                assert_eq!(
                    ui.obj_get_coords(child),
                    Area::new(x, y, x + width - 1, y + height - 1)
                );
                assert!(ui.obj(child).unwrap().w_layout && ui.obj(child).unwrap().h_layout);
                y += height;
            }
            assert_eq!(y, 480);
            x += width;
        }
        assert_eq!(x, 800);
    }

    #[test]
    fn content_tracks_ignore_spans_hidden_floating_and_ignored_children() {
        let (mut ui, screen) = setup(300, 100);
        ui.obj_set_grid_dsc_array(
            screen,
            tracks(&[GRID_CONTENT, grid_fr(1), GRID_TEMPLATE_LAST]),
            tracks(&[GRID_CONTENT, GRID_TEMPLATE_LAST]),
        );
        ui.obj_set_style_pad_column(screen, 5, StyleSelector::DEFAULT);
        let first = item(&mut ui, screen, 40, 12, 0, 0);
        // CONTENT is the child's size, not the margin box.
        ui.obj_set_style_margin_left(first, 10, StyleSelector::DEFAULT);
        let span = item(&mut ui, screen, 200, 18, 0, 0);
        ui.obj_set_grid_cell(span, GridAlign::Start, 0, 2, GridAlign::Start, 0, 1);
        let hidden = item(&mut ui, screen, 200, 80, 0, 0);
        ui.obj_set_hidden(hidden, true);
        let floating = item(&mut ui, screen, 210, 85, 0, 0);
        ui.obj_set_floating(floating, true);
        let ignored = item(&mut ui, screen, 220, 90, 0, 0);
        ui.obj_set_ignore_layout(ignored, true);
        ui.obj_update_layout(screen);
        let grid = calc(&ui, screen).unwrap();
        assert_eq!(grid.w, [40, 255]);
        assert_eq!(grid.h, [18]);
        assert_eq!(ui.obj_get_x(first), 10);
    }

    #[test]
    fn spans_margins_percent_translation_and_descendant_movement() {
        let (mut ui, screen) = setup(200, 120);
        ui.obj_set_grid_dsc_array(
            screen,
            tracks(&[40, 60, GRID_TEMPLATE_LAST]),
            tracks(&[50, GRID_TEMPLATE_LAST]),
        );
        ui.obj_set_style_pad_column(screen, 5, StyleSelector::DEFAULT);
        let child = item(&mut ui, screen, 20, 10, 0, 0);
        ui.obj_set_grid_cell(child, GridAlign::Center, 0, 2, GridAlign::Stretch, 0, 1);
        let descendant = ui.obj_create(Some(child));
        ui.obj_set_pos(descendant, 2, 3);
        ui.obj_set_style_margin_left(child, 10, StyleSelector::DEFAULT);
        ui.obj_set_style_margin_right(child, 4, StyleSelector::DEFAULT);
        ui.obj_set_style_margin_top(child, 6, StyleSelector::DEFAULT);
        ui.obj_set_style_margin_bottom(child, 2, StyleSelector::DEFAULT);
        ui.obj_set_style_translate_x(child, coord::pct(50), StyleSelector::DEFAULT);
        ui.obj_set_style_translate_y(child, coord::pct(50), StyleSelector::DEFAULT);
        ui.obj_update_layout(screen);
        assert_eq!(ui.obj_get_coords(child), Area::new(55, 27, 74, 68));
        assert_eq!(ui.obj_get_coords(descendant).x1, 57);
        assert_eq!(ui.obj_get_coords(descendant).y1, 30);
        assert!(!ui.obj(child).unwrap().w_layout);
        assert!(ui.obj(child).unwrap().h_layout);
    }

    #[test]
    fn rtl_reverses_tracks_and_multicolumn_cells_and_swaps_item_alignment() {
        let (mut ui, screen) = setup(200, 100);
        ui.obj_set_grid_dsc_array(
            screen,
            tracks(&[40, 60, 80, GRID_TEMPLATE_LAST]),
            tracks(&[50, GRID_TEMPLATE_LAST]),
        );
        ui.obj_set_style_pad_column(screen, 10, StyleSelector::DEFAULT);
        ui.obj_set_style_base_dir(screen, BaseDir::Rtl, StyleSelector::DEFAULT);
        let child = item(&mut ui, screen, 20, 10, 0, 0);
        ui.obj_set_grid_cell(child, GridAlign::Start, 0, 2, GridAlign::Start, 0, 1);
        ui.obj_update_layout(screen);
        assert_eq!(calc(&ui, screen).unwrap().x, [160, 90, 0]);
        assert_eq!(ui.obj_get_coords(child), Area::new(180, 0, 199, 9));
        ui.obj_set_grid_cell(child, GridAlign::Stretch, 0, 2, GridAlign::Start, 0, 1);
        ui.obj_update_layout(screen);
        assert_eq!(ui.obj_get_coords(child), Area::new(90, 0, 199, 9));
    }

    #[test]
    fn subgrid_uses_parent_descriptor_slice_for_missing_axis() {
        let (mut ui, screen) = setup(300, 200);
        ui.obj_set_grid_dsc_array(
            screen,
            tracks(&[40, 60, 80, GRID_TEMPLATE_LAST]),
            tracks(&[30, 50, 70, GRID_TEMPLATE_LAST]),
        );
        let child = item(&mut ui, screen, 1, 1, 1, 1);
        ui.obj_set_grid_cell(child, GridAlign::Stretch, 1, 2, GridAlign::Stretch, 1, 2);
        ui.obj_set_grid_dsc_array(child, None, None);
        let nested = item(&mut ui, child, 1, 1, 1, 1);
        ui.obj_set_grid_cell(nested, GridAlign::Stretch, 1, 1, GridAlign::Stretch, 1, 1);
        ui.obj_update_layout(screen);
        assert_eq!(ui.obj_get_coords(child), Area::new(40, 30, 179, 149));
        assert_eq!(ui.obj_get_coords(nested), Area::new(100, 80, 179, 149));
        assert_eq!(calc(&ui, child).unwrap().w, [60, 80]);
        assert_eq!(calc(&ui, child).unwrap().h, [50, 70]);
    }

    #[test]
    fn track_alignment_preserves_upstream_integer_rounding_and_auto_size() {
        for (align, expected) in [
            (GridAlign::Start, [0, 25]),
            (GridAlign::Center, [27, 52]),
            (GridAlign::End, [55, 80]),
            (GridAlign::SpaceBetween, [0, 80]),
            (GridAlign::SpaceAround, [15, 65]),
            (GridAlign::SpaceEvenly, [20, 60]),
        ] {
            let mut positions = [0; 2];
            grid_align(100, false, align, 5, &[20, 20], &mut positions, false);
            assert_eq!(positions, expected);
            grid_align(100, true, align, 5, &[20, 20], &mut positions, false);
            assert_eq!(positions, [0, 25]);
        }
        for align in [
            GridAlign::SpaceBetween,
            GridAlign::SpaceAround,
            GridAlign::SpaceEvenly,
        ] {
            let mut position = [0];
            grid_align(101, false, align, 9, &[20], &mut position, false);
            assert_eq!(position, [40]);
        }
    }

    #[test]
    fn content_size_delegates_to_child_bounds_and_rejects_invalid_descriptors() {
        let (mut ui, screen) = setup(300, 100);
        let container = ui.obj_create(Some(screen));
        ui.obj_set_size(container, coord::SIZE_CONTENT, coord::SIZE_CONTENT);
        ui.obj_set_grid_dsc_array(
            container,
            tracks(&[40, 60, GRID_TEMPLATE_LAST]),
            tracks(&[30, GRID_TEMPLATE_LAST]),
        );
        ui.obj_set_style_pad_column(container, 5, StyleSelector::DEFAULT);
        for col in 0..2 {
            let child = item(&mut ui, container, 1, 1, col, 0);
            ui.obj_set_grid_cell(child, GridAlign::Stretch, col, 1, GridAlign::Stretch, 0, 1);
        }
        ui.obj_update_layout(screen);
        assert_eq!(
            (ui.obj_get_width(container), ui.obj_get_height(container)),
            (105, 30)
        );
        let mut requested = 123;
        assert!(!grid_get_min_size(&mut ui, container, &mut requested, true));
        assert_eq!(requested, 123);
        ui.obj_set_grid_dsc_array(
            container,
            tracks(&[GRID_TEMPLATE_LAST]),
            tracks(&[30, GRID_TEMPLATE_LAST]),
        );
        assert!(calc(&ui, container).is_none());
        ui.obj_set_grid_dsc_array(container, tracks(&[40]), tracks(&[30, GRID_TEMPLATE_LAST]));
        assert!(calc(&ui, container).is_none());
        ui.obj_set_grid_dsc_array(container, None, None);
        assert!(calc(&ui, container).is_none());
    }

    #[test]
    fn invalid_cell_positions_and_spans_are_clamped_like_upstream() {
        let (mut ui, screen) = setup(100, 100);
        ui.obj_set_grid_dsc_array(
            screen,
            tracks(&[40, 60, GRID_TEMPLATE_LAST]),
            tracks(&[30, 70, GRID_TEMPLATE_LAST]),
        );
        let child = item(&mut ui, screen, 1, 1, 0, 0);
        for (col, cols, row, rows, expected) in [
            (-20, -10, -30, 0, Area::new(0, 0, 39, 29)),
            (50, 20, 60, 20, Area::new(40, 30, 99, 99)),
            (0, i32::MAX, 0, i32::MAX, Area::new(0, 0, 99, 99)),
        ] {
            ui.obj_set_grid_cell(
                child,
                GridAlign::Stretch,
                col,
                cols,
                GridAlign::Stretch,
                row,
                rows,
            );
            ui.obj_update_layout(screen);
            assert_eq!(ui.obj_get_coords(child), expected);
        }
    }

    #[test]
    fn size_and_child_callbacks_can_delete_grid_items_and_container() {
        for delete_container in [false, true] {
            let (mut ui, screen) = setup(100, 100);
            let container = ui.obj_create(Some(screen));
            ui.obj_set_size(container, 100, 100);
            ui.obj_set_grid_dsc_array(
                container,
                tracks(&[100, GRID_TEMPLATE_LAST]),
                tracks(&[100, GRID_TEMPLATE_LAST]),
            );
            let child = item(&mut ui, container, 1, 1, 0, 0);
            ui.obj_set_grid_cell(child, GridAlign::Stretch, 0, 1, GridAlign::Stretch, 0, 1);
            // Settle the explicit 1x1 size before the grid stretches it.
            ui.obj_refr_size(child);
            if delete_container {
                ui.obj_add_event_cb(
                    container,
                    alloc::rc::Rc::new(move |ui, _| ui.obj_delete(container)),
                    EventCode::CHILD_CHANGED,
                    None,
                );
            } else {
                ui.obj_add_event_cb(
                    child,
                    alloc::rc::Rc::new(move |ui, _| ui.obj_delete(child)),
                    EventCode::SIZE_CHANGED,
                    None,
                );
            }
            grid_update(&mut ui, container);
            assert!(!ui.obj_is_valid(child));
            assert_eq!(ui.obj_is_valid(container), !delete_container);
        }
    }

    #[test]
    fn content_self_size_callback_can_delete_grid_container() {
        let (mut ui, screen) = setup(100, 100);
        let container = ui.obj_create(Some(screen));
        ui.obj_set_size(container, coord::SIZE_CONTENT, coord::SIZE_CONTENT);
        ui.obj_set_grid_dsc_array(
            container,
            tracks(&[40, GRID_TEMPLATE_LAST]),
            tracks(&[30, GRID_TEMPLATE_LAST]),
        );
        item(&mut ui, container, 40, 30, 0, 0);
        ui.obj_add_event_cb(
            container,
            alloc::rc::Rc::new(move |ui, _| ui.obj_delete(container)),
            EventCode::GET_SELF_SIZE,
            None,
        );
        grid_update(&mut ui, container);
        assert!(!ui.obj_is_valid(container));
    }
}

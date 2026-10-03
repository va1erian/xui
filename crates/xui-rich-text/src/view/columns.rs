#![forbid(unsafe_code)]

//! Dragging a table's column edge: a guide follows the pointer and the
//! release sets the two neighbouring columns' widths in one undoable step.

use xui_core::Dip;
use xui_core::geometry::{Point, Rect};

use super::drag::Drag;
use super::events::Out;
use super::state::State;
use crate::edit::Command;

/// How close to a column edge, in design units, the pointer grabs it.
const GRAB: Dip = Dip(3.0);
/// The narrowest a column may be dragged to.
const MIN_COLUMN: Dip = Dip(24.0);

impl State {
    /// The table column edge under the client point: the table's index in
    /// the layout and the edge's index.
    pub(super) fn column_edge_at(&self, point: Point) -> Option<(usize, usize)> {
        let slop = GRAB.0 * self.layout.dpi() as f32 / 96.0;
        self.layout.column_edge_at(self.to_layout(point), slop)
    }

    /// Starts dragging edge `edge` of the layout's table `table`.
    pub(super) fn begin_column_drag(&mut self, table: usize, edge: usize) {
        let x = self.layout.tables()[table].edges[edge];
        self.drag = Some(Drag::Column { table, edge, x });
        self.show_column_guide(table, x);
    }

    /// Follows the pointer with the dragged edge, kept a minimum column
    /// width from its neighbours.
    pub(super) fn drag_column(&mut self, table: usize, edge: usize, to: Point) {
        let Some(t) = self.layout.tables().get(table) else {
            return;
        };
        let min = MIN_COLUMN.0 * self.layout.dpi() as f32 / 96.0;
        let (lo, hi) = (t.edges[edge - 1] + min, t.edges[edge + 1] - min);
        let x = (self.to_layout(to).x as f32).clamp(lo, hi.max(lo));
        self.drag = Some(Drag::Column { table, edge, x });
        self.show_column_guide(table, x);
    }

    fn show_column_guide(&mut self, table: usize, x: f32) {
        let t = &self.layout.tables()[table];
        let x = x.round() as i32;
        self.column_guide = Some(Rect::new(
            x,
            t.top().round() as i32,
            x + 1,
            t.bottom().round() as i32,
        ));
    }

    /// Commits the drag: the columns either side of the edge share their
    /// combined width at the new edge.
    pub(super) fn end_column_drag(&mut self, table: usize, edge: usize, x: f32, out: &mut Out) {
        self.column_guide = None;
        let Some(t) = self.layout.tables().get(table) else {
            return;
        };
        let id = t.id;
        let mut widths: Vec<f32> = t.edges.windows(2).map(|w| w[1] - w[0]).collect();
        widths[edge - 1] = x - t.edges[edge - 1];
        widths[edge] = t.edges[edge + 1] - x;
        let Some(table) = self.ed.doc.tables().get(id) else {
            return;
        };
        let table = table.clone().with_widths(&widths);
        out.absorb(&self.run(Command::SetTable { id, table }));
    }
}

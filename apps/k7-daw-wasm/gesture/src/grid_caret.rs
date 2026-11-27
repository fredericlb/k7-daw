// grid_caret.rs — the timeline's caret and `v` rectangle on a plain GRID: the piano roll's pitches by the ruler's steps.
// Row 0 is the TOP row; a box is INCLUSIVE at both ends, a column span half-open.

use crate::js;

/// One cell by index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub row: f64,
    pub col: f64,
}

/// How big the grid is: a surface with no rows or no columns has no cell to put a caret in.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub rows: f64,
    pub cols: f64,
}

/// A selection as a normalised box, inclusive at both ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridBox {
    pub row_lo: f64,
    pub row_hi: f64,
    pub col_lo: f64,
    pub col_hi: f64,
}

/// `c` pinned inside the grid: clamped rather than wrapped, and an empty grid gives (0, 0).
pub fn clamp_cell(c: Cell, b: Bounds) -> Cell {
    Cell {
        row: js::max(0.0, js::min(js::round(c.row), js::max(0.0, b.rows - 1.0))),
        col: js::max(0.0, js::min(js::round(c.col), js::max(0.0, b.cols - 1.0))),
    }
}

/// `c` moved by a whole number of rows and columns, clamped.
pub fn move_cell(c: Cell, d_row: f64, d_col: f64, b: Bounds) -> Cell {
    clamp_cell(
        Cell {
            row: c.row + d_row,
            col: c.col + d_col,
        },
        b,
    )
}

/// The rectangle two cells describe, whichever end was dragged.
pub fn sel_box(anchor: Cell, head: Cell) -> GridBox {
    GridBox {
        row_lo: js::min(anchor.row, head.row),
        row_hi: js::max(anchor.row, head.row),
        col_lo: js::min(anchor.col, head.col),
        col_hi: js::max(anchor.col, head.col),
    }
}

/// The caret's own cell as a one-by-one box, what a copy falls back on.
pub fn cell_box(c: Cell) -> GridBox {
    GridBox {
        row_lo: c.row,
        row_hi: c.row,
        col_lo: c.col,
        col_hi: c.col,
    }
}

/// The selection's span in COLUMNS, half-open: from column 4 to column 4 is one cell WIDE, not zero.
pub fn col_span(anchor: Cell, head: Cell) -> (f64, f64) {
    let b = sel_box(anchor, head);
    (b.col_lo, b.col_hi + 1.0)
}

/// A grid column in seconds of the song, the unit every transport call takes; the grid's own origin cancels.
pub fn song_seconds_of_col(col: f64, unit: f64, clip_start: f64, tempo: f64) -> f64 {
    ((clip_start + col * unit) * 60.0) / js::max(1e-6, tempo)
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Bounds = Bounds { rows: 4.0, cols: 8.0 };
    fn at(row: f64, col: f64) -> Cell {
        Cell { row, col }
    }

    #[test]
    fn the_caret_moves_by_whole_cells() {
        assert_eq!(move_cell(at(1.0, 2.0), 1.0, -1.0, B), at(2.0, 1.0));
    }

    #[test]
    fn the_caret_stops_at_the_ends_rather_than_wrapping() {
        assert_eq!(move_cell(at(0.0, 0.0), -1.0, -1.0, B), at(0.0, 0.0));
        assert_eq!(move_cell(at(3.0, 7.0), 1.0, 1.0, B), at(3.0, 7.0));
    }

    #[test]
    fn the_caret_is_pinned_back_inside_a_grid_that_shrank_under_it() {
        assert_eq!(clamp_cell(at(9.0, 9.0), B), at(3.0, 7.0));
    }

    #[test]
    fn an_empty_grid_gives_a_real_cell_rather_than_a_negative_index() {
        assert_eq!(clamp_cell(at(2.0, 2.0), Bounds { rows: 0.0, cols: 0.0 }), at(0.0, 0.0));
    }

    #[test]
    fn the_rectangle_reads_the_same_whichever_end_was_dragged() {
        assert_eq!(sel_box(at(1.0, 2.0), at(3.0, 5.0)), sel_box(at(3.0, 5.0), at(1.0, 2.0)));
        let b = sel_box(at(1.0, 2.0), at(3.0, 5.0));
        assert_eq!((b.row_lo, b.row_hi, b.col_lo, b.col_hi), (1.0, 3.0, 2.0, 5.0));
    }

    #[test]
    fn the_carets_own_cell_is_a_rectangle() {
        assert_eq!(cell_box(at(2.0, 3.0)), sel_box(at(2.0, 3.0), at(2.0, 3.0)));
    }

    #[test]
    fn the_span_a_transport_reads_runs_to_the_far_edge_of_the_last_column() {
        assert_eq!(col_span(at(0.0, 4.0), at(0.0, 4.0)), (4.0, 5.0));
        assert_eq!(col_span(at(0.0, 2.0), at(1.0, 6.0)), (2.0, 7.0));
        assert_eq!(col_span(at(0.0, 6.0), at(0.0, 2.0)), (2.0, 7.0));
    }

    #[test]
    fn a_column_is_seconds_of_the_song() {
        assert_eq!(song_seconds_of_col(4.0, 0.5, 8.0, 120.0), 5.0);
        assert!(song_seconds_of_col(1.0, 1.0, 0.0, 0.0).is_finite());
    }
}

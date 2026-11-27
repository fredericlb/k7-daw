// The tests of the grid fold (once gridFit.ts's), with their inputs and expected values: a grid folds its overflow, never through a widget.

use super::*;

fn knob(col: f64, row: f64) -> Widget {
    Widget::at("knob", col, row)
}

fn panel(cols: f64, rows: f64, widgets: Vec<(String, Widget)>) -> Panel {
    Panel {
        cols,
        rows,
        widgets: widgets.into_iter().collect(),
        ..Panel::default()
    }
}

/// A 2-wide grid, one knob per cell, `rows` tall.
fn column(rows: usize) -> Panel {
    let widgets = (0..rows)
        .flat_map(|r| (0..2).map(move |c| (format!("k{r}{c}"), knob(c as f64, r as f64))))
        .collect();
    panel(2.0, rows as f64, widgets)
}

const ROWS6: GridFit = GridFit {
    axis: Axis::Rows,
    max: 6.0,
};

fn at(p: &Panel, key: &str) -> (f64, f64) {
    let w = &p.widgets[key];
    (w.col, w.row)
}

#[test]
fn leaves_a_grid_that_fits_untouched() {
    assert!(fit_panel(&column(6), ROWS6).is_none());
}

#[test]
fn folds_rows_past_the_cap_into_a_band_of_columns_on_the_right() {
    let out = fit_panel(&column(9), ROWS6).expect("folds");
    assert_eq!((out.rows, out.cols), (6.0, 4.0));
    // Row 6, the first to overflow, lands at the top of the second band.
    assert_eq!(at(&out, "k60"), (2.0, 0.0));
    assert_eq!(at(&out, "k81"), (3.0, 2.0));
    // …and what fitted stays where it was.
    assert_eq!(at(&out, "k51"), (1.0, 5.0));
}

#[test]
fn makes_as_many_bands_as_the_overflow_needs() {
    let out = fit_panel(&column(14), ROWS6).expect("folds");
    assert_eq!(out.cols, 6.0);
    assert_eq!(at(&out, "k130"), (4.0, 1.0));
}

#[test]
fn never_cuts_through_a_widget_it_moves_whole_the_band_ending_before_it() {
    let a = Widget {
        row_span: Some(5.0),
        ..knob(0.0, 0.0)
    };
    let tall = Widget {
        row_span: Some(3.0),
        ..knob(0.0, 5.0)
    };
    let ui = panel(1.0, 8.0, vec![("a".into(), a), ("tall".into(), tall)]);
    let out = fit_panel(&ui, ROWS6).expect("folds");
    let tall = &out.widgets["tall"];
    assert_eq!((tall.col, tall.row, tall.row_span), (1.0, 0.0, Some(3.0)));
    assert_eq!(out.rows, 5.0);
}

#[test]
fn folds_the_common_widgets_and_every_tabs_with_the_same_bands() {
    let tab = |name: &str, key: &str, w: Widget| Tab {
        name: name.into(),
        widgets: [(key.to_owned(), w)].into_iter().collect(),
        ..Tab::default()
    };
    let mut ui = panel(1.0, 8.0, vec![("common".into(), knob(0.0, 7.0))]);
    ui.tabs = Some(vec![tab("A", "a", knob(0.0, 6.0)), tab("B", "b", knob(0.0, 1.0))]);
    let out = fit_panel(&ui, ROWS6).expect("folds");
    assert_eq!(at(&out, "common"), (1.0, 1.0));
    let tabs = out.tabs.expect("tabs");
    assert_eq!((tabs[0].widgets["a"].col, tabs[0].widgets["a"].row), (1.0, 0.0));
    assert_eq!((tabs[1].widgets["b"].col, tabs[1].widgets["b"].row), (0.0, 1.0));
}

#[test]
fn folds_columns_past_the_cap_into_rows_below() {
    let widgets = (0..5).map(|c| (format!("c{c}"), knob(c as f64, 0.0))).collect();
    let out = fit_panel(
        &panel(5.0, 1.0, widgets),
        GridFit {
            axis: Axis::Cols,
            max: 3.0,
        },
    )
    .expect("folds");
    assert_eq!((out.cols, out.rows), (3.0, 2.0));
    assert_eq!(at(&out, "c3"), (0.0, 1.0));
    assert_eq!(at(&out, "c4"), (1.0, 1.0));
}

#[test]
fn runs_a_band_long_rather_than_split_a_widget_taller_than_the_cap() {
    let spans = [Span { at: 1.0, len: 5.0 }];
    assert_eq!(fold_bands(9.0, 3.0, &spans), vec![(0.0, 1.0), (1.0, 6.0), (6.0, 9.0)]);
}

fn long_tab(name: &str, cut: f64) -> Tab {
    let a = Widget {
        row_span: Some(2.0),
        ..knob(0.0, cut - 1.0)
    };
    Tab {
        name: name.into(),
        widgets: [(format!("{name}-a"), a), (format!("{name}-b"), knob(1.0, 9.0))]
            .into_iter()
            .collect(),
        ..Tab::default()
    }
}

#[test]
fn folds_each_tab_on_its_own_so_one_tabs_tall_widget_does_not_hold_every_other_back() {
    let mut ui = panel(2.0, 11.0, vec![("vol".into(), knob(0.0, 0.0))]);
    ui.tabs = Some(vec![long_tab("x", 3.0), long_tab("y", 6.0)]);
    let out = fit_panel(&ui, ROWS6).expect("folds");
    assert!(out.rows <= 6.0);
    assert_eq!(at(&out, "vol"), (0.0, 0.0));
}

#[test]
fn cuts_a_frame_into_one_piece_per_band_the_label_on_the_first() {
    let sec = Widget {
        col_span: Some(2.0),
        row_span: Some(10.0),
        label: Some("OSC".into()),
        ..Widget::at("group", 0.0, 0.0)
    };
    let mut widgets: Vec<(String, Widget)> = vec![("sec".into(), sec)];
    widgets.extend((0..10).map(|r| (format!("k{r}"), knob(0.0, r as f64))));
    let out = fit_panel(
        &panel(2.0, 10.0, widgets),
        GridFit {
            axis: Axis::Rows,
            max: 5.0,
        },
    )
    .expect("folds");
    assert_eq!(out.rows, 5.0);
    let first = &out.widgets["sec"];
    assert_eq!(
        (first.row, first.row_span, first.label.as_deref()),
        (0.0, Some(5.0), Some("OSC"))
    );
    // The knobs use one column, so the second band is one column wide, and the frame's second piece with it.
    let second = &out.widgets["sec~1"];
    assert_eq!(
        (second.row, second.row_span, second.col, second.col_span),
        (0.0, Some(5.0), 1.0, Some(1.0))
    );
    assert_eq!(out.cols, 2.0);
    assert!(second.label.is_none());
}

#[test]
fn shortens_a_screen_to_its_band_rather_than_letting_it_hold_the_cut() {
    let screen = Widget {
        col_span: Some(2.0),
        row_span: Some(8.0),
        ..Widget::at("lcdscreen", 1.0, 1.0)
    };
    let mut widgets: Vec<(String, Widget)> = vec![("top".into(), knob(0.0, 0.0)), ("screen".into(), screen)];
    widgets.extend((0..8).map(|r| (format!("k{r}"), knob(0.0, r as f64 + 1.0))));
    let out = fit_panel(&panel(3.0, 9.0, widgets), ROWS6).expect("folds");
    assert!(out.rows <= 6.0);
    assert_eq!(
        (out.widgets["screen"].row, out.widgets["screen"].row_span),
        (1.0, Some(5.0))
    );
}

#[test]
fn sizes_each_band_to_the_columns_its_widgets_use() {
    let mut widgets: Vec<(String, Widget)> = Vec::new();
    for r in 0..5 {
        for c in 0..6 {
            widgets.push((format!("k{r}{c}"), knob(c as f64, r as f64)));
        }
    }
    widgets.push(("x0".into(), knob(0.0, 5.0)));
    widgets.push(("x1".into(), knob(1.0, 6.0)));
    let out = fit_panel(
        &panel(6.0, 7.0, widgets),
        GridFit {
            axis: Axis::Rows,
            max: 5.0,
        },
    )
    .expect("folds");
    assert_eq!((out.rows, out.cols), (5.0, 8.0));
    assert_eq!(at(&out, "x0"), (6.0, 0.0));
    assert_eq!(at(&out, "x1"), (7.0, 1.0));
}

#[test]
fn records_the_columns_each_tab_needs_once_folded() {
    let wide = |name: &str, cols: usize| Tab {
        name: name.into(),
        widgets: (0..8)
            .flat_map(|r| (0..cols).map(move |c| (format!("{name}{r}{c}"), knob(c as f64, r as f64))))
            .collect(),
        ..Tab::default()
    };
    let mut ui = panel(6.0, 8.0, vec![]);
    ui.tabs = Some(vec![wide("narrow", 2), wide("broad", 6)]);
    let out = fit_panel(
        &ui,
        GridFit {
            axis: Axis::Rows,
            max: 4.0,
        },
    )
    .expect("folds");
    let tabs: Vec<Option<f64>> = out.tabs.expect("tabs").iter().map(|t| t.cols).collect();
    assert_eq!(tabs, vec![Some(4.0), Some(12.0)]);
    assert_eq!(out.cols, 12.0);
}

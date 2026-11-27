// The layout's rules: one row per rank, the master run beside, the pins, the pushes, a box's members, and that the engine never fails.

use super::arrange::{NODE_SEP, RANK_SEP, push_aside_frames, push_below_inputs};
use super::*;
use crate::node::{AssemblyOf, Signal};

fn card(id: &str, kind: Kind, rank: Option<f64>) -> Node {
    let mut n = Node::plain(id, id, kind);
    n.rank = rank;
    n
}

fn edge(s: &str, t: &str) -> Edge {
    Edge::new(&format!("{s}>{t}"), s, t)
}

fn at<'a>(l: &'a Laid, id: &str) -> &'a Node {
    l.nodes.iter().find(|n| n.id == id).expect("a node")
}

fn open_box(uses: &str) -> AssemblyOf {
    AssemblyOf {
        uses: uses.into(),
        open: true,
        members: vec![],
        plugs: Default::default(),
        params: None,
        missing: None,
    }
}

fn chain() -> (Vec<Node>, Vec<Edge>) {
    let nodes = vec![
        card("a", Kind::Track, Some(0.0)),
        card("b", Kind::Module, Some(1.0)),
        card("c", Kind::Module, Some(2.0)),
        card("d", Kind::Track, Some(0.0)),
        card("e", Kind::Module, Some(1.0)),
    ];
    (
        nodes,
        vec![edge("a", "b"), edge("b", "c"), edge("d", "e"), edge("e", "c")],
    )
}

#[test]
fn every_rank_sits_on_one_row_and_rows_never_overlap() {
    let (nodes, edges) = chain();
    let l = layout(nodes, edges, None, None);
    assert_eq!(y(at(&l, "a")), y(at(&l, "d")));
    assert_eq!(y(at(&l, "b")), y(at(&l, "e")));
    assert!(y(at(&l, "b")) >= y(at(&l, "a")) + at(&l, "a").h + RANK_SEP - 1e-9);
    assert!(y(at(&l, "c")) >= y(at(&l, "b")) + at(&l, "b").h + RANK_SEP - 1e-9);
}

#[test]
fn a_row_is_parted_where_two_cards_collide() {
    let (nodes, edges) = chain();
    let l = layout(nodes, edges, None, None);
    let (a, d) = (at(&l, "a"), at(&l, "d"));
    let (left, right) = if x(a) < x(d) { (a, d) } else { (d, a) };
    assert!(x(right) >= x(left) + left.w + NODE_SEP - 1e-9);
}

#[test]
fn disconnected_pieces_and_loners_all_get_a_place() {
    let nodes = vec![
        card("a", Kind::Module, None),
        card("b", Kind::Module, None),
        card("z", Kind::Module, None),
    ];
    let l = layout(nodes, vec![edge("a", "b")], None, None);
    assert_eq!(l.nodes.len(), 3);
    let z = at(&l, "z");
    let overlaps = |o: &Node| x(z) < x(o) + o.w && x(o) < x(z) + z.w && y(z) < y(o) + o.h && y(o) < y(z) + z.h;
    assert!(!overlaps(at(&l, "a")) && !overlaps(at(&l, "b")));
}

#[test]
fn a_cycle_a_self_loop_and_a_repeated_cable_do_not_stop_it() {
    let nodes = vec![card("a", Kind::Module, None), card("b", Kind::Module, None)];
    let edges = vec![
        edge("a", "b"),
        edge("b", "a"),
        edge("a", "a"),
        Edge::new("again", "a", "b"),
        edge("a", "ghost"),
    ];
    let l = layout(nodes, edges, None, None);
    assert_eq!(l.nodes.len(), 2);
    assert_eq!(l.edges.len(), 5);
    assert!(
        l.edges
            .iter()
            .all(|e| e.points.as_ref().is_some_and(|p| p.len() >= 2) || e.target == "ghost")
    );
}

#[test]
fn the_layout_is_the_same_every_time() {
    let (n, e) = chain();
    let first = layout(n.clone(), e.clone(), None, None);
    for _ in 0..20 {
        assert_eq!(layout(n.clone(), e.clone(), None, None), first);
    }
}

#[test]
fn a_pin_by_name_outranks_the_memory_and_the_memory_outranks_the_layout() {
    let (nodes, edges) = chain();
    let pins = HashMap::from([("b".to_owned(), Pt { x: 500.0, y: 800.0 })]);
    let memory = HashMap::from([
        ("b".to_owned(), Pt { x: 1.0, y: 1.0 }),
        ("e".to_owned(), Pt { x: 7.0, y: 900.0 }),
    ]);
    let l = layout(nodes, edges, Some(&pins), Some(&memory));
    assert_eq!((x(at(&l, "b")), y(at(&l, "b"))), (500.0, 800.0));
    assert_eq!((x(at(&l, "e")), y(at(&l, "e"))), (7.0, 900.0));
}

#[test]
fn a_receiver_is_pushed_below_what_feeds_it_after_a_pin() {
    let (nodes, edges) = chain();
    let pins = HashMap::from([("b".to_owned(), Pt { x: 0.0, y: 900.0 })]);
    let l = layout(nodes, edges, Some(&pins), None);
    assert!(y(at(&l, "c")) >= 900.0 + at(&l, "b").h - 1e-9);
}

#[test]
fn the_master_run_stands_right_of_the_rest_on_the_track_row() {
    let mut sum = card("snk:MIX", Kind::Sink, Some(0.0));
    sum.label = "MIX".into();
    let nodes = vec![
        card("t", Kind::Track, Some(0.0)),
        card("m", Kind::Module, Some(1.0)),
        sum,
        card("out", Kind::Sink, Some(1.0)),
    ];
    let l = layout(nodes, vec![edge("t", "m"), edge("snk:MIX", "out")], None, None);
    let rest_right = (x(at(&l, "t")) + 160.0).max(x(at(&l, "m")) + 160.0);
    assert!(x(at(&l, "snk:MIX")) >= rest_right + NODE_SEP - 1e-9);
    assert_eq!(y(at(&l, "snk:MIX")), y(at(&l, "t")));
}

#[test]
fn an_empty_graph_measures_zero() {
    let l = layout(vec![], vec![], None, None);
    assert_eq!((l.width, l.height, l.left, l.top), (0.0, 0.0, 0.0, 0.0));
}

#[test]
fn an_open_box_grows_around_its_members_and_they_stay_inside() {
    let mut frame = card("asm:r", Kind::Assembly, Some(1.0));
    frame.module = Some("r".into());
    frame.assembly_of = Some(open_box("r"));
    let mut i = card("i", Kind::Module, Some(0.0));
    let mut j = card("j", Kind::Module, Some(1.0));
    i.assembly = Some("r".into());
    j.assembly = Some("r".into());
    let nodes = vec![
        card("t", Kind::Track, Some(0.0)),
        frame,
        i,
        j,
        card("o", Kind::Module, Some(2.0)),
    ];
    let edges = vec![edge("t", "i"), edge("i", "j"), edge("j", "o")];
    let l = layout(nodes, edges, None, None);
    let f = at(&l, "asm:r");
    for id in ["i", "j"] {
        let m = at(&l, id);
        assert_eq!(m.assembly.as_deref(), Some("r"));
        assert!(x(m) >= x(f) && x(m) + m.w <= x(f) + f.w + 1e-9);
        assert!(y(m) >= y(f) + FRAME_HEAD - 1e-9 && y(m) + m.h <= y(f) + f.h + 1e-9);
    }
    assert_eq!(l.edges.len(), 3);
    assert!(l.edges.iter().all(|e| e.points.as_ref().is_some_and(|p| p.len() == 2)));
}

#[test]
fn a_member_whose_box_is_missing_lays_out_with_the_rest() {
    let mut i = card("i", Kind::Module, None);
    i.assembly = Some("nowhere".into());
    let l = layout(vec![i, card("t", Kind::Track, None)], vec![edge("t", "i")], None, None);
    assert_eq!(l.nodes.len(), 2);
}

#[test]
fn frames_push_the_cards_beside_them_aside_and_say_where_they_were() {
    let mut frame = card("f", Kind::Assembly, None);
    frame.assembly_of = Some(open_box("r"));
    (frame.x, frame.y, frame.w, frame.h) = (Some(0.0), Some(0.0), 300.0, 200.0);
    let mut right = card("r", Kind::Module, None);
    (right.x, right.y) = (Some(250.0), Some(10.0));
    let out = push_aside_frames(vec![frame, right]);
    assert_eq!(x(&out[1]), 300.0 + NODE_SEP);
    assert_eq!(out[1].pushed, Some(Pt { x: 250.0, y: 10.0 }));
}

#[test]
fn an_event_cable_pushes_nothing_down() {
    let mut a = card("a", Kind::Module, None);
    let mut b = card("b", Kind::Module, None);
    (a.y, b.y) = (Some(0.0), Some(0.0));
    let mut ev = edge("a", "b");
    ev.signal = Some(Signal::Event);
    let out = push_below_inputs(vec![a, b], &[ev]);
    assert_eq!(y(&out[1]), 0.0);
}

#[test]
fn a_thousand_random_cyclic_graphs_all_lay_out_and_none_hangs() {
    use std::sync::mpsc;
    let mut seed = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = |m: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % m
    };
    for round in 0..1000 {
        let n = 1 + next(60) as usize;
        let ranks: Vec<f64> = (0..n).map(|_| next(6) as f64).collect();
        let pairs: Vec<(usize, usize)> = (0..next(150))
            .map(|_| (next(n as u64) as usize, next(n as u64) as usize))
            .collect();
        let (tx, rx) = mpsc::channel();
        let (p2, r2) = (pairs.clone(), ranks.clone());
        std::thread::spawn(move || {
            let nodes: Vec<Node> = (0..n)
                .map(|i| card(&format!("n{i}"), Kind::Module, Some(r2[i])))
                .collect();
            let edges: Vec<Edge> = p2
                .iter()
                .enumerate()
                .map(|(k, (a, b))| Edge::new(&format!("e{k}"), &format!("n{a}"), &format!("n{b}")))
                .collect();
            let l = layout(nodes, edges, None, None);
            let _ = tx.send((
                l.nodes.len(),
                l.edges.len(),
                l.nodes.iter().all(|c| x(c).is_finite() && y(c).is_finite()),
            ));
        });
        // The engine loops forever on some cyclic graphs, so a hang is a failure, not a wait.
        match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(done) => assert_eq!(done, (n, pairs.len(), true), "round {round}"),
            Err(_) => panic!("HANG round {round} n={n} pairs={pairs:?}"),
        }
    }
}

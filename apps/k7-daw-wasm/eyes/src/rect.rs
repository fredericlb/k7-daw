// rect.rs — what a grip does to a layer's rect when it is dragged in the frame: the edges it names move, the others stay.
// A reader would get wrong that a resize is a move in disguise: it is not, and a side never shrinks below `MIN_SIDE`.

use crate::{js_max, js_min};

/// Never let a resize invert or vanish a layer: below this it is unclickable and unrecoverable.
pub const MIN_SIDE: f64 = 16.0;

/// A layer's rect in VIEWPORT px, which is exactly what a picture lane's `frame` states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// What a press grabbed: the body (a move) or one of the eight edges and corners (a resize).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    Body,
    N,
    S,
    E,
    W,
    Ne,
    Nw,
    Se,
    Sw,
}

impl Grip {
    /// The grip a code names, the order of the page's `GRIPS` list; none for a code past it.
    pub fn from_code(code: u32) -> Option<Grip> {
        use Grip::*;
        [Body, N, S, E, W, Ne, Nw, Se, Sw].get(code as usize).copied()
    }
}

/// JavaScript's `Math.round`: halves go toward +infinity, where Rust's go away from zero.
fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// How far a grip may travel before it would take the rect under `MIN_SIDE`: the clamp, said once per axis.
fn limit(d: f64, side: f64, low_edge: bool) -> f64 {
    if low_edge {
        js_min(d, side - MIN_SIDE)
    } else {
        js_max(d, MIN_SIDE - side)
    }
}

/// The rect a drag lands on, in whole pixels, since a frame is measured in them.
pub fn drag_rect(r: Rect, grip: Grip, dx: f64, dy: f64) -> Rect {
    if grip == Grip::Body {
        return Rect {
            x: js_round(r.x + dx),
            y: js_round(r.y + dy),
            ..r
        };
    }
    use Grip::*;
    let west = matches!(grip, W | Nw | Sw);
    let east = matches!(grip, E | Ne | Se);
    let north = matches!(grip, N | Nw | Ne);
    let south = matches!(grip, S | Sw | Se);
    let Rect {
        mut x,
        mut y,
        mut w,
        mut h,
    } = r;
    if west {
        let d = limit(dx, w, true);
        x += d;
        w -= d;
    }
    if east {
        w += limit(dx, w, false);
    }
    if north {
        let d = limit(dy, h, true);
        y += d;
        h -= d;
    }
    if south {
        h += limit(dy, h, false);
    }
    Rect {
        x: js_round(x),
        y: js_round(y),
        w: js_round(w),
        h: js_round(h),
    }
}

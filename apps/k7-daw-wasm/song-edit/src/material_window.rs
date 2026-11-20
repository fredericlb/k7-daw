// material_window.rs — a foreign DAW's (length, offset, loop region) as this project's model: a WINDOW and a COUNT.
// A reader would get wrong whose each is: the window belongs to the material, the count of passes to the placement.

use crate::start_time::js_round;

/// What a foreign DAW says about one placed clip, everything in beats.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlacedExtent {
    pub length: f64,
    pub offset: Option<f64>,
    pub loop_start: Option<f64>,
    pub loop_end: Option<f64>,
}

/// A slice of the material, played `loops` times.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialWindow {
    pub from: f64,
    pub to: f64,
    pub loops: f64,
}

/// A length is "a whole number of loops" up to this: imported numbers are rounded decimals.
const EPS: f64 = 1e-6;

/// The window and how many times it is played; a WHOLE number of loops is "this region, N times".
#[must_use]
pub fn material_window(p: &PlacedExtent) -> MaterialWindow {
    let loop_start = p.loop_start.unwrap_or(0.0);
    let region = p.loop_end.unwrap_or(0.0) - loop_start;
    if region > EPS && p.length >= region - EPS {
        let n = p.length / region;
        let whole = js_round(n);
        if whole >= 1.0 && (n - whole).abs() * region < EPS {
            return MaterialWindow {
                from: loop_start,
                to: loop_start + region,
                loops: whole,
            };
        }
    }
    let from = if region > EPS {
        loop_start
    } else {
        p.offset.unwrap_or(0.0)
    };
    MaterialWindow {
        from,
        to: from + p.length,
        loops: 1.0,
    }
}

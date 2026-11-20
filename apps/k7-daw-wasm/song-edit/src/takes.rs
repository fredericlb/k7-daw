// takes.rs — the comp arithmetic the take writers need: a span laid over an event's comp, in song beats.
// A reader would get wrong the unit: a span is in SONG time, not in the take's own time (apps/k7-daw-frontend/packages/song/docs/takes.md).

const EPS: f64 = 1e-9;

/// One span of the comp: from `from` to `to` (song beats) the event plays `take`; "" plays nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct CompSpan {
    pub take: String,
    pub from: f64,
    pub to: f64,
}

/// The comp with `span` laid over it: what it covers is taken from every other span, and the base's own spans are dropped.
#[must_use]
pub fn set_comp_span(comp: &[CompSpan], span: &CompSpan, base_ref: &str) -> Vec<CompSpan> {
    let mut out = vec![];
    for s in comp {
        if s.to <= span.from + EPS || s.from >= span.to - EPS {
            out.push(s.clone());
            continue;
        }
        if s.from < span.from - EPS {
            out.push(CompSpan {
                to: span.from,
                ..s.clone()
            });
        }
        if s.to > span.to + EPS {
            out.push(CompSpan {
                from: span.to,
                ..s.clone()
            });
        }
    }
    // Picking the event's own material is the absence of a span: nothing is written for it.
    if span.take != base_ref {
        out.push(span.clone());
    }
    out.sort_by(|a, b| a.from.total_cmp(&b.from));
    out
}

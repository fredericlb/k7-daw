// title.rs — the row at the top of a right-click menu naming what it is on: the entity, and the thing's shown name.
// A reader would get wrong who names a lane or a clip: the page does, so this answers which kind of name it needs and the id to name.

use super::MenuCtx;
use serde::Serialize;

/// How the thing is named: as written, or by the page's namer for a lane or a clip, or as a note over its sequence's clip.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NameSpec {
    Plain { text: String },
    Lane { id: String },
    Clip { id: String },
    Note { note: f64, sequence: String },
}

/// What a menu is on: the kind of thing, and the name it is shown by.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MenuTarget {
    pub entity: String,
    pub name: NameSpec,
}

fn plain(s: &str) -> NameSpec {
    NameSpec::Plain { text: s.to_owned() }
}

/// The first of two names that is not empty.
fn or<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.is_empty() { b } else { a }
}

/// The target of a context's menu, or none for one that names nothing (a chain, a sink).
#[must_use]
pub fn menu_target(ctx: &MenuCtx) -> Option<MenuTarget> {
    let entity = ctx.entity()?.to_owned();
    let name = match ctx {
        MenuCtx::Track { id, .. } => NameSpec::Lane { id: id.clone() },
        MenuCtx::Clip { id, .. } => NameSpec::Clip { id: id.clone() },
        MenuCtx::Param { label, param, .. } => plain(or(label, param)),
        MenuCtx::CurvePoint { param, curve, .. } => plain(or(param, curve)),
        MenuCtx::CurveCard { param, curve, .. } => plain(param.as_deref().unwrap_or(curve)),
        MenuCtx::MarkerAdd { tc, .. } => plain(tc),
        MenuCtx::Marker { name } => plain(name),
        MenuCtx::WarpPoint { src, .. } => plain(src),
        MenuCtx::PictureShot { name, at, .. } => plain(or(name, at)),
        MenuCtx::PictureLane { lane } => NameSpec::Lane { id: lane.clone() },
        MenuCtx::Note { note, sequence, .. } => NameSpec::Note {
            note: *note,
            sequence: sequence.clone(),
        },
        MenuCtx::Tap { symbol, id } => plain(&format!("{symbol} \u{b7} {id}")),
        MenuCtx::Group { id }
        | MenuCtx::Module { id, .. }
        | MenuCtx::Assembly { id, .. }
        | MenuCtx::Timeline { id } => plain(id),
        MenuCtx::Chain { id } | MenuCtx::Sink { id } => plain(id),
    };
    Some(MenuTarget { entity, name })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(v: serde_json::Value) -> Option<MenuTarget> {
        menu_target(&serde_json::from_value(v).unwrap())
    }

    #[test]
    fn a_lane_and_a_clip_are_named_by_the_page() {
        let t = target(serde_json::json!({ "kind": "track", "id": "gtr" })).unwrap();
        assert_eq!(t.entity, "track");
        assert_eq!(t.name, NameSpec::Lane { id: "gtr".into() });
        let t = target(serde_json::json!({ "kind": "clip", "id": "riff" })).unwrap();
        assert_eq!(t.name, NameSpec::Clip { id: "riff".into() });
    }

    #[test]
    fn a_param_reads_its_label_a_curve_card_its_param_and_a_tap_its_symbol_over_its_node() {
        let t = target(serde_json::json!({ "kind": "param", "id": "amp", "param": "freq", "label": "" })).unwrap();
        assert_eq!(t.name, plain("freq"));
        let t = target(serde_json::json!({ "kind": "curve-card", "curve": "wob" })).unwrap();
        assert_eq!((t.entity.as_str(), t.name), ("param", plain("wob")));
        let t = target(serde_json::json!({ "kind": "tap", "id": "kit", "symbol": "kick" })).unwrap();
        assert_eq!(t.name, plain("kick \u{b7} kit"));
        let t = target(serde_json::json!({ "kind": "picture-shot", "lane": "m", "at": "12.5", "name": "" })).unwrap();
        assert_eq!(t.name, plain("12.5"));
    }

    #[test]
    fn a_chain_and_a_sink_name_nothing() {
        assert_eq!(target(serde_json::json!({ "kind": "chain", "id": "x" })), None);
        assert_eq!(target(serde_json::json!({ "kind": "sink", "id": "OUT" })), None);
    }
}

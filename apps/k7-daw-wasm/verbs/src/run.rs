// run.rs — one line dispatched as `runCommand` dispatches it: the longest command, the argument shapes, the two refusals before any verb.
// A reader would get wrong a verb the crate has not ported: it is refused by name, the way a SID with no handler is.

use crate::catalogue::{Spec, spec_of, validate_args};
use crate::host::{Host, Thrown};
use crate::verbs;
use crate::verbs::graph_facts::{graph_facts, sounding_picture};
use crate::verbs::shared::split_words;
use k7x::edit::js_number;
use song_edit::format_level::{FORMAT_LEVEL, format_level_of_text};
use song_edit::js_text::js_trim;
use song_edit::module_io::entity_file_for;
use song_edit::workspace_doc::ws_text;

/// `checkProjectFormat`: a project whose root declares a newer format is edited by no verb.
fn format_refusal(h: &Host) -> Option<String> {
    let file = entity_file_for(&*h.fs, &h.song_path, "", "project")?;
    let text = match ws_text(&*h.fs, &file.path) {
        Ok(t) => t,
        Err(e) => return Some(e.to_string()),
    };
    let level = format_level_of_text(&text)?;
    (level > FORMAT_LEVEL).then(|| {
        format!(
            "{}: this project was written by a newer k7 (format {}); this k7 reads format {FORMAT_LEVEL} — update k7 to open it",
            file.rel.trim_start_matches('/'),
            js_number(f64::from(level))
        )
    })
}

/// `soundingLaneRefusal`: an `event…` verb may not touch a lane whose clips are a picture's shots.
fn sounding_refusal(spec: &Spec, user_args: &str, h: &Host) -> Option<String> {
    let first = spec.args.first().map(|a| a.name.as_str());
    if !spec.sid.starts_with("event") || spec.sid == "event.remove" || !matches!(first, Some("track" | "tracks")) {
        return None;
    }
    let facts = graph_facts(h)?;
    let names = split_words(user_args).first().copied().unwrap_or("");
    names.split(',').find_map(|name| {
        sounding_picture(Some(&facts), name).map(|picture| {
            format!(
                "'{name}' plays {picture}'s sound — edit the shot instead (picture move | length | head | split {picture} …)"
            )
        })
    })
}

/// Runs `line`; its lines, refusal and effects are left on `h`, a throw out of the verb is the `Err`.
///
/// # Errors
/// What escaped the verb, as the TypeScript handler's rejection.
pub fn run_line(h: &mut Host, line: &str) -> Result<(), Thrown> {
    let s = js_trim(line);
    let Some(spec) = spec_of(s) else {
        h.refuse([format!("unknown command: {s}")]);
        return Ok(());
    };
    let Some(handler) = verbs::handler(&spec.sid) else {
        h.refuse([format!("command '{}' has no handler ({})", spec.command, spec.sid)]);
        return Ok(());
    };
    let user_args = js_trim(&s[spec.command.len()..]);
    if let Some(bad) = validate_args(spec, user_args) {
        h.refuse([format!("{}: {bad}", spec.command)]);
        return Ok(());
    }
    if !h.song_path.is_empty()
        && let Some(why) = format_refusal(h)
    {
        h.refuse([why]);
        return Ok(());
    }
    if let Some(why) = sounding_refusal(spec, user_args, h) {
        h.refuse([why]);
        return Ok(());
    }
    handler(user_args, h)
}

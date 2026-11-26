// picture_render.rs — `picture render [webm|mp4] [wav|none] [size= quality= from= to=] into=<folder>`: the picture rendered in the background.
// A reader would get wrong who draws the film: the verb builds the scene here, the front's picture job renders it in a browser.

use super::picture_shared::picture_lane_ids;
use super::shared::{Done, split_words};
use crate::host::Host;
use crate::picture::Use;
use serde_json::json;
use song_edit::js_text::js_trim;

/// The front's route that renders a scene in the background, asked once the line has landed.
const PICTURE: &str = "POST /verb-host/picture";
const FORMATS: [&str; 2] = ["webm", "mp4"];
const QUALITIES: [&str; 4] = ["low", "medium", "high", "max"];

/// `into=<folder>` or `into="<folder with spaces>"`, taken out of the line before the other words are read.
fn take_into(line: &str) -> (Option<String>, String) {
    let mut at = 0;
    while let Some(i) = line[at..].find("into=") {
        let i = at + i;
        let boundary = i == 0
            || line[..i]
                .chars()
                .next_back()
                .is_some_and(song_edit::js_text::is_js_space);
        if boundary {
            let rest = &line[i + 5..];
            let (value, len) = if let Some(q) = rest.strip_prefix('"') {
                match q.find('"') {
                    Some(end) => (q[..end].to_owned(), end + 2),
                    None => (
                        rest.split_whitespace().next().unwrap_or("").to_owned(),
                        rest.split_whitespace().next().map_or(0, str::len),
                    ),
                }
            } else {
                let w = rest.split(song_edit::js_text::is_js_space).next().unwrap_or("");
                (w.to_owned(), w.len())
            };
            if !value.is_empty() || rest.starts_with('"') {
                let cut = format!("{} {}", &line[..i], &line[i + 5 + len..]);
                return (Some(value), cut);
            }
        }
        at = i + 5;
    }
    (None, line.to_owned())
}

fn clock(s: &str) -> Option<f64> {
    let (m, sec) = match s.split_once(':') {
        Some((m, rest)) => (Some(m), rest),
        None => (None, s),
    };
    let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
    let decimal = |x: &str| match x.split_once('.') {
        Some((a, b)) => digits(a) && digits(b),
        None => digits(x),
    };
    if !decimal(sec) || m.is_some_and(|m| !digits(m)) {
        return None;
    }
    Some(sec.parse::<f64>().ok()? + m.map_or(0.0, |m| m.parse::<f64>().unwrap_or(0.0) * 60.0))
}

/// The render's own settings `words` say, as the JSON the caller reads, or None when one is not understood or the range runs backwards.
fn parse_settings(words: &[String]) -> Option<serde_json::Map<String, serde_json::Value>> {
    let mut out = serde_json::Map::new();
    let (mut from, mut to) = (None, None);
    for w in words {
        let (k, v) = w.split_once('=').map_or((w.as_str(), ""), |(k, v)| (k, v));
        match k {
            "size" => {
                let num = |x: &str| {
                    (!x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()))
                        .then(|| x.parse::<u64>().ok())
                        .flatten()
                };
                if let Some(h) = v.strip_suffix('p').and_then(num) {
                    out.insert("size".to_owned(), json!({ "height": h }));
                } else if let Some((a, b)) = v.split_once('x')
                    && let (Some(a), Some(b)) = (num(a), num(b))
                {
                    out.insert("size".to_owned(), json!({ "width": a, "height": b }));
                } else {
                    return None;
                }
            }
            "quality" if QUALITIES.contains(&v) => {
                out.insert("quality".to_owned(), json!(v));
            }
            "from" | "to" => {
                let t = clock(v)?;
                out.insert(k.to_owned(), json!(t));
                if k == "from" { from = Some(t) } else { to = Some(t) }
            }
            _ => return None,
        }
    }
    if let (Some(f), Some(t)) = (from, to)
        && t <= f
    {
        return None;
    }
    Some(out)
}

fn absolute(p: &str) -> bool {
    p.starts_with('/')
        || (p.len() >= 3 && p.as_bytes()[0].is_ascii_alphabetic() && &p[1..2] == ":" && matches!(&p[2..3], "\\" | "/"))
}

/// `picture render` — the picture rendered on the host with no browser left open, in the background, into the folder `into=` names.
pub fn picture_render(a: &str, h: &mut Host) -> Done {
    let (into, rest) = take_into(a);
    let mut words: Vec<String> = split_words(&rest).iter().map(|w| w.to_lowercase()).collect();
    let format = if words.first().is_some_and(|w| FORMATS.contains(&w.as_str())) {
        words.remove(0)
    } else {
        "webm".to_owned()
    };
    let audio = match words.first().map(String::as_str) {
        Some("none") => {
            words.remove(0);
            false
        }
        Some("wav") => {
            words.remove(0);
            true
        }
        _ => true,
    };
    let Some(settings) = parse_settings(&words) else {
        h.refuse(["usage: picture render [webm|mp4] [wav|none] [size=720p|1280x720] [quality=low|medium|high|max] [from=<t>] [to=<t>] into=<folder>".to_owned()]);
        return Ok(());
    };
    let Some(into) = into.filter(|i| absolute(i)) else {
        h.refuse(["say where the film goes: into=<folder>, the absolute path of a folder (the app asks with its folder picker)".to_owned()]);
        return Ok(());
    };
    if picture_lane_ids(h)?.is_empty() {
        h.refuse(["this project draws no picture — nothing to render".to_owned()]);
        return Ok(());
    }
    let built = crate::picture::scene_answer(&*h.fs, &mut h.cache, &h.env, &h.song_path, None, Use::Export, &|p| {
        p.to_owned()
    });
    if let Some(e) = built.get("error").and_then(|e| e.as_str()) {
        h.refuse([e.to_owned()]);
        return Ok(());
    }
    let mixes = built.get("mixes") == Some(&json!(true));
    let ask = json!({ "scene": built["scene"], "into": js_trim(&into), "format": format, "settings": settings, "audio": audio && mixes });
    h.ask(PICTURE, ask);
    let silent = if audio { "" } else { ", silent" };
    h.log([format!(
        "rendering the picture in the background ({format}{silent}) — it lands in {into}"
    )]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_is_taken_out_of_the_line_quoted_or_not() {
        assert_eq!(take_into("mp4 into=/a/b size=720p").0.as_deref(), Some("/a/b"));
        assert_eq!(take_into("into=\"/a b/c\" mp4").0.as_deref(), Some("/a b/c"));
        assert_eq!(take_into("mp4").0, None);
    }

    #[test]
    fn a_range_that_runs_backwards_is_no_setting() {
        let w = |l: &[&str]| l.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(parse_settings(&w(&["from=10", "to=5"])).is_none());
        assert!(parse_settings(&w(&["quality=max", "from=1:30", "size=1280x720"])).is_some());
        assert!(parse_settings(&w(&["fast"])).is_none());
    }
}

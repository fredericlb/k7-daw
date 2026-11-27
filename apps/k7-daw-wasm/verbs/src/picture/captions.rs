// captions.rs — a subtitle file, `.srt` or WebVTT, read into timed cues in seconds from the file's own zero.
// A reader would get wrong that a block that does not read stops the file: it is a problem said, and the rest still draws.

pub use eyes::Cue;

/// `01:02:03,456`, `02:03.456` or `02:03`: hours optional, a comma or a point before at most three digits.
fn seconds(s: &str) -> Option<f64> {
    let s = s.trim();
    let (clock, frac) = match s.find([',', '.']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let parts: Vec<&str> = clock.split(':').collect();
    let digits = |x: &str, max: usize| !x.is_empty() && x.len() <= max && x.bytes().all(|b| b.is_ascii_digit());
    let (h, m, sec) = match parts.as_slice() {
        [m, s] if digits(m, 2) && digits(s, 2) => ("0", *m, *s),
        [h, m, s] if digits(h, usize::MAX) && digits(m, 2) && digits(s, 2) => (*h, *m, *s),
        _ => return None,
    };
    let frac = match frac {
        None => 0.0,
        Some(f) if digits(f, 3) => format!("{f:0<3}").parse::<f64>().ok()? / 1000.0,
        Some(_) => return None,
    };
    Some(h.parse::<f64>().ok()? * 3600.0 + m.parse::<f64>().ok()? * 60.0 + sec.parse::<f64>().ok()? + frac)
}

/// Every `<…>` tag taken out of a line.
fn strip_tags(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>') else { break };
        out.push_str(&rest[..open]);
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// The blocks of a file: runs of lines parted by a line that holds only white space.
fn blocks(text: &str) -> Vec<Vec<&str>> {
    let mut out = vec![Vec::new()];
    for line in text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)) {
        if line.trim().is_empty() {
            if out.last().is_some_and(|b| !b.is_empty()) {
                out.push(Vec::new());
            }
        } else if let Some(b) = out.last_mut() {
            b.push(line);
        }
    }
    out
}

/// The cues of `text` in file order, and the blocks that could not be read; never a failure.
pub fn parse_captions(text: &str) -> (Vec<Cue>, Vec<String>) {
    let (mut cues, mut problems) = (Vec::new(), Vec::new());
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (i, lines) in blocks(text).iter().enumerate() {
        let Some(first) = lines.first() else { continue };
        let header = ["WEBVTT", "NOTE", "STYLE", "REGION"].iter().any(|w| {
            first
                .strip_prefix(w)
                .is_some_and(|r| !r.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
        });
        if header {
            continue;
        }
        let Some(at) = lines.iter().position(|l| l.contains("-->")) else {
            problems.push(format!("block {}: no timing line", i + 1));
            continue;
        };
        let (from, rest) = lines[at].split_once("-->").unwrap_or_default();
        let start = seconds(from);
        let end = seconds(rest.split_whitespace().next().unwrap_or(""));
        match (start, end) {
            (Some(start), Some(end)) if end > start => {
                let body: Vec<String> = lines[at + 1..]
                    .iter()
                    .map(|l| strip_tags(l).trim().to_owned())
                    .collect();
                let body = body.join("\n");
                if !body.is_empty() {
                    cues.push(Cue { start, end, text: body });
                }
            }
            _ => problems.push(format!("block {}: cannot read '{}'", i + 1, lines[at].trim())),
        }
    }
    (cues, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_srt_and_a_vtt_read_alike() {
        let srt =
            "1\r\n00:00:01,500 --> 00:00:03,000\r\n<i>Hello</i>\r\n\r\n2\r\n00:00:04,000 --> 00:00:05,000\r\nworld\r\n";
        let (cues, problems) = parse_captions(srt);
        assert!(problems.is_empty());
        assert_eq!(
            cues[0],
            Cue {
                start: 1.5,
                end: 3.0,
                text: "Hello".into()
            }
        );
        let vtt = "WEBVTT\n\n00:01.5 --> 00:03.000 align:start\nHello\n";
        assert_eq!(parse_captions(vtt).0[0].start, 1.5);
    }

    #[test]
    fn a_block_that_does_not_read_is_said() {
        let (cues, problems) = parse_captions("1\nno timing\n\n2\n00:05 --> 00:04\nback\n");
        assert!(cues.is_empty());
        assert_eq!(
            problems,
            ["block 1: no timing line", "block 2: cannot read '00:05 --> 00:04'"]
        );
    }
}

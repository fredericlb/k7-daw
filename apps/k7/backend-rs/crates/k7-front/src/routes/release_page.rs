//! release_page.rs — `/app_downloads`, the one page where people get k7's apps: the release, the latest build
//! of main, then the tray's two.
//! The thing a reader would get wrong: its dates are ICU's `dateStyle: long, timeStyle: short` in Europe/Paris,
//! written out here by hand for English and French, the only two languages the page speaks.

use crate::routes::release_store::{Channel, Release, civil};

/// Each app's current build per channel, release first.
pub type ProductBuilds = [Option<Release>; 2];

// What the page offers, in this order; the rest (latest*.yml, blockmaps, SHA256SUMS) is the updater's or the curious's.
const KINDS: [(&str, &str); 6] = [
    (".exe", "Windows"),
    (".AppImage", "Linux (AppImage)"),
    (".deb", "Linux (.deb)"),
    (".apk", "Android"),
    (".zip", "Command-line engine"),
    (".tar.gz", "Command-line engine"),
];

struct Labels {
    lang: &'static str,
    title: &'static str,
    release: &'static str,
    snapshot: &'static str,
    snapshot_note: &'static str,
    none: &'static str,
    published: &'static str,
    updates: &'static str,
    sums: &'static str,
    mac: &'static str,
    tray: &'static str,
    tray_note: &'static str,
}

const EN: Labels = Labels {
    lang: "en",
    title: "k7 — downloads",
    release: "Release",
    snapshot: "Latest build of main",
    snapshot_note: "Built from every push to main: newer, less tested.",
    none: "No build published yet.",
    published: "published",
    updates: "The tray and the Android app update themselves; with the tray's .deb, download again from here.",
    sums: "Checksums",
    mac: "On a Mac, k7 is the iOS app, run as Designed for iPad on Apple Silicon.",
    tray: "Tray",
    tray_note: "The engine alone, run from the system tray, for a machine that plays what k7 sends it. It updates itself.",
};

const FR: Labels = Labels {
    lang: "fr",
    title: "k7 — téléchargements",
    release: "Version",
    snapshot: "Dernière build de main",
    snapshot_note: "Construite à chaque push sur main : plus récente, moins testée.",
    none: "Aucune build publiée pour l'instant.",
    published: "publiée le",
    updates: "Le tray et l'app Android se mettent à jour seuls ; avec le .deb du tray, retéléchargez ici.",
    sums: "Sommes de contrôle",
    mac: "Sur un Mac, k7 est l'app iOS, lancée en « Conçue pour iPad » sur Apple Silicon.",
    tray: "Tray",
    tray_note: "Le moteur seul, lancé depuis la zone de notification, pour une machine qui joue ce que k7 lui envoie. Il se met à jour seul.",
};

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn megabytes(bytes: u64) -> String {
    format!(
        "{} MB",
        ((bytes as f64 / 1_048_576.0).round() as u64).max(1)
    )
}

/// The engine's label names its platform, from the `_<platform>-<arch>.` package-release.mjs puts in its name.
fn label_of(label: &str, name: &str) -> String {
    if !name.starts_with("k7-runtime_") {
        return label.to_string();
    }
    let platform = ["windows", "macos", "linux"]
        .iter()
        .flat_map(|p| ["x64", "arm64"].map(|a| (p, a)))
        .filter_map(|(p, a)| name.find(&format!("_{p}-{a}.")).map(|at| (at, *p, a)))
        .min_by_key(|(at, _, _)| *at)
        .map(|(_, p, a)| {
            let p = match p {
                "windows" => "Windows",
                "macos" => "macOS",
                _ => "Linux",
            };
            format!("{p} {a}")
        })
        .unwrap_or_default();
    format!("{label} ({platform})")
}

const EN_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const FR_MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

/// The day of the month of the last Sunday of `month` in `year`, as a day count since 1970.
fn last_sunday(year: i64, month: u32) -> i64 {
    let first_next = days_from_civil(year, month + 1, 1);
    let last = first_next - 1;
    // 1970-01-01 was a Thursday, so a day count's weekday is (days + 4) mod 7 with Sunday 0.
    last - (last + 4).rem_euclid(7)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let (y, m) = if m > 12 { (y + 1, m - 12) } else { (y, m) };
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Paris's offset at `ms`: summer time from the last Sunday of March to the last of October, at 01:00 UTC.
fn paris_offset_ms(ms: i64) -> i64 {
    let (year, _, _) = civil(ms.div_euclid(86_400_000));
    let start = last_sunday(year, 3) * 86_400_000 + 3_600_000;
    let end = last_sunday(year, 10) * 86_400_000 + 3_600_000;
    if (start..end).contains(&ms) {
        7_200_000
    } else {
        3_600_000
    }
}

/// `toLocaleString(lang, { dateStyle: "long", timeStyle: "short", timeZone: "Europe/Paris" })`.
pub fn paris_date(ms: i64, french: bool) -> String {
    let local = ms + paris_offset_ms(ms);
    let (y, m, d) = civil(local.div_euclid(86_400_000));
    let minutes = local.rem_euclid(86_400_000) / 60_000;
    let (h, min) = (minutes / 60, minutes % 60);
    if french {
        return format!("{d} {} {y} à {h:02}:{min:02}", FR_MONTHS[m as usize - 1]);
    }
    let (h12, half) = (
        if h % 12 == 0 { 12 } else { h % 12 },
        if h < 12 { "AM" } else { "PM" },
    );
    format!(
        "{} {d}, {y} at {h12}:{min:02} {half}",
        EN_MONTHS[m as usize - 1]
    )
}

fn section(tray: bool, channel: Channel, r: &Release, t: &Labels) -> String {
    let href = |name: &str| {
        let under = if tray { "tray/" } else { "" };
        format!("app_downloads/{under}{}/{name}", channel.name())
    };
    let mut items = String::new();
    for (ext, label) in KINDS {
        // An updater's .app.tar.gz archive is not a download: it ends in .tar.gz like an engine.
        for f in r
            .files
            .iter()
            .filter(|f| f.name.ends_with(ext) && !f.name.ends_with(".app.tar.gz"))
        {
            items.push_str(&format!(
                "<li><a href=\"{}\">{}</a><span>{} · {}</span></li>",
                href(&f.name),
                escape_html(&label_of(label, &f.name)),
                escape_html(&f.name),
                megabytes(f.bytes)
            ));
        }
    }
    let when = paris_date(r.published_ms, t.lang == "fr");
    let which = match channel {
        Channel::Release => format!("{} {}", t.release, r.stamp.get(1..).unwrap_or("")),
        Channel::Snapshot => t.snapshot.to_string(),
    };
    let name = if tray {
        format!("{} — {which}", t.tray)
    } else {
        which
    };
    let mut note = String::new();
    if tray {
        note.push_str(&format!("<p>{}</p>", t.tray_note));
    }
    if channel == Channel::Snapshot {
        note.push_str(&format!("<p>{}</p>", t.snapshot_note));
    }
    let sums = if r.files.iter().any(|f| f.name == "SHA256SUMS") {
        format!(
            "<p class=\"small\"><a href=\"{}\">{}</a></p>",
            href("SHA256SUMS"),
            t.sums
        )
    } else {
        String::new()
    };
    format!(
        "<section><h2>{}</h2><p class=\"small\">{}, {} {}</p>{note}<ul>{items}</ul>{sums}</section>",
        escape_html(&name),
        escape_html(&r.stamp),
        t.published,
        escape_html(&when)
    )
}

/// The page for the apps' builds (`main`) and the tray's, in French or English.
pub fn downloads_page(main: &ProductBuilds, tray: &ProductBuilds, french: bool) -> String {
    let t = if french { &FR } else { &EN };
    let mut sections = String::new();
    for (is_tray, builds) in [(false, main), (true, tray)] {
        for (channel, build) in [Channel::Release, Channel::Snapshot].iter().zip(builds) {
            if let Some(r) = build {
                sections.push_str(&section(is_tray, *channel, r, t));
            }
        }
    }
    let body = if sections.is_empty() {
        format!("<p>{}</p><p class=\"small\">{}</p>", t.none, t.mac)
    } else {
        format!(
            "{sections}<p class=\"small\">{}</p><p class=\"small\">{}</p>",
            t.updates, t.mac
        )
    };
    format!(
        r#"<!doctype html>
<html lang="{lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root{{color-scheme:light dark;--bg:#fff;--fg:#1c1c1e;--muted:#6b6b70;--link:#1d5fd1;--line:#e4e4e8}}
@media (prefers-color-scheme:dark){{:root{{--bg:#161618;--fg:#ececee;--muted:#9a9aa0;--link:#7aa7ff;--line:#2c2c30}}}}
body{{background:var(--bg);color:var(--fg);font:16px/1.5 system-ui,sans-serif;max-width:40rem;margin:3rem auto;padding:0 16px}}
h1{{font-size:1.5rem}}h2{{font-size:1.125rem;margin:0}}section{{border-top:1px solid var(--line);padding:1.25rem 0}}
ul{{list-style:none;padding:0;margin:.5rem 0 0}}li{{margin:.6rem 0}}p{{color:var(--muted);margin:.25rem 0}}
a{{color:var(--link);font-weight:600}}span{{display:block;color:var(--muted);font-size:.875rem;overflow-wrap:anywhere}}.small{{font-size:.875rem}}
</style></head>
<body><h1>{title}</h1>{body}</body></html>"#,
        lang = t.lang,
        title = t.title,
    )
}

struct Spec {
    prefix: String,
    full: String,
    q: f64,
    i: usize,
}

/// `parseFloat`: the longest number at the start, NaN when there is none.
fn parse_float(v: &str) -> f64 {
    let v = v.trim_start();
    (1..=v.len())
        .rev()
        .filter(|n| v.is_char_boundary(*n))
        .find_map(|n| {
            v[..n]
                .parse::<f64>()
                .ok()
                .filter(|_| !v[..n].ends_with(['e', 'E', '+']))
        })
        .unwrap_or(f64::NAN)
}

fn parse_language(s: &str, i: usize) -> Option<Spec> {
    let (tag, params) = s.split_once(';').map_or((s, None), |(t, p)| (t, Some(p)));
    let tag = tag.trim();
    if tag.is_empty() || tag.contains(char::is_whitespace) {
        return None;
    }
    let (prefix, suffix) = tag
        .split_once('-')
        .map_or((tag, None), |(p, s)| (p, Some(s)));
    if prefix.is_empty() || suffix == Some("") {
        return None;
    }
    let mut q = 1.0;
    // Neither key nor value trimmed, as negotiator splits them: `fr; q=0.5` keeps q 1.
    for p in params.unwrap_or("").split(';') {
        if let Some(("q", v)) = p.split_once('=') {
            q = parse_float(v);
        }
    }
    Some(Spec {
        prefix: prefix.to_string(),
        full: tag.to_string(),
        q,
        i,
    })
}

/// Whether `req.acceptsLanguages("en", "fr")` answers "fr", as the negotiator package ranks them.
pub fn prefers_french(accept: Option<&str>) -> bool {
    let specs: Vec<Spec> = accept
        .unwrap_or("*")
        .split(',')
        .enumerate()
        .filter_map(|(i, s)| parse_language(s, i))
        .collect();
    // For each offered language, its best spec as negotiator's getLanguagePriority picks it: (q, specificity, spec order).
    let rank = |offer: &str, index: usize| {
        let mut best = (0.0_f64, 0_i32, -1_i64);
        for spec in &specs {
            let s = if spec.full.eq_ignore_ascii_case(offer) {
                4
            } else if spec.prefix.eq_ignore_ascii_case(offer) {
                2
            } else if spec.full == "*" {
                0
            } else {
                continue;
            };
            let (q, o) = (spec.q, spec.i as i64);
            let dq = best.0 - q;
            let d = if best.1 != s {
                f64::from(best.1 - s)
            } else if dq != 0.0 && !dq.is_nan() {
                dq
            } else {
                (best.2 - o) as f64
            };
            if d < 0.0 {
                best = (q, s, o);
            }
        }
        (best.0, best.1, best.2, index)
    };
    let mut ranked: Vec<(f64, i32, i64, usize)> = [rank("en", 0), rank("fr", 1)]
        .into_iter()
        .filter(|r| r.0 > 0.0)
        .collect();
    ranked.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.1.cmp(&a.1))
            .then(a.2.cmp(&b.2))
            .then(a.3.cmp(&b.3))
    });
    ranked.first().is_some_and(|r| r.3 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_read_as_icu_writes_them() {
        let at = |iso: &str| {
            let (d, t) = iso.split_once('T').unwrap();
            let p: Vec<i64> = d.split('-').map(|x| x.parse().unwrap()).collect();
            let h: Vec<i64> = t
                .trim_end_matches('Z')
                .split(':')
                .map(|x| x.parse().unwrap())
                .collect();
            days_from_civil(p[0], p[1] as u32, p[2] as u32) * 86_400_000
                + (h[0] * 3600 + h[1] * 60 + h[2]) * 1000
        };
        let cases = [
            (
                "2026-11-02T09:05:00Z",
                "November 2, 2026 at 10:05 AM",
                "2 novembre 2026 à 10:05",
            ),
            (
                "2026-07-14T23:30:00Z",
                "July 15, 2026 at 1:30 AM",
                "15 juillet 2026 à 01:30",
            ),
            (
                "2026-03-29T00:59:59Z",
                "March 29, 2026 at 1:59 AM",
                "29 mars 2026 à 01:59",
            ),
            (
                "2026-03-29T01:00:00Z",
                "March 29, 2026 at 3:00 AM",
                "29 mars 2026 à 03:00",
            ),
            (
                "2026-10-25T00:59:00Z",
                "October 25, 2026 at 2:59 AM",
                "25 octobre 2026 à 02:59",
            ),
            (
                "2026-10-25T01:00:00Z",
                "October 25, 2026 at 2:00 AM",
                "25 octobre 2026 à 02:00",
            ),
            (
                "2026-01-01T12:00:00Z",
                "January 1, 2026 at 1:00 PM",
                "1 janvier 2026 à 13:00",
            ),
            (
                "2026-11-02T23:00:00Z",
                "November 3, 2026 at 12:00 AM",
                "3 novembre 2026 à 00:00",
            ),
        ];
        for (iso, en, fr) in cases {
            assert_eq!(paris_date(at(iso), false), en, "{iso}");
            assert_eq!(paris_date(at(iso), true), fr, "{iso}");
        }
    }

    #[test]
    fn the_page_speaks_french_only_when_it_is_preferred() {
        for (header, french) in [
            (None, false),
            (Some("fr"), true),
            (Some("fr-FR,fr;q=0.9,en;q=0.8"), true),
            (Some("en-US,en;q=0.9,fr;q=0.8"), false),
            (Some("de, fr;q=0.5"), true),
            (Some("de"), false),
            (Some("*"), false),
            (Some("fr;q=0"), false),
            (Some("en;q=0.5, fr;q=0.5"), false),
            (Some("fr;q=0.5, en;q=0.5"), true),
            (Some("fr-CA"), true),
            (Some("en; q=0.1, fr"), false),
            (Some("fr;q=abc, en;q=0.5"), false),
            (Some(""), false),
            (Some("*;q=0.5, fr;q=0.4"), false),
            (Some("en-GB;q=0.9, fr-FR"), true),
        ] {
            assert_eq!(prefers_french(header), french, "{header:?}");
        }
    }

    #[test]
    fn engines_are_labelled_with_their_platform() {
        assert_eq!(
            label_of(
                "Command-line engine",
                "k7-runtime_v0.1.0_linux-arm64.tar.gz"
            ),
            "Command-line engine (Linux arm64)"
        );
        assert_eq!(label_of("Windows", "k7_v0.1.0_windows-x64.exe"), "Windows");
        assert_eq!(megabytes(10), "1 MB");
        assert_eq!(megabytes(3 * 1_048_576 + 524_288), "4 MB");
    }
}

// put-on — what it costs to PUT a record on a deck that already holds one, timed against a running engine.
// A reader would get wrong what is measured: the engine's `applied` answer to the document `with_decks` makes, and nothing else.
//
//   cargo run -p put-on -- [ws://127.0.0.1:8765] [--minutes=2] [--runs=6] [--hold=30] [--key]
//
// `--hold=30 [--key]` asks the other question, what EIGHT platters cost turning, with or without the stretcher:
// it loads two separated decks, starts them and holds; the engine's CPU is read from outside, before and after.

use indexmap::IndexMap;
use serde_json::json;
use song::doc::{ProjectDoc, TrackDoc};
use song_schema::{STEMS, stem_lane};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tungstenite::{Message, connect};
use turntables::deck_doc::{DeckSet, deck_doc, with_decks};
use turntables::vocab::{DECKS, DeckLoad, DeckTempo};

const RATE: u32 = 48_000;
const SESSION: &str = "m9:put-on";

struct Args {
    url: String,
    minutes: f64,
    runs: usize,
    hold: f64,
    key: bool,
}

fn args() -> Args {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let number = |name: &str, fallback: f64| {
        all.iter()
            .find_map(|a| a.strip_prefix(&format!("--{name}=")).and_then(|v| v.parse().ok()))
            .unwrap_or(fallback)
    };
    Args {
        url: all
            .iter()
            .find(|a| a.starts_with("ws://"))
            .cloned()
            .unwrap_or_else(|| "ws://127.0.0.1:8765".to_owned()),
        minutes: number("minutes", 2.0),
        runs: number("runs", 6.0) as usize,
        hold: number("hold", 0.0),
        key: all.iter().any(|a| a == "--key"),
    }
}

/// One stereo PCM wav of `bits`: 16 for a record, 24 for a stem, both integer, so both take the engine's direct read.
fn wav(path: &Path, seconds: f64, bits: u16) -> std::io::Result<()> {
    let frames = (f64::from(RATE) * seconds).round() as usize;
    let bytes = usize::from(bits / 8);
    let top = f64::from((1_u32 << (bits - 1)) - 1);
    let mut data = Vec::with_capacity(frames * 2 * bytes);
    for i in 0..frames {
        let t = i as f64 / f64::from(RATE);
        let v = ((2.0 * std::f64::consts::PI * 220.0 * t).sin() * 0.2 * top).round() as i32;
        for _ in 0..2 {
            data.extend_from_slice(&v.to_le_bytes()[..bytes]);
        }
    }
    let mut out = Vec::with_capacity(44 + data.len());
    let word = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    let half = |out: &mut Vec<u8>, v: u16| out.extend_from_slice(&v.to_le_bytes());
    out.extend_from_slice(b"RIFF");
    word(&mut out, 36 + data.len() as u32);
    out.extend_from_slice(b"WAVEfmt ");
    word(&mut out, 16);
    half(&mut out, 1);
    half(&mut out, 2);
    word(&mut out, RATE);
    word(&mut out, RATE * 2 * u32::from(bits / 8));
    half(&mut out, 2 * (bits / 8));
    half(&mut out, bits);
    out.extend_from_slice(b"data");
    word(&mut out, data.len() as u32);
    out.extend_from_slice(&data);
    fs::File::create(path)?.write_all(&out)
}

struct Record {
    path: String,
    stems: bool,
}

/// Two plain records and two separated ones, so every step is a real change of file.
fn make(root: &Path, name: &str, stems: bool, seconds: f64) -> std::io::Result<Record> {
    let dir = root.join(name);
    fs::create_dir_all(&dir)?;
    let path = dir.join("original.wav");
    wav(&path, seconds, 16)?;
    if stems {
        fs::create_dir_all(dir.join("stems"))?;
        for s in STEMS {
            wav(&dir.join("stems").join(format!("{}.wav", s.word())), seconds, 24)?;
        }
    }
    Ok(Record {
        path: path.to_string_lossy().into_owned(),
        stems,
    })
}

fn load(path: &str, title: &str, id: &str, stems: bool) -> Option<DeckLoad> {
    Some(DeckLoad {
        path: path.to_owned(),
        title: title.to_owned(),
        artist: None,
        id: id.to_owned(),
        stems: stems.then_some(true),
        bpm: None,
        key: None,
    })
}

/// The set as it is scaffolded: each deck's lane, plus a lane per stem.
fn base() -> ProjectDoc {
    let mut doc = deck_doc(&DeckSet::default());
    for d in DECKS {
        for s in STEMS {
            doc.tracks.push(TrackDoc::new(&stem_lane(d, s)));
        }
    }
    doc
}

fn median(xs: &[f64]) -> f64 {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    s.get(s.len() / 2).copied().unwrap_or(f64::NAN)
}

type Socket = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

fn send(ws: &mut Socket, doc: Option<&ProjectDoc>) -> Result<(), tungstenite::Error> {
    let message = match doc {
        Some(d) => json!({ "type": "project", "doc": d, "session": SESSION }),
        None => json!({ "type": "drop-session", "session": SESSION }),
    };
    ws.send(Message::text(message.to_string()))
}

/// Block until the engine says `applied`.
fn applied(ws: &mut Socket) -> Result<(), tungstenite::Error> {
    loop {
        if let Message::Text(t) = ws.read()?
            && serde_json::from_str::<serde_json::Value>(&t).is_ok_and(|m| m["type"] == "applied")
        {
            return Ok(());
        }
    }
}

fn hold_eight(ws: &mut Socket, a: &Args, base: &ProjectDoc, sep: [&Record; 2]) -> Result<(), tungstenite::Error> {
    println!(
        "\neight platters turning, KEY {}, for {} s.",
        if a.key { "on" } else { "off" },
        a.hold
    );
    println!("  read the engine's CPU now, and again at the end.");
    let loads = IndexMap::from([
        ("deckA".to_owned(), load(&sep[0].path, "A", "songs/a", true)),
        ("deckB".to_owned(), load(&sep[1].path, "B", "songs/b", true)),
    ]);
    let tempo = DeckTempo {
        natural: 120.0,
        seconds: a.minutes * 60.0,
        target: 126.0,
        phase: None,
        keylock: Some(true),
    };
    let tempos = if a.key {
        IndexMap::from([("deckA".to_owned(), tempo.clone()), ("deckB".to_owned(), tempo)])
    } else {
        IndexMap::new()
    };
    let playing = IndexMap::from([("deckA".to_owned(), true), ("deckB".to_owned(), true)]);
    let set = DeckSet {
        loads,
        tempos,
        playing,
        ..DeckSet::default()
    };
    send(ws, Some(&with_decks(base.clone(), &set)))?;
    std::thread::sleep(std::time::Duration::from_secs_f64(a.hold));
    send(ws, None)?;
    println!("done.");
    Ok(())
}

fn run(a: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let root: PathBuf = std::env::var_os("STUDIO_SCRATCH")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join(format!("m9-puton-{}", std::process::id()));
    let seconds = a.minutes * 60.0;
    println!("preparing: 2 plain and 2 separated records, {} min each…", a.minutes);
    let records = [
        make(&root, "plainA", false, seconds)?,
        make(&root, "sepC", true, seconds)?,
        make(&root, "plainB", false, seconds)?,
        make(&root, "sepD", true, seconds)?,
    ];
    let base = base();
    let (mut ws, _) = connect(a.url.as_str())?;
    let (mut one, mut four) = (Vec::new(), Vec::new());
    // The first step is the warm-up: an empty engine loading its first record is a different question.
    for step in 0..=a.runs * 2 {
        let r = &records[step % records.len()];
        let mut loads = IndexMap::new();
        loads.insert("deckA".to_owned(), load(&r.path, "A", "songs/a", r.stems));
        loads.insert("deckB".to_owned(), None);
        let doc = with_decks(
            base.clone(),
            &DeckSet {
                loads,
                ..DeckSet::default()
            },
        );
        let sent = Instant::now();
        send(&mut ws, Some(&doc))?;
        applied(&mut ws)?;
        if step > 0 {
            (if r.stems { &mut four } else { &mut one }).push(sent.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let (m1, m4) = (median(&one), median(&four));
    println!("\nputting a record on a deck that already holds one, {} min", a.minutes);
    println!("  a plain record     : {m1:.0} ms   ({} times)", one.len());
    println!("  a separated record : {m4:.0} ms   ({} times, 4 platters)", four.len());
    println!("  overhead           : {:.0} ms", m4 - m1);
    println!("\nWhat counts is where a gesture starts to feel like a jolt, around 100 ms.");
    if a.hold > 0.0 {
        hold_eight(&mut ws, a, &base, [&records[1], &records[3]])?;
    } else {
        send(&mut ws, None)?;
    }
    fs::remove_dir_all(&root).ok();
    Ok(())
}

fn main() {
    if let Err(e) = run(&args()) {
        eprintln!("put-on: {e}");
        std::process::exit(1);
    }
}

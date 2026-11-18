//! tests_route.rs — `/export/tracks` through a fake renderer: a zip answered, the folder gone with it, and the refusals of `/export`.

use std::path::Path;
use std::sync::{Arc, Mutex};

use http_body_util::BodyExt;
use serde_json::Value;

use super::export_tracks;
use super::tests::sample;
use crate::routes::files::service::native::shares::fakes::{account, scratch, world};
use crate::routes::files::service::native::shares::{
    BoxFut, Manifest, MixDoc, Mixdown, Stem, Views,
};

/// A renderer that writes one WAV per track it is handed, named as the engine names a stem.
struct Stems {
    seen: Arc<Mutex<Vec<(Value, bool)>>>,
    up: bool,
    fail: Option<&'static str>,
}

impl Mixdown for Stems {
    fn available(&self) -> bool {
        self.up
    }

    fn render<'a>(
        &'a self,
        doc: &'a Value,
        out: &'a Path,
        stems: bool,
    ) -> BoxFut<'a, Result<Manifest, String>> {
        Box::pin(async move {
            self.seen.lock().unwrap().push((doc.clone(), stems));
            if let Some(e) = self.fail {
                return Err(e.into());
            }
            std::fs::write(out.join("mix.wav"), b"mix").unwrap();
            let mut list = vec![];
            for t in doc["tracks"].as_array().unwrap() {
                let name = t["name"].as_str().unwrap();
                std::fs::write(out.join(format!("{name}.wav")), format!("RIFF{name}")).unwrap();
                list.push(Stem {
                    name: name.into(),
                    file: format!("{name}.wav"),
                });
            }
            Ok(Manifest {
                mix: "mix.wav".into(),
                stems: list,
            })
        })
    }
}

type Seen = Arc<Mutex<Vec<(Value, bool)>>>;

struct Sample;

impl Views for Sample {
    fn mix_doc<'a>(
        &'a self,
        _dir: &'a Path,
        timeline: Option<&'a str>,
    ) -> BoxFut<'a, Result<MixDoc, String>> {
        Box::pin(async move {
            if timeline == Some("nope") {
                return Err(k7_file_service::shares::names::no_such_timeline("nope"));
            }
            Ok(MixDoc {
                doc: sample(),
                name: "Song".into(),
            })
        })
    }
}

fn ports(
    up: bool,
    fail: bool,
) -> (
    Arc<crate::routes::files::service::native::shares::Ports>,
    Seen,
) {
    ports_saying(up, fail.then_some("engine died"))
}

fn ports_saying(
    up: bool,
    fail: Option<&'static str>,
) -> (
    Arc<crate::routes::files::service::native::shares::Ports>,
    Seen,
) {
    let w = world();
    let seen = Arc::new(Mutex::default());
    let mut p = w.ports;
    let m = Arc::get_mut(&mut p).unwrap();
    m.mixdown = Arc::new(Stems {
        seen: seen.clone(),
        up,
        fail,
    });
    m.views = Arc::new(Sample);
    (p, seen)
}

fn left(a: &crate::routes::files::service::native::shares::Account) -> usize {
    std::fs::read_dir(&a.tmp).unwrap().count()
}

#[tokio::test]
async fn a_zip_is_answered_from_the_stems_and_its_folder_goes_with_the_body() {
    let base = scratch("tracks");
    let a = account(&base, &[]);
    let (p, seen) = ports(true, false);
    let packed = export_tracks(&a, &p, &a.root, None).await.unwrap();
    assert_eq!(left(&a), 1);
    let r = packed.into_response().await.unwrap();
    assert_eq!(r.headers()["content-type"], "application/zip");
    assert!(
        r.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("Song-tracks.zip")
    );
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    let z = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let names: Vec<_> = z.file_names().collect();
    assert!(
        names.contains(&"01-Drums.wav") && names.contains(&"03-Bass_Low.wav"),
        "{names:?}"
    );
    assert!(!names.contains(&"02-Keys.wav"), "{names:?}");
    let (doc, stems) = seen.lock().unwrap()[0].clone();
    assert!(stems);
    assert_eq!(doc["tracks"].as_array().unwrap().len(), 2);
    assert_eq!(doc["tracks"][0]["plugins"], serde_json::json!([]));
    assert_eq!(left(&a), 0);
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn it_refuses_as_export_does_and_leaves_nothing() {
    let base = scratch("tracks-no");
    let a = account(&base, &[]);
    let (p, _) = ports(true, false);
    let e = export_tracks(&a, &p, &a.root, Some("nope"))
        .await
        .err()
        .unwrap();
    assert_eq!(e, "no timeline \"nope\" to export");
    let (p, _) = ports(false, false);
    let e = export_tracks(&a, &p, &a.root, None).await.err().unwrap();
    assert_eq!(e, k7_file_service::shares::no_engine("tracks"));
    let (p, _) = ports(true, true);
    let e = export_tracks(&a, &p, &a.root, None).await.err().unwrap();
    assert_eq!(e, "engine died");
    assert_eq!(left(&a), 0);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn the_route_is_tagged_beside_export() {
    use super::super::routes::tag_of;
    use crate::routes::files::service::native::Tag;
    assert!(matches!(
        tag_of(&hyper::Method::GET, "/export/tracks"),
        Some(Tag::ExportTracks)
    ));
    assert!(matches!(
        tag_of(&hyper::Method::GET, "/export"),
        Some(Tag::Export)
    ));
    assert!(tag_of(&hyper::Method::POST, "/export/tracks").is_none());
}

#[test]
fn a_link_names_its_workspace_in_the_query_and_a_header_still_wins() {
    use crate::routes::files::service::lend_workspace;
    let ask = |header: Option<&str>| {
        let mut b = axum::extract::Request::builder().uri("/export/tracks");
        if let Some(h) = header {
            b = b.header("x-studio-workspace", h);
        }
        b.body(axum::body::Body::empty()).unwrap()
    };
    let seen = |r: &axum::extract::Request| {
        r.headers()
            .get("x-studio-workspace")
            .map(|v| v.to_str().unwrap().to_owned())
    };
    let mut r = ask(None);
    lend_workspace(&mut r, "/export/tracks", "workspace=My%20Songs&timeline=t1");
    assert_eq!(seen(&r).as_deref(), Some("My Songs"));
    let mut r = ask(Some("kept"));
    lend_workspace(&mut r, "/export/tracks", "workspace=other");
    assert_eq!(seen(&r).as_deref(), Some("kept"));
    let mut r = ask(None);
    lend_workspace(&mut r, "/export", "workspace=other");
    assert_eq!(seen(&r), None, "only the link's route lends it");
    let mut r = ask(None);
    lend_workspace(&mut r, "/export/tracks", "");
    assert_eq!(seen(&r), None);
}

#[tokio::test]
async fn a_render_with_nothing_audible_still_zips_the_midi_and_pages() {
    let base = scratch("tracks-silent");
    let a = account(&base, &[]);
    let (p, _) = ports_saying(
        true,
        Some("mixdown produced no audio (song has nothing audible)"),
    );
    let packed = export_tracks(&a, &p, &a.root, None).await.unwrap();
    let r = packed.into_response().await.unwrap();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    let z = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let names: Vec<_> = z.file_names().collect();
    assert!(
        names.contains(&"02-Keys.mid") && names.contains(&"01-Drums.md"),
        "{names:?}"
    );
    assert!(!names.iter().any(|n| n.ends_with(".wav")), "{names:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_cross_site_page_cannot_lend_a_workspace() {
    use crate::routes::files::service::lend_workspace;
    for (site, lent) in [
        (Some("cross-site"), false),
        (Some("same-site"), false),
        (Some("same-origin"), true),
        (Some("none"), true),
        (None, true),
    ] {
        let mut b = axum::extract::Request::builder().uri("/export/tracks");
        if let Some(s) = site {
            b = b.header("sec-fetch-site", s);
        }
        let mut r = b.body(axum::body::Body::empty()).unwrap();
        lend_workspace(&mut r, "/export/tracks", "workspace=w");
        assert_eq!(
            r.headers().contains_key("x-studio-workspace"),
            lent,
            "{site:?}"
        );
    }
}

#[tokio::test]
async fn a_refusal_before_the_first_byte_sets_the_cookie_with_its_sentence() {
    use super::super::routes::{Deps, export_tracks_route};
    let base = scratch("tracks-cookie");
    let a = account(&base, &[]);
    let (p, _) = ports(true, false);
    let home = k7_file_service::libraries::Home {
        studio: base.join("home").display().to_string(),
        libraries: base.join("libs").display().to_string(),
        admin: false,
    };
    let d = Deps {
        acct: Arc::new(a),
        ports: p,
        shares: Arc::default(),
        root: base.join("ws").display().to_string(),
        home,
    };
    let said = export_tracks_route(&d, "workspace=w&dl=tok1")
        .await
        .unwrap();
    assert_eq!(said.status(), 400);
    let c = said.headers()["set-cookie"].to_str().unwrap();
    assert!(c.starts_with("k7dl-tok1=err.no active project"), "{c}");
    let plain = export_tracks_route(&d, "workspace=w").await.unwrap();
    assert!(plain.headers().get("set-cookie").is_none());
    let _ = std::fs::remove_dir_all(&base);
}

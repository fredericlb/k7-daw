//! locate.rs — the project side of a waveform on the account's song session: the active project, its engine document, the clip
//! and its material, the confined source and the onset corrections (2425, 2426). What a reader gets wrong: a clip file the
//! typed reader refuses declines the request; the session's lock is taken only on `on_stack`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::routes::files::service::native::callbacks::BoxFuture;
use k7_file_service::confine::Disk;
use k7_file_service::media::answer::material_window;
use k7_file_service::media::onsets::Edits;
use k7_file_service::paths::{basename, is_absolute, is_within, resolve_all};
use serde_json::Value;
use song::clips::audio_def::{ClipGrid, audio_clip_def, material_ref_of};
use song::clips::{ClipDoc, ClipKind};
use song::graph::ModelNeed;
use song::session::View;

use super::waveform::{Located, Locator, ProjectView, Request, Why};
use crate::routes::files::service::native::Ctx;
use crate::routes::files::service::native::docs::{self, Asked};
use crate::routes::files::service::native::stack::on_stack;

/// An audio file a rack names by its absolute path (`AUDIO_EXT`).
fn audio_ext(p: &str) -> bool {
    p.rsplit_once('.').is_some_and(|(_, e)| {
        ["wav", "mp3", "ogg", "flac", "aif", "aiff", "m4a"]
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    })
}

/// What the engine document says that a draw reads: the grid and every clip.
pub struct Lite {
    pub grid: (f64, f64, f64),
    pub clips: Vec<ClipDoc>,
}

fn lite_of(text: &str) -> Result<Lite, Why> {
    let bad = || Why::Decline("doc-shape");
    let doc = song::session::engine_doc_of(text).ok_or_else(bad)?;
    let v: Value = serde_json::from_str(doc).map_err(|_| bad())?;
    let num = |k: &str| v.get(k).and_then(Value::as_f64).ok_or_else(bad);
    let grid = (
        num("tempo")?,
        num("timeSigNumerator")?,
        num("timeSigDenominator")?,
    );
    let mut clips = Vec::new();
    for t in v.get("tracks").and_then(Value::as_array).ok_or_else(bad)? {
        for c in t.get("clips").and_then(Value::as_array).ok_or_else(bad)? {
            clips.push(serde_json::from_value::<ClipDoc>(c.clone()).map_err(|_| bad())?);
        }
    }
    Ok(Lite { grid, clips })
}

/// The last document parsed, held with its text so the pointer that names it cannot be reused while it is kept.
#[derive(Default)]
pub struct Memo(Mutex<Option<(Arc<str>, Arc<Lite>)>>);

impl Memo {
    fn lite(&self, text: &Arc<str>) -> Result<Arc<Lite>, Why> {
        let mut held = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((_, l)) = held.as_ref().filter(|(t, _)| Arc::ptr_eq(t, text)) {
            return Ok(l.clone());
        }
        let l = Arc::new(lite_of(text)?);
        *held = Some((text.clone(), l.clone()));
        Ok(l)
    }
}

/// The account's song session as the locator of one workspace.
pub struct Session {
    pub ctx: Ctx,
    pub memo: Arc<Memo>,
    /// What `nativeAsk` answered, given by a host call; none makes the session ask itself.
    given: Option<Asked>,
}

impl Session {
    pub fn new(ctx: Ctx, memo: Arc<Memo>) -> Session {
        Session {
            ctx,
            memo,
            given: None,
        }
    }

    /// A session that never asks the account: a verb holds it while it waits on a host call (review fix 6).
    pub(super) fn given(ctx: Ctx, memo: Arc<Memo>, asked: Asked) -> Session {
        Session {
            given: Some(asked),
            ..Session::new(ctx, memo)
        }
    }

    fn asked(&self) -> BoxFuture<Result<Asked, Why>> {
        match &self.given {
            Some(a) => {
                let a = a.clone();
                Box::pin(async move { Ok(a) })
            }
            None => Box::pin(ask(self.ctx.clone())),
        }
    }
}

async fn ask(ctx: Ctx) -> Result<Asked, Why> {
    ctx.docs.as_ref().ok_or(Why::Decline("no-docs"))?;
    docs::asked_of(&ctx.home, &ctx.root)
        .await
        .map_err(Why::Decline)
}

impl Locator for Session {
    fn locate(&self, req: Request) -> BoxFuture<Result<Located, Why>> {
        let me = (self.ctx.clone(), self.memo.clone());
        let asked = self.asked();
        Box::pin(async move {
            let asked = asked.await?;
            if asked.active.is_none() {
                return Err(Why::Refuse(
                    400,
                    "no active project — select a project in the workspace first".into(),
                ));
            }
            match on_stack(move || locate_sync(&me.0, &me.1, asked, &req)).await {
                Ok(r) => r,
                Err(_) => Err(Why::Decline("panic")),
            }
        })
    }

    fn need(&self, node: String) -> BoxFuture<Result<ModelNeed, Why>> {
        let ctx = self.ctx.clone();
        let asked = self.asked();
        Box::pin(async move {
            let asked = asked.await?;
            if asked.active.is_none() {
                let m = "no active project — select a project in the workspace first";
                return Err(Why::Refuse(404, m.into()));
            }
            match on_stack(move || docs::model_need(&ctx, asked, &node)).await {
                Ok(r) => r.map_err(Why::Decline),
                Err(_) => Err(Why::Decline("panic")),
            }
        })
    }

    fn project(&self) -> BoxFuture<Result<ProjectView, Why>> {
        let me = (self.ctx.clone(), self.memo.clone());
        let asked = self.asked();
        Box::pin(async move {
            let asked = asked.await?;
            if asked.active.is_none() {
                return Err(Why::Decline("no-active-project"));
            }
            match on_stack(move || project_sync(&me.0, &me.1, asked)).await {
                Ok(r) => r,
                Err(_) => Err(Why::Decline("panic")),
            }
        })
    }
}

fn prepared(ctx: &Ctx, memo: &Memo, asked: Asked) -> Result<(docs::Prepared, Arc<Lite>), Why> {
    let p = docs::prepare(ctx, asked).map_err(Why::Decline)?;
    let text = docs::view_text(ctx, &p, View::Engine).map_err(Why::Decline)?;
    Ok((p, memo.lite(&text)?))
}

fn project_sync(ctx: &Ctx, memo: &Memo, asked: Asked) -> Result<ProjectView, Why> {
    let (p, lite) = prepared(ctx, memo, asked)?;
    let d = ctx.docs.as_ref().ok_or(Why::Decline("no-docs"))?;
    let project = p.ask.roots.project.clone();
    Ok(ProjectView {
        roots: roots_of(ctx, &project, d),
        clips: lite
            .clips
            .iter()
            .filter(|c| c.kind == ClipKind::Audio)
            .cloned()
            .collect(),
        grid: lite.grid,
        project,
    })
}

/// Whether a rack's absolute audio lies in the workspace root (any spelling of it), links followed.
fn rack_inside(root: &str, abs: &str) -> bool {
    is_within(&resolve_all(&[root]), abs)
        && k7_file_service::confine::confine_to(&[root], abs, &Disk).is_some()
}

/// The clip a request names, its material, its confined file and its corrections.
fn locate_sync(ctx: &Ctx, memo: &Memo, asked: Asked, req: &Request) -> Result<Located, Why> {
    let (p, lite) = prepared(ctx, memo, asked)?;
    let d = ctx.docs.as_ref().ok_or(Why::Decline("no-docs"))?;
    let project = p.ask.roots.project.clone();
    let grid = lite.grid;
    let refuse = |m: String| Why::Refuse(400, m);
    // A rack's audio is an absolute path inside the workspace, links followed (2426).
    if is_absolute(&req.clip) && audio_ext(&req.clip) {
        let abs = resolve_all(&[&req.clip]);
        if !rack_inside(&ctx.root, &abs) {
            return Err(refuse("audio path is outside the workspace".into()));
        }
        return Ok(Located {
            project,
            clip_key: req.clip.clone(),
            clip: whole_of(&req.clip, &abs, None, None),
            window: None,
            source: PathBuf::from(&abs),
            edits: Edits::default(),
            grid,
            rack: true,
        });
    }
    let rf = material_ref_of(&req.clip).to_string();
    let placed = lite.clips.iter().find(|c| c.id == req.clip);
    let cg = Some(ClipGrid {
        tempo: grid.0,
        num: grid.1,
        den: grid.2,
    });
    let def = || audio_clip_def(&p.disk, &project, &rf, cg, p.rule).map_err(refuse);
    let of_def = |d: song::clips::audio_def::AudioClipDef| {
        whole_of(&d.id, &d.source, d.source_from, d.source_to)
    };
    let missing = || refuse(format!("no clip '{}'", req.clip));
    let clip = if req.whole {
        match def()? {
            Some(d) => of_def(d),
            None => placed.cloned().ok_or_else(missing)?,
        }
    } else {
        match placed {
            Some(c) => c.clone(),
            None => def()?.map(of_def).ok_or_else(missing)?,
        }
    };
    if clip.kind != ClipKind::Audio {
        return Err(refuse(format!("'{}' is not an audio clip", req.clip)));
    }
    let window = req
        .whole
        .then(|| material_window(placed.unwrap_or(&clip)))
        .flatten();
    let roots = roots_of(ctx, &project, d);
    let rooted: Vec<&str> = roots.iter().map(String::as_str).collect();
    let full = resolve_all(&[&project, &clip.source]);
    let source = (!clip.source.is_empty())
        .then(|| k7_file_service::confine::confine_to(&rooted, &full, &Disk))
        .flatten()
        .ok_or_else(|| refuse(format!("'{}': its file is outside the project", req.clip)))?;
    let edits = clip_edits(&p, &project, &rf)?;
    Ok(Located {
        project,
        clip_key: format!("{}{}", req.clip, if req.whole { "::whole" } else { "" }),
        clip,
        window,
        source: PathBuf::from(source),
        edits,
        grid,
        rack: false,
    })
}

/// The clip `loadAudioClipDoc` makes of a material: its source and window, every other field zero.
fn whole_of(id: &str, source: &str, from: Option<f64>, to: Option<f64>) -> ClipDoc {
    ClipDoc {
        id: id.into(),
        kind: ClipKind::Audio,
        name: basename(source),
        source: source.into(),
        start: 0.0,
        length: 0.0,
        offset: 0.0,
        source_from: from,
        source_to: to,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_db: None,
        loop_start: 0.0,
        loop_end: 0.0,
        notes: Vec::new(),
        warp: None,
        warp_off: None,
        disabled: None,
        event: None,
        comp: None,
        crossfade: None,
        nest: None,
    }
}

/// `clipOnsets`: the corrections a clip file holds; a file the typed reader refuses declines (2425).
fn clip_edits(p: &docs::Prepared, project: &str, rf: &str) -> Result<Edits, Why> {
    let family =
        song::read_family::<song::Clip>(&p.disk, &mut song::Cache::with_paths(p.rule), project)
            .map_err(|_| Why::Decline("clip-file"))?;
    let named = |f: &song::Finding| {
        let n = f.file.rsplit('/').next().unwrap_or("");
        n.len() > 4 && n[..n.len() - 4] == *rf && n[n.len() - 4..].eq_ignore_ascii_case(".k7x")
    };
    match family.get(rf).map(Arc::as_ref) {
        Some(song::Clip::Audio(a)) => Ok(a.onsets.as_ref().map_or_else(Edits::default, |o| {
            let added = o.added.as_ref().filter(|l| !l.is_empty());
            let removed = o.removed.as_ref().filter(|l| !l.is_empty());
            Edits {
                added: added.map(|l| l.iter().map(|n| n.0).collect()),
                removed: removed.map(|l| l.iter().map(|n| n.0).collect()),
            }
        })),
        Some(_) => Ok(Edits::default()),
        None if family.findings.iter().any(named) => Err(Why::Decline("clip-file")),
        None => Ok(Edits::default()),
    }
}

/// `projectRoots`, from `confine.rs`, with the code's `@meta` and the account's models folder.
fn roots_of(ctx: &Ctx, project: &str, d: &docs::Docs) -> Vec<String> {
    let code = resolve_all(&[&d.code_root]);
    let models = k7_file_service::media::names::models_root(
        d.env.get("STUDIO_MODELS").map(String::as_str),
        &ctx.home.studio,
    );
    k7_file_service::confine::project_roots(
        &ctx.home,
        &k7_file_service::paths::resolve_in(&code, &[project]),
        &k7_file_service::paths::join(&[&code, "packages", "song", "meta"]),
        &models,
    )
}

#[cfg(test)]
mod rack_tests {
    use super::rack_inside;

    // A root in the host's backslash spelling holds the slashed path a resolve gives.
    #[test]
    fn a_backslash_root_holds_its_racks_audio() {
        assert!(rack_inside("C:\\ws\\a", "C:/ws/a/kick.wav"));
        assert!(!rack_inside("C:\\ws\\a", "C:/ws/ab/kick.wav"));
    }
}

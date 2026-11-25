// providers.rs — the named completion providers behind commands.yaml's `complete:`, each a list of values off the project.
// A reader would get wrong that `nodes` is a project's USES of modules: `modules` adds the inherited refs, which no chain uses yet.

use super::{Ctx, files};
use song::doc::ProjectDoc;
use song::graph::node_places;
use song::modules::types::{
    IO_MODULES, MODULE_TYPES, completion_order, is_engine_ref, is_io_source, module_status_of, type_meta,
};
use song::modules::{Vocabulary, base_module_refs};
use song_edit::shared_folders::account_libraries;
use std::collections::HashSet;

/// What a provider answers: the values, and the labels a list shows for those a person cannot read.
type Answer = (Vec<String>, Vec<(String, String)>);

/// The main timeline's name as a `timeline lane copy` target.
const SONG_TARGET: &str = "main";

/// A provider by its catalogue name; one nobody knows offers nothing.
pub fn ask(name: &str, ctx: &mut Ctx) -> Answer {
    let values = match name {
        "baseModules" => base_modules(ctx).into_iter().filter(|r| !is_engine_ref(r)).collect(),
        "engineModules" => base_modules(ctx).into_iter().filter(|r| is_engine_ref(r)).collect(),
        "assemblies" => assemblies(ctx),
        "assemblyTemplates" => files::library_assemblies(ctx),
        "categories" => categories(),
        "moduleTargets" => module_targets(ctx),
        "modules" => modules(ctx),
        "nodes" => return nodes(ctx),
        "tracks" => tracks(ctx),
        "timelines" => timelines(ctx),
        "laneTargets" => [SONG_TARGET.to_owned()].into_iter().chain(timelines(ctx)).collect(),
        "timelineLanes" => timeline_lanes(ctx),
        "moduleAlternatives" => module_alternatives(ctx),
        "moduleEngines" => module_engines(ctx),
        "markerNames" => ctx
            .project()
            .map(|p| p.markers.iter().map(|m| m.name.clone()).collect())
            .unwrap_or_default(),
        "libraries" => account_libraries(ctx.fs, &ctx.env, Some(&ctx.song_path))
            .into_iter()
            .map(|l| l.name)
            .collect(),
        "playlists" => files::playlists(ctx),
        "pictureLanes" => picture_lanes(ctx),
        "clipTargets" => clip_targets(ctx),
        "chains" => chains(ctx),
        "curves" => files::curves(ctx),
        "surfaceTypes" => crate::verbs::project_more::SURFACE_TYPES.map(str::to_owned).to_vec(),
        "sequenceFiles" => files::sequence_files(ctx),
        "samples" => samples(ctx),
        "audioFiles" => files::audio_files(ctx),
        _ => Vec::new(),
    };
    (values, Vec::new())
}

/// The refs a new module may extend, from the vocabulary; a project that does not load has none.
fn base_modules(ctx: &mut Ctx) -> Vec<String> {
    ctx.vocabulary()
        .map(|v| base_module_refs(&v.modules))
        .unwrap_or_default()
}

/// `moduleStatuses`: each ref's `status:`, for the providers that rank by it.
fn status_of<'v>(vocab: &'v Option<std::sync::Arc<Vocabulary>>) -> impl Fn(&String) -> &'static str + 'v {
    move |name| {
        let status = vocab
            .as_ref()
            .and_then(|v| v.modules.get(name))
            .and_then(|m| m.authored.status)
            .map(|s| format!("{s:?}").to_lowercase());
        module_status_of(status.as_deref())
    }
}

/// The module half of each plugin a track or the master run holds, in the order the compile lists them.
fn plugin_modules(p: &ProjectDoc) -> Vec<String> {
    p.tracks
        .iter()
        .flat_map(|t| t.plugins.iter())
        .chain(p.master_plugins.iter())
        .map(|pl| crate::verbs::node_shared::plugin_module(&pl.id).to_owned())
        .collect()
}

fn assemblies(ctx: &mut Ctx) -> Vec<String> {
    let roots = song_edit::loader::roots_for(ctx.fs, &ctx.env, &ctx.song_path);
    let mut cache = song::Cache::new();
    song::assemblies::project_assemblies(ctx.fs, &mut cache, &roots.project)
        .map(|a| a.assemblies.into_iter().map(|x| x.id).collect())
        .unwrap_or_default()
}

fn categories() -> Vec<String> {
    MODULE_TYPES.iter().map(|t| format!("{}/", t.name)).collect()
}

/// What `node new` makes a node OF: the project's modules, the io sources whole, then the folders for a new name.
fn module_targets(ctx: &mut Ctx) -> Vec<String> {
    let mut out: Vec<String> = base_modules(ctx)
        .into_iter()
        .filter(|r| !r.starts_with('@') && r.split('/').next().is_some_and(|c| type_meta(c).is_some()))
        .collect();
    out.extend(
        IO_MODULES
            .iter()
            .filter(|(_, io)| is_io_source(io))
            .map(|(name, _)| format!("io/{name}")),
    );
    out.extend(categories());
    out
}

/// The project's nodes by module plus the inherited refs, the hidden dropped and the favourites first.
fn modules(ctx: &mut Ctx) -> Vec<String> {
    let used = ctx.project().map(plugin_modules).unwrap_or_default();
    let inherited = base_modules(ctx)
        .into_iter()
        .filter(|r| r.starts_with('@') && !is_engine_ref(r));
    let vocab = ctx.vocabulary();
    completion_order(used.into_iter().chain(inherited).collect(), status_of(&vocab))
}

/// This project's nodes, with a chain project's uuid shown as its module and its place.
fn nodes(ctx: &mut Ctx) -> Answer {
    let values = ctx.project().map(plugin_modules).unwrap_or_default();
    let labels = ctx
        .inputs()
        .and_then(|i| i.routing.project.as_ref())
        .map(|p| {
            node_places(p)
                .into_iter()
                .map(|(name, pl)| {
                    let caption = match pl.uses {
                        Some(u) => format!("{u} · {}", pl.place),
                        None => pl.place,
                    };
                    (name.to_owned(), caption)
                })
                .collect()
        })
        .unwrap_or_default();
    (values, labels)
}

/// The tracks, and a timeline's lanes addressed `<timeline>><lane>`.
fn tracks(ctx: &mut Ctx) -> Vec<String> {
    let Some(p) = ctx.project() else {
        return Vec::new();
    };
    let lanes = p
        .timelines
        .iter()
        .flatten()
        .flat_map(|t| t.lanes.iter().map(|l| l.id.clone()));
    p.tracks.iter().map(|t| t.id.clone()).chain(lanes).collect()
}

fn timelines(ctx: &mut Ctx) -> Vec<String> {
    ctx.project()
        .map(|p| p.timelines.iter().flatten().map(|t| t.id.clone()).collect())
        .unwrap_or_default()
}

fn timeline_lanes(ctx: &mut Ctx) -> Vec<String> {
    let mut out: Vec<String> = ctx
        .project()
        .map(|p| {
            p.timelines
                .iter()
                .flatten()
                .flat_map(|t| t.lanes.iter().map(|l| l.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    out.extend(files::timeline_pictures(ctx).into_iter().map(|(_, lane)| lane));
    out
}

/// Every alternative name of every module, deduped: a provider is not told which module was named.
fn module_alternatives(ctx: &mut Ctx) -> Vec<String> {
    let Ok(modules) = song_edit::loader::load_modules(ctx.fs, &ctx.env, &ctx.song_path) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    modules
        .values()
        .flat_map(|m| song::resolve::alternatives_of(&m.authored).into_iter().skip(1))
        .map(|a| a.name)
        .filter(|n| seen.insert(n.clone()))
        .collect()
}

/// Every engine name the project's modules offer, deduped and sorted.
fn module_engines(ctx: &mut Ctx) -> Vec<String> {
    let Some(vocab) = ctx.vocabulary() else {
        return Vec::new();
    };
    let mut out: Vec<String> = vocab
        .modules
        .values()
        .flat_map(|m| m.engine_refs.iter().flat_map(|r| r.keys().cloned()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    out.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    out
}

/// The song's picture lanes, then each timeline's addressed `<timeline>><lane>`.
fn picture_lanes(ctx: &mut Ctx) -> Vec<String> {
    let mut own = ctx.picture_lanes();
    own.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    own.extend(
        files::timeline_pictures(ctx)
            .into_iter()
            .map(|(t, l)| song::clips::timeline_lane_id(&t, &l)),
    );
    own
}

/// What `clip process` accepts: audio clips, then timelines, then audio files, and no track.
fn clip_targets(ctx: &mut Ctx) -> Vec<String> {
    let clips: Vec<String> = ctx
        .project()
        .map(|p| {
            p.tracks
                .iter()
                .flat_map(|t| &t.clips)
                .filter(|c| c.kind == song::clips::ClipKind::Audio && !c.source.is_empty())
                .map(|c| c.id.clone())
                .collect()
        })
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut out: Vec<String> = clips.into_iter().filter(|c| seen.insert(c.clone())).collect();
    out.extend(timelines(ctx));
    out.extend(files::audio_files(ctx));
    out
}

/// The chains a bounce can run: the project's own by their `<chain>:` prefix, then the libraries'.
fn chains(ctx: &mut Ctx) -> Vec<String> {
    let mut own: Vec<String> = ctx
        .project()
        .map(|p| {
            p.tracks
                .iter()
                .flat_map(|t| &t.plugins)
                .filter_map(|pl| pl.id.find(':').filter(|at| *at > 0).map(|at| pl.id[..at].to_owned()))
                .collect::<HashSet<_>>()
                .into_iter()
                .collect()
        })
        .unwrap_or_default();
    own.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    own.extend(files::library_chains(ctx));
    own
}

/// The samples a rack tap may play, `@<library>/<path>` and `@project/<path>`.
fn samples(ctx: &Ctx) -> Vec<String> {
    let mut v = crate::verbs::node_shared::library_files_of(ctx.fs, &ctx.env, &ctx.song_path, "samples");
    v.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    v
}

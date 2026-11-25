// ctx.rs — what a provider reads: the files, the compiled project and the module vocabulary, each read once per completion.
// A reader would get wrong that a read that fails is an empty answer: a failed provider costs its list, never the command bar.

use song::Cache;
use song::doc::{ProjectDoc, Request, Vsts, compile};
use song::graph::load_graph;
use song::lanes::inputs::{Inputs, load_inputs};
use song::lanes::rig::Rig;
use song::modules::Vocabulary;
use song_edit::Fs;
use song_edit::confine::{Env, workspace_dir_of};
use song_edit::loader::roots_for;
use std::sync::Arc;

pub struct Ctx<'a> {
    pub fs: &'a dyn Fs,
    pub env: Env,
    /// The project folder, absolute in the `Fs`'s spelling.
    pub song_path: String,
    cache: &'a mut Cache,
    doc: Option<Option<ProjectDoc>>,
    inputs: Option<Option<Inputs>>,
}

impl<'a> Ctx<'a> {
    pub fn new(fs: &'a dyn Fs, cache: &'a mut Cache, env: Env, song_path: &str) -> Ctx<'a> {
        Ctx {
            fs,
            env,
            song_path: song_path.to_owned(),
            cache,
            doc: None,
            inputs: None,
        }
    }

    /// The workspace the project stands in: where `_playlists/` lives.
    pub fn workspace(&self) -> String {
        workspace_dir_of(&self.env, &self.song_path)
    }

    /// The document the engine is sent, or None when the project does not compile.
    pub fn project(&mut self) -> Option<&ProjectDoc> {
        if self.doc.is_none() {
            let req = Request {
                roots: roots_for(self.fs, &self.env, &self.song_path),
                rig: Rig::default(),
                vsts: Vsts::Scan,
                thawed: Vec::new(),
            };
            let doc = compile(self.fs, self.cache, &req).ok().and_then(|c| c.engine.ok());
            self.doc = Some(doc);
        }
        self.doc.as_ref().and_then(Option::as_ref)
    }

    /// The project's inputs as the graph reads them, or None when they do not load.
    pub fn inputs(&mut self) -> Option<&Inputs> {
        if self.inputs.is_none() {
            let roots = roots_for(self.fs, &self.env, &self.song_path);
            let read = load_inputs(self.fs, self.cache, &roots, Rig::default()).ok();
            self.inputs = Some(read);
        }
        self.inputs.as_ref().and_then(Option::as_ref)
    }

    /// The picture lanes of the song's graph; a graph that does not load has none.
    pub fn picture_lanes(&self) -> Vec<String> {
        let roots = roots_for(self.fs, &self.env, &self.song_path);
        let mut cache = Cache::new();
        let Ok(inputs) = load_inputs(self.fs, &mut cache, &roots, Rig::default()) else {
            return Vec::new();
        };
        load_graph(self.fs, &mut cache, &roots, &inputs)
            .map(|g| g.video_tracks.keys().map(|k| (*k).to_owned()).collect())
            .unwrap_or_default()
    }

    /// Every module a project can name by ref, as `loadModuleRefs` answers.
    pub fn vocabulary(&mut self) -> Option<Arc<Vocabulary>> {
        let roots = roots_for(self.fs, &self.env, &self.song_path);
        self.cache.vocabulary(self.fs, &roots).ok()
    }
}

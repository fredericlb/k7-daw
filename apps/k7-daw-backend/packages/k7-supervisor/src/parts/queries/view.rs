//! view.rs — what the engine told the queries part, as the slice of the snapshot it owns: the interfaces and
//! their menu signature, the catalogue, the plugin instances, the sessions and the pairing lists. The part
//! hands the core a copy as a patch; the core drops it when the user stops the engine.

use crate::protocol::{
    AudioDevice, EditPlugin, PairRequest, PairedDevice, Plugin, Session, Snapshot,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    pub devices: Vec<AudioDevice>,
    pub menu_sig: String,
    /// None is the spinner: the catalogue is not known yet, or a rescan was asked.
    pub plugin_count: Option<u32>,
    pub plugins: Vec<Plugin>,
    pub plugins_sig: String,
    pub edit_plugins: Vec<EditPlugin>,
    pub edit_plugins_sig: String,
    pub sessions: Vec<Session>,
    pub sessions_sig: String,
    pub pair_requests: Vec<PairRequest>,
    pub paired_devices: Vec<PairedDevice>,
}

impl View {
    pub fn apply(&self, s: &mut Snapshot) {
        s.audio_devices.clone_from(&self.devices);
        s.audio_menu_sig.clone_from(&self.menu_sig);
        s.plugin_count = self.plugin_count;
        s.plugins.clone_from(&self.plugins);
        s.plugins_sig.clone_from(&self.plugins_sig);
        s.edit_plugins.clone_from(&self.edit_plugins);
        s.edit_plugins_sig.clone_from(&self.edit_plugins_sig);
        s.sessions.clone_from(&self.sessions);
        s.sessions_sig.clone_from(&self.sessions_sig);
        s.pair_requests.clone_from(&self.pair_requests);
        s.paired_devices.clone_from(&self.paired_devices);
    }
}

/// The catalogue's signature: format and name per plugin, so a changed manufacturer is no changed catalogue.
pub fn plugins_sig(plugins: &[Plugin]) -> String {
    plugins
        .iter()
        .map(|p| format!("{}\0{}\n", p.format, p.name))
        .collect()
}

/// The OPENABLE ids, session-qualified: a name moving is not a menu that changed.
pub fn edit_plugins_sig(plugins: &[EditPlugin]) -> String {
    plugins
        .iter()
        .filter(|p| p.has_native_plugin_ui)
        .map(|p| format!("{}\0{}\n", p.session, p.id))
        .collect()
}

/// What the menu SHOWS, the playhead excluded.
pub fn sessions_sig(sessions: &[Session]) -> String {
    sessions
        .iter()
        .map(|s| {
            format!(
                "{id}\u{1f}{project}\u{1f}{playing}\u{1f}{tracks}\u{1f}{listeners}\n",
                id = s.id,
                project = s.project,
                playing = u8::from(s.playing),
                tracks = s.tracks,
                listeners = s.listeners
            )
        })
        .collect()
}

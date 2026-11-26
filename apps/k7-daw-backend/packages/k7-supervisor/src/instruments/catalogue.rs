//! catalogue.rs — every instrument the engine can play, what an app's Downloads list shows. Each is pinned by
//! the archive's hash: a GitHub commit's zip for the piano and the drums, plain files for the others.

use std::sync::OnceLock;

/// One instrument: its folder name, what an app calls it, the archive it unpacks from, and that archive's identity.
#[derive(Debug, Clone, PartialEq)]
pub struct Instrument {
    pub name: String,
    pub label: String,
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
}

const ROWS: &[(&str, &str, &str, &str, u64)] = &[
    (
        "salamander-grand-piano",
        "Salamander Grand Piano",
        "https://codeload.github.com/sfzinstruments/SalamanderGrandPiano/zip/3382bf9496bba2486f5ab0de55a264d1dfc38404",
        "4f64bc2dadf2eb893e7fb824c2982c03962025e1c82ab9e593e2f7269c50a476",
        748522601,
    ),
    (
        "nbo-stereo-rhodes",
        "Stereo Rhodes",
        "https://www.bandshed.net/sounds/sfz/stereo_rhodes.zip",
        "d54168cf4e3ae231bd91634855ad75fa534d48cd38a3258ea6fe77da0eb40a14",
        74489660,
    ),
    (
        "nbo-wurt",
        "Wurlitzer",
        "https://www.bandshed.net/sounds/sfz/wurt.zip",
        "b7c3b7c0bf76b22e5ab3c6b3d7f997b488a3815608f795b570a0452c31dc79ba",
        7947927,
    ),
    (
        "lithalean-clavinet",
        "Clavinet",
        "https://musical-artifacts.com/artifacts/646/Clavinet.zip",
        "b434bd4b3eff7232948663a0b343ce79512229da912c8edb09f1a6fa2abc060d",
        57271531,
    ),
    // An acoustic drum kit, Wilkinson Audio's (CC BY 4.0), at one commit of sfzinstruments' port.
    (
        "naked-drums",
        "Naked Drums",
        "https://codeload.github.com/sfzinstruments/WilkinsonAudio.NakedDrums/zip/407732a548606ac715ad8f3fbac63d9f4df77c0d",
        "a58f73d1c143f77809e45395965895b513890fa2cc7f902172e407862fcc7f11",
        1283095413,
    ),
    // …and a second, Karoryfer's (CC0), with brushes, mallets and a rusty character of its own.
    (
        "big-rusty-drums",
        "Big Rusty Drums",
        "https://codeload.github.com/sfzinstruments/karoryfer.big-rusty-drums/zip/f07ce00df34a46b6b08375be56fe116cf15782bc",
        "e1f9944f43570ffe0c4d1d57750805c605e3df63dc943b622f5d6dc7b35e2781",
        619428922,
    ),
    // Six vintage drum machines, one per octave — The Tic Tok Men's, at one commit of sfzinstruments' port.
    (
        "retro-drums",
        "Retro Drums",
        "https://codeload.github.com/sfzinstruments/tictokmen.retrodrums1/zip/0d1ba0a79f7616b302deb762f7390ba3c3cd1662",
        "84a1743ce653fdcc6b8f7dda03248882f632087d57b9990f58d9496c450d56e7",
        1916192,
    ),
];

/// Every instrument the engine can play.
pub fn all() -> &'static [Instrument] {
    static LIST: OnceLock<Vec<Instrument>> = OnceLock::new();
    LIST.get_or_init(|| {
        ROWS.iter()
            .map(|r| Instrument {
                name: r.0.into(),
                label: r.1.into(),
                url: r.2.into(),
                sha256: r.3.into(),
                bytes: r.4,
            })
            .collect()
    })
}

/// The one named `name`.
pub fn find(name: &str) -> Option<&'static Instrument> {
    all().iter().find(|i| i.name == name)
}

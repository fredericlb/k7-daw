//! The wire structs hold the routes' own JSON: what a builder writes reads back through them, unknown keys refused.
//! A reader would get wrong that a struct here is the route's output type: it is the TypeScript type's source, and this is what keeps it honest.

use k7_file_service::engine::sync::SyncState;
use k7_file_service::projects::{Info, info_json};
use k7_file_service::runtime::list::Table;
use k7_file_service::wire::*;
use k7_file_service::workspaces::{
    Project, Row, Summary, Surfaced, rows_json, summaries_json, surfaced_json,
};

fn rows() -> Vec<Row> {
    let surface = |id: &str, kind: &str| Surfaced {
        id: id.into(),
        kind: kind.into(),
        playlist: None,
    };
    let project = |path: &str, tags: &[&str], demo: bool, edited: Option<f64>| Project {
        path: path.into(),
        app: "k7",
        tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        surfaces: vec![surface("main", "timeline"), surface("decks", "turntables")],
        active: path == "a.k7",
        edited_at: edited,
        demo,
    };
    vec![
        Row {
            name: "songs".into(),
            single: false,
            projects: vec![
                project("a.k7", &["rock"], false, Some(1.7e12)),
                project("b/c.k7", &[], true, None),
            ],
        },
        Row {
            name: "solo.k7".into(),
            single: true,
            projects: vec![Project {
                surfaces: vec![],
                ..project(".", &[], false, None)
            }],
        },
    ]
}

#[test]
fn the_listings_read_back() {
    let sums = [Summary {
        name: "w".into(),
        single: false,
        count: 3,
    }];
    let list: WorkspaceList = serde_json::from_str(&summaries_json(&sums)).unwrap();
    assert_eq!(list.workspaces[0].project_count, 3);
    let all: AccountWorkspaceList = serde_json::from_str(&rows_json(&rows())).unwrap();
    assert_eq!(
        all.workspaces[0].projects[0].tags.as_deref(),
        Some(&["rock".to_owned()][..])
    );
    assert_eq!(all.workspaces[0].projects[1].demo, Some(true));
    let surfaced: SurfacedProjectList = serde_json::from_str(&surfaced_json(&rows())).unwrap();
    assert_eq!(surfaced.projects.len(), 2);
    assert_eq!(surfaced.projects[0].surfaces[1].type_, "turntables");
}

#[test]
fn the_open_workspace_reads_back() {
    let info = Info {
        root: "/r".into(),
        root_is_project: false,
        active: Some("a.k7".into()),
        projects: vec!["a.k7".into()],
        active_is_demo: true,
    };
    let tail = r#""lastTake":null,"engineSync":{"files":1,"filesDone":0,"bytes":10,"bytesDone":0},"engineLoad":"copying","engineScan":null,"processes":[],"rev":"3""#;
    let read: WorkspaceInfo = serde_json::from_str(&info_json(&info, tail)).unwrap();
    assert_eq!(read.engine_load, Some(EngineLoadPhase::Copying));
    assert_eq!(read.active_is_demo, Some(true));
}

#[test]
fn a_process_and_a_copy_read_back() {
    let mut table = Table::new();
    let id = table.start("render", 0, 1_000);
    table.progress(&id, Some(0.5), Some("frame 3"));
    let row = table.list(1_000).remove(0).to_json();
    let read: ServiceProcess = serde_json::from_value(row).unwrap();
    assert_eq!(
        (read.state, read.progress),
        (ProcessState::Running, Some(0.5))
    );
    let state = SyncState {
        files: 2,
        files_done: 1,
        bytes: 9,
        bytes_done: 4,
        failed: Some(1),
        failures: Some(vec!["a: no".into()]),
        error: Some("x".into()),
    };
    let read: EngineSyncState = serde_json::from_value(state.to_json()).unwrap();
    assert_eq!((read.files_done, read.failed), (1, Some(1)));
}

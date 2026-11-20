//! The cut's chain: `resolveColdRun` rule for rule, and `findLibraryChainBy`'s two orders (1p-D14, 1p-D25).

use k7_file_service::chains::{ChainSettings, ColdRun, find_by, resolve_cold_run};

use crate::support::chains::World;

const VOX: &str =
    "cold(true)\nplugin({ type: \"offline\", name: \"vox\" })\nparams({ onnx: \"vox.onnx\" })\n";

fn sep(module: &str) -> String {
    format!("steps([id(\"s\").extends(\"@k7-core/{module}\"), to([chain(\"@out\")])])\n")
}

fn world(name: &str) -> World {
    let s = sep("cold/vox");
    World::new(
        name,
        &[
            ("libraries/k7-core/modules/cold/vox/module.k7x", VOX),
            ("libraries/k7-core/modules/cold/vox/assets/vox.onnx", "m"),
            ("libraries/k7-core/chains/x/lead.k7x", &s),
            ("libraries/zed/chains/lead.k7x", &s),
            ("libraries/zed/chains/x/lead.k7x", &s),
            ("studio/library/chains/lead.k7x", &s),
        ],
    )
}

#[test]
fn a_bare_name_matches_a_path_in_any_library_before_a_last_segment_in_the_default_one() {
    let w = world("chains-run-find");
    let c = w.chains();
    let ref_of = |library: Option<&str>, want: &str| {
        find_by(&c, library, want).map(|i| c.refs()[i].to_owned())
    };
    // The path `lead` is held by `@account` and `@zed`, the default library holds only `x/lead`: the path wins.
    assert_eq!(ref_of(None, "lead").as_deref(), Some("@account/lead"));
    assert_eq!(ref_of(None, "x/lead").as_deref(), Some("@k7-core/x/lead"));
    assert_eq!(ref_of(Some("zed"), "lead").as_deref(), Some("@zed/lead"));
    assert_eq!(
        ref_of(Some("k7-core"), "lead").as_deref(),
        Some("@k7-core/x/lead")
    );
    assert_eq!(ref_of(None, "nothing"), None);
}

#[test]
fn each_refusal_has_its_status_and_the_typescripts_sentence() {
    let w = world("chains-run-refuse");
    let c = w.chains();
    let none = ChainSettings::default();
    let refused = |want: &str, chain: &str, prefs: &ChainSettings| match resolve_cold_run(
        &c, prefs, want, chain,
    ) {
        Ok(ColdRun::Refused { status, error }) => (status, error),
        other => panic!("{want}/{chain}: {other:?}"),
    };
    assert_eq!(
        refused("both", "x", &none),
        (400, "want must be 'stems' or 'instrumental'".into())
    );
    assert_eq!(refused("", "", &none).0, 409);
    assert!(
        refused("instrumental", "", &none)
            .1
            .contains("makes no instrumental")
    );
    let (status, error) = refused("stems", "@k7-core/missing", &none);
    assert_eq!(status, 400);
    assert!(
        error.starts_with("no chain '@k7-core/missing' — known here: @account/lead,"),
        "{error}"
    );
    assert_eq!(
        refused("instrumental", "@zed/lead", &none),
        (
            400,
            "'@zed/lead' writes drums, bass, other, vocals and no instrumental".into()
        )
    );
    assert!(matches!(
        resolve_cold_run(&c, &none, " STEMS ", "  @zed/lead "),
        Ok(ColdRun::Ok { .. })
    ));
    let set = ChainSettings {
        stems: Some("@zed/x/lead".into()),
        instrumental: None,
    };
    assert!(matches!(
        resolve_cold_run(&c, &set, "", ""),
        Ok(ColdRun::Ok { .. })
    ));
}

#[test]
fn a_chain_with_a_live_step_is_refused_as_not_separating_and_no_library_is_a_409() {
    let live = "plugin({ type: \"internal\", name: \"Comp\" })\n";
    let live_chain = sep("audio/comp");
    let w = World::new(
        "chains-run-live",
        &[
            ("libraries/k7-core/modules/audio/comp/module.k7x", live),
            ("libraries/k7-core/chains/c.k7x", &live_chain),
        ],
    );
    let none = ChainSettings::default();
    assert_eq!(
        resolve_cold_run(&w.chains(), &none, "stems", "c"),
        Ok(ColdRun::Refused {
            status: 400,
            error: "'c' does not separate — it puts the signal through 1 module(s)".into()
        })
    );
    let w = World::new(
        "chains-run-none",
        &[("studio/library/chains/c.k7x", &live_chain)],
    );
    assert!(matches!(
        resolve_cold_run(&w.chains(), &none, "stems", "c"),
        Ok(ColdRun::Refused { status: 409, .. })
    ));
}

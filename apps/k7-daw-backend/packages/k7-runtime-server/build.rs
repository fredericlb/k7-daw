//! build.rs — the build stamp `wire::version::build()` reports: `$K7_BUILD`, else the `version` of k7-runtime's
//! package.json (the root's, copied), else "dev".

fn stamp(json: &str) -> Option<String> {
    let rest = json.split("\"version\"").nth(1)?;
    let start = rest.find('"')? + 1;
    let len = rest[start..].find('"')?;
    Some(rest[start..start + len].to_string())
}

fn main() {
    println!("cargo:rerun-if-env-changed=K7_BUILD");
    let file = "../../../k7-daw-engine/k7-runtime/package.json";
    println!("cargo:rerun-if-changed={file}");
    let build = std::env::var("K7_BUILD")
        .ok()
        .filter(|b| !b.is_empty())
        .or_else(|| stamp(&std::fs::read_to_string(file).unwrap_or_default()))
        .unwrap_or_else(|| "dev".into());
    println!("cargo:rustc-env=K7_BUILD_STAMP={build}");
}

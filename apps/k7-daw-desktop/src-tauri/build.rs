// build.rs — K7_RELEASE, the version every build names itself by: the root package.json's, the one source of it.
fn main() {
    let root = std::path::Path::new("../../../package.json");
    println!("cargo:rerun-if-changed={}", root.display());
    let text = std::fs::read_to_string(root).expect("the root package.json");
    let json: serde_json::Value =
        serde_json::from_str(&text).expect("the root package.json is JSON");
    let version = json["version"]
        .as_str()
        .expect("a version in the root package.json");
    println!("cargo:rustc-env=K7_RELEASE={version}");
    tauri_build::build();
}

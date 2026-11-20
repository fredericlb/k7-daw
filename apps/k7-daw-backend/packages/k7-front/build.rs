//! The release version is the app's: `apps/k7-daw-frontend/package.json`, which scripts/check-version.mjs keeps equal to the root's.
//! Not the root's own file, which changes with every script and would rebuild the front and miss the image's cache.

fn main() {
    let path = "../../../k7-daw-frontend/package.json";
    println!("cargo:rerun-if-changed={path}");
    let text = std::fs::read_to_string(path).expect("apps/k7-daw-frontend/package.json");
    let version = text
        .split("\"version\"")
        .nth(1)
        .and_then(|rest| rest.split('"').nth(1))
        .expect("a version in apps/k7-daw-frontend/package.json");
    println!("cargo:rustc-env=K7_VERSION={version}");
}

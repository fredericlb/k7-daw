// als_zip.rs — an Ableton project ZIPPED: the folder "Collect All and Save" leaves, sent as one file.
// What a reader gets wrong: Live keeps old saves under Backup/, each a .als too, and those are never the project.

use std::collections::HashMap;

use crate::ableton::{SampleRef, parse_als};
use crate::inflate::{MAX_INFLATED_BYTES, bounded_unzip};
use crate::model::{Bytes, Translation};

/// Entries no project is made of: Live's backups, and the resource forks a Mac zip carries.
fn noise(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let folder = |name: &str| lower.starts_with(&format!("{name}/")) || lower.contains(&format!("/{name}/"));
    folder("backup") || folder("__macosx") || lower.starts_with("._") || lower.contains("/._")
}

fn dir_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

/// `a/b/../c` is `a/c`: a sample path is relative to the set, and may climb out of its folder.
fn normalise(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    out.join("/")
}

/// The set a zip holds: the one `.als` outside Backup/, the shallowest when a folder nests another.
pub fn pick_set(paths: &[&str]) -> Result<String, String> {
    let sets: Vec<&str> = paths
        .iter()
        .copied()
        .filter(|p| p.to_ascii_lowercase().ends_with(".als") && !noise(p))
        .collect();
    let depth = |p: &str| p.split('/').count();
    let Some(top) = sets.iter().map(|p| depth(p)).min() else {
        return Err("this zip holds no Ableton set (.als) — zip the project folder Live's Collect All and Save leaves".into());
    };
    let shallow: Vec<&str> = sets.into_iter().filter(|p| depth(p) == top).collect();
    if shallow.len() > 1 {
        return Err(format!(
            "this zip holds {} Ableton sets ({}) — zip one project at a time",
            shallow.len(),
            shallow.join(", ")
        ));
    }
    Ok(shallow[0].to_owned())
}

/// Translates a zipped Ableton project; `fallback` names the song when the set states none.
pub fn parse_als_zip(
    bytes: &[u8],
    fallback: &str,
    inflate_limit: Option<usize>,
) -> Result<(Translation, String), String> {
    if bytes.len() < 4 || bytes[0] != 0x50 || bytes[1] != 0x4b {
        return Err("this is not a zip file".into());
    }
    let limit = inflate_limit.unwrap_or(MAX_INFLATED_BYTES);
    let entries = bounded_unzip(bytes, limit)?;
    let by_path: HashMap<&str, &Bytes> = entries.iter().map(|(p, b)| (p.as_str(), b)).collect();
    let names: Vec<&str> = entries.iter().map(|(p, _)| p.as_str()).collect();
    let set = pick_set(&names)?;
    let base = dir_of(&set).to_owned();
    let mut by_name: HashMap<String, Vec<&str>> = HashMap::new();
    for p in &names {
        if noise(p) || p.ends_with('/') {
            continue;
        }
        let name = p.rsplit('/').next().unwrap_or(p).to_ascii_lowercase();
        by_name.entry(name).or_default().push(p);
    }
    // Where the set says, relative to itself; else the one file of that name — an absolute path names another machine.
    let read = |r: &SampleRef| -> Option<Bytes> {
        if let Some(rel) = &r.relative {
            let joined = if base.is_empty() { rel.clone() } else { format!("{base}/{rel}") };
            if let Some(b) = by_path.get(normalise(&joined).as_str()) {
                return Some((*b).clone());
            }
        }
        match by_name.get(&r.name.to_ascii_lowercase()).map(Vec::as_slice) {
            Some([only]) => by_path.get(only).map(|b| (*b).clone()),
            _ => None,
        }
    };
    let stem = {
        let file = set.rsplit('/').next().unwrap_or(&set);
        file[..file.len() - ".als".len()].to_owned()
    };
    let name = if stem.is_empty() { fallback } else { &stem };
    let set_bytes = by_path.get(set.as_str()).ok_or("the zip's set vanished")?;
    let t = parse_als(set_bytes, name, Some(&read), inflate_limit)?;
    Ok((t, set))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_and_mac_forks_are_never_the_set() {
        let paths = ["p/Backup/p [2025].als", "__MACOSX/p/._p.als", "p/p.als", "p/Samples/a.wav"];
        assert_eq!(pick_set(&paths).unwrap(), "p/p.als");
        assert!(pick_set(&["Backup/x.als"]).is_err());
        assert!(pick_set(&["a.als", "b.als"]).unwrap_err().contains("2 Ableton sets"));
        assert_eq!(normalise("p/Samples/../x/./a.wav"), "p/x/a.wav");
    }
}

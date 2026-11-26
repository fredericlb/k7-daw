// project_tags.rs — a project's TAGS, `project.tags` in its project.k7x: free words a person files a song under, shown beside it on the home page.
// A reader would get wrong that a file that does not parse has no tags rather than an error: a listing must not fail on one.

use k7x::edit::{Value, parse};

/// The tags of a project.k7x's text: its non-empty strings, none for a file that does not parse.
#[must_use]
pub fn project_tags_of(text: &str) -> Vec<String> {
    let Ok(data) = parse(text) else {
        return Vec::new();
    };
    match data.get("project").and_then(|p| p.get("tags")) {
        Some(Value::List(tags)) => tags
            .iter()
            .filter_map(|t| match t {
                Value::Str(s) if !s.is_empty() => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_project_tags_and_tolerates_a_broken_file() {
        assert_eq!(project_tags_of("project({ name: \"a\", tags: [\"live\", 3, \"\"] })\n"), ["live"]);
        assert!(project_tags_of("project({ name: \"a\" })\n").is_empty());
        assert!(project_tags_of("project([\n").is_empty());
        assert!(project_tags_of("project({ tags: \"live\" })\n").is_empty());
    }
}

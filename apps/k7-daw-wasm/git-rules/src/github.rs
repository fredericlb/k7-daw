// github.rs — how the picker lists a GitHub account's repositories: k7 workspaces first, each list most recently pushed first.
// A reader would get wrong that a repository with no push date (an empty one) sorts last, where the page's old sort left it in place.

use chrono::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct GithubRepo {
    pub full_name: String,
    pub ssh_url: String,
    pub private: bool,
    #[serde(default)]
    pub pushed_at: Option<String>,
    /// Carries the `k7-workspace` topic.
    pub k7: bool,
}

/// The picker's two lists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Arranged {
    pub k7: Vec<GithubRepo>,
    pub others: Vec<GithubRepo>,
}

/// An ISO time in milliseconds since the epoch, or none when it is not one.
fn millis(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso).ok().map(|t| t.timestamp_millis())
}

/// The picker's two lists: k7 workspaces under their own heading, then the rest, each most recently pushed first.
#[must_use]
pub fn arrange(repos: &[GithubRepo], filter: &str) -> Arranged {
    let q = filter.trim().to_lowercase();
    let mut shown: Vec<&GithubRepo> = repos
        .iter()
        .filter(|r| r.full_name.to_lowercase().contains(&q))
        .collect();
    let pushed = |r: &GithubRepo| r.pushed_at.as_deref().and_then(millis).unwrap_or(i64::MIN);
    shown.sort_by_key(|r| std::cmp::Reverse(pushed(r)));
    let (k7, others) = shown.into_iter().cloned().partition(|r| r.k7);
    Arranged { k7, others }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum AgoUnit {
    Now,
    Minutes,
    Hours,
    Days,
    Months,
    Years,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Ago {
    pub unit: AgoUnit,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub n: i64,
}

/// How long ago a push was, in the largest whole unit; no date reads as ninety-nine years.
#[must_use]
pub fn pushed_ago(iso: Option<&str>, now_ms: f64) -> Ago {
    let Some(t) = iso.and_then(millis) else {
        return Ago {
            unit: AgoUnit::Years,
            n: 99,
        };
    };
    let min = (((now_ms - t as f64) / 60_000.0).floor()).max(0.0) as i64;
    let ago = |unit, n| Ago { unit, n };
    if min < 1 {
        return ago(AgoUnit::Now, 0);
    }
    if min < 60 {
        return ago(AgoUnit::Minutes, min);
    }
    if min < 60 * 24 {
        return ago(AgoUnit::Hours, min / 60);
    }
    let days = min / (60 * 24);
    if days < 30 {
        ago(AgoUnit::Days, days)
    } else if days < 365 {
        ago(AgoUnit::Months, days / 30)
    } else {
        ago(AgoUnit::Years, days / 365)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(full: &str, pushed: &str, k7: bool) -> GithubRepo {
        GithubRepo {
            full_name: full.into(),
            ssh_url: format!("git@github.com:{full}.git"),
            private: true,
            pushed_at: Some(pushed.into()),
            k7,
        }
    }
    fn all() -> Vec<GithubRepo> {
        vec![
            repo("o/old", "2026-01-01T00:00:00Z", false),
            repo("o/song", "2026-03-01T00:00:00Z", true),
            repo("o/new", "2026-05-01T00:00:00Z", false),
            repo("o/band", "2026-02-01T00:00:00Z", true),
        ]
    }
    fn names(v: &[GithubRepo]) -> Vec<&str> {
        v.iter().map(|r| r.full_name.as_str()).collect()
    }

    #[test]
    fn puts_k7_workspaces_under_their_own_heading_each_most_recent_first() {
        let a = arrange(&all(), "");
        assert_eq!(names(&a.k7), ["o/song", "o/band"]);
        assert_eq!(names(&a.others), ["o/new", "o/old"]);
    }

    #[test]
    fn filters_on_the_full_name_without_case_across_both_lists() {
        let a = arrange(&all(), " SO");
        assert_eq!(names(&a.k7), ["o/song"]);
        assert!(a.others.is_empty());
    }

    #[test]
    fn sorts_a_repository_with_no_push_date_last() {
        let mut v = all();
        v.push(GithubRepo {
            pushed_at: None,
            ..repo("o/empty", "", false)
        });
        assert_eq!(names(&arrange(&v, "").others), ["o/new", "o/old", "o/empty"]);
    }

    #[test]
    fn takes_the_largest_whole_unit() {
        let now = DateTime::parse_from_rfc3339("2026-06-01T12:00:00Z")
            .unwrap()
            .timestamp_millis() as f64;
        let ago = |iso| pushed_ago(Some(iso), now);
        let is = |a: Ago, unit, n| assert_eq!(a, Ago { unit, n });
        is(ago("2026-06-01T11:59:50Z"), AgoUnit::Now, 0);
        is(ago("2026-06-01T11:15:00Z"), AgoUnit::Minutes, 45);
        is(ago("2026-06-01T07:00:00Z"), AgoUnit::Hours, 5);
        is(ago("2026-05-29T12:00:00Z"), AgoUnit::Days, 3);
        is(ago("2026-03-01T12:00:00Z"), AgoUnit::Months, 3);
        is(ago("2024-06-01T12:00:00Z"), AgoUnit::Years, 2);
        is(pushed_ago(None, now), AgoUnit::Years, 99);
        is(pushed_ago(Some("nonsense"), now), AgoUnit::Years, 99);
    }
}

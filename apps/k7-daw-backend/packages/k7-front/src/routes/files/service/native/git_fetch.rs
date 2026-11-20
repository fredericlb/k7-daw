//! git_fetch.rs — when a workspace with a remote fetches: at open, every 5 minutes while the page says it is in front, when it
//! comes back to the front, and no more than every 30 minutes after a failure, which is said once.
//! What a reader gets wrong: `Cadence` holds no clock, the caller passes seconds, so the rules are tested without waiting;
//! `wake` rouses the fetching task at once, since its own look is only every `LOOK`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How often a healthy workspace in front fetches.
pub const EVERY: u64 = 5 * 60;
/// How seldom one that failed tries again.
pub const BACKOFF: u64 = 30 * 60;
/// How long a page's word that it is in front holds without being repeated: the page repeats it with each state poll.
pub const FRONT_FOR: u64 = 2 * EVERY;

/// What the last fetch came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub at: u64,
    pub error: Option<(String, String)>,
}

#[derive(Default)]
pub struct Cadence {
    front: bool,
    said_at: u64,
    wanted: bool,
    last_try: Option<u64>,
    failing: bool,
    pub last: Option<Outcome>,
    /// Notified when a fetch becomes wanted, so a user opening a workspace does not wait out `LOOK`.
    pub wake: Arc<tokio::sync::Notify>,
}

impl Cadence {
    /// The workspace was opened: fetch as soon as it is in front.
    pub fn opened(&mut self) {
        self.wanted = true;
        self.wake.notify_one();
    }

    /// The page reports at `now` that it is in front or away; coming back to the front asks for a fetch.
    pub fn focus(&mut self, front: bool, now: u64) {
        if front && !self.in_front(now) {
            self.wanted = true;
            self.wake.notify_one();
        }
        self.front = front;
        self.said_at = now;
    }

    /// In front only while the page keeps saying so: a closed tab or a killed web view sends nothing.
    fn in_front(&self, now: u64) -> bool {
        self.front && now.saturating_sub(self.said_at) < FRONT_FOR
    }

    /// Whether a fetch is due at `now` seconds: never while away (a phone has no background slot).
    pub fn due(&self, now: u64) -> bool {
        if !self.in_front(now) {
            return false;
        }
        if self.wanted && !self.failing {
            return true;
        }
        let gap = if self.failing { BACKOFF } else { EVERY };
        self.last_try.is_none_or(|t| now.saturating_sub(t) >= gap)
    }

    /// A fetch ended; the sentence to say when it is the first failure of a run, none when it repeats one.
    pub fn done(&mut self, now: u64, result: Result<(), (String, String)>) -> Option<String> {
        self.wanted = false;
        self.last_try = Some(now);
        let said = match &result {
            Err((_, m)) if !self.failing => Some(m.clone()),
            _ => None,
        };
        self.failing = result.is_err();
        self.last = Some(Outcome {
            at: now,
            error: result.err(),
        });
        said
    }
}

/// The cadence of each workspace the page has told us about, by root.
#[derive(Default)]
pub struct Fetchers {
    by_root: Mutex<HashMap<String, Arc<Mutex<Cadence>>>>,
}

impl Fetchers {
    pub fn of(&self, root: &str) -> Arc<Mutex<Cadence>> {
        self.by_root
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(root.to_string())
            .or_default()
            .clone()
    }
}

/// Seconds since the epoch, the cadence's clock.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// How often a running task looks at its cadence.
pub const LOOK: Duration = Duration::from_secs(20);

#[cfg(test)]
mod tests {
    use super::*;

    fn err() -> Result<(), (String, String)> {
        Err(("unreachable".into(), "no network".into()))
    }

    #[test]
    fn a_workspace_fetches_when_it_opens_then_every_five_minutes_while_in_front() {
        let mut c = Cadence::default();
        c.focus(true, 0);
        c.opened();
        assert!(c.due(0));
        assert_eq!(c.done(0, Ok(())), None);
        assert!(!c.due(EVERY - 1));
        assert!(c.due(EVERY));
    }

    #[test]
    fn nothing_is_fetched_while_the_page_is_away_and_a_return_fetches_at_once() {
        let mut c = Cadence::default();
        c.focus(true, 0);
        c.done(0, Ok(()));
        c.focus(false, 1);
        assert!(!c.due(10 * EVERY), "no fetch in the background");
        c.focus(true, 59);
        assert!(c.due(60), "coming back asks for one");
        c.done(60, Ok(()));
        assert!(!c.due(61));
    }

    #[test]
    fn a_failure_backs_off_to_thirty_minutes_and_is_said_once() {
        let mut c = Cadence::default();
        c.focus(true, 0);
        assert_eq!(c.done(0, err()), Some("no network".into()));
        c.focus(true, EVERY);
        assert!(!c.due(EVERY));
        c.focus(true, BACKOFF - 1);
        assert!(!c.due(BACKOFF - 1));
        c.focus(true, BACKOFF);
        assert!(c.due(BACKOFF));
        assert_eq!(
            c.done(BACKOFF, err()),
            None,
            "the same failure is not said again"
        );
        assert_eq!(c.done(2 * BACKOFF, Ok(())), None);
        assert_eq!(
            c.done(3 * BACKOFF, err()),
            Some("no network".into()),
            "a new run of failures is said"
        );
        assert_eq!(
            c.last.as_ref().and_then(|o| o.error.clone()),
            Some(("unreachable".into(), "no network".into()))
        );
    }

    #[test]
    fn a_page_that_says_nothing_more_is_not_in_front_for_ever() {
        let mut c = Cadence::default();
        c.focus(true, 0);
        c.done(0, Ok(()));
        assert!(c.due(EVERY), "a page that spoke a period ago is in front");
        assert!(
            !c.due(3 * EVERY),
            "silent for two periods: no periodic fetch"
        );
        c.focus(true, 3 * EVERY);
        assert!(c.due(3 * EVERY), "the page is back and asks for one");
    }

    #[test]
    fn a_page_that_repeats_it_stays_in_front() {
        let mut c = Cadence::default();
        c.focus(true, 0);
        c.done(0, Ok(()));
        for t in 1..=6 {
            c.focus(true, t * EVERY);
            assert!(c.due(t * EVERY));
            c.done(t * EVERY, Ok(()));
        }
    }

    #[test]
    fn a_return_to_the_front_does_not_override_the_backoff() {
        let mut c = Cadence::default();
        c.focus(true, 0);
        c.done(0, err());
        c.focus(false, 1);
        c.focus(true, 59);
        assert!(!c.due(60), "a failing host is not asked at every return");
    }

    #[tokio::test]
    async fn opening_a_workspace_wakes_the_fetching_task_at_once() {
        let mut c = Cadence::default();
        c.opened();
        let wake = c.wake.clone();
        tokio::time::timeout(Duration::from_secs(1), wake.notified())
            .await
            .expect("an opened workspace does not wait out the look");
    }
}

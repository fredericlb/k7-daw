//! The link's counters (2262): what the front logs every 30 s while it moves, and what a run that loaded nothing must not hide.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct Stats {
    pub loads: AtomicU64,
    pub dry: AtomicU64,
    pub resends: AtomicU64,
    pub redials: AtomicU64,
    pub copies: AtomicU64,
    pub polls: AtomicU64,
    pub panics: AtomicU64,
    /// Documents prepared for sending: one per document, which a test counts (the line leaves it out).
    pub prepares: AtomicU64,
}

impl Stats {
    pub fn bump(c: &AtomicU64) {
        c.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get(c: &AtomicU64) -> u64 {
        c.load(Ordering::Relaxed)
    }

    /// The line to log when the counters moved since `said` and the link did something; `said` then holds it.
    pub fn report(&self, account: &str, said: &mut String) -> Option<String> {
        let line = self.line(account);
        if line == *said || Stats::get(&self.loads) + Stats::get(&self.polls) == 0 {
            return None;
        }
        said.clone_from(&line);
        Some(line)
    }

    /// The line the front logs, which `bedCounters.mjs` reads: the account is in it, since every account's link logs one.
    pub fn line(&self, account: &str) -> String {
        let g = Stats::get;
        format!(
            "[files] native engine {}: loads={} dry={} resends={} redials={} copies={} polls={} panics={}",
            account,
            g(&self.loads),
            g(&self.dry),
            g(&self.resends),
            g(&self.redials),
            g(&self.copies),
            g(&self.polls),
            g(&self.panics)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_reported_once_the_link_moved_and_again_only_when_it_moves_on() {
        let st = Stats::default();
        let mut said = String::new();
        assert_eq!(st.report("a@b.c", &mut said), None);
        Stats::bump(&st.loads);
        let first = st.report("a@b.c", &mut said).expect("a load is a movement");
        assert!(
            first.starts_with("[files] native engine a@b.c: loads=1 "),
            "{first}"
        );
        assert_eq!(st.report("a@b.c", &mut said), None);
        Stats::bump(&st.polls);
        assert!(
            st.report("a@b.c", &mut said)
                .is_some_and(|l| l.contains("polls=1 "))
        );
    }
}

//! stop.rs — an account's stop: its members one after the other under `runtime::stop::Stop`'s plan, awaited (2670 to 2672).
//! What a reader gets wrong: the plan decides and this drives it; a member that hangs is left running and named.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::routes::files::service::native::callbacks::BoxFuture;
use k7_file_service::runtime::stop::{Step, Stop, leaving_line};

use super::super::super::services::Held;

/// What one member of an account may take in the process' stop: three stalls fit in 5 s.
pub const MEMBER: Duration = Duration::from_millis(1500);

/// A member's stop, run once.
pub type Member = Box<dyn FnOnce() -> BoxFuture<()> + Send>;

/// What the pieces register to be stopped with their account, in the order they must stop (2671).
#[derive(Default)]
pub struct Members(Mutex<Vec<(String, Member)>>);

impl Members {
    pub fn add(&self, name: &str, stop: Member) {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((name.to_string(), stop));
    }

    fn take(&self) -> Vec<(String, Member)> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|p| p.into_inner()))
    }
}

/// Runs `members` in order under the plan: `each` bounds one, `total` the whole; the plan, for what is left.
pub async fn drive(
    members: Vec<(String, Member)>,
    each: Option<Duration>,
    total: Option<Duration>,
) -> Stop {
    let names: Vec<&str> = members.iter().map(|(n, _)| n.as_str()).collect();
    let mut plan = Stop::new(&names, each, total, Instant::now());
    let mut fns: Vec<(String, Option<Member>)> =
        members.into_iter().map(|(n, f)| (n, Some(f))).collect();
    let mut cur: Option<(String, tokio::task::JoinHandle<()>)> = None;
    loop {
        match plan.step(Instant::now()) {
            Step::Done => break,
            Step::Run(name) => {
                let f = fns
                    .iter_mut()
                    .find(|(n, _)| *n == name)
                    .and_then(|(_, f)| f.take());
                match f {
                    Some(f) => cur = Some((name, tokio::spawn(f()))),
                    None => {
                        plan.finished(&name);
                    }
                }
            }
            Step::Wait(at) => {
                let Some((name, handle)) = cur.as_mut() else {
                    break;
                };
                tokio::select! {
                    _ = handle => {
                        let name = name.clone();
                        plan.finished(&name);
                        cur = None;
                    }
                    _ = tokio::time::sleep_until(at.into()) => {}
                }
            }
        }
    }
    plan
}

impl Held {
    /// The account stopped in the order of 2671: its registered members, then its capture; the ring last.
    pub async fn stop(&self, each: Option<Duration>, total: Option<Duration>) -> Stop {
        if let Some(m) = &self.media {
            m.prefetch.stop();
        }
        if let Some(e) = &self.engine {
            e.link.close();
        }
        let mut members = self.members.take();
        if self.native_capture {
            let env = self.account.env.clone();
            members.push((
                "captures".to_string(),
                Box::new(move || {
                    Box::pin(async move { super::super::capture_start::stop_waited(&env).await })
                }),
            ));
        }
        let plan = drive(members, each, total).await;
        if let Some(r) = &self.runtime {
            r.dropped();
        }
        plan
    }
}

/// Every account stopped at once, bounded in all; the one line naming what had not stopped, if any (2670).
pub async fn stop_all(helds: Vec<Arc<Held>>, total: Duration) -> Option<String> {
    let plans =
        futures_util::future::join_all(helds.iter().map(|h| h.stop(Some(MEMBER), Some(total))))
            .await;
    let named: Vec<(&str, &Stop)> = helds
        .iter()
        .zip(&plans)
        .map(|(h, p)| (h.account.slug.as_str(), p))
        .collect();
    leaving_line(&named)
}

/// The reaped accounts' stops still running, awaited up to `total`; the line naming how many had not finished.
pub async fn await_reaped(
    mut reaped: Vec<tokio::task::JoinHandle<()>>,
    total: Duration,
) -> Option<String> {
    let all = futures_util::future::join_all(reaped.iter_mut());
    let _ = tokio::time::timeout(total, all).await;
    let left = reaped.iter().filter(|h| !h.is_finished()).count();
    (left > 0).then(|| format!("stop: {left} reaped account(s) had not finished stopping"))
}

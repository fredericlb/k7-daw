//! drive.rs — the driver a running front answers the host members with: the members this build ports, 503 `off` for the rest.
//! What a reader gets wrong: a member's driver is the account's own `Jobs`, so a front with `engine-jobs` off serves none.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::response::Response;
use k7_file_service::verb_host::Error;
use k7_file_service::verb_host::gate::remove_out;
use k7_file_service::verb_host::jobs::{Cancel, Table};

use super::{
    Answer, Checked, Driver, Member, cut, failure, media, now, picture, prepare, render, stems,
    take, transcribe,
};
use crate::routes::files::service::native::jobs::{Gone, Jobs};
use crate::routes::files::service::native::runtime::Runtime;
use crate::routes::files::service::native::shares::picture::{Browser, Chromium};

/// What the front lends a member: the account's jobs.
pub struct Production {
    pub jobs: Arc<Jobs>,
    /// The account's background registry where `runtime` answers: a prepare is listed in it.
    pub runtime: Option<Arc<Runtime>>,
    /// The browser a picture is rendered in; a test names a fake.
    pub browser: Arc<dyn Browser>,
}

/// The driver of one request, or none when the account's jobs are off.
#[cfg(test)]
pub fn production(jobs: Option<Arc<Jobs>>) -> Option<Arc<dyn Driver>> {
    production_in(jobs, None)
}

/// `production` for an account whose runtime lists the background work.
pub fn production_in(
    jobs: Option<Arc<Jobs>>,
    runtime: Option<Arc<Runtime>>,
) -> Option<Arc<dyn Driver>> {
    production_with(jobs, runtime, Arc::new(Chromium))
}

/// `production_in` with the browser a picture is rendered in.
pub fn production_with(
    jobs: Option<Arc<Jobs>>,
    runtime: Option<Arc<Runtime>>,
    browser: Arc<dyn Browser>,
) -> Option<Arc<dyn Driver>> {
    jobs.map(|jobs| {
        Arc::new(Production {
            jobs,
            runtime,
            browser,
        }) as Arc<dyn Driver>
    })
}

impl Driver for Production {
    fn call<'a>(&'a self, req: &'a Checked) -> Answer<'a> {
        Box::pin(async move {
            match req.member {
                Member::Render => render::render(req, &self.jobs).await,
                Member::Peak => render::peak(req).await,
                Member::Separate => cut::separate(req, &self.jobs).await,
                Member::Probe => media::probe(req).await,
                Member::Extract => media::extract(req).await,
                Member::TakeSave => take::save(req).await,
                Member::Prepare => prepare::prepare(req, self.runtime.clone()).await,
                Member::Transcribe => transcribe::transcribe(req).await,
                Member::Picture => {
                    picture::picture(req, &self.jobs, self.runtime.clone(), self.browser.clone())
                        .await
                }
                Member::PlaylistStems => stems::playlist(req, &self.jobs).await,
                _ => failure(&Error::coded(
                    503,
                    "this host member is not served yet",
                    "off",
                )),
            }
        })
    }
}

/// Resolves once the job's page stopped polling it, which is when its work is dropped.
pub fn gone(cancel: Cancel) -> Gone {
    Box::pin(async move {
        while !cancel.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
}

/// Ends a job with the sentence of why; a job already stopped keeps its stop.
pub fn fail(job: &str, error: &str, code: Option<&str>) {
    Table::global().fail(job, error, code, now());
}

/// A job's driver: holds the job's place until dropped, and a job that ended without its value leaves no files.
pub struct Run {
    pub id: String,
    pub cancel: Cancel,
    studio: String,
}

impl Run {
    pub fn new(id: String, cancel: Cancel, studio: String) -> Run {
        Run { id, cancel, studio }
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let done = Table::global().done(&self.id);
        Table::global().release(&self.id, now());
        if !done {
            remove_out(&self.studio, &self.id);
        }
    }
}

/// The 503 of a member that needs a machine and has none.
pub fn no_machine(why: &str) -> Response {
    failure(&Error::coded(503, why, "no-machine"))
}

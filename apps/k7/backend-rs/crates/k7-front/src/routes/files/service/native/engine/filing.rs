//! filing.rs — a finished take filed natively (3j O1): staged through song-edit, landed whole under the account's write lock.
//! What a reader gets wrong: no journal step (a take is not undoable), so it takes the gate alone to keep every record out of its way.

use k7_file_service::edits;
use k7_file_service::edits::take::Meter;
use k7_file_service::engine::docs::TakeArm;
use k7_file_service::engine::wire::Take;
use k7_file_service::journal::Journals;
use k7_file_service::libraries::Home;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use serde_json::Value;

use super::super::entries::told;
use super::super::locks;

/// `fileStamp`: the local time as `YYMMDDTHHMMSS`, the same in a take's name and a bounce's.
pub fn file_stamp() -> String {
    chrono::Local::now().format("%y%m%dT%H%M%S").to_string()
}

/// The project's tempo and metre as its compile says them.
pub fn meter_of(doc: &Value) -> Result<Meter, String> {
    let num = |k: &str| {
        doc.get(k)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("the compiled project has no {k}"))
    };
    Ok(Meter {
        tempo: num("tempo")?,
        num: num("timeSigNumerator")?,
        den: num("timeSigDenominator")?,
    })
}

/// What a filing needs of its account.
pub struct Filing<'a> {
    pub home: &'a Home,
    pub rt: &'a tokio::runtime::Handle,
    pub root: &'a str,
    pub project: &'a str,
}

/// The take filed as one landing under the account's write lock; the clip's id.
pub fn file_take(
    f: &Filing<'_>,
    take: &Take,
    arm: Option<&TakeArm>,
    meter: Meter,
    stamp: &str,
) -> Result<String, String> {
    // The journal's gate alone, as an undo takes it, then the lock: no record is open while the take lands, so none absorbs it.
    let gate = k7_file_service::paths::resolve_all(&[f.root]);
    let _gate = f.rt.block_on(Journals::global().exclusive(&gate));
    let _lock = f.rt.block_on(locks::write(&f.home.studio));
    let (id, landed) = edits::take::file_take(f.project, take, arm, meter, stamp)?;
    if !landed.touched.is_empty() {
        Revisions::global().bump(Space::open(f.home, f.root).root());
    }
    f.rt.block_on(told(f.root, "after"));
    Ok(id)
}

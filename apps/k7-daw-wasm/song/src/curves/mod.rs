// curves/mod.rs — the curve files of a project, read through the Source; the budget every bake spends from.
// A reader would get wrong that a broken curve file is a WARNING and not a failure: that curve is absent, its siblings read.
// The bake is in bake.rs and baker.rs, and compiles nothing by itself (docs/rustCompilerCurves.md).

mod bake;
mod baker;
mod conditions;
mod file;
mod resolved;

pub use bake::{BakedCurve, bake_adsr, bake_curve, cut_points};
pub use baker::{CurveBaker, MidiWindow};
pub use conditions::{CurveRun, EnvelopeFire, curve_runs, envelope_fires, is_note_on_event};
pub use file::decode_curve;
pub use file::{CurveCondition, CurveFile};
pub use resolved::{Condition, ResolvedCurve};

use crate::cache::Cache;
use crate::files::{Finding, Severity};
use crate::read::read_family;
use crate::source::{Source, SourceError};
use indexmap::IndexMap;
use std::sync::Arc;

/// What one `CurveBaker` may write, laid and visited together (decision 211).
pub const BUDGET: usize = 12_000_000;

/// The weight a bake may still spend, and the weight it has spent.
#[derive(Clone, Debug)]
pub struct Budget {
    left: usize,
    spent: usize,
}

/// The budget ran out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Over;

impl Budget {
    pub fn new(limit: usize) -> Budget {
        Budget { left: limit, spent: 0 }
    }

    /// Weighs `n` more output; past the limit it answers `Over` and nothing is left.
    pub fn spend(&mut self, n: usize) -> Result<(), Over> {
        if n > self.left {
            self.left = 0;
            return Err(Over);
        }
        self.left -= n;
        self.spent = self.spent.saturating_add(n);
        Ok(())
    }

    /// The weight spent so far.
    pub fn spent(&self) -> usize {
        self.spent
    }
}

/// The curves of a project by id, and the warnings of the files that did not read.
pub struct CurvesRead {
    pub curves: IndexMap<String, Arc<CurveFile>>,
    pub findings: Vec<Finding>,
}

const CURVE_REF_PREFIX: &str = "@meta/curves/";

/// The bare curve id an authored `curve:` field names, in whichever form it was written.
pub fn curve_id_from_ref(r: &str) -> &str {
    r.strip_prefix(CURVE_REF_PREFIX).unwrap_or(r)
}

/// A warning of the curves folder or of one file in it, at 1:1.
fn warning(file: String, message: &str) -> Finding {
    Finding {
        file,
        line: 1,
        col: 1,
        message: message.to_owned(),
        severity: Severity::Warning,
    }
}

/// Every `curves/<id>.k7x`; a file that does not decode is a warning and that curve is absent (decisions 204, 205, 206).
pub fn read_curves(source: &dyn Source, cache: &mut Cache, project: &str) -> Result<CurvesRead, SourceError> {
    match read_family::<CurveFile>(source, cache, project) {
        Ok(fam) => Ok(CurvesRead {
            curves: fam.values.into_iter().collect(),
            findings: fam
                .findings
                .into_iter()
                .map(|f| Finding {
                    severity: Severity::Warning,
                    ..f
                })
                .collect(),
        }),
        Err(SourceError::Outside) => Ok(CurvesRead {
            curves: IndexMap::new(),
            findings: vec![warning(
                "curves".to_owned(),
                "the curves folder lies outside what may be read, so no curve was read",
            )],
        }),
        Err(e) => Err(e),
    }
}

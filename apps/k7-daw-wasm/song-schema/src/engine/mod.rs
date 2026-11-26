// engine — the page's rules about an engine, as pure functions: how good a link is, how well an engine fits a project, which one to take, what a warning says.
// A reader would get wrong that nothing here measures or fetches: the page measures and asks, these only decide.

mod choose;
mod compat;
mod quality;
mod warnings;

pub use choose::{Candidate, Choice, Latency, best_engine, choose_engine};
pub use compat::{CataloguePlugin, Compatibility, PluginNeed, compatibility};
pub use quality::{EngineLatency, Quality, audio_latency_ms, engine_latency, quality_of};
pub use warnings::{Duplicated, MissingItem, WarningKind, Why, missing_of, warning_kind};

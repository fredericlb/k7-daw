//! wire.rs — what this supervisor says it speaks on the engine wire: `ENGINE_PROTOCOL`, mirrored in
//! `apps/k7-daw-frontend/packages/engine-protocol/version.ts` and kept equal by that package's test, and the
//! build stamp it was made from (`build.rs`). An engine of another protocol or build is not ours to adopt.

/// The engine wire's version; what bumps it is `apps/k7-daw-frontend/packages/engine-protocol/docs/version.md`.
pub const ENGINE_PROTOCOL: i64 = 1;

/// The root package.json's version, as k7-runtime's copy states it.
pub fn build() -> &'static str {
    env!("K7_BUILD_STAMP")
}

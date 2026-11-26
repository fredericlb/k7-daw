//! version.rs — what this engine says it speaks: `ENGINE_PROTOCOL`, held to engine-protocol's `version.ts` by
//! `tests/all/catalogue.rs`, and the version it was built from (`build.rs`).

/// The engine wire's version; what bumps it is `apps/k7-daw-frontend/packages/engine-protocol/docs/version.md`.
pub const ENGINE_PROTOCOL: u32 = 1;

/// The root package.json's version, or `$K7_BUILD` at build time.
pub fn build() -> &'static str {
    env!("K7_BUILD_STAMP")
}

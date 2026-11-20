//! programs.rs — whether this host may start a program, which an iOS app may not (apps/k7-daw-engine/k7-runtime/docs/ios.md).
//! What a reader gets wrong: the simulator could start one, and answers as a phone does, so it never promises what a phone refuses.

/// True where no program can be started: the members that need one answer as a machine without it.
pub const NONE: bool = cfg!(target_os = "ios");

/// The reason a member gives on such a host, after what it could not do.
pub const WHY: &str = "an iOS app cannot start a program";

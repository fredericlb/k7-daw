// One test binary for the whole crate: every test file is a module here, run as `cargo test -p k7-capture --test all <name>::`.
// The one thing to get wrong: a count of tests is the same on every host, so a case that cannot run returns early.

mod history_device;
mod history_fixes;
mod history_landing;
mod rig;
mod support;

mod account;
mod account_attach;
mod account_folders;
mod capture;
mod capture_blobs;
mod capture_hostile;
mod capture_start;
mod capture_steps;
mod disk;
mod registry_stops;
mod settler_scale;
mod vectors;
mod worker;
mod worker_ends;
mod worker_load;
mod worker_watch;
mod worker_window;

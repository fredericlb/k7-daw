//! auth — `/auth/*` as a device needs it: the account's `me` and `logout`, and `/auth/local`, the one session a
//! machine has. There is no sign-in against an identity provider here; the choices: `apps/k7/backend-rs/docs/routes.md`.

pub mod account;
pub mod ctx;
pub mod express;
pub mod local;
pub mod target;
mod warm;

//! k7-auth — what makes a studio session or token: the session cookie, the shared JSON files, and the API tokens.
//! These formats are frozen by vectors written once (`apps/k7/backend-rs/fixtures`, `apps/k7/backend-rs/docs/sessions.md`).

pub mod api_token;
mod cookie_session;
mod json_file;
mod keygrip;
mod revocations;
mod session;

pub use cookie_session::{CookieOptions, CookieSession};
pub use json_file::{SharedJsonFile, js_number};
pub use keygrip::{sign, verify};
pub use revocations::Revocations;
pub use session::{
    GROUPS_MAX_AGE_MS, SESSION_MAX_AGE_MS, Session, cookie_header, open, parse_cookie_header, seal,
    valid_session_user,
};

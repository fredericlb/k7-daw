// error.rs — what a song-edit call fails with: the editor's own error, or a sentence the TypeScript threw as an Error.
// A reader would get wrong that `Display` is the whole contract: a vector compares the TypeScript's `message` as text.

use k7x::SyntaxError;
use k7x::edit::EditError;
use song::SourceError;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Edit(EditError),
    Msg(String),
}

pub type Res<T> = Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Edit(e) => e.fmt(f),
            Error::Msg(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

impl From<EditError> for Error {
    fn from(e: EditError) -> Error {
        Error::Edit(e)
    }
}

impl From<SyntaxError> for Error {
    fn from(e: SyntaxError) -> Error {
        Error::Edit(EditError::Syntax(e))
    }
}

/// A failure with a sentence, as `throw new Error(message)`.
pub fn fail<T>(message: impl Into<String>) -> Res<T> {
    Err(Error::Msg(message.into()))
}

/// Node's message for a failed `syscall` on `path`.
pub fn io_error(e: &SourceError, syscall: &str, path: &str) -> Error {
    let what = match e {
        SourceError::NotFound | SourceError::Missing => "ENOENT: no such file or directory".to_owned(),
        SourceError::Outside => "EACCES: permission denied".to_owned(),
        SourceError::Io(m) => return Error::Msg(m.clone()),
    };
    Error::Msg(format!("{what}, {syscall} '{path}'"))
}

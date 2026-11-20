// error.rs — what refuses a timecode or a curve; the facade maps each variant to one error code.
// A reader would get wrong that BadTimecode's text is shown to the user as is: it is timecode.ts' message, byte for byte.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaError {
    BadTimecode(String),
    BadCurve(String),
}

impl SchemaError {
    /// The message a person reads.
    pub fn message(&self) -> &str {
        match self {
            SchemaError::BadTimecode(m) | SchemaError::BadCurve(m) => m,
        }
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for SchemaError {}

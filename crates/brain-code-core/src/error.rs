//! The error vocabulary of the code-index substrate. Every failure names
//! itself: a count that cannot be derived is not written, a refusal carries
//! its reason, an ingest that fails says so loudly.

use std::fmt;

#[derive(Debug)]
pub enum CodeIndexError {
    Sql(rusqlite::Error),
    Io(std::io::Error),
    /// A named refusal — profile change on re-ingest, unreadable `.git`,
    /// dimension mismatch. The string is the operator-facing reason.
    Refusal(String),
}

impl fmt::Display for CodeIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodeIndexError::Sql(e) => write!(f, "code index sql error: {e}"),
            CodeIndexError::Io(e) => write!(f, "code index io error: {e}"),
            CodeIndexError::Refusal(reason) => write!(f, "code index refused: {reason}"),
        }
    }
}

impl std::error::Error for CodeIndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CodeIndexError::Sql(e) => Some(e),
            CodeIndexError::Io(e) => Some(e),
            CodeIndexError::Refusal(_) => None,
        }
    }
}

impl From<rusqlite::Error> for CodeIndexError {
    fn from(e: rusqlite::Error) -> Self {
        CodeIndexError::Sql(e)
    }
}

impl From<std::io::Error> for CodeIndexError {
    fn from(e: std::io::Error) -> Self {
        CodeIndexError::Io(e)
    }
}

//! The error type.

use std::fmt;

/// Why a value cannot be written.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Error {
    /// A number the file cannot hold or the entity forbids: NaN, an
    /// infinity, or a value out of the entity's range.
    InvalidNumber {
        /// Which input the number was given for.
        what: &'static str,
        value: f64,
    },
    /// A HEADER string longer than the 256 characters Part 21 allows.
    HeaderTooLong {
        /// The [`Header`](crate::Header) field.
        field: &'static str,
        /// Its length in characters.
        chars: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNumber { what, value } => write!(f, "invalid {what}: {value}"),
            Self::HeaderTooLong { field, chars } => write!(
                f,
                "header {field} is {chars} characters long; Part 21 allows 256"
            ),
        }
    }
}

impl std::error::Error for Error {}

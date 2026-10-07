//! The error type.

use std::fmt;

/// Why a [`StepWriter`](crate::StepWriter) call failed.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Error {
    /// A number that cannot be written: NaN, an infinity, or a value outside
    /// the range its entity allows.
    InvalidNumber {
        /// Which input the number was given for.
        what: &'static str,
        /// The number itself.
        value: f64,
    },
    /// A direction of length zero.
    ZeroVector {
        /// Which input the direction was given for.
        what: &'static str,
    },
    /// A frame whose axis and reference direction are parallel.
    ParallelAxes {
        /// Which input the frame was given for.
        what: &'static str,
    },
    /// A straight edge whose two vertices are at the same point.
    ZeroLengthLine,
    /// A list with fewer entries than it needs.
    Empty {
        /// Which list.
        what: &'static str,
    },
    /// A NURBS curve or surface that breaks one of STEP's B-spline rules.
    InvalidNurbs {
        /// The rule it breaks.
        reason: &'static str,
    },
    /// A face with more than one outer bound.
    MultipleOuterBounds,
    /// A handle made by a different [`StepWriter`](crate::StepWriter).
    ForeignHandle,
    /// A part with no solid in it.
    EmptyPart {
        /// The part's name.
        name: String,
    },
    /// A HEADER string longer than the 256 characters Part 21 allows.
    HeaderTooLong {
        /// The [`Header`](crate::Header) field that is too long.
        field: &'static str,
        /// Its length in characters.
        chars: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNumber { what, value } => write!(f, "invalid {what}: {value}"),
            Self::ZeroVector { what } => write!(f, "zero direction in {what}"),
            Self::ParallelAxes { what } => {
                write!(f, "axis and reference direction are parallel in {what}")
            }
            Self::ZeroLengthLine => {
                f.write_str("straight edge with both vertices at the same point")
            }
            Self::Empty { what } => write!(f, "too few {what}"),
            Self::InvalidNurbs { reason } => write!(f, "invalid NURBS: {reason}"),
            Self::MultipleOuterBounds => f.write_str("face with more than one outer bound"),
            Self::ForeignHandle => f.write_str("handle from a different writer"),
            Self::EmptyPart { name } => write!(f, "part {name:?} has no solid"),
            Self::HeaderTooLong { field, chars } => write!(
                f,
                "header {field} is {chars} characters long; Part 21 allows 256"
            ),
        }
    }
}

impl std::error::Error for Error {}

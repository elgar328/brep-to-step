//! [`StepWriter`], the entry point.

use crate::context::{self, Skeleton, Units};
use crate::error::Error;
use crate::header::{self, Header};
use crate::p21::Data;

/// Writes one STEP AP242 file.
///
/// [`new`](Self::new) lays down the skeleton every file carries;
/// [`finish`](Self::finish) adds the header and returns the file's text.
#[derive(Debug)]
pub struct StepWriter {
    data: Data,
    #[expect(dead_code, reason = "read by the part chain (step 3)")]
    skeleton: Skeleton,
}

impl StepWriter {
    /// Start a file in the given units.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidNumber`] if `units.uncertainty` is not finite and
    /// positive.
    pub fn new(units: Units) -> Result<Self, Error> {
        let mut data = Data::new();
        let skeleton = context::write_skeleton(&mut data, units)?;
        Ok(Self { data, skeleton })
    }

    /// Finish the file and return its Part 21 text.
    ///
    /// # Errors
    ///
    /// [`Error::HeaderTooLong`] if a `header` string is longer than Part 21
    /// allows.
    pub fn finish(self, header: &Header) -> Result<String, Error> {
        header::write_file(header, &self.data.into_body())
    }
}

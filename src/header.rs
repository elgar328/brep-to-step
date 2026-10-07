//! The Part 21 HEADER section and the file envelope.

use crate::ap;
use crate::error::Error;
use crate::p21::write_str;

/// The HEADER section's free-text fields. Every field may be left empty.
///
/// Each string, and each entry of `authors` and `organizations`, may be at
/// most 256 characters long — the limit Part 21 sets on header strings.
#[derive(Debug, Clone, Default)]
pub struct Header {
    /// `FILE_NAME.name` — customarily the file's own name.
    pub file_name: String,
    /// `FILE_DESCRIPTION.description`.
    pub description: String,
    /// `FILE_NAME.time_stamp`, written as given — customarily ISO 8601
    /// (`2026-10-07T12:00:00`). The writer never reads a clock, so the same
    /// input always gives the same file.
    pub timestamp: String,
    /// `FILE_NAME.author`.
    pub authors: Vec<String>,
    /// `FILE_NAME.organization`.
    pub organizations: Vec<String>,
    /// `FILE_NAME.originating_system` — the application or kernel exporting.
    pub originating_system: String,
    /// `FILE_NAME.authorization`.
    pub authorisation: String,
}

/// `FILE_NAME.preprocessor_version`: this crate.
const PREPROCESSOR: &str = concat!("brep-to-step ", env!("CARGO_PKG_VERSION"));

/// Part 21 types every header string as `STRING(256)`.
const MAX_CHARS: usize = 256;

/// The whole file: the HEADER section built from `header`, then `data_body`
/// as the DATA section.
pub(crate) fn write_file(header: &Header, data_body: &str) -> Result<String, Error> {
    check_length("file_name", &header.file_name)?;
    check_length("description", &header.description)?;
    check_length("timestamp", &header.timestamp)?;
    for author in &header.authors {
        check_length("authors", author)?;
    }
    for organization in &header.organizations {
        check_length("organizations", organization)?;
    }
    check_length("originating_system", &header.originating_system)?;
    check_length("authorisation", &header.authorisation)?;

    let mut out = String::with_capacity(data_body.len() + 1024);
    out.push_str("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((");
    write_str(&mut out, &header.description);
    out.push_str("),");
    write_str(&mut out, "2;1");
    out.push_str(");\nFILE_NAME(");
    write_str(&mut out, &header.file_name);
    out.push(',');
    write_str(&mut out, &header.timestamp);
    out.push(',');
    write_list(&mut out, &header.authors);
    out.push(',');
    write_list(&mut out, &header.organizations);
    out.push(',');
    write_str(&mut out, PREPROCESSOR);
    out.push(',');
    write_str(&mut out, &header.originating_system);
    out.push(',');
    write_str(&mut out, &header.authorisation);
    out.push_str(");\nFILE_SCHEMA((");
    write_str(&mut out, ap::FILE_SCHEMA);
    out.push_str("));\nENDSEC;\nDATA;\n");
    out.push_str(data_body);
    out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Ok(out)
}

fn check_length(field: &'static str, s: &str) -> Result<(), Error> {
    let chars = s.chars().count();
    if chars > MAX_CHARS {
        return Err(Error::HeaderTooLong { field, chars });
    }
    Ok(())
}

/// A header string list; an empty list is written as the customary `('')`,
/// since Part 21 requires at least one entry.
fn write_list(out: &mut String, items: &[String]) {
    out.push('(');
    if items.is_empty() {
        write_str(out, "");
    }
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_str(out, item);
    }
    out.push(')');
}

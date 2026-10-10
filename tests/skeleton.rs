//! The file skeleton — contexts, units, header — with no parts: against
//! step-io's, read back strictly, and the input checks.

mod common;

use brep_to_step::{Error, Header, LengthUnit, StepWriter, Units};
use common::compare::{assert_same_structure, compare, decode, header_attributes};
use step_io::StepBuilder;
use step_io::build::{HeaderInput, LengthUnit as StepIoLength, UnitsInput};
use step_io::parser::{Attribute, parse};

const STAMP: &str = "2026-10-07T12:00:00";

fn header() -> Header {
    Header {
        timestamp: STAMP.to_owned(),
        ..Default::default()
    }
}

fn ours(units: Units) -> String {
    StepWriter::new(Vec::new(), &header(), units)
        .expect("writer")
        .finish_to_string()
        .expect("finish")
}

fn step_io(units: Units) -> String {
    let mut b = StepBuilder::new_with(&UnitsInput {
        length: match units.length {
            LengthUnit::Millimetre => StepIoLength::Millimetre,
            LengthUnit::Metre => StepIoLength::Metre,
            other => panic!("no step-io unit for {other:?}"),
        },
        uncertainty: units.uncertainty,
    })
    .expect("builder");
    b.header(&HeaderInput {
        timestamp: Some(STAMP.to_owned()),
        ..Default::default()
    });
    b.finish().expect("finish")
}

fn unit_choices() -> [Units; 3] {
    [
        Units::default(),
        Units {
            length: LengthUnit::Metre,
            uncertainty: 1e-6,
        },
        Units {
            length: LengthUnit::Millimetre,
            uncertainty: 0.01,
        },
    ]
}

#[test]
fn skeleton_matches_step_io() {
    for units in unit_choices() {
        assert_same_structure(&ours(units), &step_io(units));
    }
}

/// The comparison must fail on a real difference — and report only that.
#[test]
fn comparison_reports_a_difference() {
    let tighter = Units {
        uncertainty: 1e-6,
        ..Units::default()
    };
    let report = compare(&ours(tighter), &step_io(Units::default()))
        .expect_err("different uncertainties must not compare equal");
    assert!(report.contains("UNCERTAINTY_MEASURE_WITH_UNIT"), "{report}");
    assert!(
        !report.contains("APPLICATION_PROTOCOL_DEFINITION"),
        "{report}"
    );
    assert!(!report.contains("known difference"), "{report}");
}

#[test]
fn reads_back_cleanly() {
    for units in unit_choices() {
        let text = ours(units);
        let (_, report) = step_io::read(text.as_bytes()).expect("read");
        assert!(report.dropped.is_empty(), "dropped: {:?}", report.dropped);
        assert!(report.norm.is_empty(), "normalized: {:?}", report.norm);
    }
}

/// A header attribute's string, decoded.
fn text(a: &Attribute) -> String {
    match a {
        Attribute::String(s) => decode(s),
        other => panic!("not a string: {other:?}"),
    }
}

/// A header attribute's string list, decoded.
fn texts(a: &Attribute) -> Vec<String> {
    match a {
        Attribute::List(items) => items.iter().map(text).collect(),
        other => panic!("not a list: {other:?}"),
    }
}

#[test]
fn header_fields_round_trip() {
    let header = Header {
        file_name: "부품 ①.step".to_owned(),
        description: r"it's a \ test 😀".to_owned(),
        timestamp: STAMP.to_owned(),
        authors: vec!["홍길동".to_owned(), "Ann O'Neil".to_owned()],
        organizations: Vec::new(),
        originating_system: "nacre".to_owned(),
        authorisation: String::new(),
    };
    let file = StepWriter::new(Vec::new(), &header, Units::default())
        .expect("writer")
        .finish_to_string()
        .expect("finish");
    assert!(file.is_ascii(), "non-ASCII output:\n{file}");

    let graph = parse(&file).expect("parse");
    let name = header_attributes(&graph.header, "FILE_NAME").expect("FILE_NAME");
    assert_eq!(text(&name[0]), header.file_name);
    assert_eq!(text(&name[1]), header.timestamp);
    assert_eq!(texts(&name[2]), header.authors);
    assert_eq!(texts(&name[3]), [""], "an empty list is written as ('')");
    assert_eq!(
        text(&name[4]),
        concat!("brep-to-step ", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(text(&name[5]), header.originating_system);
    assert_eq!(text(&name[6]), header.authorisation);

    let description =
        header_attributes(&graph.header, "FILE_DESCRIPTION").expect("FILE_DESCRIPTION");
    assert_eq!(
        texts(&description[0]),
        std::slice::from_ref(&header.description)
    );
    assert_eq!(text(&description[1]), "2;1");
}

#[test]
fn same_input_same_bytes() {
    assert_eq!(ours(Units::default()), ours(Units::default()));
}

#[test]
fn uncertainty_must_be_finite_and_positive() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1e-7] {
        let result = StepWriter::new(
            Vec::new(),
            &header(),
            Units {
                uncertainty: bad,
                ..Units::default()
            },
        );
        assert!(
            matches!(
                result,
                Err(Error::InvalidNumber {
                    what: "uncertainty",
                    ..
                })
            ),
            "uncertainty {bad} gave {result:?}"
        );
    }
}

#[test]
fn header_strings_are_at_most_256_characters() {
    // The header is written first, so `new` checks it.
    let start = |header: &Header| StepWriter::new(Vec::new(), header, Units::default());

    let at_limit = Header {
        file_name: "가".repeat(256),
        ..header()
    };
    assert!(start(&at_limit).is_ok());

    let long_name = Header {
        file_name: "가".repeat(257),
        ..header()
    };
    assert!(matches!(
        start(&long_name),
        Err(Error::HeaderTooLong {
            field: "file_name",
            chars: 257
        })
    ));

    let long_author = Header {
        authors: vec!["ok".to_owned(), "a".repeat(300)],
        ..header()
    };
    assert!(matches!(
        start(&long_author),
        Err(Error::HeaderTooLong {
            field: "authors",
            chars: 300
        })
    ));
}

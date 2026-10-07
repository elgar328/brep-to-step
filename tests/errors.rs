//! Input checks: every rejected input gives its error, and a rejected call
//! leaves nothing behind in the file.

mod common;

use brep_to_step::{Bound, Curve, Error, Frame, StepWriter, Surface, Units};
use common::fixtures::{LineKind, cube};
use common::scene::{Handles, header, replay, write_ours};

fn writer() -> StepWriter {
    StepWriter::new(Units::default()).expect("writer")
}

fn frame() -> Frame {
    Frame {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        ref_dir: [1.0, 0.0, 0.0],
    }
}

fn plane(frame: Frame) -> Surface {
    Surface::Plane(frame)
}

/// A writer holding the unit cube, and its handles.
fn with_cube() -> (StepWriter, Handles) {
    let mut w = writer();
    let handles = replay(&mut w, &cube("cube", [0.0; 3], 1.0, LineKind::Along));
    (w, handles)
}

#[test]
fn non_finite_numbers() {
    let (mut w, h) = with_cube();
    assert!(matches!(
        w.vertex([0.0, f64::NAN, 0.0]),
        Err(Error::InvalidNumber { what: "vertex", .. })
    ));
    assert!(matches!(
        w.edge(
            h.vertices[0],
            h.vertices[1],
            Curve::LineAlong([f64::INFINITY, 0.0, 0.0])
        ),
        Err(Error::InvalidNumber {
            what: "line direction",
            ..
        })
    ));
    let bound = Bound::outer(vec![(h.edges[0], true)]);
    for bad in [
        Frame {
            origin: [f64::NAN, 0.0, 0.0],
            ..frame()
        },
        Frame {
            axis: [0.0, 0.0, f64::NEG_INFINITY],
            ..frame()
        },
        Frame {
            ref_dir: [f64::NAN, 0.0, 0.0],
            ..frame()
        },
    ] {
        assert!(matches!(
            w.face(plane(bad), true, std::slice::from_ref(&bound)),
            Err(Error::InvalidNumber {
                what: "plane frame",
                ..
            })
        ));
    }
}

#[test]
fn zero_vectors() {
    let (mut w, h) = with_cube();
    assert!(matches!(
        w.edge(h.vertices[0], h.vertices[1], Curve::LineAlong([0.0; 3])),
        Err(Error::ZeroVector {
            what: "line direction"
        })
    ));
    let bound = Bound::outer(vec![(h.edges[0], true)]);
    for bad in [
        Frame {
            axis: [0.0; 3],
            ..frame()
        },
        Frame {
            ref_dir: [0.0; 3],
            ..frame()
        },
    ] {
        assert!(matches!(
            w.face(plane(bad), true, std::slice::from_ref(&bound)),
            Err(Error::ZeroVector {
                what: "plane frame"
            })
        ));
    }
}

#[test]
fn parallel_axes() {
    let (mut w, h) = with_cube();
    let bound = Bound::outer(vec![(h.edges[0], true)]);
    // Anti-parallel is parallel too.
    let bad = Frame {
        ref_dir: [0.0, 0.0, -2.0],
        ..frame()
    };
    assert!(matches!(
        w.face(plane(bad), true, &[bound]),
        Err(Error::ParallelAxes {
            what: "plane frame"
        })
    ));
}

#[test]
fn zero_length_lines() {
    let (mut w, h) = with_cube();
    let twin = w.vertex([0.0; 3]).expect("vertex at the cube's vertex 0");
    let v0 = h.vertices[0];
    for (start, end) in [(v0, v0), (v0, twin)] {
        assert!(matches!(
            w.edge(start, end, Curve::Line),
            Err(Error::ZeroLengthLine)
        ));
        assert!(matches!(
            w.edge(start, end, Curve::LineAlong([1.0, 0.0, 0.0])),
            Err(Error::ZeroLengthLine)
        ));
    }
}

#[test]
fn empty_lists() {
    let (mut w, h) = with_cube();
    assert!(matches!(
        w.face(plane(frame()), true, &[]),
        Err(Error::Empty {
            what: "face bounds"
        })
    ));
    assert!(matches!(
        w.face(
            plane(frame()),
            true,
            &[
                Bound::outer(vec![(h.edges[0], true)]),
                Bound::inner(Vec::new())
            ]
        ),
        Err(Error::Empty {
            what: "bound edges"
        })
    ));
    assert!(matches!(
        w.solid(h.parts[0], &[]),
        Err(Error::Empty {
            what: "solid faces"
        })
    ));
}

#[test]
fn multiple_outer_bounds() {
    let (mut w, h) = with_cube();
    assert!(matches!(
        w.face(
            plane(frame()),
            true,
            &[
                Bound::outer(vec![(h.edges[0], true)]),
                Bound::outer(vec![(h.edges[1], true)])
            ]
        ),
        Err(Error::MultipleOuterBounds)
    ));
}

#[test]
fn foreign_handles() {
    let (mut w, h) = with_cube();
    let (_other_writer, other) = with_cube();
    let mine = Bound::outer(vec![(h.edges[0], true)]);
    let mixed = Bound::outer(vec![(h.edges[0], true), (other.edges[1], true)]);

    for (start, end) in [
        (other.vertices[0], h.vertices[1]),
        (h.vertices[0], other.vertices[1]),
    ] {
        assert!(matches!(
            w.edge(start, end, Curve::Line),
            Err(Error::ForeignHandle)
        ));
    }
    assert!(matches!(
        w.face(plane(frame()), true, &[mixed]),
        Err(Error::ForeignHandle)
    ));
    assert!(matches!(
        w.solid(h.parts[0], &[h.faces[0], other.faces[1]]),
        Err(Error::ForeignHandle)
    ));
    assert!(matches!(
        w.solid(other.parts[0], &[h.faces[0]]),
        Err(Error::ForeignHandle)
    ));
    // The writer's own handles still work.
    assert!(w.face(plane(frame()), true, &[mine]).is_ok());
}

#[test]
fn part_without_a_solid() {
    let (mut w, _) = with_cube();
    w.part("hollow promise");
    assert!(matches!(
        w.finish(&header()),
        Err(Error::EmptyPart { name }) if name == "hollow promise"
    ));
}

/// Rejected calls write nothing and use up no `#id`: a writer that made
/// them finishes byte for byte like one that did not.
#[test]
fn rejected_calls_leave_no_trace() {
    let scene = cube("cube", [0.0; 3], 1.0, LineKind::Along);
    let clean = write_ours(&scene);

    let mut w = writer();
    let h = replay(&mut w, &scene);
    let (_other_writer, other) = with_cube();
    let edge = h.edges[0];
    let parallel = Frame {
        ref_dir: [0.0, 0.0, 1.0],
        ..frame()
    };

    assert!(w.vertex([f64::NAN, 0.0, 0.0]).is_err());
    assert!(w.edge(h.vertices[0], h.vertices[0], Curve::Line).is_err());
    assert!(
        w.edge(h.vertices[0], h.vertices[1], Curve::LineAlong([0.0; 3]))
            .is_err()
    );
    assert!(
        w.edge(other.vertices[0], h.vertices[1], Curve::Line)
            .is_err()
    );
    // A good surface with a bad bound: the surface must not be written.
    assert!(
        w.face(
            plane(frame()),
            true,
            &[Bound::outer(vec![(edge, true)]), Bound::inner(Vec::new())]
        )
        .is_err()
    );
    assert!(
        w.face(
            plane(frame()),
            true,
            &[
                Bound::outer(vec![(edge, true)]),
                Bound::outer(vec![(edge, false)])
            ]
        )
        .is_err()
    );
    assert!(
        w.face(
            plane(frame()),
            true,
            &[Bound::outer(vec![(edge, true), (other.edges[0], true)])]
        )
        .is_err()
    );
    assert!(
        w.face(plane(parallel), true, &[Bound::outer(vec![(edge, true)])])
            .is_err()
    );
    assert!(w.solid(h.parts[0], &[]).is_err());
    assert!(w.solid(h.parts[0], &[h.faces[0], other.faces[0]]).is_err());

    assert_eq!(w.finish(&header()).expect("finish"), clean);
}

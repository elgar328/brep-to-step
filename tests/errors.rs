//! Input checks: every rejected input gives its error, and a rejected call
//! leaves nothing behind in the file.

mod common;

use std::f64::consts::FRAC_PI_2;

use brep_to_step::{
    Bound, Curve, Error, Frame, NurbsCurve, NurbsSurface, Profile, StepWriter, Surface, Units,
    VoidShellNormals,
};
use common::fixtures::{LineKind, cube, cubic, grid};
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

/// Distinct vertices so close together, or so far apart, that the line's
/// length underflows to 0 or overflows to infinity: rejected, leaving no
/// trace. Nearer 1 the same arithmetic works and the edge is written.
#[test]
fn unrepresentable_line_lengths() {
    let vertices_only = |a: [f64; 3], b: [f64; 3]| {
        let mut w = writer();
        w.vertex(a).expect("start");
        w.vertex(b).expect("end");
        w.finish(&header()).expect("finish")
    };
    for (a, b) in [
        ([0.0; 3], [1e-200, 0.0, 0.0]),
        ([-1e308, 0.0, 0.0], [1e308, 0.0, 0.0]),
    ] {
        let mut w = writer();
        let start = w.vertex(a).expect("start");
        let end = w.vertex(b).expect("end");
        assert!(invalid_number(
            &w.edge(start, end, Curve::Line).map(|_| ()),
            "line length"
        ));
        assert_eq!(w.finish(&header()).expect("finish"), vertices_only(a, b));
    }
    let mut w = writer();
    let start = w.vertex([0.0; 3]).expect("start");
    let end = w.vertex([1e-100, 0.0, 0.0]).expect("end");
    assert!(w.edge(start, end, Curve::Line).is_ok());
}

/// Deterministic xorshift64, so a failure reproduces.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Alternately an arbitrary finite bit pattern — subnormals and values
    /// near `f64::MAX` included — and a coordinate-like value.
    fn finite(&mut self) -> f64 {
        loop {
            let bits = self.next();
            let v = if bits & 1 == 0 {
                f64::from_bits(bits)
            } else {
                #[allow(clippy::cast_precision_loss)] // below 2^21, exact
                let n = (bits >> 43) as f64;
                n / 1000.0 - 1000.0
            };
            if v.is_finite() {
                return v;
            }
        }
    }

    fn point(&mut self) -> [f64; 3] {
        [self.finite(), self.finite(), self.finite()]
    }
}

/// Straight edges and extrusions from arbitrary finite values, tiny and huge
/// alike: each call is accepted or rejected but never panics, and every REAL
/// the file holds is one Part 21 can read.
#[test]
fn extreme_values_never_panic() {
    const CALLS: usize = 2000;
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let (mut w, h) = with_cube();
    let bound = Bound::outer(vec![(h.edges[0], true)]);
    let (mut lines, mut sweeps) = (0, 0);
    for _ in 0..CALLS {
        let start = w.vertex(rng.point()).expect("finite vertex");
        let end = w.vertex(rng.point()).expect("finite vertex");
        lines += usize::from(w.edge(start, end, Curve::Line).is_ok());
        let extrusion = Surface::LinearExtrusion {
            profile: Profile::Line {
                point: [0.0; 3],
                direction: [1.0, 0.0, 0.0],
            },
            sweep: rng.point(),
        };
        sweeps += usize::from(
            w.face(extrusion, true, std::slice::from_ref(&bound))
                .is_ok(),
        );
    }
    // Both outcomes occur, so the run exercises the checks and the writes.
    assert!(
        0 < lines && lines < CALLS,
        "{lines} of {CALLS} lines written"
    );
    assert!(
        0 < sweeps && sweeps < CALLS,
        "{sweeps} of {CALLS} sweeps written"
    );
    let text = w.finish(&header()).expect("finish");
    if let Err(e) = step_io::parser::parse(&text) {
        panic!("output does not parse: {e}");
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

    reject_curved_calls(&mut w, &h, &other);

    assert_eq!(w.finish(&header()).expect("finish"), clean);
}

/// Rejected calls on curved geometry, voids, and NURBS, for
/// [`rejected_calls_leave_no_trace`].
fn reject_curved_calls(w: &mut StepWriter, h: &Handles, other: &Handles) {
    let edge = h.edges[0];
    let circle = |radius| Curve::Circle {
        frame: frame(),
        radius,
    };
    assert!(w.edge(h.vertices[0], h.vertices[0], circle(0.0)).is_err());
    assert!(
        w.face(
            Surface::Cylinder {
                frame: frame(),
                radius: -1.0
            },
            true,
            &[Bound::outer(vec![(edge, true)])]
        )
        .is_err()
    );
    let away = VoidShellNormals::AwayFromMaterial;
    assert!(w.solid_with_voids(h.parts[0], &h.faces, &[], away).is_err());
    assert!(
        w.solid_with_voids(h.parts[0], &h.faces, &[vec![other.faces[0]]], away)
            .is_err()
    );

    let mut bad_curve = cubic();
    bad_curve.knots = vec![0.0, 0.5, 0.5];
    assert!(
        w.edge(
            h.vertices[0],
            h.vertices[1],
            Curve::Nurbs(bad_curve.clone())
        )
        .is_err()
    );
    let bound = Bound::outer(vec![(edge, true)]);
    let bad_extrusion = |profile, sweep| Surface::LinearExtrusion { profile, sweep };
    assert!(
        w.face(
            bad_extrusion(Profile::Nurbs(bad_curve), [0.0, 0.0, 1.0]),
            true,
            std::slice::from_ref(&bound)
        )
        .is_err()
    );
    for sweep in [[0.0; 3], [0.0, 1e-200, 0.0]] {
        assert!(
            w.face(
                bad_extrusion(Profile::Nurbs(cubic()), sweep),
                true,
                std::slice::from_ref(&bound)
            )
            .is_err()
        );
    }
}

#[test]
fn radii_must_be_finite_and_positive() {
    let (mut w, h) = with_cube();
    let bound = Bound::outer(vec![(h.edges[0], true)]);
    for bad in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        assert!(matches!(
            w.edge(
                h.vertices[0],
                h.vertices[0],
                Curve::Circle {
                    frame: frame(),
                    radius: bad
                }
            ),
            Err(Error::InvalidNumber {
                what: "circle radius",
                ..
            })
        ));
        assert!(matches!(
            w.face(
                Surface::Cylinder {
                    frame: frame(),
                    radius: bad
                },
                true,
                std::slice::from_ref(&bound)
            ),
            Err(Error::InvalidNumber {
                what: "cylinder radius",
                ..
            })
        ));
    }
}

#[test]
fn circle_and_cylinder_frames_are_checked() {
    let (mut w, h) = with_cube();
    let bound = Bound::outer(vec![(h.edges[0], true)]);
    let circle = |frame| Curve::Circle { frame, radius: 1.0 };
    let v0 = h.vertices[0];
    assert!(matches!(
        w.edge(
            v0,
            v0,
            circle(Frame {
                origin: [f64::NAN, 0.0, 0.0],
                ..frame()
            })
        ),
        Err(Error::InvalidNumber {
            what: "circle frame",
            ..
        })
    ));
    assert!(matches!(
        w.edge(
            v0,
            v0,
            circle(Frame {
                axis: [0.0; 3],
                ..frame()
            })
        ),
        Err(Error::ZeroVector {
            what: "circle frame"
        })
    ));
    assert!(matches!(
        w.face(
            Surface::Cylinder {
                frame: Frame {
                    ref_dir: [0.0, 0.0, 3.0],
                    ..frame()
                },
                radius: 1.0
            },
            true,
            &[bound]
        ),
        Err(Error::ParallelAxes {
            what: "cylinder frame"
        })
    ));
}

/// Unlike a straight edge, a circle may start and end at one vertex.
#[test]
fn circle_edges_may_close() {
    let (mut w, h) = with_cube();
    let circle = Curve::Circle {
        frame: frame(),
        radius: 1.0,
    };
    assert!(w.edge(h.vertices[0], h.vertices[0], circle.clone()).is_ok());
    assert!(w.edge(h.vertices[0], h.vertices[1], circle).is_ok());
}

#[test]
fn voids_need_faces() {
    let (mut w, h) = with_cube();
    let away = VoidShellNormals::AwayFromMaterial;
    let some = vec![h.faces[0]];
    assert!(matches!(
        w.solid_with_voids(h.parts[0], &[], std::slice::from_ref(&some), away),
        Err(Error::Empty {
            what: "solid faces"
        })
    ));
    assert!(matches!(
        w.solid_with_voids(h.parts[0], &h.faces, &[], away),
        Err(Error::Empty { what: "voids" })
    ));
    assert!(matches!(
        w.solid_with_voids(h.parts[0], &h.faces, &[some, Vec::new()], away),
        Err(Error::Empty { what: "void faces" })
    ));
}

#[test]
fn voids_reject_foreign_handles() {
    let (mut w, h) = with_cube();
    let (_other_writer, other) = with_cube();
    let away = VoidShellNormals::AwayFromMaterial;
    assert!(matches!(
        w.solid_with_voids(other.parts[0], &h.faces, &[vec![h.faces[0]]], away),
        Err(Error::ForeignHandle)
    ));
    assert!(matches!(
        w.solid_with_voids(h.parts[0], &other.faces, &[vec![h.faces[0]]], away),
        Err(Error::ForeignHandle)
    ));
    assert!(matches!(
        w.solid_with_voids(h.parts[0], &h.faces, &[vec![other.faces[0]]], away),
        Err(Error::ForeignHandle)
    ));
}

/// A face on `surface`, bounded by the cube's first edge.
fn face_on(surface: Surface) -> Result<(), Error> {
    let (mut w, h) = with_cube();
    w.face(surface, true, &[Bound::outer(vec![(h.edges[0], true)])])
        .map(|_| ())
}

/// An edge between the cube's first two vertices along `curve`.
fn edge_along(curve: Curve) -> Result<(), Error> {
    let (mut w, h) = with_cube();
    w.edge(h.vertices[0], h.vertices[1], curve).map(|_| ())
}

fn invalid_number(result: &Result<(), Error>, expected: &str) -> bool {
    matches!(result, Err(Error::InvalidNumber { what, .. }) if *what == expected)
}

#[test]
fn analytic_sizes() {
    for bad in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        let ellipse = |a, b| Curve::Ellipse {
            frame: frame(),
            semi_axis_1: a,
            semi_axis_2: b,
        };
        assert!(invalid_number(
            &edge_along(ellipse(bad, 1.0)),
            "ellipse semi-axis"
        ));
        assert!(invalid_number(
            &edge_along(ellipse(1.0, bad)),
            "ellipse semi-axis"
        ));
        assert!(invalid_number(
            &face_on(Surface::Sphere {
                frame: frame(),
                radius: bad
            }),
            "sphere radius"
        ));
        let torus = |major_radius, minor_radius| Surface::Torus {
            frame: frame(),
            major_radius,
            minor_radius,
        };
        assert!(invalid_number(
            &face_on(torus(bad, 1.0)),
            "torus major radius"
        ));
        assert!(invalid_number(
            &face_on(torus(5.0, bad)),
            "torus minor radius"
        ));
    }
}

#[test]
fn cone_radius_and_angle() {
    let cone = |radius, semi_angle| Surface::Cone {
        frame: frame(),
        radius,
        semi_angle,
    };
    assert!(face_on(cone(0.0, 0.4)).is_ok(), "an apex at the origin");
    for bad in [f64::NAN, -1.0] {
        assert!(invalid_number(&face_on(cone(bad, 0.4)), "cone radius"));
    }
    for bad in [f64::NAN, 0.0, -0.4, FRAC_PI_2, 2.0] {
        assert!(invalid_number(&face_on(cone(1.0, bad)), "cone semi-angle"));
    }
}

#[test]
fn polylines() {
    assert!(matches!(
        edge_along(Curve::Polyline(vec![[0.0; 3]])),
        Err(Error::Empty {
            what: "polyline points"
        })
    ));
    assert!(invalid_number(
        &edge_along(Curve::Polyline(vec![[0.0; 3], [f64::NAN, 0.0, 0.0]])),
        "polyline point"
    ));
}

#[test]
fn swept_surface_directions() {
    let profile = || Profile::Nurbs(cubic());
    assert!(matches!(
        face_on(Surface::LinearExtrusion {
            profile: profile(),
            sweep: [0.0; 3]
        }),
        Err(Error::ZeroVector {
            what: "extrusion sweep"
        })
    ));
    assert!(matches!(
        face_on(Surface::Revolution {
            profile: profile(),
            axis_origin: [0.0; 3],
            axis_direction: [0.0; 3]
        }),
        Err(Error::ZeroVector {
            what: "revolution axis"
        })
    ));
    assert!(invalid_number(
        &face_on(Surface::Revolution {
            profile: profile(),
            axis_origin: [f64::NAN, 0.0, 0.0],
            axis_direction: [0.0, 0.0, 1.0]
        }),
        "revolution axis"
    ));
    assert!(matches!(
        face_on(Surface::LinearExtrusion {
            profile: Profile::Line {
                point: [0.0; 3],
                direction: [0.0; 3]
            },
            sweep: [0.0, 0.0, 1.0]
        }),
        Err(Error::ZeroVector {
            what: "profile line"
        })
    ));
    for sweep in [[0.0, 1e-200, 0.0], [1e200, 1e200, 0.0]] {
        assert!(invalid_number(
            &face_on(Surface::LinearExtrusion {
                profile: profile(),
                sweep
            }),
            "extrusion sweep length"
        ));
    }
}

/// A change that breaks one NURBS curve rule.
type CurveEdit = Box<dyn FnOnce(&mut NurbsCurve)>;

/// The rule a NURBS curve breaks, if any.
fn nurbs_curve_error(edit: impl FnOnce(&mut NurbsCurve)) -> Result<(), Error> {
    let mut curve = cubic();
    edit(&mut curve);
    edge_along(Curve::Nurbs(curve))
}

fn nurbs_reason(result: Result<(), Error>) -> &'static str {
    match result {
        Err(Error::InvalidNurbs { reason }) => reason,
        other => panic!("expected InvalidNurbs, got {other:?}"),
    }
}

#[test]
fn nurbs_curve_rules() {
    assert!(nurbs_curve_error(|_| {}).is_ok());
    let cases: [(&str, CurveEdit); 11] = [
        ("degree must be at least 1", Box::new(|c| c.degree = 0)),
        (
            "needs at least degree + 1 control points",
            Box::new(|c| c.control_points.truncate(3)),
        ),
        // `degree + 1` itself would overflow.
        (
            "needs at least degree + 1 control points",
            Box::new(|c| c.degree = usize::MAX),
        ),
        (
            "knots and multiplicities differ in count",
            Box::new(|c| {
                c.multiplicities.pop();
            }),
        ),
        (
            "needs at least two distinct knots",
            Box::new(|c| {
                c.knots = vec![0.0];
                c.multiplicities = vec![9];
            }),
        ),
        ("knots must be finite", Box::new(|c| c.knots[1] = f64::NAN)),
        ("knots must increase", Box::new(|c| c.knots[1] = 1.0)),
        (
            "multiplicities must be at least 1",
            Box::new(|c| c.multiplicities = vec![4, 0, 4]),
        ),
        (
            "end multiplicities may be at most degree + 1",
            Box::new(|c| c.multiplicities = vec![5, 1, 3]),
        ),
        (
            "interior multiplicities may be at most degree",
            Box::new(|c| {
                c.multiplicities = vec![2, 4, 3];
            }),
        ),
        (
            "multiplicities must sum to control points + degree + 1",
            Box::new(|c| c.multiplicities = vec![4, 2, 4]),
        ),
    ];
    for (reason, edit) in cases {
        assert_eq!(nurbs_reason(nurbs_curve_error(edit)), reason);
    }
    assert_eq!(
        nurbs_reason(nurbs_curve_error(|c| c.weights = Some(vec![1.0; 4]))),
        "needs one weight per control point"
    );
    assert!(invalid_number(
        &nurbs_curve_error(|c| c.weights = Some(vec![1.0, 1.0, 0.0, 1.0, 1.0])),
        "weight"
    ));
    assert!(invalid_number(
        &nurbs_curve_error(|c| c.control_points[2][1] = f64::INFINITY),
        "control point"
    ));
}

/// The rule a NURBS surface breaks, if any.
fn nurbs_surface_error(edit: impl FnOnce(&mut NurbsSurface)) -> Result<(), Error> {
    let mut surface = grid(false);
    edit(&mut surface);
    face_on(Surface::Nurbs(surface))
}

#[test]
fn nurbs_surface_rules() {
    assert!(nurbs_surface_error(|_| {}).is_ok());
    assert_eq!(
        nurbs_reason(nurbs_surface_error(|s| {
            s.control_points[1].pop();
        })),
        "control point rows differ in length"
    );
    assert_eq!(
        nurbs_reason(nurbs_surface_error(|s| s.degree_u = usize::MAX)),
        "needs at least u degree + 1 control point rows"
    );
    assert_eq!(
        nurbs_reason(nurbs_surface_error(|s| s.knots_u = vec![1.0, 0.0])),
        "u knots must increase"
    );
    assert_eq!(
        nurbs_reason(nurbs_surface_error(|s| s.multiplicities_v = vec![4, 3])),
        "v multiplicities must sum to control points per row + degree + 1"
    );
    assert_eq!(
        nurbs_reason(nurbs_surface_error(
            |s| s.weights = Some(vec![vec![1.0; 4]; 2])
        )),
        "needs one weight per control point"
    );
    assert!(invalid_number(
        &nurbs_surface_error(|s| s.weights = Some(vec![vec![1.0, 1.0, -0.5, 1.0]; 3])),
        "weight"
    ));
}

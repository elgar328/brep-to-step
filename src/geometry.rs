//! Geometry entities: points, directions, placements, curves, and surfaces.

use std::f64::consts::FRAC_PI_2;

use crate::error::Error;
use crate::nurbs::{self, NurbsCurve, NurbsSurface};
use crate::p21::{Data, Param, Ref};

/// A right-handed placement: an origin plus the local Z (`axis`) and local X
/// (`ref_dir`) directions — STEP's `AXIS2_PLACEMENT_3D`.
///
/// The directions are written as given. They need not be unit length or
/// exactly perpendicular (STEP projects `ref_dir` onto the plane normal to
/// `axis`), but neither may be zero and they must not be parallel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The local origin.
    pub origin: [f64; 3],
    /// The local Z direction.
    pub axis: [f64; 3],
    /// The local X direction.
    pub ref_dir: [f64; 3],
}

/// The curve an edge lies on.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Curve {
    /// A straight edge whose direction is computed from its two vertices.
    Line,
    /// A straight edge along a direction the caller supplies, written bit
    /// for bit. A kernel that knows the exact direction keeps it, instead of
    /// the one implied by two rounded vertex positions. If the direction
    /// points from the end vertex back to the start, it is negated exactly so
    /// that the line runs the same way as the edge. Keeping the direction
    /// parallel to the edge is up to the caller.
    LineAlong([f64; 3]),
    /// A circle of `radius` in the frame's XY plane, centred at the frame's
    /// origin. If the start and end vertices are the same, the edge is the
    /// full circle; otherwise it is the arc from start to end, running
    /// counter-clockwise about the axis. (For a clockwise arc, negate the
    /// axis.)
    Circle {
        /// The circle's placement: centre, normal, and where its parameter starts.
        frame: Frame,
        /// The circle's radius; positive.
        radius: f64,
    },
    /// An ellipse in the frame's XY plane, centred at the frame's origin,
    /// with `semi_axis_1` along the reference direction and `semi_axis_2`
    /// perpendicular to it. As with a [`Circle`](Self::Circle), the edge is
    /// the full ellipse or a counter-clockwise arc.
    Ellipse {
        /// The ellipse's placement: centre, normal, and its first axis.
        frame: Frame,
        /// The semi-axis along the frame's reference direction; positive.
        semi_axis_1: f64,
        /// The semi-axis across it; positive.
        semi_axis_2: f64,
    },
    /// A piecewise-linear curve through at least two points. The first and
    /// last points usually sit at the edge's vertices.
    Polyline(Vec<[f64; 3]>),
    /// A B-spline curve, rational or not.
    Nurbs(NurbsCurve),
}

/// A curve without vertices: the profile swept by a
/// [`Surface::LinearExtrusion`] or [`Surface::Revolution`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Profile {
    /// An infinite line through `point` along `direction`.
    Line {
        /// A point the line passes through.
        point: [f64; 3],
        /// The line's direction; not zero.
        direction: [f64; 3],
    },
    /// A full circle, placed as in [`Curve::Circle`].
    Circle {
        /// The circle's placement: centre, normal, and where its parameter starts.
        frame: Frame,
        /// The circle's radius; positive.
        radius: f64,
    },
    /// A full ellipse, placed as in [`Curve::Ellipse`].
    Ellipse {
        /// The ellipse's placement: centre, normal, and its first axis.
        frame: Frame,
        /// The semi-axis along the frame's reference direction; positive.
        semi_axis_1: f64,
        /// The semi-axis across it; positive.
        semi_axis_2: f64,
    },
    /// A B-spline curve, rational or not.
    Nurbs(NurbsCurve),
}

/// The surface a face lies on.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Surface {
    /// A plane through the frame's origin, normal to its axis.
    Plane(Frame),
    /// A cylinder of `radius` around the frame's axis, through its origin.
    Cylinder {
        /// The cylinder's placement: a point on its axis, the axis, and where
        /// its angle starts.
        frame: Frame,
        /// The cylinder's radius; positive.
        radius: f64,
    },
    /// A sphere of `radius` centred at the frame's origin.
    Sphere {
        /// The sphere's placement: its centre and the axes its angles are
        /// measured from.
        frame: Frame,
        /// The sphere's radius; positive.
        radius: f64,
    },
    /// A torus around the frame's axis: a tube of `minor_radius` whose centre
    /// follows the circle of `major_radius` in the frame's XY plane.
    Torus {
        /// The torus's placement: its centre, its axis, and where its angle starts.
        frame: Frame,
        /// The radius of the circle the tube follows; positive.
        major_radius: f64,
        /// The radius of the tube; positive.
        minor_radius: f64,
    },
    /// A cone around the frame's axis, with `radius` in the frame's XY plane
    /// and its side at `semi_angle` to the axis. A `radius` of 0 puts the
    /// apex at the origin.
    Cone {
        /// The cone's placement: a point on its axis, the axis, and where its
        /// angle starts.
        frame: Frame,
        /// The radius in the frame's XY plane; zero or more.
        radius: f64,
        /// The angle between the side and the axis, in radians; strictly
        /// between 0 and π/2.
        semi_angle: f64,
    },
    /// The surface swept out by moving `profile` along `sweep`.
    LinearExtrusion {
        /// The curve being swept.
        profile: Profile,
        /// The sweep's direction and length; not zero.
        sweep: [f64; 3],
    },
    /// The surface swept out by revolving `profile` about the axis through
    /// `axis_origin` along `axis_direction`.
    Revolution {
        /// The curve being revolved.
        profile: Profile,
        /// A point on the axis of revolution.
        axis_origin: [f64; 3],
        /// The axis's direction; not zero.
        axis_direction: [f64; 3],
    },
    /// A B-spline surface, rational or not.
    Nurbs(NurbsSurface),
}

/// Every component must be finite.
pub(crate) fn check_finite(what: &'static str, v: [f64; 3]) -> Result<(), Error> {
    match v.iter().find(|c| !c.is_finite()) {
        Some(&value) => Err(Error::InvalidNumber { what, value }),
        None => Ok(()),
    }
}

/// A direction must be finite and not zero.
// Exact comparisons intended: only exactly degenerate input is rejected.
#[allow(clippy::float_cmp)]
fn check_direction(what: &'static str, d: [f64; 3]) -> Result<(), Error> {
    check_finite(what, d)?;
    if d == [0.0; 3] {
        return Err(Error::ZeroVector { what });
    }
    Ok(())
}

/// A length — radius, semi-axis — must be finite and positive.
fn check_positive(what: &'static str, value: f64) -> Result<(), Error> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(Error::InvalidNumber { what, value })
    }
}

/// `v` scaled to unit length, and its length. The arithmetic follows
/// step-io's `StepBuilder` term for term, so both write the same bits.
fn unit(v: [f64; 3]) -> ([f64; 3], f64) {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    ([v[0] / len, v[1] / len, v[2] / len], len)
}

/// [`unit`] must give a finite direction and a finite, positive length. A
/// vector of finite, nonzero components can still fail: its squared length
/// underflows to 0 or overflows to infinity.
fn check_unit(what: &'static str, v: [f64; 3]) -> Result<(), Error> {
    let (direction, len) = unit(v);
    check_positive(what, len)?;
    check_finite(what, direction)
}

/// A straight edge needs two distinct points.
// Exactly coincident only, as in `check_direction`.
#[allow(clippy::float_cmp)]
fn check_not_coincident(start: [f64; 3], end: [f64; 3]) -> Result<(), Error> {
    if start == end {
        return Err(Error::ZeroLengthLine);
    }
    Ok(())
}

impl Frame {
    #[allow(clippy::float_cmp)] // exactly parallel only, as in `check_direction`
    fn check(&self, what: &'static str) -> Result<(), Error> {
        check_finite(what, self.origin)?;
        check_direction(what, self.axis)?;
        check_direction(what, self.ref_dir)?;
        if cross(self.axis, self.ref_dir) == [0.0; 3] {
            return Err(Error::ParallelAxes { what });
        }
        Ok(())
    }
}

fn check_circle(frame: &Frame, radius: f64) -> Result<(), Error> {
    frame.check("circle frame")?;
    check_positive("circle radius", radius)
}

fn check_ellipse(frame: &Frame, semi_axis_1: f64, semi_axis_2: f64) -> Result<(), Error> {
    frame.check("ellipse frame")?;
    check_positive("ellipse semi-axis", semi_axis_1)?;
    check_positive("ellipse semi-axis", semi_axis_2)
}

impl Curve {
    /// Check the curve for an edge from `start` to `end`. Only a straight
    /// edge needs distinct ends: a circle's full-circle edge starts and ends
    /// at one vertex.
    pub(crate) fn check(&self, start: [f64; 3], end: [f64; 3]) -> Result<(), Error> {
        match self {
            Curve::Line => {
                check_not_coincident(start, end)?;
                check_unit("line length", sub(end, start))
            }
            Curve::LineAlong(direction) => {
                check_direction("line direction", *direction)?;
                check_not_coincident(start, end)
            }
            Curve::Circle { frame, radius } => check_circle(frame, *radius),
            Curve::Ellipse {
                frame,
                semi_axis_1,
                semi_axis_2,
            } => check_ellipse(frame, *semi_axis_1, *semi_axis_2),
            Curve::Polyline(points) => {
                if points.len() < 2 {
                    return Err(Error::Empty {
                        what: "polyline points",
                    });
                }
                points
                    .iter()
                    .try_for_each(|&p| check_finite("polyline point", p))
            }
            Curve::Nurbs(curve) => curve.check(),
        }
    }
}

impl Profile {
    fn check(&self) -> Result<(), Error> {
        match self {
            Profile::Line { point, direction } => {
                check_finite("profile line", *point)?;
                check_direction("profile line", *direction)
            }
            Profile::Circle { frame, radius } => check_circle(frame, *radius),
            Profile::Ellipse {
                frame,
                semi_axis_1,
                semi_axis_2,
            } => check_ellipse(frame, *semi_axis_1, *semi_axis_2),
            Profile::Nurbs(curve) => curve.check(),
        }
    }
}

impl Surface {
    pub(crate) fn check(&self) -> Result<(), Error> {
        match self {
            Surface::Plane(frame) => frame.check("plane frame"),
            Surface::Cylinder { frame, radius } => {
                frame.check("cylinder frame")?;
                check_positive("cylinder radius", *radius)
            }
            Surface::Sphere { frame, radius } => {
                frame.check("sphere frame")?;
                check_positive("sphere radius", *radius)
            }
            Surface::Torus {
                frame,
                major_radius,
                minor_radius,
            } => {
                frame.check("torus frame")?;
                check_positive("torus major radius", *major_radius)?;
                check_positive("torus minor radius", *minor_radius)
            }
            Surface::Cone {
                frame,
                radius,
                semi_angle,
            } => {
                frame.check("cone frame")?;
                if !(radius.is_finite() && *radius >= 0.0) {
                    return Err(Error::InvalidNumber {
                        what: "cone radius",
                        value: *radius,
                    });
                }
                if !(semi_angle.is_finite() && *semi_angle > 0.0 && *semi_angle < FRAC_PI_2) {
                    return Err(Error::InvalidNumber {
                        what: "cone semi-angle",
                        value: *semi_angle,
                    });
                }
                Ok(())
            }
            Surface::LinearExtrusion { profile, sweep } => {
                profile.check()?;
                check_direction("extrusion sweep", *sweep)?;
                check_unit("extrusion sweep length", *sweep)
            }
            Surface::Revolution {
                profile,
                axis_origin,
                axis_direction,
            } => {
                profile.check()?;
                check_finite("revolution axis", *axis_origin)?;
                check_direction("revolution axis", *axis_direction)
            }
            Surface::Nurbs(surface) => surface.check(),
        }
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn write_point(data: &mut Data, p: [f64; 3]) -> Ref {
    data.simple("CARTESIAN_POINT", &[Param::Str(""), Param::Reals(&p)])
}

fn write_direction(data: &mut Data, d: [f64; 3]) -> Ref {
    data.simple("DIRECTION", &[Param::Str(""), Param::Reals(&d)])
}

pub(crate) fn write_placement(data: &mut Data, frame: &Frame) -> Ref {
    let location = write_point(data, frame.origin);
    let axis = write_direction(data, frame.axis);
    let ref_direction = write_direction(data, frame.ref_dir);
    data.simple(
        "AXIS2_PLACEMENT_3D",
        &[
            Param::Str(""),
            Param::Ref(location),
            Param::Ref(axis),
            Param::Ref(ref_direction),
        ],
    )
}

/// An entity on a placement with some real parameters: `NAME('',#pos,r..)`.
fn write_placed(data: &mut Data, name: &'static str, frame: &Frame, values: &[f64]) -> Ref {
    let position = write_placement(data, frame);
    let mut params = vec![Param::Str(""), Param::Ref(position)];
    params.extend(values.iter().map(|&v| Param::Real(v)));
    data.simple(name, &params)
}

/// The curve of an edge from `start` to `end`. The arithmetic follows
/// step-io's `StepBuilder::edge` term for term, so both write the same bits.
pub(crate) fn write_curve(data: &mut Data, curve: &Curve, start: [f64; 3], end: [f64; 3]) -> Ref {
    let delta = sub(end, start);
    match curve {
        Curve::Line => write_line(data, start, unit(delta).0),
        Curve::LineAlong(direction) => {
            let along = direction[0] * delta[0] + direction[1] * delta[1] + direction[2] * delta[2];
            let direction = if along < 0.0 {
                direction.map(|c| -c)
            } else {
                *direction
            };
            write_line(data, start, direction)
        }
        Curve::Circle { frame, radius } => write_placed(data, "CIRCLE", frame, &[*radius]),
        Curve::Ellipse {
            frame,
            semi_axis_1,
            semi_axis_2,
        } => write_placed(data, "ELLIPSE", frame, &[*semi_axis_1, *semi_axis_2]),
        Curve::Polyline(points) => {
            let points: Vec<Ref> = points.iter().map(|&p| write_point(data, p)).collect();
            data.simple("POLYLINE", &[Param::Str(""), Param::Refs(&points)])
        }
        Curve::Nurbs(curve) => nurbs::write_curve(data, curve),
    }
}

/// A `LINE` through `start`. It gets its own point even though the start
/// vertex holds one at the same place, and its `VECTOR` has magnitude 1: the
/// magnitude only scales the parameter, and the edge is bounded by its
/// vertices.
fn write_line(data: &mut Data, start: [f64; 3], direction: [f64; 3]) -> Ref {
    let point = write_point(data, start);
    let orientation = write_direction(data, direction);
    let vector = data.simple(
        "VECTOR",
        &[Param::Str(""), Param::Ref(orientation), Param::Real(1.0)],
    );
    data.simple(
        "LINE",
        &[Param::Str(""), Param::Ref(point), Param::Ref(vector)],
    )
}

fn write_profile(data: &mut Data, profile: &Profile) -> Ref {
    match profile {
        Profile::Line { point, direction } => write_line(data, *point, *direction),
        Profile::Circle { frame, radius } => write_placed(data, "CIRCLE", frame, &[*radius]),
        Profile::Ellipse {
            frame,
            semi_axis_1,
            semi_axis_2,
        } => write_placed(data, "ELLIPSE", frame, &[*semi_axis_1, *semi_axis_2]),
        Profile::Nurbs(curve) => nurbs::write_curve(data, curve),
    }
}

/// The geometry of a face. Swept surfaces follow step-io's
/// `StepBuilder::face` term for term, so both write the same bits.
pub(crate) fn write_surface(data: &mut Data, surface: &Surface) -> Ref {
    match surface {
        Surface::Plane(frame) => write_placed(data, "PLANE", frame, &[]),
        Surface::Cylinder { frame, radius } => {
            write_placed(data, "CYLINDRICAL_SURFACE", frame, &[*radius])
        }
        Surface::Sphere { frame, radius } => {
            write_placed(data, "SPHERICAL_SURFACE", frame, &[*radius])
        }
        Surface::Torus {
            frame,
            major_radius,
            minor_radius,
        } => write_placed(
            data,
            "TOROIDAL_SURFACE",
            frame,
            &[*major_radius, *minor_radius],
        ),
        Surface::Cone {
            frame,
            radius,
            semi_angle,
        } => write_placed(data, "CONICAL_SURFACE", frame, &[*radius, *semi_angle]),
        Surface::LinearExtrusion { profile, sweep } => {
            let swept = write_profile(data, profile);
            let (direction, len) = unit(*sweep);
            let orientation = write_direction(data, direction);
            let vector = data.simple(
                "VECTOR",
                &[Param::Str(""), Param::Ref(orientation), Param::Real(len)],
            );
            data.simple(
                "SURFACE_OF_LINEAR_EXTRUSION",
                &[Param::Str(""), Param::Ref(swept), Param::Ref(vector)],
            )
        }
        Surface::Revolution {
            profile,
            axis_origin,
            axis_direction,
        } => {
            let swept = write_profile(data, profile);
            let location = write_point(data, *axis_origin);
            let axis = write_direction(data, *axis_direction);
            let position = data.simple(
                "AXIS1_PLACEMENT",
                &[Param::Str(""), Param::Ref(location), Param::Ref(axis)],
            );
            data.simple(
                "SURFACE_OF_REVOLUTION",
                &[Param::Str(""), Param::Ref(swept), Param::Ref(position)],
            )
        }
        Surface::Nurbs(surface) => nurbs::write_surface(data, surface),
    }
}

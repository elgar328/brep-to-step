//! Geometry entities: points, directions, placements, curves, and surfaces.

use crate::error::Error;
use crate::p21::{Data, Param, Ref};

/// A right-handed placement: an origin plus the local Z (`axis`) and local X
/// (`ref_dir`) directions — STEP's `AXIS2_PLACEMENT_3D`.
///
/// The directions are written as given; they need not be unit length or
/// exactly perpendicular (STEP projects `ref_dir` onto the plane normal to
/// `axis`), but neither may be zero and they may not be parallel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub origin: [f64; 3],
    pub axis: [f64; 3],
    pub ref_dir: [f64; 3],
}

/// The curve an edge lies on.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Curve {
    /// A straight edge whose direction is computed from its two vertices.
    Line,
    /// A straight edge along a direction the caller already holds, written
    /// bit for bit — a kernel that knows the exact direction keeps it rather
    /// than the one two rounded vertex positions imply. A direction pointing
    /// from the end vertex back to the start is negated exactly, so the line
    /// runs with the edge. Keeping it parallel to the edge is the caller's
    /// part.
    LineAlong([f64; 3]),
    /// The circle of `radius` in the frame's XY plane, centred at its
    /// origin. With equal start and end vertices the edge is the full
    /// circle; otherwise it is the arc from start to end, counter-clockwise
    /// about the axis. (A clockwise arc is the same circle with its axis
    /// negated.)
    Circle { frame: Frame, radius: f64 },
}

/// The surface a face lies on.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Surface {
    /// The plane through the frame's origin, normal to its axis.
    Plane(Frame),
    /// The cylinder of `radius` around the line through the frame's origin
    /// along its axis.
    Cylinder { frame: Frame, radius: f64 },
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

/// A radius must be finite and positive.
fn check_radius(what: &'static str, radius: f64) -> Result<(), Error> {
    if radius.is_finite() && radius > 0.0 {
        Ok(())
    } else {
        Err(Error::InvalidNumber {
            what,
            value: radius,
        })
    }
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

impl Curve {
    /// Check the curve for an edge from `start` to `end`. Only a straight
    /// edge needs distinct ends: a circle's full-circle edge starts and ends
    /// at one vertex.
    pub(crate) fn check(&self, start: [f64; 3], end: [f64; 3]) -> Result<(), Error> {
        match self {
            Curve::Line => check_not_coincident(start, end),
            Curve::LineAlong(direction) => {
                check_direction("line direction", *direction)?;
                check_not_coincident(start, end)
            }
            Curve::Circle { frame, radius } => {
                frame.check("circle frame")?;
                check_radius("circle radius", *radius)
            }
        }
    }
}

impl Surface {
    pub(crate) fn check(&self) -> Result<(), Error> {
        match self {
            Surface::Plane(frame) => frame.check("plane frame"),
            Surface::Cylinder { frame, radius } => {
                frame.check("cylinder frame")?;
                check_radius("cylinder radius", *radius)
            }
        }
    }
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

/// The curve of an edge from `start` to `end`. The arithmetic follows
/// step-io's `StepBuilder::edge` term for term, so both write the same bits.
pub(crate) fn write_curve(data: &mut Data, curve: &Curve, start: [f64; 3], end: [f64; 3]) -> Ref {
    let delta = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    match curve {
        Curve::Circle { frame, radius } => {
            let position = write_placement(data, frame);
            data.simple(
                "CIRCLE",
                &[Param::Str(""), Param::Ref(position), Param::Real(*radius)],
            )
        }
        Curve::Line => {
            let len = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
            let direction = [delta[0] / len, delta[1] / len, delta[2] / len];
            write_line(data, start, direction)
        }
        Curve::LineAlong(direction) => {
            let along = direction[0] * delta[0] + direction[1] * delta[1] + direction[2] * delta[2];
            let direction = if along < 0.0 {
                direction.map(|c| -c)
            } else {
                *direction
            };
            write_line(data, start, direction)
        }
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

pub(crate) fn write_surface(data: &mut Data, surface: &Surface) -> Ref {
    match surface {
        Surface::Plane(frame) => {
            let position = write_placement(data, frame);
            data.simple("PLANE", &[Param::Str(""), Param::Ref(position)])
        }
        Surface::Cylinder { frame, radius } => {
            let position = write_placement(data, frame);
            data.simple(
                "CYLINDRICAL_SURFACE",
                &[Param::Str(""), Param::Ref(position), Param::Real(*radius)],
            )
        }
    }
}

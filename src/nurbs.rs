//! NURBS curves and surfaces: B-splines, rational or not.

use crate::error::Error;
use crate::geometry::{check_finite, write_point};
use crate::p21::{Data, Param, Ref};

/// A B-spline curve, rational or not, in STEP's own form.
///
/// The knot vector is given as STEP writes it: its distinct values, with the
/// multiplicity of each. The expanded vector `[0, 0, 0, 0, 0.5, 1, 1, 1, 1]`,
/// for example, is `knots: [0.0, 0.5, 1.0]` with `multiplicities: [4, 1, 4]`.
/// The knots must strictly increase. Each multiplicity must be at least 1,
/// and at most `degree + 1` at the ends or `degree` in between. The
/// multiplicities must sum to the number of control points plus
/// `degree + 1`. A kernel that holds the expanded vector can split it with
/// [`compress_knots`].
///
/// `weights` decides whether the curve is rational. `None` writes a plain
/// B-spline; `Some`, with one positive weight per control point, writes a
/// rational one, even if every weight is 1.
///
/// ```
/// use brep_to_step::NurbsCurve;
///
/// // A cubic with one interior knot.
/// let curve = NurbsCurve {
///     degree: 3,
///     control_points: vec![
///         [0.0, 0.0, 0.0],
///         [1.0, 2.0, 0.0],
///         [2.0, -1.0, 0.0],
///         [3.0, 1.0, 0.0],
///         [4.0, 0.0, 0.0],
///     ],
///     weights: None,
///     knots: vec![0.0, 0.5, 1.0],
///     multiplicities: vec![4, 1, 4],
/// };
///
/// // A quarter circle, exactly: rational, degree 2.
/// let arc = NurbsCurve {
///     degree: 2,
///     control_points: vec![[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
///     weights: Some(vec![1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0]),
///     knots: vec![0.0, 1.0],
///     multiplicities: vec![3, 3],
/// };
/// # let _ = (curve, arc);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct NurbsCurve {
    /// The polynomial degree; at least 1.
    pub degree: usize,
    /// The control points; at least `degree + 1`.
    pub control_points: Vec<[f64; 3]>,
    /// One positive weight per control point, or `None` for a non-rational
    /// curve.
    pub weights: Option<Vec<f64>>,
    /// The distinct knot values, increasing.
    pub knots: Vec<f64>,
    /// How many times each knot repeats.
    pub multiplicities: Vec<usize>,
}

/// A B-spline surface, rational or not, in STEP's own form. Each parameter
/// direction follows the rules of [`NurbsCurve`]; an expanded knot vector in
/// either direction can be split with [`compress_knots`]. The control points
/// (and the weights) form a grid indexed `[u][v]`: one row per u index, every
/// row the same length.
#[derive(Debug, Clone, PartialEq)]
pub struct NurbsSurface {
    /// The polynomial degree in u; at least 1.
    pub degree_u: usize,
    /// The polynomial degree in v; at least 1.
    pub degree_v: usize,
    /// The control points, one row per u: at least `degree_u + 1` rows of
    /// at least `degree_v + 1` points each, every row the same length.
    pub control_points: Vec<Vec<[f64; 3]>>,
    /// One positive weight per control point, in the same grid, or `None`
    /// for a non-rational surface.
    pub weights: Option<Vec<Vec<f64>>>,
    /// The distinct u knot values, increasing.
    pub knots_u: Vec<f64>,
    /// How many times each u knot repeats.
    pub multiplicities_u: Vec<usize>,
    /// The distinct v knot values, increasing.
    pub knots_v: Vec<f64>,
    /// How many times each v knot repeats.
    pub multiplicities_v: Vec<usize>,
}

/// Split an expanded knot vector — each knot repeated as many times as its
/// multiplicity, as many kernels hold it — into the distinct knots and
/// multiplicities that [`NurbsCurve`] and [`NurbsSurface`] take.
///
/// Nothing is computed: runs of exactly equal neighbouring values become
/// one knot, written as given, and two values that differ in the last bit
/// stay two knots. `-0.0` and `0.0` are equal, so they become one knot (the
/// first of them); the file writes either as `0.`.
///
/// Nothing is checked either. A vector that breaks STEP's B-spline rules
/// gives knots and multiplicities that break them too, and the writer
/// rejects them with [`Error::InvalidNurbs`] when the curve or surface is
/// written. An expanded vector holds as many knots as the control points
/// plus `degree + 1`; one of the wrong length is reported as multiplicities
/// that do not sum to that. Some libraries, openNURBS among them, store the
/// vector without its first and last knot: repeat the first and the last
/// value once more before splitting it.
///
/// ```
/// use brep_to_step::{NurbsCurve, compress_knots};
///
/// let (knots, multiplicities) = compress_knots(&[0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]);
/// assert_eq!(knots, [0.0, 0.5, 1.0]);
/// assert_eq!(multiplicities, [4, 1, 4]);
///
/// let curve = NurbsCurve {
///     degree: 3,
///     control_points: vec![
///         [0.0, 0.0, 0.0],
///         [1.0, 2.0, 0.0],
///         [2.0, -1.0, 0.0],
///         [3.0, 1.0, 0.0],
///         [4.0, 0.0, 0.0],
///     ],
///     weights: None,
///     knots,
///     multiplicities,
/// };
/// # let _ = curve;
/// ```
#[must_use]
pub fn compress_knots(expanded: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let (mut knots, mut multiplicities) = (Vec::new(), Vec::new());
    for &k in expanded {
        match multiplicities.last_mut() {
            Some(m) if knots.last() == Some(&k) => *m += 1,
            _ => {
                knots.push(k);
                multiplicities.push(1);
            }
        }
    }
    (knots, multiplicities)
}

/// The reasons [`check_knots`] gives, worded for one parameter direction.
struct Rules {
    degree: &'static str,
    points: &'static str,
    counts: &'static str,
    two_knots: &'static str,
    finite: &'static str,
    increase: &'static str,
    at_least_one: &'static str,
    ends: &'static str,
    interior: &'static str,
    sum: &'static str,
}

const CURVE: Rules = Rules {
    degree: "degree must be at least 1",
    points: "needs at least degree + 1 control points",
    counts: "knots and multiplicities differ in count",
    two_knots: "needs at least two distinct knots",
    finite: "knots must be finite",
    increase: "knots must increase",
    at_least_one: "multiplicities must be at least 1",
    ends: "end multiplicities may be at most degree + 1",
    interior: "interior multiplicities may be at most degree",
    sum: "multiplicities must sum to control points + degree + 1",
};

const U: Rules = Rules {
    degree: "u degree must be at least 1",
    points: "needs at least u degree + 1 control point rows",
    counts: "u knots and multiplicities differ in count",
    two_knots: "needs at least two distinct u knots",
    finite: "u knots must be finite",
    increase: "u knots must increase",
    at_least_one: "u multiplicities must be at least 1",
    ends: "u end multiplicities may be at most degree + 1",
    interior: "u interior multiplicities may be at most degree",
    sum: "u multiplicities must sum to control point rows + degree + 1",
};

const V: Rules = Rules {
    degree: "v degree must be at least 1",
    points: "needs at least v degree + 1 control points per row",
    counts: "v knots and multiplicities differ in count",
    two_knots: "needs at least two distinct v knots",
    finite: "v knots must be finite",
    increase: "v knots must increase",
    at_least_one: "v multiplicities must be at least 1",
    ends: "v end multiplicities may be at most degree + 1",
    interior: "v interior multiplicities may be at most degree",
    sum: "v multiplicities must sum to control points per row + degree + 1",
};

fn invalid(reason: &'static str) -> Error {
    Error::InvalidNurbs { reason }
}

/// STEP's B-spline constraints on one parameter direction, for `points`
/// control points.
fn check_knots(
    rules: &Rules,
    degree: usize,
    points: usize,
    knots: &[f64],
    multiplicities: &[usize],
) -> Result<(), Error> {
    if degree < 1 {
        return Err(invalid(rules.degree));
    }
    // `points < degree + 1`, without overflow; past here `degree + 1` fits.
    if points <= degree {
        return Err(invalid(rules.points));
    }
    if knots.len() != multiplicities.len() {
        return Err(invalid(rules.counts));
    }
    if knots.len() < 2 {
        return Err(invalid(rules.two_knots));
    }
    if !knots.iter().all(|k| k.is_finite()) {
        return Err(invalid(rules.finite));
    }
    if !knots.windows(2).all(|w| w[0] < w[1]) {
        return Err(invalid(rules.increase));
    }
    if multiplicities.contains(&0) {
        return Err(invalid(rules.at_least_one));
    }
    let last = multiplicities.len() - 1;
    if multiplicities[0] > degree + 1 || multiplicities[last] > degree + 1 {
        return Err(invalid(rules.ends));
    }
    if multiplicities[1..last].iter().any(|&m| m > degree) {
        return Err(invalid(rules.interior));
    }
    if multiplicities.iter().sum::<usize>() != points + degree + 1 {
        return Err(invalid(rules.sum));
    }
    Ok(())
}

/// Weights must be finite and positive.
fn check_weights(weights: &[f64]) -> Result<(), Error> {
    match weights.iter().find(|w| !(w.is_finite() && **w > 0.0)) {
        Some(&value) => Err(Error::InvalidNumber {
            what: "weight",
            value,
        }),
        None => Ok(()),
    }
}

impl NurbsCurve {
    pub(crate) fn check(&self) -> Result<(), Error> {
        check_knots(
            &CURVE,
            self.degree,
            self.control_points.len(),
            &self.knots,
            &self.multiplicities,
        )?;
        for &p in &self.control_points {
            check_finite("control point", p)?;
        }
        if let Some(weights) = &self.weights {
            if weights.len() != self.control_points.len() {
                return Err(invalid("needs one weight per control point"));
            }
            check_weights(weights)?;
        }
        Ok(())
    }
}

impl NurbsSurface {
    pub(crate) fn check(&self) -> Result<(), Error> {
        let rows = self.control_points.len();
        let columns = self.control_points.first().map_or(0, Vec::len);
        if self.control_points.iter().any(|row| row.len() != columns) {
            return Err(invalid("control point rows differ in length"));
        }
        check_knots(
            &U,
            self.degree_u,
            rows,
            &self.knots_u,
            &self.multiplicities_u,
        )?;
        check_knots(
            &V,
            self.degree_v,
            columns,
            &self.knots_v,
            &self.multiplicities_v,
        )?;
        for &p in self.control_points.iter().flatten() {
            check_finite("control point", p)?;
        }
        if let Some(weights) = &self.weights {
            if weights.len() != rows || weights.iter().any(|row| row.len() != columns) {
                return Err(invalid("needs one weight per control point"));
            }
            for row in weights {
                check_weights(row)?;
            }
        }
        Ok(())
    }
}

/// A count already checked against the control points, which bound it.
fn int(n: usize) -> i64 {
    i64::try_from(n).expect("bounded by the number of control points")
}

fn ints(ns: &[usize]) -> Vec<i64> {
    ns.iter().map(|&n| int(n)).collect()
}

fn write_points(data: &mut Data, points: &[[f64; 3]]) -> Vec<Ref> {
    points.iter().map(|&p| write_point(data, p)).collect()
}

/// A `B_SPLINE_CURVE_WITH_KNOTS`, or for a rational curve the customary
/// complex instance with `RATIONAL_B_SPLINE_CURVE`.
pub(crate) fn write_curve(data: &mut Data, curve: &NurbsCurve) -> Ref {
    let points = write_points(data, &curve.control_points);
    let degree = int(curve.degree);
    let multiplicities = ints(&curve.multiplicities);
    match &curve.weights {
        None => data.simple(
            "B_SPLINE_CURVE_WITH_KNOTS",
            &[
                Param::Str(""),
                Param::Int(degree),
                Param::Refs(&points),
                Param::Enum("UNSPECIFIED"),
                Param::Enum("U"),
                Param::Enum("U"),
                Param::Ints(&multiplicities),
                Param::Reals(&curve.knots),
                Param::Enum("UNSPECIFIED"),
            ],
        ),
        Some(weights) => data.complex(&[
            ("BOUNDED_CURVE", &[]),
            (
                "B_SPLINE_CURVE",
                &[
                    Param::Int(degree),
                    Param::Refs(&points),
                    Param::Enum("UNSPECIFIED"),
                    Param::Enum("U"),
                    Param::Enum("U"),
                ],
            ),
            (
                "B_SPLINE_CURVE_WITH_KNOTS",
                &[
                    Param::Ints(&multiplicities),
                    Param::Reals(&curve.knots),
                    Param::Enum("UNSPECIFIED"),
                ],
            ),
            ("CURVE", &[]),
            ("GEOMETRIC_REPRESENTATION_ITEM", &[]),
            ("RATIONAL_B_SPLINE_CURVE", &[Param::Reals(weights)]),
            ("REPRESENTATION_ITEM", &[Param::Str("")]),
        ]),
    }
}

/// A `B_SPLINE_SURFACE_WITH_KNOTS`, or for a rational surface the customary
/// complex instance with `RATIONAL_B_SPLINE_SURFACE`.
pub(crate) fn write_surface(data: &mut Data, surface: &NurbsSurface) -> Ref {
    let rows: Vec<Vec<Ref>> = surface
        .control_points
        .iter()
        .map(|row| write_points(data, row))
        .collect();
    let grid = || Param::List(rows.iter().map(|row| Param::Refs(row)).collect());
    let (degree_u, degree_v) = (int(surface.degree_u), int(surface.degree_v));
    let multiplicities_u = ints(&surface.multiplicities_u);
    let multiplicities_v = ints(&surface.multiplicities_v);
    match &surface.weights {
        None => data.simple(
            "B_SPLINE_SURFACE_WITH_KNOTS",
            &[
                Param::Str(""),
                Param::Int(degree_u),
                Param::Int(degree_v),
                grid(),
                Param::Enum("UNSPECIFIED"),
                Param::Enum("U"),
                Param::Enum("U"),
                Param::Enum("U"),
                Param::Ints(&multiplicities_u),
                Param::Ints(&multiplicities_v),
                Param::Reals(&surface.knots_u),
                Param::Reals(&surface.knots_v),
                Param::Enum("UNSPECIFIED"),
            ],
        ),
        Some(weights) => data.complex(&[
            ("BOUNDED_SURFACE", &[]),
            (
                "B_SPLINE_SURFACE",
                &[
                    Param::Int(degree_u),
                    Param::Int(degree_v),
                    grid(),
                    Param::Enum("UNSPECIFIED"),
                    Param::Enum("U"),
                    Param::Enum("U"),
                    Param::Enum("U"),
                ],
            ),
            (
                "B_SPLINE_SURFACE_WITH_KNOTS",
                &[
                    Param::Ints(&multiplicities_u),
                    Param::Ints(&multiplicities_v),
                    Param::Reals(&surface.knots_u),
                    Param::Reals(&surface.knots_v),
                    Param::Enum("UNSPECIFIED"),
                ],
            ),
            ("GEOMETRIC_REPRESENTATION_ITEM", &[]),
            (
                "RATIONAL_B_SPLINE_SURFACE",
                &[Param::List(
                    weights.iter().map(|row| Param::Reals(row)).collect(),
                )],
            ),
            ("REPRESENTATION_ITEM", &[Param::Str("")]),
            ("SURFACE", &[]),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::compress_knots;
    use step_io::StepBuilder;
    use step_io::build::CurveInput;
    use step_io::parser::{Attribute, RawEntity, parse};
    use step_io::scene::NurbsCurve as StepIoNurbsCurve;

    /// The bits of each knot, so that -0 and +0 differ.
    fn bits(knots: &[f64]) -> Vec<u64> {
        knots.iter().map(|k| k.to_bits()).collect()
    }

    /// STEP's distinct knots and multiplicities, expanded back.
    fn expand(knots: &[f64], multiplicities: &[usize]) -> Vec<f64> {
        knots
            .iter()
            .zip(multiplicities)
            .flat_map(|(&k, &m)| std::iter::repeat_n(k, m))
            .collect()
    }

    #[test]
    fn runs_of_equal_values_become_one_knot() {
        let (knots, multiplicities) =
            compress_knots(&[0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]);
        assert_eq!(bits(&knots), bits(&[0.0, 0.5, 1.0]));
        assert_eq!(multiplicities, [4, 1, 4]);
        let (knots, multiplicities) = compress_knots(&[2.5, 2.5, 7.0, 7.0, 7.0]);
        assert_eq!(bits(&knots), bits(&[2.5, 7.0]));
        assert_eq!(multiplicities, [2, 3]);
    }

    /// Only neighbours are compared, and only exactly: whatever is wrong
    /// with the input is still there for the writer's checks to find.
    #[test]
    fn nothing_is_tidied_up() {
        assert_eq!(compress_knots(&[]), (Vec::new(), Vec::new()));
        let (knots, multiplicities) = compress_knots(&[0.0, 1.0, 0.0]);
        assert_eq!(bits(&knots), bits(&[0.0, 1.0, 0.0]));
        assert_eq!(multiplicities, [1, 1, 1]);
        let (knots, multiplicities) = compress_knots(&[f64::NAN, f64::NAN]);
        assert!(knots.iter().all(|k| k.is_nan()));
        assert_eq!(multiplicities, [1, 1]);
        let next = f64::from_bits(0.5f64.to_bits() + 1);
        let (knots, multiplicities) = compress_knots(&[0.5, next]);
        assert_eq!(bits(&knots), bits(&[0.5, next]));
        assert_eq!(multiplicities, [1, 1]);
    }

    /// -0 and +0 compare equal, so they are one knot, the first one given.
    #[test]
    fn signed_zeros_are_one_knot() {
        let (knots, multiplicities) = compress_knots(&[-0.0, 0.0, 1.0, 1.0]);
        assert_eq!(bits(&knots), bits(&[-0.0, 1.0]));
        assert_eq!(multiplicities, [2, 2]);
    }

    #[test]
    fn expanding_the_result_gives_the_input_back() {
        for expanded in [
            vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            vec![-3.25, -3.25, 0.1, 0.1, 1e300, 1e300, 1e300],
        ] {
            let (knots, multiplicities) = compress_knots(&expanded);
            assert_eq!(bits(&expand(&knots, &multiplicities)), bits(&expanded));
        }
    }

    /// The knots and multiplicities step-io writes for a B-spline edge given
    /// the expanded vector `knots`, read off its `B_SPLINE_CURVE_WITH_KNOTS`.
    fn step_io_knots(degree: usize, points: usize, knots: &[f64]) -> (Vec<f64>, Vec<usize>) {
        let mut b = StepBuilder::new();
        b.header_mut().time_stamp = String::new();
        let start = b.vertex([0.0; 3]).expect("vertex");
        #[allow(clippy::cast_precision_loss)] // a handful of points
        let control_points: Vec<[f64; 3]> = (0..points).map(|i| [i as f64, 0.0, 0.0]).collect();
        let end = b.vertex(control_points[points - 1]).expect("vertex");
        let curve = StepIoNurbsCurve {
            degree,
            control_points,
            weights: vec![1.0; points],
            knots: knots.to_vec(),
        };
        b.edge(start, end, CurveInput::Nurbs(curve)).expect("edge");
        let mut out = Vec::new();
        step_io::write(&b.finish(), &mut out).expect("write");
        let graph = parse(out.as_slice()).expect("parse");
        let attributes = graph
            .entities
            .values()
            .find_map(|e| match e {
                RawEntity::Simple {
                    name, attributes, ..
                } if name == "B_SPLINE_CURVE_WITH_KNOTS" => Some(attributes),
                _ => None,
            })
            .expect("a B-spline curve");
        let (Attribute::List(multiplicities), Attribute::List(knots)) =
            (&attributes[6], &attributes[7])
        else {
            panic!("unexpected attributes {attributes:?}");
        };
        let multiplicities = multiplicities
            .iter()
            .map(|m| match m {
                Attribute::Integer(m) => usize::try_from(*m).expect("positive"),
                other => panic!("not an integer: {other:?}"),
            })
            .collect();
        let knots = knots
            .iter()
            .map(|k| match k {
                Attribute::Real(k) => *k,
                other => panic!("not a real: {other:?}"),
            })
            .collect();
        (knots, multiplicities)
    }

    /// step-io takes the expanded vector itself and splits it the same way.
    #[test]
    fn splits_as_step_io_does() {
        for (degree, points, expanded) in [
            (3, 6, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0]),
            (2, 4, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
        ] {
            let (knots, multiplicities) = compress_knots(&expanded);
            let (theirs, their_multiplicities) = step_io_knots(degree, points, &expanded);
            assert_eq!(bits(&knots), bits(&theirs), "{expanded:?}");
            assert_eq!(multiplicities, their_multiplicities, "{expanded:?}");
        }
    }
}

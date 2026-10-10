//! A shape recipe as plain data, and two players: one writes it with
//! brep-to-step, the other with step-io's `StepBuilder`, so the two files
//! can be compared.

use brep_to_step::{
    Bound, Curve, Edge, Face, Frame, Header, NurbsCurve, NurbsSurface, Orientation, Part, Profile,
    StepWriter, Surface, Units, Vertex, VoidShellNormals,
};
use step_io::StepBuilder;
use step_io::build::{
    CurveInput, FaceBoundInput, HeaderInput, NurbsCurve as StepIoNurbsCurve,
    NurbsSurface as StepIoNurbsSurface, ProfileInput, SurfaceInput,
};

/// The header time stamp both players write.
pub const STAMP: &str = "2026-10-07T12:00:00";

/// Shapes by index: edges name vertices, faces name edges, solids name faces.
#[derive(Debug, Clone)]
pub struct Scene {
    pub parts: Vec<PartSpec>,
    pub vertices: Vec<[f64; 3]>,
    pub edges: Vec<EdgeSpec>,
    pub faces: Vec<FaceSpec>,
}

#[derive(Debug, Clone)]
pub struct PartSpec {
    pub name: String,
    pub solids: Vec<SolidSpec>,
}

/// A solid: the faces of its outer shell, and of each void shell.
#[derive(Debug, Clone)]
pub struct SolidSpec {
    pub faces: Vec<usize>,
    pub voids: Vec<Vec<usize>>,
    /// How the void shells are wound; unused without voids.
    pub normals: VoidShellNormals,
}

impl SolidSpec {
    /// A solid without voids.
    pub fn plain(faces: Vec<usize>) -> Self {
        Self {
            faces,
            voids: Vec::new(),
            normals: VoidShellNormals::AwayFromMaterial,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EdgeSpec {
    pub start: usize,
    pub end: usize,
    pub curve: Curve,
}

#[derive(Debug, Clone)]
pub struct FaceSpec {
    pub surface: Surface,
    pub same_sense: bool,
    pub bounds: Vec<BoundSpec>,
}

#[derive(Debug, Clone)]
pub struct BoundSpec {
    pub outer: bool,
    /// Edge index, and whether the loop runs along the edge.
    pub edges: Vec<(usize, bool)>,
}

impl Scene {
    /// Append `other`'s vertices, edges, faces, and parts, shifting the
    /// indices inside them to their new places.
    pub fn merge(&mut self, other: Scene) {
        let (dv, de, df) = (self.vertices.len(), self.edges.len(), self.faces.len());
        self.vertices.extend(other.vertices);
        self.edges.extend(other.edges.into_iter().map(|e| EdgeSpec {
            start: e.start + dv,
            end: e.end + dv,
            curve: e.curve,
        }));
        self.faces.extend(other.faces.into_iter().map(|f| {
            FaceSpec {
                bounds: f
                    .bounds
                    .into_iter()
                    .map(|b| BoundSpec {
                        outer: b.outer,
                        edges: b.edges.into_iter().map(|(i, fwd)| (i + de, fwd)).collect(),
                    })
                    .collect(),
                ..f
            }
        }));
        let shift = |faces: Vec<usize>| faces.into_iter().map(|i| i + df).collect();
        self.parts.extend(other.parts.into_iter().map(|p| {
            PartSpec {
                name: p.name,
                solids: p
                    .solids
                    .into_iter()
                    .map(|s| SolidSpec {
                        faces: shift(s.faces),
                        voids: s.voids.into_iter().map(shift).collect(),
                        normals: s.normals,
                    })
                    .collect(),
            }
        }));
    }
}

/// The brep-to-step handles a [`replay`] made, by scene index.
pub struct Handles {
    pub parts: Vec<Part>,
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
}

/// A recipe's flag as brep-to-step takes it. Recipes keep step-io's booleans,
/// so the two players are given the same flags and the differential
/// comparison catches a conversion that reads them the wrong way round.
fn orientation(forward: bool) -> Orientation {
    if forward {
        Orientation::Forward
    } else {
        Orientation::Reversed
    }
}

/// Write `scene` into `w`: parts, vertices, edges, faces, then solids.
pub fn replay<W: std::io::Write>(w: &mut StepWriter<W>, scene: &Scene) -> Handles {
    let parts: Vec<Part> = scene.parts.iter().map(|p| w.part(&p.name)).collect();
    let vertices: Vec<Vertex> = scene
        .vertices
        .iter()
        .map(|&p| w.vertex(p).expect("vertex"))
        .collect();
    let edges: Vec<Edge> = scene
        .edges
        .iter()
        .map(|e| {
            w.edge(vertices[e.start], vertices[e.end], e.curve.clone())
                .expect("edge")
        })
        .collect();
    let faces: Vec<Face> = scene
        .faces
        .iter()
        .map(|f| {
            let bounds: Vec<Bound> = f
                .bounds
                .iter()
                .map(|b| {
                    let loop_edges = b
                        .edges
                        .iter()
                        .map(|&(i, forward)| (edges[i], orientation(forward)))
                        .collect();
                    if b.outer {
                        Bound::outer(loop_edges)
                    } else {
                        Bound::inner(loop_edges)
                    }
                })
                .collect();
            w.face(f.surface.clone(), orientation(f.same_sense), &bounds)
                .expect("face")
        })
        .collect();
    for (part, spec) in parts.iter().zip(&scene.parts) {
        for solid in &spec.solids {
            let pick =
                |indices: &[usize]| -> Vec<Face> { indices.iter().map(|&i| faces[i]).collect() };
            let shell = pick(&solid.faces);
            if solid.voids.is_empty() {
                w.solid(*part, &shell).expect("solid");
            } else {
                let voids: Vec<Vec<Face>> = solid.voids.iter().map(|v| pick(v)).collect();
                w.solid_with_voids(*part, &shell, &voids, solid.normals)
                    .expect("solid with voids");
            }
        }
    }
    Handles {
        parts,
        vertices,
        edges,
        faces,
    }
}

/// The header both players write.
pub fn header() -> Header {
    Header {
        timestamp: STAMP.to_owned(),
        ..Default::default()
    }
}

/// `scene` written by brep-to-step.
pub fn write_ours(scene: &Scene) -> String {
    let mut w = StepWriter::new(Vec::new(), &header(), Units::default()).expect("writer");
    replay(&mut w, scene);
    w.finish_to_string().expect("finish")
}

/// `scene` written by step-io's `StepBuilder`, call for call.
pub fn write_step_io(scene: &Scene) -> String {
    let mut b = StepBuilder::new().expect("builder");
    b.header(&HeaderInput {
        timestamp: Some(STAMP.to_owned()),
        ..Default::default()
    });
    let parts: Vec<_> = scene
        .parts
        .iter()
        .map(|p| b.part(&p.name).expect("part"))
        .collect();
    let vertices: Vec<_> = scene
        .vertices
        .iter()
        .map(|&p| b.vertex(p).expect("vertex"))
        .collect();
    let edges: Vec<_> = scene
        .edges
        .iter()
        .map(|e| {
            b.edge(vertices[e.start], vertices[e.end], step_io_curve(&e.curve))
                .expect("edge")
        })
        .collect();
    let faces: Vec<_> = scene
        .faces
        .iter()
        .map(|f| {
            let bounds = f
                .bounds
                .iter()
                .map(|bound| {
                    let loop_edges = bound
                        .edges
                        .iter()
                        .map(|&(i, fwd)| (edges[i], fwd))
                        .collect();
                    if bound.outer {
                        FaceBoundInput::outer(loop_edges)
                    } else {
                        FaceBoundInput::inner(loop_edges)
                    }
                })
                .collect();
            b.face(step_io_surface(&f.surface), f.same_sense, bounds)
                .expect("face")
        })
        .collect();
    for (part, spec) in parts.iter().zip(&scene.parts) {
        for solid in &spec.solids {
            let pick =
                |indices: &[usize]| -> Vec<_> { indices.iter().map(|&i| faces[i]).collect() };
            // brep-to-step writes every solid name empty.
            if solid.voids.is_empty() {
                b.solid(*part, "", pick(&solid.faces)).expect("solid");
            } else {
                let voids = solid.voids.iter().map(|v| pick(v)).collect();
                let normals = match solid.normals {
                    VoidShellNormals::AwayFromMaterial => {
                        step_io::build::VoidShellNormals::AwayFromMaterial
                    }
                    VoidShellNormals::TowardMaterial => {
                        step_io::build::VoidShellNormals::TowardMaterial
                    }
                };
                b.solid_with_voids(*part, "", pick(&solid.faces), voids, normals)
                    .expect("solid with voids");
            }
        }
    }
    b.finish().expect("finish")
}

fn step_io_frame(f: &Frame) -> step_io::build::Frame {
    step_io::build::Frame {
        origin: f.origin,
        axis: f.axis,
        ref_dir: f.ref_dir,
    }
}

fn step_io_curve(curve: &Curve) -> CurveInput {
    match curve {
        Curve::Line => CurveInput::Line,
        Curve::LineAlong(direction) => CurveInput::LineAlong(*direction),
        Curve::Circle { frame, radius } => CurveInput::Circle(step_io_frame(frame), *radius),
        Curve::Ellipse {
            frame,
            semi_axis_1,
            semi_axis_2,
        } => CurveInput::Ellipse(step_io_frame(frame), *semi_axis_1, *semi_axis_2),
        Curve::Polyline(points) => CurveInput::Polyline(points.clone()),
        Curve::Nurbs(curve) => CurveInput::Nurbs(step_io_nurbs_curve(curve)),
        other => panic!("no step-io mapping yet for {other:?}"),
    }
}

fn step_io_surface(surface: &Surface) -> SurfaceInput {
    match surface {
        Surface::Plane(frame) => SurfaceInput::Plane(step_io_frame(frame)),
        Surface::Cylinder { frame, radius } => {
            SurfaceInput::Cylinder(step_io_frame(frame), *radius)
        }
        Surface::Sphere { frame, radius } => SurfaceInput::Sphere(step_io_frame(frame), *radius),
        Surface::Torus {
            frame,
            major_radius,
            minor_radius,
        } => SurfaceInput::Torus(step_io_frame(frame), *major_radius, *minor_radius),
        Surface::Cone {
            frame,
            radius,
            semi_angle,
        } => SurfaceInput::Cone(step_io_frame(frame), *radius, *semi_angle),
        Surface::LinearExtrusion { profile, sweep } => {
            SurfaceInput::LinearExtrusion(step_io_profile(profile), *sweep)
        }
        Surface::Revolution {
            profile,
            axis_origin,
            axis_direction,
        } => SurfaceInput::Revolution(step_io_profile(profile), *axis_origin, *axis_direction),
        Surface::Nurbs(surface) => SurfaceInput::Nurbs(step_io_nurbs_surface(surface)),
        other => panic!("no step-io mapping yet for {other:?}"),
    }
}

fn step_io_profile(profile: &Profile) -> ProfileInput {
    match profile {
        Profile::Line { point, direction } => ProfileInput::Line(*point, *direction),
        Profile::Circle { frame, radius } => ProfileInput::Circle(step_io_frame(frame), *radius),
        Profile::Ellipse {
            frame,
            semi_axis_1,
            semi_axis_2,
        } => ProfileInput::Ellipse(step_io_frame(frame), *semi_axis_1, *semi_axis_2),
        Profile::Nurbs(curve) => ProfileInput::Nurbs(step_io_nurbs_curve(curve)),
        other => panic!("no step-io mapping yet for {other:?}"),
    }
}

/// STEP's distinct knots and multiplicities as the expanded knot vector
/// step-io takes (it compresses them back the same way).
fn expand(knots: &[f64], multiplicities: &[usize]) -> Vec<f64> {
    knots
        .iter()
        .zip(multiplicities)
        .flat_map(|(&k, &m)| std::iter::repeat_n(k, m))
        .collect()
}

/// step-io takes weights always, and writes all-1.0 as non-rational.
fn step_io_nurbs_curve(curve: &NurbsCurve) -> StepIoNurbsCurve {
    StepIoNurbsCurve {
        degree: curve.degree,
        control_points: curve.control_points.clone(),
        weights: curve
            .weights
            .clone()
            .unwrap_or_else(|| vec![1.0; curve.control_points.len()]),
        knots: expand(&curve.knots, &curve.multiplicities),
    }
}

fn step_io_nurbs_surface(surface: &NurbsSurface) -> StepIoNurbsSurface {
    StepIoNurbsSurface {
        degree_u: surface.degree_u,
        degree_v: surface.degree_v,
        control_points: surface.control_points.clone(),
        weights: surface.weights.clone().unwrap_or_else(|| {
            surface
                .control_points
                .iter()
                .map(|row| vec![1.0; row.len()])
                .collect()
        }),
        knots_u: expand(&surface.knots_u, &surface.multiplicities_u),
        knots_v: expand(&surface.knots_v, &surface.multiplicities_v),
    }
}

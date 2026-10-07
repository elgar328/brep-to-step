//! A shape recipe as plain data, and two players: one writes it with
//! brep-to-step, the other with step-io's `StepBuilder`, so the two files
//! can be compared.

use brep_to_step::{
    Bound, Curve, Edge, Face, Frame, Header, Part, StepWriter, Surface, Units, Vertex,
};
use step_io::StepBuilder;
use step_io::build::{CurveInput, FaceBoundInput, HeaderInput, SurfaceInput};

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
    /// Each solid as the faces of its shell.
    pub solids: Vec<Vec<usize>>,
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

/// The brep-to-step handles a [`replay`] made, by scene index.
pub struct Handles {
    pub parts: Vec<Part>,
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
}

/// Write `scene` into `w`: parts, vertices, edges, faces, then solids.
pub fn replay(w: &mut StepWriter, scene: &Scene) -> Handles {
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
                    let loop_edges = b.edges.iter().map(|&(i, fwd)| (edges[i], fwd)).collect();
                    if b.outer {
                        Bound::outer(loop_edges)
                    } else {
                        Bound::inner(loop_edges)
                    }
                })
                .collect();
            w.face(f.surface.clone(), f.same_sense, &bounds)
                .expect("face")
        })
        .collect();
    for (part, spec) in parts.iter().zip(&scene.parts) {
        for solid in &spec.solids {
            let shell: Vec<Face> = solid.iter().map(|&i| faces[i]).collect();
            w.solid(*part, &shell).expect("solid");
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
    let mut w = StepWriter::new(Units::default()).expect("writer");
    replay(&mut w, scene);
    w.finish(&header()).expect("finish")
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
            let shell = solid.iter().map(|&i| faces[i]).collect();
            // brep-to-step writes every solid name empty.
            b.solid(*part, "", shell).expect("solid");
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
        other => panic!("no step-io mapping yet for {other:?}"),
    }
}

fn step_io_surface(surface: &Surface) -> SurfaceInput {
    match surface {
        Surface::Plane(frame) => SurfaceInput::Plane(step_io_frame(frame)),
        other => panic!("no step-io mapping yet for {other:?}"),
    }
}

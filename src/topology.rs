//! Topology entities: vertices, edges, loops, faces, shells, and solids.

use crate::geometry::write_point;
use crate::p21::{Data, Param, Ref};

/// A vertex written by [`StepWriter::vertex`](crate::StepWriter::vertex).
/// Valid only with the writer that made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Vertex {
    pub(crate) writer: u64,
    pub(crate) index: usize,
}

/// An edge written by [`StepWriter::edge`](crate::StepWriter::edge).
/// Valid only with the writer that made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge {
    pub(crate) writer: u64,
    pub(crate) entity: Ref,
}

/// A face written by [`StepWriter::face`](crate::StepWriter::face).
/// Valid only with the writer that made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Face {
    pub(crate) writer: u64,
    pub(crate) entity: Ref,
}

/// One boundary loop of a face: its edges in order, each with `true` if the
/// loop runs along the edge from its start vertex to its end vertex.
#[derive(Debug, Clone)]
pub struct Bound {
    pub(crate) edges: Vec<(Edge, bool)>,
    pub(crate) outer: bool,
}

impl Bound {
    /// The face's outer boundary. A face has at most one.
    #[must_use]
    pub fn outer(edges: Vec<(Edge, bool)>) -> Self {
        Self { edges, outer: true }
    }

    /// An inner boundary — a hole in the face.
    #[must_use]
    pub fn inner(edges: Vec<(Edge, bool)>) -> Self {
        Self {
            edges,
            outer: false,
        }
    }
}

pub(crate) fn write_vertex(data: &mut Data, p: [f64; 3]) -> Ref {
    let point = write_point(data, p);
    data.simple("VERTEX_POINT", &[Param::Str(""), Param::Ref(point)])
}

pub(crate) fn write_edge(data: &mut Data, start: Ref, end: Ref, curve: Ref) -> Ref {
    data.simple(
        "EDGE_CURVE",
        &[
            Param::Str(""),
            Param::Ref(start),
            Param::Ref(end),
            Param::Ref(curve),
            Param::Bool(true),
        ],
    )
}

pub(crate) fn write_face(data: &mut Data, surface: Ref, same_sense: bool, bounds: &[Bound]) -> Ref {
    let mut face_bounds = Vec::with_capacity(bounds.len());
    for bound in bounds {
        let oriented: Vec<Ref> = bound
            .edges
            .iter()
            .map(|(edge, forward)| {
                data.simple(
                    "ORIENTED_EDGE",
                    &[
                        Param::Str(""),
                        Param::Derived,
                        Param::Derived,
                        Param::Ref(edge.entity),
                        Param::Bool(*forward),
                    ],
                )
            })
            .collect();
        let edge_loop = data.simple("EDGE_LOOP", &[Param::Str(""), Param::Refs(&oriented)]);
        let kind = if bound.outer {
            "FACE_OUTER_BOUND"
        } else {
            "FACE_BOUND"
        };
        face_bounds.push(data.simple(
            kind,
            &[Param::Str(""), Param::Ref(edge_loop), Param::Bool(true)],
        ));
    }
    data.simple(
        "ADVANCED_FACE",
        &[
            Param::Str(""),
            Param::Refs(&face_bounds),
            Param::Ref(surface),
            Param::Bool(same_sense),
        ],
    )
}

pub(crate) fn write_solid(data: &mut Data, faces: &[Ref]) -> Ref {
    let shell = data.simple("CLOSED_SHELL", &[Param::Str(""), Param::Refs(faces)]);
    data.simple("MANIFOLD_SOLID_BREP", &[Param::Str(""), Param::Ref(shell)])
}

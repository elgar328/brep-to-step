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

/// Which way a face, or an edge in a face's loop, is used relative to the
/// geometry it lies on.
///
/// For a face this is STEP's `ADVANCED_FACE.same_sense`; for an edge in a
/// loop, `ORIENTED_EDGE.orientation`. Both are booleans in STEP, so there are
/// only ever these two values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// A face: its normal is its surface's normal. An edge in a loop: the
    /// loop runs along the edge from its start vertex to its end vertex.
    Forward,
    /// A face: its normal is against its surface's normal. An edge in a loop:
    /// the loop runs along the edge from its end vertex back to its start.
    Reversed,
}

impl Orientation {
    /// The STEP boolean: `.T.` for [`Forward`](Self::Forward).
    pub(crate) fn is_forward(self) -> bool {
        self == Self::Forward
    }
}

/// One boundary loop of a face: its edges in order, each with the
/// [`Orientation`] in which the loop runs along it.
#[derive(Debug, Clone)]
pub struct Bound {
    pub(crate) edges: Vec<(Edge, Orientation)>,
    pub(crate) outer: bool,
}

impl Bound {
    /// The face's outer boundary. A face has at most one.
    #[must_use]
    pub fn outer(edges: Vec<(Edge, Orientation)>) -> Self {
        Self { edges, outer: true }
    }

    /// An inner boundary — a hole in the face.
    #[must_use]
    pub fn inner(edges: Vec<(Edge, Orientation)>) -> Self {
        Self {
            edges,
            outer: false,
        }
    }
}

/// Which way the face normals of a solid's void shells point. Declaring it
/// lets the writer orient each cavity without evaluating geometry.
///
/// STEP orients every bounding shell so that its face normals point away
/// from the material: out into free space for the outer shell, and into the
/// empty cavity for a void. Follow the normal of a cavity wall: if it points
/// into the empty cavity, choose [`AwayFromMaterial`](Self::AwayFromMaterial);
/// if it points into the surrounding solid, choose
/// [`TowardMaterial`](Self::TowardMaterial). The wrong choice turns the void
/// inside out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoidShellNormals {
    /// The faces already point into the cavity, following the same rule as
    /// the outer shell. Most kernels produce cavities this way, as a reversed
    /// shell. Written as given.
    AwayFromMaterial,
    /// The faces point into the surrounding material, as if the cavity were
    /// an ordinary solid facing outward. Written reversed.
    TowardMaterial,
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

pub(crate) fn write_face(
    data: &mut Data,
    surface: Ref,
    orientation: Orientation,
    bounds: &[Bound],
) -> Ref {
    let mut face_bounds = Vec::with_capacity(bounds.len());
    for bound in bounds {
        let oriented: Vec<Ref> = bound
            .edges
            .iter()
            .map(|(edge, along)| {
                data.simple(
                    "ORIENTED_EDGE",
                    &[
                        Param::Str(""),
                        Param::Derived,
                        Param::Derived,
                        Param::Ref(edge.entity),
                        Param::Bool(along.is_forward()),
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
            Param::Bool(orientation.is_forward()),
        ],
    )
}

pub(crate) fn write_solid(data: &mut Data, faces: &[Ref]) -> Ref {
    let shell = data.simple("CLOSED_SHELL", &[Param::Str(""), Param::Refs(faces)]);
    data.simple("MANIFOLD_SOLID_BREP", &[Param::Str(""), Param::Ref(shell)])
}

/// A solid with internal voids: the outer shell, and each void shell wrapped
/// in an `ORIENTED_CLOSED_SHELL` that keeps (`.T.`) or reverses (`.F.`) it.
pub(crate) fn write_solid_with_voids(
    data: &mut Data,
    outer: &[Ref],
    voids: &[Vec<Ref>],
    normals: VoidShellNormals,
) -> Ref {
    let outer = data.simple("CLOSED_SHELL", &[Param::Str(""), Param::Refs(outer)]);
    let keep = match normals {
        VoidShellNormals::AwayFromMaterial => true,
        VoidShellNormals::TowardMaterial => false,
    };
    let oriented: Vec<Ref> = voids
        .iter()
        .map(|faces| {
            let shell = data.simple("CLOSED_SHELL", &[Param::Str(""), Param::Refs(faces)]);
            data.simple(
                "ORIENTED_CLOSED_SHELL",
                &[
                    Param::Str(""),
                    Param::Derived,
                    Param::Ref(shell),
                    Param::Bool(keep),
                ],
            )
        })
        .collect();
    data.simple(
        "BREP_WITH_VOIDS",
        &[Param::Str(""), Param::Ref(outer), Param::Refs(&oriented)],
    )
}

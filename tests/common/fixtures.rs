//! Test shapes, built in code.

use std::collections::HashMap;

use brep_to_step::{Curve, Frame, Surface};

use super::scene::{BoundSpec, EdgeSpec, FaceSpec, PartSpec, Scene};

/// How a polyhedron's straight edges are given.
#[derive(Debug, Clone, Copy)]
pub enum LineKind {
    /// [`Curve::Line`]: the writer computes the direction from the vertices.
    FromVertices,
    /// [`Curve::LineAlong`]: the direction is passed in, as a kernel would.
    Along,
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

fn unit(v: [f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / len, v[1] / len, v[2] / len]
}

/// A closed polyhedron, one part with one solid. Each cycle lists a face's
/// vertices counter-clockwise as seen from outside. Edges are shared between
/// the two faces that meet at them, each running from its lower-numbered
/// vertex to its higher one; each face is a plane through its first vertex,
/// its normal from its first two sides, its reference direction along the
/// first side.
pub fn polyhedron(
    name: &str,
    vertices: Vec<[f64; 3]>,
    cycles: &[&[usize]],
    line: LineKind,
) -> Scene {
    let mut edges = Vec::new();
    let mut edge_index: HashMap<(usize, usize), usize> = HashMap::new();
    let mut faces = Vec::new();
    for cycle in cycles {
        let mut loop_edges = Vec::with_capacity(cycle.len());
        for k in 0..cycle.len() {
            let (a, b) = (cycle[k], cycle[(k + 1) % cycle.len()]);
            let key = (a.min(b), a.max(b));
            let i = *edge_index.entry(key).or_insert_with(|| {
                let (start, end) = key;
                let curve = match line {
                    LineKind::FromVertices => Curve::Line,
                    LineKind::Along => Curve::LineAlong(unit(sub(vertices[end], vertices[start]))),
                };
                edges.push(EdgeSpec { start, end, curve });
                edges.len() - 1
            });
            loop_edges.push((i, a < b));
        }
        let first = sub(vertices[cycle[1]], vertices[cycle[0]]);
        let second = sub(vertices[cycle[2]], vertices[cycle[1]]);
        let frame = Frame {
            origin: vertices[cycle[0]],
            axis: unit(cross(first, second)),
            ref_dir: unit(first),
        };
        faces.push(FaceSpec {
            surface: Surface::Plane(frame),
            same_sense: true,
            bounds: vec![BoundSpec {
                outer: true,
                edges: loop_edges,
            }],
        });
    }
    Scene {
        parts: vec![PartSpec {
            name: name.to_owned(),
            solids: vec![(0..faces.len()).collect()],
        }],
        vertices,
        edges,
        faces,
    }
}

/// An axis-aligned cube with its minimum corner at `origin`. Vertex `i` is
/// at `origin + size * (x, y, z)` for `i = x + 2y + 4z`.
pub fn cube(name: &str, origin: [f64; 3], size: f64, line: LineKind) -> Scene {
    let vertices = (0..8u8)
        .map(|i| {
            let bit = |k: u8| f64::from((i >> k) & 1);
            [
                origin[0] + size * bit(0),
                origin[1] + size * bit(1),
                origin[2] + size * bit(2),
            ]
        })
        .collect();
    polyhedron(
        name,
        vertices,
        &[
            &[0, 2, 3, 1],
            &[4, 5, 7, 6],
            &[0, 1, 5, 4],
            &[2, 6, 7, 3],
            &[0, 4, 6, 2],
            &[1, 3, 7, 5],
        ],
        line,
    )
}

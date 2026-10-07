//! Test shapes, built in code.

use std::collections::HashMap;

use brep_to_step::{Curve, Frame, Surface, VoidShellNormals};

use super::scene::{BoundSpec, EdgeSpec, FaceSpec, PartSpec, Scene, SolidSpec};

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
            solids: vec![SolidSpec::plain((0..faces.len()).collect())],
        }],
        vertices,
        edges,
        faces,
    }
}

/// An axis-aligned box with its minimum corner at `origin`. Vertex `i` is
/// at `origin + size * (x, y, z)` (component-wise) for `i = x + 2y + 4z`.
/// Face 0 is the bottom (`z = 0`), face 1 the top.
pub fn boxed(name: &str, origin: [f64; 3], size: [f64; 3], line: LineKind) -> Scene {
    let vertices = (0..8u8)
        .map(|i| {
            let bit = |k: u8| f64::from((i >> k) & 1);
            [
                origin[0] + size[0] * bit(0),
                origin[1] + size[1] * bit(1),
                origin[2] + size[2] * bit(2),
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

/// An axis-aligned cube; see [`boxed`].
pub fn cube(name: &str, origin: [f64; 3], size: f64, line: LineKind) -> Scene {
    boxed(name, origin, [size; 3], line)
}

/// `scene` with every face turned over — as a kernel reverses a shell to
/// make it a cavity: the face's sense flips, and each loop runs backwards.
pub fn inverted(mut scene: Scene) -> Scene {
    for face in &mut scene.faces {
        face.same_sense = !face.same_sense;
        for bound in &mut face.bounds {
            bound.edges.reverse();
            for (_, forward) in &mut bound.edges {
                *forward = !*forward;
            }
        }
    }
    scene
}

/// A frame at `origin` with its axis along +z.
fn upright(origin: [f64; 3]) -> Frame {
    Frame {
        origin,
        axis: [0.0, 0.0, 1.0],
        ref_dir: [1.0, 0.0, 0.0],
    }
}

/// A rim: the circle of `radius` about `centre` in the plane z = centre.z,
/// counter-clockwise about +z, as one closed edge or two half arcs. Pushes
/// its vertices and edges and returns the edges in counter-clockwise order.
fn rim(
    vertices: &mut Vec<[f64; 3]>,
    edges: &mut Vec<EdgeSpec>,
    centre: [f64; 3],
    radius: f64,
    arcs: usize,
) -> Vec<usize> {
    let [x, y, z] = centre;
    let circle = Curve::Circle {
        frame: upright(centre),
        radius,
    };
    let a = vertices.len();
    vertices.push([x + radius, y, z]);
    let first = edges.len();
    match arcs {
        1 => edges.push(EdgeSpec {
            start: a,
            end: a,
            curve: circle,
        }),
        2 => {
            vertices.push([x - radius, y, z]);
            edges.push(EdgeSpec {
                start: a,
                end: a + 1,
                curve: circle.clone(),
            });
            edges.push(EdgeSpec {
                start: a + 1,
                end: a,
                curve: circle,
            });
        }
        _ => panic!("a rim is one circle or two arcs"),
    }
    (first..edges.len()).collect()
}

/// A rim's edges as a loop: counter-clockwise about +z, or clockwise.
fn rim_loop(rim: &[usize], counter_clockwise: bool) -> Vec<(usize, bool)> {
    if counter_clockwise {
        rim.iter().map(|&e| (e, true)).collect()
    } else {
        rim.iter().rev().map(|&e| (e, false)).collect()
    }
}

/// An upright cylinder, as nacre writes one: a circle (or two arcs, for
/// `arcs_per_rim = 2`) round each rim, the two caps, and the side bounded
/// by the two rims — the bottom one its outer bound, the top one an inner.
pub fn cylinder(
    name: &str,
    centre: [f64; 3],
    radius: f64,
    height: f64,
    arcs_per_rim: usize,
) -> Scene {
    let top_centre = [centre[0], centre[1], centre[2] + height];
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let bottom = rim(&mut vertices, &mut edges, centre, radius, arcs_per_rim);
    let top = rim(&mut vertices, &mut edges, top_centre, radius, arcs_per_rim);
    let outer = |edges| vec![BoundSpec { outer: true, edges }];
    let faces = vec![
        FaceSpec {
            surface: Surface::Plane(Frame {
                axis: [0.0, 0.0, -1.0],
                ..upright(centre)
            }),
            same_sense: true,
            bounds: outer(rim_loop(&bottom, false)),
        },
        FaceSpec {
            surface: Surface::Plane(upright(top_centre)),
            same_sense: true,
            bounds: outer(rim_loop(&top, true)),
        },
        FaceSpec {
            surface: Surface::Cylinder {
                frame: upright(centre),
                radius,
            },
            same_sense: true,
            bounds: vec![
                BoundSpec {
                    outer: true,
                    edges: rim_loop(&bottom, true),
                },
                BoundSpec {
                    outer: false,
                    edges: rim_loop(&top, false),
                },
            ],
        },
    ];
    Scene {
        parts: vec![PartSpec {
            name: name.to_owned(),
            solids: vec![SolidSpec::plain(vec![0, 1, 2])],
        }],
        vertices,
        edges,
        faces,
    }
}

/// A box with a round hole straight through it, top to bottom.
pub fn plate_with_hole(name: &str, size: [f64; 3], hole_radius: f64) -> Scene {
    let mut scene = boxed(name, [0.0; 3], size, LineKind::Along);
    let bottom_centre = [size[0] / 2.0, size[1] / 2.0, 0.0];
    let top_centre = [size[0] / 2.0, size[1] / 2.0, size[2]];
    let bottom = rim(
        &mut scene.vertices,
        &mut scene.edges,
        bottom_centre,
        hole_radius,
        1,
    );
    let top = rim(
        &mut scene.vertices,
        &mut scene.edges,
        top_centre,
        hole_radius,
        1,
    );
    // A hole runs clockwise round the face it pierces, seen from outside.
    scene.faces[0].bounds.push(BoundSpec {
        outer: false,
        edges: rim_loop(&bottom, true),
    });
    scene.faces[1].bounds.push(BoundSpec {
        outer: false,
        edges: rim_loop(&top, false),
    });
    // The wall faces the axis, against the cylinder's own normal.
    scene.faces.push(FaceSpec {
        surface: Surface::Cylinder {
            frame: upright(bottom_centre),
            radius: hole_radius,
        },
        same_sense: false,
        bounds: vec![
            BoundSpec {
                outer: true,
                edges: rim_loop(&bottom, false),
            },
            BoundSpec {
                outer: false,
                edges: rim_loop(&top, true),
            },
        ],
    });
    let wall = scene.faces.len() - 1;
    scene.parts[0].solids[0].faces.push(wall);
    scene
}

/// A cube with a cube-shaped cavity, its cavity wound as `normals` says.
pub fn hollow_box(normals: VoidShellNormals) -> Scene {
    let mut scene = cube("hollow box", [0.0; 3], 10.0, LineKind::Along);
    let cavity = cube("cavity", [3.0; 3], 4.0, LineKind::Along);
    scene.merge(match normals {
        VoidShellNormals::AwayFromMaterial => inverted(cavity),
        VoidShellNormals::TowardMaterial => cavity,
    });
    let cavity = scene.parts.pop().expect("the cavity's part");
    let solid = &mut scene.parts[0].solids[0];
    solid.voids.push(cavity.solids[0].faces.clone());
    solid.normals = normals;
    scene
}

/// Two cubes apart, each its own part.
pub fn two_parts() -> Scene {
    let mut scene = cube("left", [0.0; 3], 1.0, LineKind::Along);
    scene.merge(cube("right", [2.0, 0.0, 0.0], 1.0, LineKind::Along));
    scene
}

/// Two cubes apart, both solids of one part.
pub fn two_solids_one_part() -> Scene {
    let mut scene = two_parts();
    let right = scene.parts.pop().expect("the right part");
    "pair".clone_into(&mut scene.parts[0].name);
    scene.parts[0].solids.extend(right.solids);
    scene
}

//! A minimal STEP AP242 exporter for B-rep CAD kernels.
//!
//! brep-to-step only writes, and it only writes shapes: geometry (points,
//! placements, [curves](Curve), and [surfaces](Surface), up to NURBS) and
//! topology (vertices, edges, faces, and solids, including solids with
//! voids). Around them it writes only the product, unit, and context
//! structure a valid AP242 edition 2 file requires. The API is
//! kernel-neutral: the inputs are plain values, so a small adapter in the
//! kernel can convert its own types into them.
//!
//! # Example
//!
//! Writing a unit cube from the vertices and face loops a kernel holds:
//!
//! ```
//! use std::collections::HashMap;
//!
//! use brep_to_step::{Bound, Curve, Frame, Header, StepWriter, Surface, Units};
//!
//! // Vertex i sits at (x, y, z), where i = x + 2y + 4z.
//! let points: Vec<[f64; 3]> = (0..8u8)
//!     .map(|i| [f64::from(i & 1), f64::from((i >> 1) & 1), f64::from(i >> 2)])
//!     .collect();
//! // Each face: its vertices, counter-clockwise as seen from outside, and its
//! // outward normal.
//! let faces = [
//!     ([0, 2, 3, 1], [0.0, 0.0, -1.0]),
//!     ([4, 5, 7, 6], [0.0, 0.0, 1.0]),
//!     ([0, 1, 5, 4], [0.0, -1.0, 0.0]),
//!     ([2, 6, 7, 3], [0.0, 1.0, 0.0]),
//!     ([0, 4, 6, 2], [-1.0, 0.0, 0.0]),
//!     ([1, 3, 7, 5], [1.0, 0.0, 0.0]),
//! ];
//!
//! let mut w = StepWriter::new(Units::default())?;
//! let part = w.part("cube");
//! let vertices = points
//!     .iter()
//!     .map(|&p| w.vertex(p))
//!     .collect::<Result<Vec<_>, _>>()?;
//!
//! // Every edge is shared by two faces: write it once, keep its handle (keyed
//! // here by its two vertices), and pass that handle to both faces.
//! let mut edges = HashMap::new();
//! let mut shell = Vec::new();
//! for (cycle, normal) in faces {
//!     let mut bound = Vec::new();
//!     for k in 0..4 {
//!         let (a, b) = (cycle[k], cycle[(k + 1) % 4]);
//!         let key = (a.min(b), a.max(b));
//!         let edge = match edges.get(&key) {
//!             Some(&edge) => edge,
//!             None => {
//!                 let edge = w.edge(vertices[key.0], vertices[key.1], Curve::Line)?;
//!                 edges.insert(key, edge);
//!                 edge
//!             }
//!         };
//!         // `true` if the loop runs along the edge from its start to its end.
//!         bound.push((edge, a < b));
//!     }
//!     let [p, q] = [points[cycle[0]], points[cycle[1]]];
//!     let plane = Frame {
//!         origin: p,
//!         axis: normal,
//!         ref_dir: [q[0] - p[0], q[1] - p[1], q[2] - p[2]],
//!     };
//!     shell.push(w.face(Surface::Plane(plane), true, &[Bound::outer(bound)])?);
//! }
//! w.solid(part, &shell)?;
//!
//! let step = w.finish(&Header {
//!     timestamp: "2026-10-08T12:00:00".to_owned(),
//!     originating_system: "my kernel".to_owned(),
//!     ..Header::default()
//! })?;
//! assert!(step.starts_with("ISO-10303-21;"));
//! # Ok::<(), brep_to_step::Error>(())
//! ```
//!
//! # Shapes are built from the bottom up
//!
//! Call [`StepWriter::vertex`], then [`edge`](StepWriter::edge) and
//! [`face`](StepWriter::face), and finally [`solid`](StepWriter::solid),
//! which adds the solid to a [`part`](StepWriter::part). Each call returns a
//! handle that later calls take. Every call writes new entities, so a vertex
//! or edge shared by several faces should be written once and its handle
//! reused. Only the caller knows which of its vertices and edges are the
//! same, so the caller keeps that map.
//!
//! # Values are written exactly
//!
//! Every `f64` is written in the shortest form that reads back to the same
//! bits. Nothing is normalized or recomputed: a kernel that knows an edge's
//! exact direction can pass it as [`Curve::LineAlong`], and the file keeps
//! that direction as it is.
//!
//! # Errors leave nothing behind
//!
//! Each call checks its input before writing anything. It rejects, among
//! other things, numbers that are not finite, zero directions, parallel frame
//! axes, straight edges of zero length, empty lists, NURBS data that breaks
//! STEP's B-spline rules, and handles from another writer. A call that
//! returns an [`Error`] writes nothing, so the writer stays usable. Whether
//! the geometry agrees with the topology (a vertex lying on its edge's curve,
//! a loop that closes) is not checked; that is up to the caller.
//!
//! # What is not written
//!
//! - Colours, names other than part names, metadata, assemblies, meshes, and
//!   PMI. These are permanently out of scope: they belong to a CAD
//!   application, not a kernel.
//! - Parameter-space curves (`PCURVE`). Edges carry only their 3D curves. A
//!   reader that needs a face's 2D curves has to compute them; readers built
//!   on Open CASCADE, FreeCAD among them, do so when they import.
//!
//! # Reproducible
//!
//! The same input always produces the same bytes. The header's time stamp is
//! whatever [`Header::timestamp`] holds, and is empty unless you set it.

mod ap;
mod context;
mod error;
mod geometry;
mod header;
mod nurbs;
mod p21;
mod product;
mod topology;
mod writer;

pub use context::{LengthUnit, Units};
pub use error::Error;
pub use geometry::{Curve, Frame, Profile, Surface};
pub use header::Header;
pub use nurbs::{NurbsCurve, NurbsSurface};
pub use product::Part;
pub use topology::{Bound, Edge, Face, Vertex, VoidShellNormals};
pub use writer::StepWriter;

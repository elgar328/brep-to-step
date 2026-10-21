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
//! use brep_to_step::{Bound, Curve, Frame, Header, Orientation, StepWriter, Surface, Units};
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
//! // The header comes first in the file, so the writer takes it at the start.
//! // A `Vec<u8>` gathers the text in memory; pass a `File` to write a file.
//! let header = Header {
//!     timestamp: "2026-10-08T12:00:00".to_owned(),
//!     originating_system: "my kernel".to_owned(),
//!     ..Header::default()
//! };
//! let mut w = StepWriter::new(Vec::new(), &header, Units::default())?;
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
//!         // The edge was written from its lower vertex to its higher one.
//!         let along = if a < b {
//!             Orientation::Forward
//!         } else {
//!             Orientation::Reversed
//!         };
//!         bound.push((edge, along));
//!     }
//!     let [p, q] = [points[cycle[0]], points[cycle[1]]];
//!     let plane = Frame {
//!         origin: p,
//!         axis: normal,
//!         ref_dir: [q[0] - p[0], q[1] - p[1], q[2] - p[2]],
//!     };
//!     let face = w.face(Surface::Plane(plane), Orientation::Forward, &[Bound::outer(bound)])?;
//!     shell.push(face);
//! }
//! w.solid(part, &shell)?;
//!
//! let step = w.finish_to_string()?;
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
//! same, so the caller keeps that map. A part given no solid is written as a
//! part with no shape.
//!
//! # Which way an edge runs
//!
//! An edge runs from its start vertex to its end vertex, and its curve must
//! run the same way. For some curves the writer works out that direction
//! from the vertices; the rest carry their own direction, which the caller
//! keeps in step:
//!
//! | Curve | Vertex order |
//! |---|---|
//! | [`Line`](Curve::Line), [`LineAlong`](Curve::LineAlong), [`Parabola`](Curve::Parabola), [`Hyperbola`](Curve::Hyperbola) | Either: the line's direction is computed or turned round, and a conic edge's sense is found from its vertices. |
//! | [`Circle`](Curve::Circle), [`Ellipse`](Curve::Ellipse) | The arc runs counter-clockwise about the frame's axis from start to end; negate the axis for the other arc. |
//! | [`Polyline`](Curve::Polyline), [`Nurbs`](Curve::Nurbs) | The start vertex where the curve starts (a polyline's first point), the end vertex where it ends. |
//!
//! A face whose loop crosses an edge the other way uses it with
//! [`Orientation::Reversed`]; that is how two faces share one edge.
//! A kernel that starts each edge where its curve's parameter is lowest can
//! pass its edges as they are.
//!
//! # Streaming
//!
//! The writer sends the file to its output as it goes, in chunks of 64 KiB,
//! instead of holding the text in memory. What it keeps grows only with the
//! vertices — 32 bytes each, for the edges between them that are worked out
//! from their vertices — and with the parts and their solids, a small
//! fraction of the file. To get the file as text, write to a `Vec<u8>` and
//! end with [`finish_to_string`](StepWriter::finish_to_string), as above;
//! the whole file is then in memory, so write a large one to a
//! [`File`](std::fs::File) instead.
//!
//! # Values are written exactly
//!
//! Every `f64` is written in the shortest form that reads back to the same
//! bits. No value given is normalized or recomputed: a kernel that knows an
//! edge's exact direction can pass it as [`Curve::LineAlong`], and the file
//! keeps that direction as it is.
//!
//! A few things are derived from the input rather than given: a
//! [`Curve::Line`]'s direction, from its vertices; a [`Curve::LineAlong`]
//! direction negated when it points from the end back to the start; an
//! extrusion's unit direction and length, from its sweep vector; and whether
//! a [`Curve::Parabola`] or [`Curve::Hyperbola`] edge runs the way its
//! curve's parameter grows, found from its vertices and written as the
//! edge's flag.
//!
//! # Errors leave nothing behind
//!
//! Each call checks its input before writing anything. It rejects, among
//! other things, numbers that are not finite, zero directions, parallel frame
//! axes, straight edges of zero length, parabola and hyperbola edges with
//! no span, empty lists, NURBS data that breaks
//! STEP's B-spline rules, and handles from another writer. A call that
//! returns an [`Error`] writes nothing, so the writer stays usable. Whether
//! the geometry agrees with the topology (a vertex lying on its edge's curve,
//! a loop that closes) is not checked; that is up to the caller.
//!
//! Writing to the output can fail too — a full disk, a closed pipe. That
//! failure is not the input's, so the calls go on returning `Ok`; the writer
//! skips the writes that would follow, and [`finish`](StepWriter::finish)
//! reports it as [`Error::Io`]. The output then holds an incomplete file to
//! discard. The file is complete only once `finish` returns `Ok`.
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
pub use topology::{Bound, Edge, Face, Orientation, Vertex, VoidShellNormals};
pub use writer::StepWriter;

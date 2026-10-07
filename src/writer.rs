//! [`StepWriter`], the entry point.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::context::{self, Skeleton, Units};
use crate::error::Error;
use crate::geometry::{self, Curve, Surface};
use crate::header::{self, Header};
use crate::p21::{Data, Ref};
use crate::product::{self, Part, PendingPart};
use crate::topology::{self, Bound, Edge, Face, Vertex, VoidShellNormals};

/// Gives every writer its own id, which its handles carry.
static NEXT_WRITER: AtomicU64 = AtomicU64::new(0);

/// Writes one STEP AP242 file.
///
/// [`new`](Self::new) lays down the skeleton every file carries; shapes are
/// built bottom up — [`vertex`](Self::vertex), [`edge`](Self::edge),
/// [`face`](Self::face), then [`solid`](Self::solid) into a
/// [`part`](Self::part) — and [`finish`](Self::finish) adds the header and
/// returns the file's text.
///
/// Each call writes new entities: a vertex or edge shared by several faces
/// is written once and its handle passed to each of them. A call that
/// returns an error writes nothing.
#[derive(Debug)]
pub struct StepWriter {
    id: u64,
    data: Data,
    skeleton: Skeleton,
    /// Each vertex's entity and position; straight edges need the position.
    vertices: Vec<(Ref, [f64; 3])>,
    parts: Vec<PendingPart>,
}

impl StepWriter {
    /// Start a file in the given units.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidNumber`] if `units.uncertainty` is not finite and
    /// positive.
    pub fn new(units: Units) -> Result<Self, Error> {
        let mut data = Data::new();
        let skeleton = context::write_skeleton(&mut data, units)?;
        Ok(Self {
            id: NEXT_WRITER.fetch_add(1, Ordering::Relaxed),
            data,
            skeleton,
            vertices: Vec::new(),
            parts: Vec::new(),
        })
    }

    /// Add a part — one product, named `name` — to hold solids.
    pub fn part(&mut self, name: &str) -> Part {
        let pending = product::write_part(&mut self.data, &self.skeleton, name);
        self.parts.push(pending);
        Part {
            writer: self.id,
            index: self.parts.len() - 1,
        }
    }

    /// Add a vertex at `point`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidNumber`] if a coordinate is not finite.
    pub fn vertex(&mut self, point: [f64; 3]) -> Result<Vertex, Error> {
        geometry::check_finite("vertex", point)?;
        let entity = topology::write_vertex(&mut self.data, point);
        self.vertices.push((entity, point));
        Ok(Vertex {
            writer: self.id,
            index: self.vertices.len() - 1,
        })
    }

    /// Add an edge from `start` to `end` along `curve`.
    ///
    /// # Errors
    ///
    /// [`Error::ForeignHandle`] for a vertex from another writer;
    /// [`Error::InvalidNumber`] or [`Error::ZeroVector`] for a bad line
    /// direction; [`Error::ZeroLengthLine`] for a straight edge whose
    /// vertices are at the same point.
    // Taken by value like every input: callers build one per call.
    #[allow(clippy::needless_pass_by_value)]
    pub fn edge(&mut self, start: Vertex, end: Vertex, curve: Curve) -> Result<Edge, Error> {
        self.check_writer(start.writer)?;
        self.check_writer(end.writer)?;
        let (start_entity, start_point) = self.vertices[start.index];
        let (end_entity, end_point) = self.vertices[end.index];
        curve.check(start_point, end_point)?;
        let geometry = geometry::write_curve(&mut self.data, &curve, start_point, end_point);
        let entity = topology::write_edge(&mut self.data, start_entity, end_entity, geometry);
        Ok(Edge {
            writer: self.id,
            entity,
        })
    }

    /// Add a face on `surface`, bounded by `bounds`. `same_sense` is `true`
    /// when the face's outward normal is the surface's own normal.
    ///
    /// # Errors
    ///
    /// [`Error::ForeignHandle`] for an edge from another writer;
    /// [`Error::Empty`] for no bounds or a bound with no edges;
    /// [`Error::MultipleOuterBounds`]; and the surface's own errors
    /// ([`Error::InvalidNumber`], [`Error::ZeroVector`],
    /// [`Error::ParallelAxes`]).
    #[allow(clippy::needless_pass_by_value)] // see `edge`
    pub fn face(
        &mut self,
        surface: Surface,
        same_sense: bool,
        bounds: &[Bound],
    ) -> Result<Face, Error> {
        surface.check()?;
        if bounds.is_empty() {
            return Err(Error::Empty {
                what: "face bounds",
            });
        }
        if bounds.iter().filter(|b| b.outer).count() > 1 {
            return Err(Error::MultipleOuterBounds);
        }
        for bound in bounds {
            if bound.edges.is_empty() {
                return Err(Error::Empty {
                    what: "bound edges",
                });
            }
            for (edge, _) in &bound.edges {
                self.check_writer(edge.writer)?;
            }
        }
        let geometry = geometry::write_surface(&mut self.data, &surface);
        let entity = topology::write_face(&mut self.data, geometry, same_sense, bounds);
        Ok(Face {
            writer: self.id,
            entity,
        })
    }

    /// Close `faces` into a shell and add the solid it bounds to `part`.
    ///
    /// # Errors
    ///
    /// [`Error::ForeignHandle`] for a part or face from another writer;
    /// [`Error::Empty`] for no faces.
    pub fn solid(&mut self, part: Part, faces: &[Face]) -> Result<(), Error> {
        self.check_writer(part.writer)?;
        if faces.is_empty() {
            return Err(Error::Empty {
                what: "solid faces",
            });
        }
        for face in faces {
            self.check_writer(face.writer)?;
        }
        let faces: Vec<Ref> = faces.iter().map(|f| f.entity).collect();
        let solid = topology::write_solid(&mut self.data, &faces);
        self.parts[part.index].solids.push(solid);
        Ok(())
    }

    /// Add a solid with internal voids to `part`: `outer` closes into its
    /// outer shell and each group in `voids` into a cavity shell, oriented
    /// as `normals` declares.
    ///
    /// # Errors
    ///
    /// [`Error::ForeignHandle`] for a part or face from another writer;
    /// [`Error::Empty`] for no outer faces, no voids, or a void with no
    /// faces.
    pub fn solid_with_voids(
        &mut self,
        part: Part,
        outer: &[Face],
        voids: &[Vec<Face>],
        normals: VoidShellNormals,
    ) -> Result<(), Error> {
        self.check_writer(part.writer)?;
        if outer.is_empty() {
            return Err(Error::Empty {
                what: "solid faces",
            });
        }
        if voids.is_empty() {
            return Err(Error::Empty { what: "voids" });
        }
        if voids.iter().any(Vec::is_empty) {
            return Err(Error::Empty { what: "void faces" });
        }
        for face in outer.iter().chain(voids.iter().flatten()) {
            self.check_writer(face.writer)?;
        }
        let outer: Vec<Ref> = outer.iter().map(|f| f.entity).collect();
        let voids: Vec<Vec<Ref>> = voids
            .iter()
            .map(|faces| faces.iter().map(|f| f.entity).collect())
            .collect();
        let solid = topology::write_solid_with_voids(&mut self.data, &outer, &voids, normals);
        self.parts[part.index].solids.push(solid);
        Ok(())
    }

    /// Finish the file and return its Part 21 text.
    ///
    /// # Errors
    ///
    /// [`Error::EmptyPart`] for a part with no solid;
    /// [`Error::HeaderTooLong`] if a `header` string is longer than Part 21
    /// allows.
    pub fn finish(mut self, header: &Header) -> Result<String, Error> {
        if let Some(part) = self.parts.iter().find(|p| p.solids.is_empty()) {
            return Err(Error::EmptyPart {
                name: part.name.clone(),
            });
        }
        header::check(header)?;
        product::write_shapes(&mut self.data, &self.skeleton, &self.parts);
        Ok(header::write_file(header, &self.data.into_body()))
    }

    fn check_writer(&self, writer: u64) -> Result<(), Error> {
        if writer == self.id {
            Ok(())
        } else {
            Err(Error::ForeignHandle)
        }
    }
}

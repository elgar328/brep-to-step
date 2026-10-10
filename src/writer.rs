//! [`StepWriter`], the entry point.

use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::context::{self, Skeleton, Units};
use crate::error::Error;
use crate::geometry::{self, Curve, Surface};
use crate::header::{self, Header};
use crate::p21::{Data, Ref};
use crate::product::{self, Part, PendingPart};
use crate::topology::{self, Bound, Edge, Face, Orientation, Vertex, VoidShellNormals};

/// Gives every writer its own id, which its handles carry.
static NEXT_WRITER: AtomicU64 = AtomicU64::new(0);

/// How much text the writer gathers before sending it to its output.
const CHUNK: usize = 64 * 1024;

/// Writes one STEP AP242 file to an [`io::Write`].
///
/// [`new`](Self::new) writes the header and the structure every file needs.
/// Shapes are then built from the bottom up: [`vertex`](Self::vertex),
/// [`edge`](Self::edge), [`face`](Self::face), and finally
/// [`solid`](Self::solid), which adds the solid to a [`part`](Self::part).
/// [`finish`](Self::finish) closes the file and returns the output.
///
/// The text goes to the output as it is written, in chunks of 64 KiB, so a
/// [`File`](std::fs::File) needs no `BufWriter`. The file is complete only
/// once `finish` returns `Ok`; a writer dropped before then leaves it cut
/// short.
///
/// Each call writes new entities, so write a vertex or edge shared by several
/// faces once and pass its handle to each face. A call rejected for its input
/// writes nothing, and the writer stays usable. A failure to write to the
/// output is reported by `finish` instead.
#[derive(Debug)]
pub struct StepWriter<W> {
    id: u64,
    out: W,
    /// The first failure to write to `out`; later writes are skipped.
    io_error: Option<io::Error>,
    data: Data,
    skeleton: Skeleton,
    /// Each vertex's entity and position; straight edges need the position.
    vertices: Vec<(Ref, [f64; 3])>,
    parts: Vec<PendingPart>,
}

impl<W: Write> StepWriter<W> {
    /// Start a file on `out` with `header` and in the given units.
    ///
    /// To write a file, pass it; to get the text instead, pass a `Vec<u8>`
    /// and end with [`finish_to_string`](StepWriter::finish_to_string):
    ///
    /// ```no_run
    /// use std::fs::File;
    ///
    /// use brep_to_step::{Header, StepWriter, Units};
    ///
    /// let file = File::create("part.step")?;
    /// let w = StepWriter::new(file, &Header::default(), Units::default())?;
    /// // ... parts and solids ...
    /// w.finish()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::HeaderTooLong`] if a `header` string is longer than Part 21
    /// allows; [`Error::InvalidNumber`] if `units.uncertainty` is not finite
    /// and positive. Either way nothing has been written to `out`.
    pub fn new(out: W, header: &Header, units: Units) -> Result<Self, Error> {
        header::check(header)?;
        let mut data = Data::new();
        header::write_header(&mut data, header);
        let skeleton = context::write_skeleton(&mut data, units)?;
        let mut writer = Self {
            id: NEXT_WRITER.fetch_add(1, Ordering::Relaxed),
            out,
            io_error: None,
            data,
            skeleton,
            vertices: Vec::new(),
            parts: Vec::new(),
        };
        writer.spill();
        Ok(writer)
    }

    /// Add a part named `name`: one product to hold solids. A part given no
    /// solid is written as a part with no shape.
    pub fn part(&mut self, name: &str) -> Part {
        let pending = product::write_part(&mut self.data, &self.skeleton, name);
        self.parts.push(pending);
        self.spill();
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
        self.spill();
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
    /// vertices are at the same point, and [`Error::InvalidNumber`] for one
    /// whose vertices are so close together or so far apart that its length
    /// cannot be computed.
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
        self.spill();
        Ok(Edge {
            writer: self.id,
            entity,
        })
    }

    /// Add a face on `surface`, bounded by `bounds`. `orientation` says
    /// whether the face's normal is the surface's normal or its reverse. On a
    /// solid's outer shell the face's normal must point out of the material,
    /// into free space; on a void's shell it may point either way, as
    /// declared by [`VoidShellNormals`].
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
        orientation: Orientation,
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
        let entity = topology::write_face(&mut self.data, geometry, orientation, bounds);
        self.spill();
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
        self.spill();
        Ok(())
    }

    /// Add a solid with internal voids to `part`. `outer` forms its outer
    /// shell and each group in `voids` a cavity shell, oriented as `normals`
    /// declares.
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
        self.spill();
        Ok(())
    }

    /// Close the file — each part's shape representation, then the end of
    /// the DATA section — send the rest to the output, flush it, and return
    /// it.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if writing to the output failed, here or in any earlier
    /// call. The output then holds an incomplete file and is not returned.
    pub fn finish(mut self) -> Result<W, Error> {
        product::write_shapes(&mut self.data, &self.skeleton, &self.parts);
        self.data.raw(header::FOOTER);
        self.send();
        if self.io_error.is_none() {
            if let Err(e) = self.out.flush() {
                self.io_error = Some(e);
            }
        }
        match self.io_error {
            Some(e) => Err(Error::Io(e)),
            None => Ok(self.out),
        }
    }

    /// Send the pending text on once a chunk has gathered.
    fn spill(&mut self) {
        if self.data.pending().len() >= CHUNK {
            self.send();
        }
    }

    /// Send the pending text to the output, unless an earlier write failed,
    /// and clear it either way so that it cannot pile up.
    fn send(&mut self) {
        if self.io_error.is_none() {
            if let Err(e) = self.out.write_all(self.data.pending().as_bytes()) {
                self.io_error = Some(e);
            }
        }
        self.data.clear();
    }

    fn check_writer(&self, writer: u64) -> Result<(), Error> {
        if writer == self.id {
            Ok(())
        } else {
            Err(Error::ForeignHandle)
        }
    }
}

impl StepWriter<Vec<u8>> {
    /// [`finish`](StepWriter::finish), and return the file as text.
    ///
    /// The whole file is then held in memory; a large file is better written
    /// to a [`File`](std::fs::File) as it goes.
    ///
    /// # Errors
    ///
    /// None in practice: writing to a `Vec<u8>` does not fail. The
    /// [`Error`] is `finish`'s.
    // The output is ASCII by construction — every STRING is escaped to
    // printable ASCII — so it is always UTF-8.
    #[allow(clippy::missing_panics_doc)]
    pub fn finish_to_string(self) -> Result<String, Error> {
        let bytes = self.finish()?;
        Ok(String::from_utf8(bytes).expect("Part 21 output is ASCII"))
    }
}

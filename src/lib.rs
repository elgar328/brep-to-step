//! Write B-rep solids from a CAD kernel as STEP AP242 (Edition 2) files.
//!
//! Write-only and shape-only: geometry (points, directions, placements,
//! curves, surfaces), topology (vertices through solids, including solids
//! with voids), and the minimal product, unit, and context structure a valid
//! AP242 file needs. Colours, names and metadata, assemblies, meshes, and PMI
//! are out of scope.
//!
//! The API is kernel-neutral: a kernel-specific adapter translates its own
//! types into the inputs here.

mod ap;
mod context;
mod error;
mod geometry;
mod header;
mod p21;
mod product;
mod topology;

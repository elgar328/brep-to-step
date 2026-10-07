# brep-to-step

Write B-rep solids from a CAD kernel as STEP AP242 (Edition 2) files.

- **Write-only, shape-only.** Geometry (points, directions, placements,
  curves, surfaces), topology (vertices through solids, including solids with
  voids), and the minimal product, unit, and context structure a valid AP242
  file needs.
- **Kernel-neutral.** A small adapter in the kernel translates its own types
  into the inputs here.
- **Out of scope, permanently:** colours, names and metadata, assemblies,
  meshes, and PMI. Those belong to a CAD application, not a kernel.
- **No runtime dependencies.**

Status: early development; the API is not settled yet.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

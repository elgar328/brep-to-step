# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-10-11

### Added

- `Error::Io` reports a failure to write to the output.
- `StepWriter::<Vec<u8>>::finish_to_string` ends a file written to memory
  and returns its text.
- `Orientation` (`Forward` or `Reversed`) says which way a face, or an edge in
  a face's loop, runs relative to the geometry it lies on.

### Changed

- `StepWriter` writes to any `io::Write`, such as a file or a `Vec<u8>`, and
  sends the text on in 64 KiB chunks instead of holding the whole file in a
  `String`.
- `StepWriter::new(out, &header, units)` takes the output and the header,
  since the header comes first in the file. Header strings over 256
  characters are therefore rejected by `new` rather than `finish`.
- `StepWriter::finish` returns the output instead of a `String`.
- A failure to write to the output does not make the calls that follow fail;
  `finish` reports it.
- A part with no solid is written as a part with no shape, as step-io writes
  it, rather than causing an error.
- `Error` no longer implements `Clone`, because it can hold an `io::Error`.
- `StepWriter::face` takes an `Orientation` instead of the `same_sense`
  boolean, and `Bound::outer` and `Bound::inner` take `(Edge, Orientation)`
  pairs instead of `(Edge, bool)`.

### Removed

- `Error::EmptyPart` is removed, since a part with no solid is no longer an
  error.

## [0.1.0] - 2026-10-08

### Added

- `StepWriter`, which writes B-rep solids as STEP AP242 (edition 2) files.
  Each part becomes one product, inside the application, product, and unit
  contexts a valid file needs. Every HEADER text field can be set.
- Shapes are built from the bottom up: `vertex`, `edge`, `face`, then `solid`
  or `solid_with_voids` into a `part`. Calls return handles, so a vertex or
  edge shared by several faces is written once.
- Curves: lines, circles, ellipses, polylines, and B-splines, rational or
  not. A line's direction is either computed from its vertices or given by
  the caller and kept bit for bit.
- Surfaces: planes, cylinders, cones, spheres, tori, linear extrusions,
  surfaces of revolution, and B-spline surfaces, rational or not.
- B-spline knots are given in STEP's own form (distinct values with their
  multiplicities), and whether a B-spline is rational is stated explicitly.
- Solids with voids; `VoidShellNormals` declares which way the void shells'
  faces point.
- Lengths in millimetres or metres, with a configurable length uncertainty.
- Every `f64` is written in the shortest form that reads back to the same
  bits.
- Non-ASCII text is escaped as `\X2\` / `\X4\`, so files are pure ASCII.
- Input is checked before anything is written, and a call that returns an
  error writes nothing. The checks cover non-finite numbers, zero
  directions, parallel frame axes, straight edges of zero length, empty
  lists, B-spline data that breaks STEP's rules, handles from another
  writer, parts with no solid, and header strings over 256 characters.
- Reproducible output: the same input always produces the same bytes.

[Unreleased]: https://github.com/elgar328/brep-to-step/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/elgar328/brep-to-step/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/elgar328/brep-to-step/releases/tag/v0.1.0

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `Error::Io`, for a failure to write to the output.
- `StepWriter::<Vec<u8>>::finish_to_string`, which ends a file written to
  memory and returns its text.
- `Orientation` (`Forward` or `Reversed`): which way a face, or an edge in a
  face's loop, is used relative to the geometry it lies on.

### Changed

- `StepWriter` writes to any `io::Write` — a file, a pipe, a `Vec<u8>` — and
  sends the file on in chunks of 64 KiB as it is written, instead of holding
  its text in a `String`. `StepWriter::new(out, &header, units)` takes the
  output and the header, which comes first in the file, so header strings
  over 256 characters are now rejected by `new`; `finish()` returns the
  output.
- A failure to write to the output does not fail the calls that follow:
  `finish` reports it.
- A part with no solid is written as a part with no shape, as step-io writes
  it, instead of being an error.
- `Error` no longer implements `Clone`, since it can hold an `io::Error`.
- `StepWriter::face` takes an `Orientation` instead of the `same_sense`
  boolean, and `Bound::outer` and `Bound::inner` take `(Edge, Orientation)`
  pairs instead of `(Edge, bool)`, so a call says which way it means.

### Removed

- `Error::EmptyPart`: a part with no solid is no longer an error.

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

[Unreleased]: https://github.com/elgar328/brep-to-step/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/elgar328/brep-to-step/releases/tag/v0.1.0

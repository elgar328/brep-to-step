# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

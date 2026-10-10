# brep-to-step

[![crates.io](https://img.shields.io/crates/v/brep-to-step.svg)](https://crates.io/crates/brep-to-step)
[![docs.rs](https://img.shields.io/docsrs/brep-to-step)](https://docs.rs/brep-to-step)
[![CI](https://github.com/elgar328/brep-to-step/actions/workflows/ci.yml/badge.svg)](https://github.com/elgar328/brep-to-step/actions/workflows/ci.yml)
[![license](https://img.shields.io/crates/l/brep-to-step.svg)](#license)

A minimal STEP AP242 exporter for B-rep CAD kernels.

> ⚠️ **Experimental** — early stage; expect breaking API changes.

## Features

- **Write-only, shape-only** — writes geometry and topology, plus only the
  product, unit, and context structure a valid AP242 edition 2 file
  requires.
- **Kernel-neutral** — takes plain inputs (`[f64; 3]`, frames, curves,
  surfaces), so a small adapter is all a kernel needs.
- **Exact** — every `f64` is written in the shortest form that reads back to
  the same bits; nothing is normalized or recomputed.
- **Fails cleanly** — input is checked before anything is written, so a call
  that returns an error leaves the file untouched.
- **Streaming** — writes the file as it goes instead of holding its text in
  memory.
- **Reproducible** — the same input always produces the same file.
- **No runtime dependencies.**

See the [documentation](https://docs.rs/brep-to-step) for usage and an
example.

## brep-to-step or step-io?

[step-io](https://crates.io/crates/step-io) is a full STEP library: it reads
all mainstream APs and writes AP242, including colours, assemblies, meshes,
and PMI. brep-to-step only writes a kernel's shapes, which keeps it small and
free of dependencies. Use brep-to-step to export a kernel's B-rep; use step-io
to read STEP, or to write more than shapes.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

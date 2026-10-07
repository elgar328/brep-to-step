//! Write every real test solid as a STEP file, for checking in other CAD
//! software.
//!
//! ```text
//! cargo run --example dump -- <dir>
//! ```
//!
//! Writes `<dir>/ours/<name>.step` (brep-to-step) and
//! `<dir>/reference/<name>.step` (the same shape written by step-io, to tell
//! a shape's own fault from the writer's), and `<dir>/expected.tsv`: each
//! shape's name, volume, face count, part count, and whether every face is
//! planar, to compare with what a CAD program measures. A reader that reads
//! without repairing can rebuild faces on curved surfaces only with help, so
//! the planar shapes are the ones such a strict reading can judge.
//!
//! The coverage shapes are left out: their shells are open, so a CAD program
//! rightly complains about them.

#[path = "../tests/common/mod.rs"]
mod common;

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use brep_to_step::Surface;
use common::fixtures::real_solids;
use common::scene::{Scene, write_ours, write_step_io};

/// Every face of every solid, voids included.
fn face_count(scene: &Scene) -> usize {
    scene
        .parts
        .iter()
        .flat_map(|p| &p.solids)
        .map(|s| s.faces.len() + s.voids.iter().map(Vec::len).sum::<usize>())
        .sum()
}

/// Whether every face lies on a plane.
fn planar(scene: &Scene) -> bool {
    scene
        .faces
        .iter()
        .all(|f| matches!(f.surface, Surface::Plane(_)))
}

fn dump(dir: &Path) -> std::io::Result<()> {
    let ours = dir.join("ours");
    let reference = dir.join("reference");
    std::fs::create_dir_all(&ours)?;
    std::fs::create_dir_all(&reference)?;
    let mut expected = String::from("name\tvolume\tfaces\tparts\tplanar\n");
    for solid in real_solids() {
        let file = format!("{}.step", solid.name);
        std::fs::write(ours.join(&file), write_ours(&solid.scene))?;
        std::fs::write(reference.join(&file), write_step_io(&solid.scene))?;
        let _ = writeln!(
            expected,
            "{}\t{:?}\t{}\t{}\t{}",
            solid.name,
            solid.volume,
            face_count(&solid.scene),
            solid.scene.parts.len(),
            planar(&solid.scene)
        );
    }
    std::fs::write(dir.join("expected.tsv"), expected)
}

fn main() -> ExitCode {
    let Some(dir) = std::env::args_os().nth(1) else {
        eprintln!("usage: cargo run --example dump -- <dir>");
        return ExitCode::from(2);
    };
    match dump(Path::new(&dir)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dump failed: {e}");
            ExitCode::FAILURE
        }
    }
}

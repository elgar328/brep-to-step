//! Every test shape, written by brep-to-step and by step-io: the two files
//! must have the same structure, and step-io must read brep-to-step's back
//! without dropping or fixing anything.

mod common;

use common::compare::assert_same_structure;
use common::fixtures::{LineKind, cube};
use common::scene::{Scene, write_ours, write_step_io};

/// Compare with step-io, read back strictly, and return our file.
fn check(scene: &Scene) -> String {
    let ours = write_ours(scene);
    assert_same_structure(&ours, &write_step_io(scene));
    let (_, report) = step_io::read(ours.as_bytes()).expect("read");
    assert!(report.dropped.is_empty(), "dropped: {:?}", report.dropped);
    assert!(report.norm.is_empty(), "normalized: {:?}", report.norm);
    ours
}

/// How many simple instances of `entity` the file holds.
fn count(file: &str, entity: &str) -> usize {
    file.matches(&format!(" = {entity}(")).count()
}

#[test]
fn cube_with_lines_from_vertices() {
    check(&cube("cube", [0.0; 3], 1.0, LineKind::FromVertices));
}

#[test]
fn cube_with_kernel_directions() {
    check(&cube("cube", [0.0; 3], 1.0, LineKind::Along));
}

#[test]
fn cube_off_origin() {
    for line in [LineKind::FromVertices, LineKind::Along] {
        check(&cube("cube", [0.1, -2.5, 1e-3], 12.7, line));
    }
}

#[test]
fn cube_entity_counts() {
    let file = check(&cube("cube", [0.0; 3], 1.0, LineKind::Along));
    assert_eq!(count(&file, "VERTEX_POINT"), 8);
    assert_eq!(count(&file, "EDGE_CURVE"), 12);
    assert_eq!(count(&file, "ORIENTED_EDGE"), 24);
    assert_eq!(count(&file, "ADVANCED_FACE"), 6);
    assert_eq!(count(&file, "MANIFOLD_SOLID_BREP"), 1);
    // 8 vertices + 12 line starts + 6 plane origins + 1 part origin: every
    // line and placement has its own point, as step-io writes them.
    assert_eq!(count(&file, "CARTESIAN_POINT"), 27);
}

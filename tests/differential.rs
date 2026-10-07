//! Every test shape, written by brep-to-step and by step-io: the two files
//! must have the same structure, and step-io must read brep-to-step's back
//! without dropping or fixing anything.

mod common;

use brep_to_step::{Curve, Surface, VoidShellNormals};
use common::compare::{assert_same_structure, compare};
use common::fixtures::{
    LineKind, coverage, cube, cubic, cylinder, grid, hollow_box, plate_with_hole, quarter_arc,
    real_solids, two_parts, two_solids_one_part,
};
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

/// [`check`] for a named shape from a list, naming it on failure.
fn check_named(name: &str, scene: &Scene) -> String {
    let ours = write_ours(scene);
    if let Err(report) = compare(&ours, &write_step_io(scene)) {
        panic!("{name}: brep-to-step and step-io files differ:\n{report}");
    }
    let (_, report) = step_io::read(ours.as_bytes()).expect("read");
    assert!(
        report.dropped.is_empty(),
        "{name}: dropped {:?}",
        report.dropped
    );
    assert!(
        report.norm.is_empty(),
        "{name}: normalized {:?}",
        report.norm
    );
    ours
}

/// The shape `name` from a fixture list.
fn find(list: Vec<(&'static str, Scene)>, name: &str) -> Scene {
    list.into_iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("no shape {name}"))
        .1
}

/// How many simple instances of `entity` the file holds.
fn count(file: &str, entity: &str) -> usize {
    file.matches(&format!(" = {entity}(")).count()
}

/// How many times `part` appears inside complex instances. A simple
/// instance of the same name would count too, so use it only for names that
/// are always parts.
fn count_part(file: &str, part: &str) -> usize {
    file.matches(&format!(" {part}(")).count()
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

#[test]
fn cylinder_with_full_circle_rims() {
    let file = check(&cylinder("cylinder", [0.0; 3], 2.5, 7.0, 1));
    assert_eq!(count(&file, "CYLINDRICAL_SURFACE"), 1);
    assert_eq!(count(&file, "ADVANCED_FACE"), 3);
    assert_eq!(count(&file, "CIRCLE"), 2);
    assert_eq!(count(&file, "VERTEX_POINT"), 2);
}

#[test]
fn cylinder_with_arc_rims() {
    let file = check(&cylinder("cylinder", [1.5, -0.25, 3.0], 0.8, 12.7, 2));
    assert_eq!(count(&file, "CYLINDRICAL_SURFACE"), 1);
    assert_eq!(count(&file, "ADVANCED_FACE"), 3);
    assert_eq!(count(&file, "CIRCLE"), 4);
    assert_eq!(count(&file, "VERTEX_POINT"), 4);
}

#[test]
fn plate_with_a_hole() {
    let file = check(&plate_with_hole("plate", [40.0, 30.0, 5.0], 6.0));
    // An inner bound on the top, on the bottom, and on the hole's wall.
    assert_eq!(count(&file, "FACE_BOUND"), 3);
    assert_eq!(count(&file, "CYLINDRICAL_SURFACE"), 1);
}

#[test]
fn hollow_box_with_both_void_windings() {
    for (normals, flag) in [
        (VoidShellNormals::AwayFromMaterial, ".T."),
        (VoidShellNormals::TowardMaterial, ".F."),
    ] {
        let file = check(&hollow_box(normals));
        assert_eq!(count(&file, "BREP_WITH_VOIDS"), 1);
        assert_eq!(count(&file, "ORIENTED_CLOSED_SHELL"), 1);
        assert_eq!(count(&file, "CLOSED_SHELL"), 2);
        let oriented = file
            .lines()
            .find(|l| l.contains(" = ORIENTED_CLOSED_SHELL("))
            .expect("oriented shell");
        assert!(
            oriented.ends_with(&format!("{flag});")),
            "{normals:?}: {oriented}"
        );
    }
}

#[test]
fn two_parts_in_one_file() {
    let file = check(&two_parts());
    assert_eq!(count(&file, "PRODUCT"), 2);
    assert_eq!(count(&file, "SHAPE_DEFINITION_REPRESENTATION"), 2);
    assert_eq!(count(&file, "PRODUCT_RELATED_PRODUCT_CATEGORY"), 1);
}

#[test]
fn two_solids_in_one_part() {
    let file = check(&two_solids_one_part());
    assert_eq!(count(&file, "PRODUCT"), 1);
    assert_eq!(count(&file, "MANIFOLD_SOLID_BREP"), 2);
}

#[test]
fn every_real_solid() {
    for solid in real_solids() {
        check_named(solid.name, &solid.scene);
    }
}

#[test]
fn every_coverage_shape() {
    for (name, scene) in coverage() {
        check_named(name, &scene);
    }
}

#[test]
fn nurbs_curves_rational_or_not() {
    let plain = check_named("nurbs_edge", &find(coverage(), "nurbs_edge"));
    assert_eq!(count(&plain, "B_SPLINE_CURVE_WITH_KNOTS"), 1);
    assert_eq!(count_part(&plain, "RATIONAL_B_SPLINE_CURVE"), 0);

    let rational = check_named(
        "rational_nurbs_edge",
        &find(coverage(), "rational_nurbs_edge"),
    );
    assert_eq!(count(&rational, "B_SPLINE_CURVE_WITH_KNOTS"), 0);
    assert_eq!(count_part(&rational, "RATIONAL_B_SPLINE_CURVE"), 1);
}

#[test]
fn nurbs_surface_points() {
    let file = check_named("nurbs_surface", &find(coverage(), "nurbs_surface"));
    // 12 control points + the boundary's vertex + its circle's placement
    // origin + the part origin.
    assert_eq!(count(&file, "CARTESIAN_POINT"), 15);
    assert_eq!(count(&file, "B_SPLINE_SURFACE_WITH_KNOTS"), 1);
}

/// `Some` weights write a rational B-spline even when every weight is 1 —
/// the caller says what the curve is. step-io writes these non-rational, so
/// only brep-to-step's own file is checked.
#[test]
fn unit_weights_stay_rational() {
    let mut scene = find(coverage(), "rational_nurbs_edge");
    scene.edges[0].curve = Curve::Nurbs(quarter_arc(Some(vec![1.0; 3])));
    let file = write_ours(&scene);
    assert_eq!(count_part(&file, "RATIONAL_B_SPLINE_CURVE"), 1);
    assert_eq!(count(&file, "B_SPLINE_CURVE_WITH_KNOTS"), 0);

    let mut scene = find(coverage(), "nurbs_surface");
    let mut surface = grid(false);
    surface.weights = Some(vec![vec![1.0; 4]; 3]);
    scene.faces[0].surface = Surface::Nurbs(surface);
    let file = write_ours(&scene);
    assert_eq!(count_part(&file, "RATIONAL_B_SPLINE_SURFACE"), 1);
    assert_eq!(count(&file, "B_SPLINE_SURFACE_WITH_KNOTS"), 0);

    // And with no weights, the same data is plain.
    let mut scene = find(coverage(), "nurbs_edge");
    scene.edges[0].curve = Curve::Nurbs(cubic());
    assert_eq!(count(&write_ours(&scene), "B_SPLINE_CURVE_WITH_KNOTS"), 1);
}

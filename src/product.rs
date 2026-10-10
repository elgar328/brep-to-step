//! The product structure: one product definition chain per part, and its
//! shape representation.

use crate::ap;
use crate::context::Skeleton;
use crate::geometry::{Frame, write_placement};
use crate::p21::{Data, Param, Ref};

/// A part made by [`StepWriter::part`](crate::StepWriter::part): one product
/// that holds the solids added to it — or none, for a part with no shape.
/// Valid only with the writer that made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Part {
    pub(crate) writer: u64,
    pub(crate) index: usize,
}

/// A part's product chain, and the solids collected for its shape
/// representation, which is written once all are in.
#[derive(Debug)]
pub(crate) struct PendingPart {
    product: Ref,
    shape: Ref,
    origin: Ref,
    pub(crate) solids: Vec<Ref>,
}

/// The product chain: product → formation → definition → definition shape,
/// plus the part's origin placement.
pub(crate) fn write_part(data: &mut Data, skeleton: &Skeleton, name: &str) -> PendingPart {
    let origin = write_placement(
        data,
        &Frame {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            ref_dir: [1.0, 0.0, 0.0],
        },
    );
    let product = data.simple(
        "PRODUCT",
        &[
            Param::Str(name),
            Param::Str(name),
            Param::Unset,
            Param::Refs(&[skeleton.product_context]),
        ],
    );
    let formation = data.simple(
        "PRODUCT_DEFINITION_FORMATION",
        &[Param::Str(""), Param::Unset, Param::Ref(product)],
    );
    let definition = data.simple(
        "PRODUCT_DEFINITION",
        &[
            Param::Str(ap::PRODUCT_DEFINITION_ID),
            Param::Unset,
            Param::Ref(formation),
            Param::Ref(skeleton.product_definition_context),
        ],
    );
    let shape = data.simple(
        "PRODUCT_DEFINITION_SHAPE",
        &[Param::Str(""), Param::Unset, Param::Ref(definition)],
    );
    PendingPart {
        product,
        shape,
        origin,
        solids: Vec::new(),
    }
}

/// Each part's shape representation and its link to the part, then the
/// category every part belongs to. A part with solids gets the customary
/// `ADVANCED_BREP_SHAPE_REPRESENTATION`; one without gets a plain
/// `SHAPE_REPRESENTATION` holding only its origin, since an advanced B-rep
/// representation must hold a solid. step-io's `StepBuilder` does the same.
pub(crate) fn write_shapes(data: &mut Data, skeleton: &Skeleton, parts: &[PendingPart]) {
    for part in parts {
        let mut items = Vec::with_capacity(1 + part.solids.len());
        items.push(part.origin);
        items.extend(&part.solids);
        let kind = if part.solids.is_empty() {
            "SHAPE_REPRESENTATION"
        } else {
            "ADVANCED_BREP_SHAPE_REPRESENTATION"
        };
        let representation = data.simple(
            kind,
            &[
                Param::Str(""),
                Param::Refs(&items),
                Param::Ref(skeleton.ctx),
            ],
        );
        data.simple(
            "SHAPE_DEFINITION_REPRESENTATION",
            &[Param::Ref(part.shape), Param::Ref(representation)],
        );
    }
    if !parts.is_empty() {
        let products: Vec<Ref> = parts.iter().map(|p| p.product).collect();
        data.simple(
            "PRODUCT_RELATED_PRODUCT_CATEGORY",
            &[
                Param::Str(ap::PRODUCT_CATEGORY),
                Param::Unset,
                Param::Refs(&products),
            ],
        );
    }
}

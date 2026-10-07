//! The fixed file skeleton: application context and protocol definition,
//! product contexts, units, uncertainty, and the geometric representation
//! context every shape representation refers to.

use crate::ap;
use crate::error::Error;
use crate::p21::{Data, Param, Ref};

/// The file's units: the length unit coordinates are written in, and the
/// modelling uncertainty in that unit. Angles are always radians.
#[derive(Debug, Clone, Copy)]
pub struct Units {
    pub length: LengthUnit,
    /// The length below which two points are the same point
    /// (`distance_accuracy_value`), in `length` units. Must be finite and
    /// positive.
    pub uncertainty: f64,
}

impl Default for Units {
    /// Millimetres, with an uncertainty of `1e-7` mm.
    fn default() -> Self {
        Self {
            length: LengthUnit::Millimetre,
            uncertainty: 1e-7,
        }
    }
}

/// The SI length unit coordinates are written in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LengthUnit {
    #[default]
    Millimetre,
    Metre,
}

/// The skeleton entities later entities refer to.
#[derive(Debug)]
pub(crate) struct Skeleton {
    /// The geometric representation context, with units and uncertainty.
    pub(crate) ctx: Ref,
    pub(crate) product_context: Ref,
    pub(crate) product_definition_context: Ref,
}

/// Write the skeleton every file carries, whatever its shapes.
pub(crate) fn write_skeleton(data: &mut Data, units: Units) -> Result<Skeleton, Error> {
    if !(units.uncertainty.is_finite() && units.uncertainty > 0.0) {
        return Err(Error::InvalidNumber {
            what: "uncertainty",
            value: units.uncertainty,
        });
    }

    let application = data.simple("APPLICATION_CONTEXT", &[Param::Str(ap::APPLICATION)]);
    data.simple(
        "APPLICATION_PROTOCOL_DEFINITION",
        &[
            Param::Str(ap::APD_STATUS),
            Param::Str(ap::APD_SCHEMA_NAME),
            Param::Int(ap::APD_YEAR),
            Param::Ref(application),
        ],
    );
    let product_context = data.simple(
        "PRODUCT_CONTEXT",
        &[
            Param::Str(ap::PRODUCT_CONTEXT_NAME),
            Param::Ref(application),
            Param::Str(ap::PRODUCT_CONTEXT_DISCIPLINE),
        ],
    );
    let product_definition_context = data.simple(
        "PRODUCT_DEFINITION_CONTEXT",
        &[
            Param::Str(ap::PRODUCT_DEFINITION_CONTEXT_NAME),
            Param::Ref(application),
            Param::Str(ap::PRODUCT_DEFINITION_CONTEXT_STAGE),
        ],
    );

    let prefix = match units.length {
        LengthUnit::Millimetre => Param::Enum("MILLI"),
        LengthUnit::Metre => Param::Unset,
    };
    let length = data.complex(&[
        ("LENGTH_UNIT", &[]),
        ("NAMED_UNIT", &[Param::Derived]),
        ("SI_UNIT", &[prefix, Param::Enum("METRE")]),
    ]);
    let plane_angle = data.complex(&[
        ("NAMED_UNIT", &[Param::Derived]),
        ("PLANE_ANGLE_UNIT", &[]),
        ("SI_UNIT", &[Param::Unset, Param::Enum("RADIAN")]),
    ]);
    let solid_angle = data.complex(&[
        ("NAMED_UNIT", &[Param::Derived]),
        ("SI_UNIT", &[Param::Unset, Param::Enum("STERADIAN")]),
        ("SOLID_ANGLE_UNIT", &[]),
    ]);
    let uncertainty = data.simple(
        "UNCERTAINTY_MEASURE_WITH_UNIT",
        &[
            Param::Measure("LENGTH_MEASURE", units.uncertainty),
            Param::Ref(length),
            Param::Str("distance_accuracy_value"),
            Param::Str("confusion accuracy"),
        ],
    );
    let ctx = data.complex(&[
        ("GEOMETRIC_REPRESENTATION_CONTEXT", &[Param::Int(3)]),
        (
            "GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT",
            &[Param::Refs(&[uncertainty])],
        ),
        (
            "GLOBAL_UNIT_ASSIGNED_CONTEXT",
            &[Param::Refs(&[length, plane_angle, solid_angle])],
        ),
        (
            "REPRESENTATION_CONTEXT",
            &[Param::Str(""), Param::Str("3D")],
        ),
    ]);

    Ok(Skeleton {
        ctx,
        product_context,
        product_definition_context,
    })
}

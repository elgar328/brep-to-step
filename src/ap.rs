//! The application protocol identity: every fixed value that names the AP and
//! its edition (`FILE_SCHEMA`, the application protocol definition, and the
//! application and product context labels). Moving to another AP edition
//! changes this file only.

/// The `FILE_SCHEMA` entry: AP242 edition 2.
pub(crate) const FILE_SCHEMA: &str =
    "AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF { 1 0 10303 442 3 1 4 }";

/// `APPLICATION_CONTEXT.application`.
pub(crate) const APPLICATION: &str = "managed model based 3d engineering";

/// `APPLICATION_PROTOCOL_DEFINITION.status`.
pub(crate) const APD_STATUS: &str = "international standard";
/// `APPLICATION_PROTOCOL_DEFINITION.application_interpreted_model_schema_name`.
pub(crate) const APD_SCHEMA_NAME: &str = "ap242_managed_model_based_3d_engineering_mim_lf";
/// `APPLICATION_PROTOCOL_DEFINITION.application_protocol_year`: the year
/// edition 2 was published.
pub(crate) const APD_YEAR: i64 = 2020;

/// `PRODUCT_CONTEXT.name`.
pub(crate) const PRODUCT_CONTEXT_NAME: &str = "";
/// `PRODUCT_CONTEXT.discipline_type`.
pub(crate) const PRODUCT_CONTEXT_DISCIPLINE: &str = "mechanical";

/// `PRODUCT_DEFINITION_CONTEXT.name`.
pub(crate) const PRODUCT_DEFINITION_CONTEXT_NAME: &str = "part definition";
/// `PRODUCT_DEFINITION_CONTEXT.life_cycle_stage`.
pub(crate) const PRODUCT_DEFINITION_CONTEXT_STAGE: &str = "design";

/// `PRODUCT_DEFINITION.id`.
pub(crate) const PRODUCT_DEFINITION_ID: &str = "design";

/// `PRODUCT_RELATED_PRODUCT_CATEGORY.name`, the category of every part.
pub(crate) const PRODUCT_CATEGORY: &str = "part";

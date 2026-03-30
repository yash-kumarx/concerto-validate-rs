//! CTO parser stub.
//!
//! I left this in because the project is supposed to grow into it, but a real
//! CTO parser would have blown up the scope fast. Validation was the priority.

use crate::error::ConcertoError;
use crate::model_file::ModelFile;

/// Parses `.cto` source into a `ModelFile`.
///
/// Not implemented yet. Callers should use the JSON metamodel loader for now.
///
/// # Errors
///
/// Always returns `ConcertoError::Parse` for now.
#[allow(dead_code)]
pub(crate) fn parse_cto(_input: &str) -> Result<ModelFile, ConcertoError> {
    Err(ConcertoError::Parse(
        "cto parser not implemented yet -- use ModelManager::add_model_from_json()".to_string(),
    ))
}

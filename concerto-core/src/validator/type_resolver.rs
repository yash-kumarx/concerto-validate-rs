//! Tiny type-name splitting helpers.
//!
//! This file is intentionally boring. That is a good sign. I only wanted one
//! place doing the string slicing so `ModelManager` and the validator don't
//! drift apart.

use crate::declaration::Declaration;
use crate::error::ConcertoError;
use crate::model_manager::ModelManager;

pub(crate) fn split_fqn(fqn: &str) -> Option<(&str, &str)> {
    // split from the right. namespace can contain dots, so splitting from the
    // left is wrong and yes i did make that mistake once
    let pos = fqn.rfind('.')?;
    Some((&fqn[..pos], &fqn[pos + 1..]))
}

pub(crate) fn resolve<'a>(
    mm: &'a ModelManager,
    fqn: &str,
) -> Result<&'a Declaration, ConcertoError> {
    mm.resolve_type(fqn)
}

pub(crate) fn resolve_in_context<'a>(
    mm: &'a ModelManager,
    type_name: &str,
    context_namespace: &str,
) -> Result<&'a Declaration, ConcertoError> {
    mm.resolve_type_in_context(type_name, context_namespace)
}

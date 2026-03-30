//! Numeric constraint helpers.
//!
//! Integer, Long, and Double all do the same lower/upper bound dance. One tiny
//! helper beat copy-pasting the same thing three times.

use crate::property::NumericValidator;
use crate::validator::instance_validator::{ErrorKind, ValidationError};
use std::fmt::Display;

pub(crate) fn validate_numeric<T>(
    value: T,
    validator: &Option<NumericValidator<T>>,
    path: &str,
    label: &str,
) -> Vec<ValidationError>
where
    T: PartialOrd + Display + Copy,
{
    let mut errors = Vec::new();

    let numeric_validator = match validator {
        Some(numeric_validator) => numeric_validator,
        None => return errors,
    };

    if let Some(lower) = numeric_validator.lower {
        if value < lower {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!("'{label}' is {value}, min is {lower}"),
                error_type: ErrorKind::ConstraintViolation {
                    constraint: format!("lower={lower}"),
                },
            });
        }
    }

    if let Some(upper) = numeric_validator.upper {
        if value > upper {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!("'{label}' is {value}, max is {upper}"),
                error_type: ErrorKind::ConstraintViolation {
                    constraint: format!("upper={upper}"),
                },
            });
        }
    }

    errors
}

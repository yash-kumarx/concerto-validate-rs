//! String validation helpers.
//!
//! Small file, still worth having. Keeps the regex and length logic out of the
//! main validator walk and caches regexes so repeated runs are less wasteful.

use crate::property::StringValidator;
use crate::validator::instance_validator::{ErrorKind, ValidationError};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;

static REGEX_CACHE: Lazy<Mutex<HashMap<String, Regex>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub(crate) fn validate_string(
    value: &str,
    validator: &Option<StringValidator>,
    path: &str,
    property_name: &str,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let string_validator = match validator {
        Some(string_validator) => string_validator,
        None => return errors,
    };

    if let Some(pattern) = &string_validator.regex {
        // regex validity is checked when the model loads. if compilation fails
        // here something else is badly off, so we just treat it as non-match
        let is_match = {
            let mut cache = REGEX_CACHE
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            if !cache.contains_key(pattern) {
                if let Ok(re) = Regex::new(pattern) {
                    cache.insert(pattern.clone(), re);
                }
            }

            cache.get(pattern).is_some_and(|re| re.is_match(value))
        };

        if !is_match {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!("'{}' doesn't match /{}/", value, pattern),
                error_type: ErrorKind::PatternMismatch {
                    pattern: pattern.clone(),
                },
            });
        }
    }

    // .len() counts bytes. needed chars() here or unicode strings get judged
    // by utf-8 storage size, which is obviously not what the model means
    let char_count = value.chars().count();

    if let Some(min_length) = string_validator.min_length {
        if char_count < min_length {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!("'{property_name}' is too short: {char_count} < {min_length}"),
                error_type: ErrorKind::ConstraintViolation {
                    constraint: format!("minLength={min_length}"),
                },
            });
        }
    }

    if let Some(max_length) = string_validator.max_length {
        if char_count > max_length {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!("'{property_name}' is too long: {char_count} > {max_length}"),
                error_type: ErrorKind::ConstraintViolation {
                    constraint: format!("maxLength={max_length}"),
                },
            });
        }
    }

    errors
}

//! C ABI wrapper for the validator.
//!
//! I had not done much Rust FFI before this project, so this file is where I
//! learned the very boring very important rules: no panics across the boundary,
//! no borrowed strings escaping, and every returned allocation needs one clear
//! owner.

use concerto_core::ModelManager;
use serde_json::Value;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Opaque model-manager handle for C callers.
///
/// C and Python callers just treat this as a black box pointer.
pub struct ConcertoModelManager(ModelManager);

/// Allocates a new empty model manager.
///
/// The returned pointer must be freed with `concerto_model_manager_free`.
#[no_mangle]
pub extern "C" fn concerto_model_manager_new() -> *mut ConcertoModelManager {
    let mm = Box::new(ConcertoModelManager(ModelManager::new()));
    Box::into_raw(mm)
}

/// Frees a model manager allocated by `concerto_model_manager_new`.
///
/// Passing `NULL` is allowed.
///
/// # Safety
///
/// `mm` must be a pointer returned by `concerto_model_manager_new` and must not
/// be freed twice.
#[no_mangle]
pub unsafe extern "C" fn concerto_model_manager_free(mm: *mut ConcertoModelManager) {
    if !mm.is_null() {
        drop(Box::from_raw(mm));
    }
}

/// Loads one Concerto model into the manager.
///
/// Returns `NULL` on success. On failure it returns an owned C string that must
/// be freed with `concerto_free_string`.
///
/// # Safety
///
/// `mm` and `json` must be non-null valid pointers. `json` must point to a
/// null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn concerto_add_model(
    mm: *mut ConcertoModelManager,
    json: *const c_char,
) -> *mut c_char {
    if mm.is_null() || json.is_null() {
        return make_error_string("concerto_add_model: null pointer");
    }

    let json_str = match CStr::from_ptr(json).to_str() {
        Ok(text) => text,
        Err(_) => return make_error_string("concerto_add_model: json is not utf-8"),
    };

    let mm = &mut (*mm).0;
    match mm.add_model_from_json(json_str) {
        Ok(()) => std::ptr::null_mut(),
        Err(error) => make_error_string(&error.to_string()),
    }
}

/// Validates one JSON instance against one type.
///
/// Returns an owned JSON string. The caller must free it with
/// `concerto_free_string`.
///
/// # Safety
///
/// All pointers must be non-null valid pointers to null-terminated UTF-8
/// strings for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn concerto_validate_instance(
    mm: *const ConcertoModelManager,
    instance_json: *const c_char,
    type_name: *const c_char,
) -> *mut c_char {
    if mm.is_null() || instance_json.is_null() || type_name.is_null() {
        return json_error_string("null pointer");
    }

    let instance_str = match CStr::from_ptr(instance_json).to_str() {
        Ok(text) => text,
        Err(_) => return json_error_string("instance_json is not utf-8"),
    };

    let type_name = match CStr::from_ptr(type_name).to_str() {
        Ok(text) => text,
        Err(_) => return json_error_string("type_name is not utf-8"),
    };

    let instance: Value = match serde_json::from_str(instance_str) {
        Ok(value) => value,
        Err(error) => return json_error_string(&format!("bad instance json: {error}")),
    };

    let mm = &(*mm).0;
    match mm.validate_instance(&instance, type_name) {
        Ok(result) => {
            let errors = result
                .errors
                .iter()
                .map(|error| {
                    serde_json::json!({
                        "path": error.path,
                        "message": error.message,
                    })
                })
                .collect::<Vec<_>>();

            make_c_string(
                &serde_json::json!({
                    "valid": result.valid,
                    "errors": errors,
                })
                .to_string(),
            )
        }
        Err(error) => json_error_string(&error.to_string()),
    }
}

/// Frees a string returned by this library.
///
/// Passing `NULL` is allowed.
///
/// # Safety
///
/// `s` must be a pointer returned by this library and must not be freed twice.
#[no_mangle]
pub unsafe extern "C" fn concerto_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

fn make_error_string(msg: &str) -> *mut c_char {
    make_c_string(msg)
}

fn json_error_string(msg: &str) -> *mut c_char {
    make_c_string(&serde_json::json!({ "error": msg }).to_string())
}

fn make_c_string(s: &str) -> *mut c_char {
    // embedded NUL would break CString::new. shouldn't happen, but i'd rather
    // replace it than crash while formatting an error string
    let clean = s.replace('\0', "<NUL>");
    match CString::new(clean) {
        Ok(cs) => cs.into_raw(),
        Err(_) => match CString::new("internal error: could not build CString") {
            Ok(fallback) => fallback.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
    }
}

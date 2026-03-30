//! WASM wrapper around `concerto-core`.
//!
//! Main rule here was keep it thin. I really did not want a browser-specific
//! validator quietly drifting away from the native one.

use concerto_core::ModelManager;
use wasm_bindgen::prelude::*;

/// WebAssembly wrapper around `ModelManager`.
///
/// This exists so browser and Node callers can hit the same validator core
/// instead of some separate JS-only reimplementation.
#[wasm_bindgen]
pub struct WasmModelManager {
    inner: ModelManager,
}

impl Default for WasmModelManager {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WasmModelManager {
    /// Creates a new empty model manager.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmModelManager {
        WasmModelManager {
            inner: ModelManager::new(),
        }
    }

    /// Loads one metamodel JSON string.
    ///
    /// Throws a JS error if the model cannot be parsed.
    #[wasm_bindgen(js_name = addModel)]
    pub fn add_model(&mut self, json: &str) -> Result<(), JsValue> {
        self.inner
            .add_model_from_json(json)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Loads one model from an already-parsed JS value.
    ///
    /// This ended up being the nicer API for browser and Node callers because
    /// they usually already have a plain JS object.
    #[wasm_bindgen(js_name = addModelValue)]
    pub fn add_model_value(&mut self, model_value: JsValue) -> Result<(), JsValue> {
        let json = js_value_to_json_string(model_value)?;
        self.add_model(&json)
    }

    /// Validates one JSON string instance against a Concerto type.
    ///
    /// Returns a JS object shaped like `{ valid, errors }`.
    ///
    /// # Errors
    ///
    /// Throws a JS error if JSON parsing or validation setup fails.
    #[wasm_bindgen(js_name = validateInstance)]
    pub fn validate_instance(
        &self,
        instance_json: &str,
        type_name: &str,
    ) -> Result<JsValue, JsValue> {
        let instance = parse_json_string(instance_json)?;
        validation_result_to_js(self.inner.validate_instance(&instance, type_name))
    }

    /// Validates an already-parsed JS object against a Concerto type.
    ///
    /// This is the API I actually wanted for integration work since it skips the
    /// stringify/parse churn on the JS side.
    ///
    /// # Errors
    ///
    /// Throws a JS error if the JS value cannot be converted or validation
    /// setup fails.
    #[wasm_bindgen(js_name = validateInstanceValue)]
    pub fn validate_instance_value(
        &self,
        instance_value: JsValue,
        type_name: &str,
    ) -> Result<JsValue, JsValue> {
        let instance = js_value_to_json_value(instance_value)?;
        validation_result_to_js(self.inner.validate_instance(&instance, type_name))
    }

    /// Returns loaded namespaces as a JS array.
    #[wasm_bindgen(js_name = loadedNamespaces)]
    pub fn loaded_namespaces(&self) -> Result<JsValue, JsValue> {
        let namespaces: Vec<&str> = self.inner.namespaces().collect();
        serde_wasm_bindgen::to_value(&namespaces)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

/// Validates one instance with one model in a single call.
///
/// Handy for small JS call sites that don't want to manage a model manager.
/// I ended up using this a lot in smoke tests and quick demo code.
///
/// # Errors
///
/// Throws a JS error if loading the model or validating the instance fails.
#[wasm_bindgen(js_name = validateInstanceWithModel)]
pub fn validate_instance_with_model(
    model_value: JsValue,
    instance_value: JsValue,
    type_name: &str,
) -> Result<JsValue, JsValue> {
    let mut mm = WasmModelManager::new();
    mm.add_model_value(model_value)?;
    mm.validate_instance_value(instance_value, type_name)
}

fn validation_result_to_js(
    result: Result<concerto_core::ValidationResult, concerto_core::ConcertoError>,
) -> Result<JsValue, JsValue> {
    let result = result.map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&result).map_err(|error| JsValue::from_str(&error.to_string()))
}

fn parse_json_string(json: &str) -> Result<serde_json::Value, JsValue> {
    serde_json::from_str(json).map_err(|error| JsValue::from_str(&format!("bad json: {error}")))
}

fn js_value_to_json_value(value: JsValue) -> Result<serde_json::Value, JsValue> {
    serde_wasm_bindgen::from_value(value)
        .map_err(|error| JsValue::from_str(&format!("can't convert JS value: {error}")))
}

fn js_value_to_json_string(value: JsValue) -> Result<String, JsValue> {
    let json_value = js_value_to_json_value(value)?;
    serde_json::to_string(&json_value)
        .map_err(|error| JsValue::from_str(&format!("can't serialize JSON: {error}")))
}

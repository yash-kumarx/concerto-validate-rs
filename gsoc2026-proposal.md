# Porting Concerto Entity Validation to Rust: A Multi-Platform Runtime for the Accord Project

---

## 1. Contact Information

| Field | Value |
|---|---|
| **Name** | Yash Kumar |
| **Age** | 19 |
| **Institution** | IIIT Sonepat, B.Tech CSE (Sophomore) |
| **Email** | yashr17042006@gmail.com |
| **GitHub** | [github.com/yash-kumarx](https://github.com/yash-kumarx) |
| **Timezone** | IST (UTC+5:30) |
| **Available hours/week** | 30–35 hours |
| **Mentors** | Ertugrul Karademir ([@ekarademir](https://github.com/ekarademir)), Jamie Shorten ([@jamieshorten](https://github.com/jamieshorten)) |

---

## 2. Synopsis

The Accord Project's Concerto runtime is written entirely in JavaScript. That was the right call for its original context — Node and the browser. It is the wrong call if a C# legal-tech platform needs to validate a contract instance without bundling Node into production, or if a Python compliance tool needs to check 10,000 contract payloads per second without the overhead of a JS interpreter.

In the Accord Project Technology Working Group call, the mentor said directly:

> *"Entity validation is the hotspot that we're looking to particularly optimize."*

And:

> *"The goal is to get entity validation working, and then things can follow on from after that."*

This proposal describes a Rust port of that hotspot. The deliverable at the end of summer is a production-ready `concerto-core` crate that validates JSON instances against Concerto models, compiled to both WebAssembly for browser and Node.js callers, and exposed via a C ABI for C#, Python, and Java — with zero dependency on a JavaScript runtime. I have already built a working proof of concept. This proposal is the plan to turn it into the real thing.

---

## 3. The Problem — Why This Matters

Concerto is a schema language for legal contracts. The runtime today lives entirely in JavaScript. That works perfectly on Node and in the browser, but it creates a wall the moment any other ecosystem tries to use it.

```
Current State: Embedding Concerto in a C# system
┌──────────────────────────────────────────────────────┐
│  C# Legal Contract Platform                          │
│                                                      │
│  contract.Validate()  ────────────────────────────► │──► spawn Node.js process
│                                                      │         │
│                        ◄─────────────────────────── │◄── JSON over stdin/stdout
│                        100–500ms cold start          │
│                        + 50MB Node.js in container   │
└──────────────────────────────────────────────────────┘

With concerto-rs: same validation via P/Invoke
┌──────────────────────────────────────────────────────┐
│  C# Legal Contract Platform                          │
│                                                      │
│  contract.Validate()  ────────────────────────────► │──► P/Invoke → libconcerto_ffi.so
│                                                      │         │
│                        ◄─────────────────────────── │◄── ValidationResult struct
│                        ~15µs per call                │
│                        + 2MB shared library          │
└──────────────────────────────────────────────────────┘
```

There are three concrete problems this project solves.

**Embedding cost.** To call the JS validator from C# or Python today, you shell out to Node, pay the process startup cost, and serialize data across process boundaries. That is not a validation call — it is an RPC call with all the brittleness that entails. A Rust library compiled to a `.so` or `.dll` costs a function call.

**Performance at scale.** Legal platforms validate at volume. Template engines re-validate on every data change. The JS interpreter adds overhead on every call that a compiled Rust binary eliminates. The mentor named this explicitly: there are *"several parts of Concerto that are slow just because of the overhead of running JavaScript."*

**Browser/edge deployment.** The current JS bundle is too large for edge runtimes and complicates tree-shaking in browser builds. A WASM binary compiled from Rust is smaller, loads faster, and runs at near-native speed in the browser with `wasm-bindgen` bindings that look like ordinary TypeScript to the caller.

---

## 4. Background — What is Concerto

Concerto is a data modeling language used across the Accord Project ecosystem to define the structure of legal contract data. It sits between the template (what the contract looks like) and the instance (what a specific contract says). The runtime's job is to answer one question: is this JSON object a valid instance of this Concerto type?

The type system has the following shape:

```
Concerto Declaration Hierarchy
                       Declaration
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
   ClassDecl           EnumDecl           ScalarDecl     MapDecl
        │
   ┌────┴──────────────────────────┐
Concept  Asset  Participant  Transaction  Event
        │
   Properties (declared per type, inherited from supertypes)
   ┌────┴────────────────────────────────────────────┐
   │        │         │         │          │         │
String  Integer   Double  Boolean  DateTime  ObjectRef  Relationship
   │        │         │
   │    Range     Range
  Regex/Length  validator
  validator
```

A `.cto` model file declares types in a namespace:

```
namespace org.example@1.0.0

concept Person {
  o String  email  regex=/^[^@]+@[^@]+\.[^@]+$/
  o Integer age    range=[0, 150]
  o String  firstName
  o String  lastName optional
  o DateTime dateOfBirth optional
}
```

The Accord Project toolchain converts this to a JSON metamodel:

```json
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.example@1.0.0",
  "declarations": [{
    "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
    "name": "Person",
    "isAbstract": false,
    "properties": [
      {
        "$class": "concerto.metamodel@1.0.0.StringProperty",
        "name": "email",
        "isArray": false,
        "isOptional": false,
        "validator": {
          "$class": "concerto.metamodel@1.0.0.StringRegexValidator",
          "pattern": "^[^@]+@[^@]+\\.[^@]+$",
          "flags": ""
        }
      },
      {
        "$class": "concerto.metamodel@1.0.0.IntegerProperty",
        "name": "age",
        "isArray": false,
        "isOptional": false,
        "validator": {
          "$class": "concerto.metamodel@1.0.0.IntegerDomainValidator",
          "lower": 0,
          "upper": 150
        }
      }
    ]
  }]
}
```

**Entity validation vs structural validation** are two distinct problems. Structural validation asks: is this JSON a well-formed Concerto model definition? Entity validation asks: is this JSON instance a valid instance of a given Concerto type? The mentor was explicit:

> *"Let's say we have an agreement, and we have a schema that represents the data in that agreement. Is that agreement a valid instance of that schema? Does it comply with the model, and therefore is compatible with the template? That's the business problem."*

The mentor also noted the recursive benefit: entity validation applied to the metamodel gives structural validation for free.

> *"Entity validation gives us probably 99% of model structural validation because you can validate against the meta model. We use Concerto to validate itself."*

---

## 5. Current State of Existing Rust POCs

Two Rust experiments exist in the `accordproject` GitHub organization: `concerto-validate-rs` and `concerto-rust`. I read both.

`concerto-validate-rs` validates Concerto ASTs against the metamodel using a hardcoded `ModelManager`. It handles structural validation only — it checks whether a JSON blob is a valid Concerto model definition, not whether a JSON instance is a valid instance of a user-defined type. It also walks only one supertype level and reports only the first error per check.

`concerto-rust` has a model manager and declaration structures, but the instance validator is not implemented.

The mentor said in the TWG call:

> *"Those are all experimental repos. There are ideas we can borrow, but don't assume that because code is there, that that is the structure we must follow. The scope and structure is something we'd love to see in your proposals."*

My POC takes a different path from both. The core design decision is that `ModelManager` owns all loaded namespaces and the validator is a pure function over it. That makes the WASM and FFI bindings straightforward: one opaque handle in both cases, no shared state, no threading issues.

---

## 6. My Proof of Concept — The Technical Centerpiece

### 6.1 — What I Built and Why

The POC lives at [github.com/accordproject/concerto-validate-rs](https://github.com/accordproject/concerto-validate-rs) and is a Cargo workspace with five crates:

```
concerto-validate-rs/          ← workspace root
├── Cargo.toml                 ← workspace manifest, shared deps
├── concerto-core/             ← the validator library (the part that matters)
│   ├── src/
│   │   ├── lib.rs             ← public API: ModelManager, ConcertoError, ValidationResult
│   │   ├── error.rs           ← ConcertoError enum
│   │   ├── declaration.rs     ← Declaration enum + ClassDeclaration, EnumDeclaration,
│   │   │                          ScalarDeclaration, MapDeclaration
│   │   ├── property.rs        ← Property, PropertyType, StringValidator, NumericValidator<T>
│   │   ├── model_file.rs      ← ModelFile (one namespace, its declarations, its imports)
│   │   ├── model_manager.rs   ← ModelManager (registry + validate_instance + resolvers)
│   │   └── parser/
│   │       ├── json_loader.rs ← metamodel JSON → Rust types (the whole $class dispatch)
│   │       ├── cto_parser.rs  ← stub (stretch goal for GSoC)
│   │       └── mod.rs
│   │   └── validator/
│   │       ├── instance_validator.rs ← THE CORE: validate_instance, inheritance walk
│   │       ├── type_resolver.rs      ← split_fqn, resolve, resolve_in_context
│   │       ├── string_validator.rs   ← regex cache, length checks (chars not bytes)
│   │       ├── numeric_validator.rs  ← generic lower/upper bounds for i64 and f64
│   │       └── mod.rs
│   ├── tests/
│   │   └── entity_validation_tests.rs ← 300+ lines of unit tests
│   └── benches/
│       └── validation_bench.rs        ← criterion benchmarks: valid, invalid, missing
├── concerto-wasm/             ← wasm-bindgen bindings
│   ├── src/lib.rs             ← WasmModelManager, validateInstance, validateInstanceValue
│   └── demo/index.html        ← working browser demo
├── concerto-ffi/              ← C ABI for cross-language use
│   ├── src/lib.rs             ← concerto_model_manager_new/free, concerto_add_model,
│   │                              concerto_validate_instance, concerto_free_string
│   ├── include/concerto.h     ← C header
│   └── demo/demo.py           ← working ctypes demo (Python)
├── concerto-cli/              ← command-line tool
│   └── src/main.rs            ← validate / check / info / bench subcommands
└── concerto-conformance/      ← fixture-driven conformance tests
    ├── src/lib.rs             ← ScenarioFixture, ScenarioExpectation loaders
    └── tests/
        ├── conformance_tests.rs  ← typed unit tests for specific behaviors
        └── fixture_scenarios.rs  ← reads every JSON in fixtures/scenarios/ automatically
```

The workspace `Cargo.toml` pins shared versions: `serde 1.0`, `serde_json 1.0`, `thiserror 1.0`, `regex 1.10`, `chrono 0.4`, `once_cell 1.19`, `log 0.4`.

The architecture principle I kept throughout: `concerto-core` has no knowledge of WASM, FFI, or CLI. Those three crates are thin wrappers. Adding a new binding target means writing one new file that calls `ModelManager::new()`, `add_model_from_json()`, and `validate_instance()`.

### 6.2 — The Entity Validation Algorithm

The entry point is `validate_instance` in `concerto-core/src/validator/instance_validator.rs`:

```rust
pub fn validate_instance(
    model_manager: &ModelManager,
    instance: &Value,
    type_name: &str,
) -> Result<ValidationResult, ConcertoError>
```

It returns `Result<ValidationResult, ConcertoError>`. The `Err` path is reserved for hard failures — type not found, namespace not loaded, circular inheritance. Validation failures (wrong type, missing field, bad regex) land in `ValidationResult.errors`, not in `Err`. That distinction matters: callers check `result.valid` for business logic, not `?` for error propagation.

The algorithm, traced through the actual code:

```
validate_instance(model_manager, instance, type_name)
              │
              ▼
      validate_object(mm, instance, type_name, "$", &mut errors)
              │
              ├─ instance is not an object?
              │    └── push TypeMismatch at "$", return Ok(())
              │
              ▼
      resolve_class(mm, instance, "$", type_name, &mut errors)
              │
              ├─ no $class field?
              │    └── push MissingRequiredProperty at "$.$class"
              │
              ├─ $class can't be resolved in ModelManager?
              │    └── push UnknownType at "$.$class"
              │
              ├─ actual type not assignable to requested type?
              │    └── push TypeMismatch at "$.$class"
              │         (is_assignable_to walks supertype chain with visited set)
              │
              ├─ actual type is abstract?
              │    └── push AbstractTypeInstantiation at "$.$class"
              │
              ▼
      collect_properties(mm, decl, &mut errors)
              │
              └── collect_properties_inner(mm, decl, &mut errors, visited=HashSet::new())
                        │
                        ├─ for each property in decl.properties: insert into HashMap
                        │
                        └─ if decl.super_type exists: resolve it, recurse
                                    visited set prevents infinite loop on circular models
                                    child properties overwrite parent on name collision
              │
              ▼
      check_missing_properties(instance, &props, "$", &mut errors)
              │
              └── for each (name, prop) in props where !prop.is_optional:
                       if instance.get(name).is_none():
                            push MissingRequiredProperty at "$.{name}"
              │
              ▼
      check_unknown_properties(instance, &props, "$", &mut errors)
              │
              └── for each key in instance.as_object():
                       if key != "$class" && !props.contains_key(key):
                            push UnknownProperty at "$.{key}"
              │
              ▼
      validate_present_properties(mm, instance, &props, "$", &mut errors)
              │
              └── for each (name, val) in instance.as_object():
                       skip "$class"
                       skip optional fields where val.is_null()
                       if prop.is_array:
                            check val.is_array(), then for each item: validate_scalar_property
                       else:
                            validate_scalar_property(mm, val, prop, "$.{name}", &mut errors)
              │
              ▼
      return Ok(ValidationResult { valid: errors.is_empty(), errors })
              └── ALL errors collected before returning — not fail-fast
```

The `validate_scalar_property` delegates to `validate_type` which dispatches on `PropertyType`:

- `String(validator)` → `string_validator::validate_string()`
- `Boolean` → check `val.is_boolean()`
- `Integer(validator)` → check `val.as_i64()` with `val.is_i64()` guard, then `numeric_validator::validate_numeric()`
- `Long(validator)` → same path as Integer
- `Double(validator)` → `val.as_f64()`, then `numeric_validator::validate_numeric()`
- `DateTime` → `chrono::DateTime::parse_from_rfc3339()`
- `Relationship { type_ref }` → parse `resource:Type#id` URI, verify type is loaded
- `ObjectRef { type_ref }` → dispatch to enum/scalar/concept/map branches, recurse with `validate_object()` for nested concepts

The collect-all-errors design was deliberate. I looked at how the JS validator reports errors and it does not stop at the first bad field — it returns everything. A browser demo that shows only one error at a time is useless for contract review workflows.

### 6.3 — Inheritance Resolution

This was the hardest part of the POC. The first version of `collect_properties_inner` only walked one supertype level. That worked on shallow fixtures and failed immediately on a three-level chain. The rewrite uses explicit recursion with a `HashSet<String>` visited set:

```rust
fn collect_properties_inner<'a>(
    model_manager: &'a ModelManager,
    decl: &'a Declaration,
    errors: &mut Vec<ValidationError>,
    visited: &mut HashSet<String>,
) -> Result<HashMap<&'a str, &'a Property>, ConcertoError>
```

For a model like `Employee extends Person extends Entity`:

```
validate_instance(data, "org.hr@1.0.0.Employee")
          │
          ▼
collect_properties_inner("Employee", visited={})
          │
          ├── Employee.properties: { employeeId: String }
          │   visited = {"org.hr@1.0.0.Employee"}
          │
          └── super_type = "Person" → resolve → recurse
                    │
                    ├── Person.properties: { email: String, firstName: String, age: Integer }
                    │   visited = {"org.hr@1.0.0.Employee", "org.hr@1.0.0.Person"}
                    │
                    └── super_type = "Entity" → resolve → recurse
                              │
                              └── Entity.properties: { $identifier: String }
                                  visited = {all three}
                                  super_type = None → stop

Final merged property map:
  $identifier → Entity.properties.$identifier
  email       → Person.properties.email
  firstName   → Person.properties.firstName
  age         → Person.properties.age
  employeeId  → Employee.properties.employeeId

Child wins on name collision — same rule as the JS validator.
```

The visited set guards against circular inheritance:

```rust
let current_fqn = format!("{}.{}", class_decl.namespace, class_decl.name);
if visited.contains(&current_fqn) {
    return Err(ConcertoError::CircularDependency(current_fqn));
}
visited.insert(current_fqn);
```

If a model defines `A extends B extends A`, the third call returns `CircularDependency("org.example@1.0.0.A")` up to `validate_instance`, which returns `Err`. That is the right behavior — a circular model is not a validation failure, it is a model error.

The same recursive pattern exists in `is_assignable_to`, used to check whether the `$class` on an instance is a subtype of the requested type.

### 6.4 — The Type System in Rust

The `Declaration` enum in `declaration.rs` covers all Concerto declaration kinds:

```rust
pub enum Declaration {
    Concept(ClassDeclaration),
    Asset(ClassDeclaration),
    Participant(ClassDeclaration),
    Transaction(ClassDeclaration),
    Event(ClassDeclaration),
    Enum(EnumDeclaration),
    Scalar(ScalarDeclaration),
    Map(MapDeclaration),
}
```

`ClassDeclaration` holds `is_abstract: bool` and `super_type: Option<String>`. Abstract type instantiation is detected in `resolve_class`:

```rust
if matches!(
    actual_decl,
    Declaration::Concept(class_decl) | ... if class_decl.is_abstract
) {
    errors.push(ValidationError { error_type: ErrorKind::AbstractTypeInstantiation, ... });
}
```

The `PropertyType` enum covers every property kind the Concerto metamodel defines:

```rust
pub enum PropertyType {
    String(Option<StringValidator>),
    Boolean,
    Integer(Option<NumericValidator<i64>>),
    Long(Option<NumericValidator<i64>>),
    Double(Option<NumericValidator<f64>>),
    DateTime,
    Relationship { type_ref: String },
    ObjectRef { type_ref: String },
}
```

`StringValidator` holds `regex: Option<String>`, `min_length: Option<usize>`, `max_length: Option<usize>`. `NumericValidator<T>` is generic over `T: PartialOrd + Display + Copy` and holds inclusive `lower` and `upper` bounds. The same struct handles Integer, Long, and Double with the type parameter doing the work.

### 6.5 — The JSON Loader

`json_loader.rs` handles the full `$class` dispatch surface of the Concerto metamodel. The constants at the top of the file document every supported class string:

- Declarations: `ConceptDeclaration`, `AssetDeclaration`, `ParticipantDeclaration`, `TransactionDeclaration`, `EventDeclaration`, `EnumDeclaration`, `MapDeclaration`, plus all six scalar types
- Imports: `ImportAll` (wildcard), `ImportType` (single), `ImportTypes` (list with optional aliases)
- Properties: `StringProperty`, `IntegerProperty`, `LongProperty`, `DoubleProperty`, `BooleanProperty`, `DateTimeProperty`, `ObjectProperty`, `RelationshipProperty`, `EnumProperty`
- Validators: `StringRegexValidator`, `StringLengthValidator`, `IntegerDomainValidator`, `LongDomainValidator`, `DoubleDomainValidator`
- Map endpoints: all ten `*MapKeyType` and `*MapValueType` variants

The design choice I made here: unknown declaration kinds are skipped with a `log::warn!` rather than failing the whole load. Missing required fields on known types are hard `ConcertoError::Parse` errors. That split came from testing with real fixtures — a model that uses a custom extension type I don't know about should not prevent loading the types I do know about.

### 6.6 — Namespace and Import Resolution

`ModelManager` in `model_manager.rs` stores `HashMap<String, ModelFile>`. Each `ModelFile` holds its namespace's declarations and an `Imports` struct with two fields:

```rust
pub struct Imports {
    pub explicit: HashMap<String, String>,      // "Person" → "org.example@1.0.0.Person"
    pub wildcard_namespaces: Vec<String>,        // ["org.address@1.0.0", ...]
}
```

`resolve_type_in_context` handles the lookup chain:

1. If `type_name` looks fully qualified (`split_fqn` returns `Some`), go to `resolve_type` directly.
2. Check local declarations in `context_namespace`.
3. Check explicit imports.
4. Walk wildcard-imported namespaces. If exactly one match: return it. If more than one: return `ConcertoError::Semantic` (ambiguous). If zero: return `TypeNotFound`.

`ModelManager` also handles versionless namespace lookup. If a caller requests `org.example` but only `org.example@1.0.0` is loaded, `find_versionless_match` resolves it. If two versions are loaded, it returns `None` rather than guessing.

### 6.7 — The WASM Integration

The WASM crate (`concerto-wasm/src/lib.rs`) wraps `concerto-core` with `wasm-bindgen`. The wrapper is intentionally thin — the only logic it adds is converting between `JsValue` and `serde_json::Value`:

```
Browser / Node.js                      Rust compiled to WASM
─────────────────                      ─────────────────────

import { WasmModelManager }            #[wasm_bindgen]
  from './pkg/concerto_wasm.js'        pub struct WasmModelManager {
                                           inner: ModelManager,
const mm = new WasmModelManager()      }

mm.addModel(modelJson)  ────────────►  pub fn add_model(&mut self, json: &str)
                                           → self.inner.add_model_from_json(json)
                                           → Result<(), JsValue>

mm.addModelValue(jsObj) ────────────►  pub fn add_model_value(&mut self, model: JsValue)
                                           → serde_wasm_bindgen::from_value(model)
                                           → add_model(&json)

const result =
  mm.validateInstance(   ────────────►  pub fn validate_instance(
    instanceJson,                            &self,
    "org.example@1.0.0.Person"               instance_json: &str,
  )                                          type_name: &str,
                        ◄────────────      ) → Result<JsValue, JsValue>
                                               serde_wasm_bindgen::to_value(&result)
{ valid: true, errors: [] }

// also available: validateInstanceValue(jsObj, typeName)
// skips the JSON roundtrip if caller already has a JS object
```

There is also a standalone function `validateInstanceWithModel(model, instance, typeName)` for callers that do not want to manage a model manager lifetime.

The WASM binary is compiled to both `pkg/` (browser target, ES module) and `pkg-node/` (Node.js target, CommonJS). A working browser demo is at `concerto-wasm/demo/index.html`. A Node smoke test is at `concerto-wasm/scripts/node_smoke.mjs`.

### 6.8 — The C FFI Layer

The FFI crate (`concerto-ffi/src/lib.rs`) exposes a C ABI using `#[no_mangle] pub extern "C"` functions. The design uses an opaque handle:

```
C# / Python / Java                     Rust (.so / .dylib / .dll)
──────────────────                     ──────────────────────────

IntPtr mm =                            #[no_mangle]
  concerto_model_manager_new()         pub extern "C" fn
                                       concerto_model_manager_new()
                                           → *mut ConcertoModelManager
                                             (Box::into_raw(Box::new(...)))

IntPtr err =                           #[no_mangle]
  concerto_add_model(mm, modelJson)    pub unsafe extern "C" fn
// null → success                      concerto_add_model(
// non-null → error CString                mm: *mut ConcertoModelManager,
                                           json: *const c_char,
                                       ) → *mut c_char
                                           // null on success
                                           // owned error string on failure

IntPtr result =                        #[no_mangle]
  concerto_validate_instance(          pub unsafe extern "C" fn
    mm,                                concerto_validate_instance(
    instanceJson,                          mm: *const ConcertoModelManager,
    typeName)                              instance_json: *const c_char,
// parse JSON result                        type_name: *const c_char,
                                       ) → *mut c_char
                                           // JSON: {"valid":bool,"errors":[...]}

concerto_model_manager_free(mm)        #[no_mangle]
concerto_free_string(result)           pub unsafe extern "C" fn
                                       concerto_free_string(s: *mut c_char)
                                           // drop(CString::from_raw(s))
```

The Python demo at `concerto-ffi/demo/demo.py` uses `ctypes` to call this API end-to-end. It validates a valid `Person` instance and an invalid one (age out of range, extra field) and prints the JSON results. The comment at the top of `demo.py` documents the `restype` footgun: if you forget to set it, `ctypes` assumes `c_int` and corrupts the pointer on 64-bit systems. The demo sets `restype = ctypes.c_void_p` on all functions that return pointers.

The C header `concerto-ffi/include/concerto.h` documents the four functions with ownership semantics in comments.

### 6.9 — Conformance Test Suite

The `concerto-conformance` crate runs fixture-driven tests. Each scenario is a JSON file in `fixtures/scenarios/`:

```json
{
  "name": "abstract type cannot be instantiated",
  "model_files": ["models/abstract_model.json"],
  "instance_file": "instances/abstract_base_record.json",
  "type_name": "org.abstracts@1.0.0.BaseRecord",
  "expect": {
    "valid": false,
    "error_count": 1,
    "error_paths": ["$.$class"]
  }
}
```

`fixture_scenarios.rs` reads every file in `fixtures/scenarios/` automatically — adding a new scenario means dropping a JSON file, not writing Rust code.

The twelve scenarios currently passing:

| # | Scenario | What It Checks |
|---|---|---|
| 01 | Valid person instance passes | Happy path baseline |
| 02 | Missing required field | All missing fields reported, not just first |
| 03 | Unknown property rejected | Strict mode, extra fields are errors |
| 04 | Child accepted as parent | `is_assignable_to` / inheritance chain |
| 05 | Unrelated type rejected | `$class` mismatch with full path reported |
| 06 | Invalid datetime | `chrono::parse_from_rfc3339` rejection |
| 07 | Regex constraint | Email pattern `^[^@]+@[^@]+\.[^@]+$` |
| 08 | Invalid enum value | Enum membership check via `ObjectRef` |
| 09 | Invalid relationship | `resource:Type#id` URI format + type check |
| 10 | Abstract type rejection | `AbstractTypeInstantiation` at `$.$class` |
| 11 | Scalar constraint | Named scalar with underlying type and validator |
| 12 | Map validation | Key type + value type both validated |

Scenario 10 is worth calling out. The mentor said:

> *"Entity validation gives us probably 99% of model structural validation because you can validate against the meta model."*

The concrete mechanism: the Concerto metamodel is itself a Concerto model. Validating a user model file as an instance of the metamodel is entity validation. The POC does this in `concerto-validate-rs` (the original structural validation crate). The GSoC work unifies this: one entity validator that works on user instances and on metamodel instances.

### 6.10 — Technical Challenges Encountered

**Inheritance resolution across namespaces.** The first version of the supertype walk assumed all types lived in one namespace. That broke the moment a model imported a base type from a different namespace. The fix was passing `context_namespace` into `type_resolver::resolve_in_context` at every recursion step, which then delegates to `ModelManager::resolve_type_in_context` with the correct import lookup chain.

**Versioned vs unversioned `$class` strings.** Concerto instances use fully qualified names like `org.example@1.0.0.Person`. Older fixtures and some examples use `org.example.Person` without the version. Both show up in real data. I added `find_versionless_match` to `ModelManager`: if a versionless name is requested and exactly one versioned namespace matches the prefix, return it. If more than one version is loaded, refuse to guess. This is the rule the JS runtime uses and matching it was necessary for the conformance fixtures to pass.

**`serde_json::Number` doesn't know if it's an int or float.** JSON has one numeric type. Rust has many. Concerto has Integer, Long, and Double. The rule I settled on after checking how the JS validator behaves: Integer and Long accept values where `val.is_i64()` or where `val.as_f64()` has zero fractional part and fits in i64 range. Double accepts any JSON number including integers, because JS does the same thing and rejecting `{"price": 30}` for a Double field would break real contracts.

**Unicode string length.** `String::len()` in Rust counts bytes, not characters. A Japanese name with three characters costs 9 bytes in UTF-8. If the model says `maxLength=3` and you measure bytes, you reject valid data. The fix in `string_validator.rs` is one line:

```rust
let char_count = value.chars().count();
```

That took a non-obvious test to catch — I was checking non-ASCII strings against length constraints and the byte count was consistently wrong.

### 6.11 — What the POC Does Not Cover (and Why)

**CTO text parser.** The `.cto` text format is not handled — the POC loads JSON metamodel only. The mentor said explicitly: *"Any sort of inference or parsing would be a bonus."* `concerto-core/src/parser/cto_parser.rs` is a stub. Parsing CTO text is the stretch goal for Weeks 10–11.

**Upstream JS integration.** The WASM binary works as a standalone library. Wiring it as a drop-in inside `@accordproject/concerto-core` (so the JS `validateInstance` call optionally delegates to WASM) is a GSoC deliverable, not a POC deliverable.

**Cucumber conformance.** The POC uses a JSON fixture format with a Rust test harness. The project requires Cucumber. Adding Cucumber is Week 2 of the GSoC timeline — the fixture format I built maps directly to Cucumber feature files.

**`@openapi` decorator.** The `@openapi` decorator on a Concerto model relaxes the unknown-property check. The POC always runs in strict mode. Decorator parsing is out of scope.

---

## 7. Architecture and Design

The full system design for the complete GSoC deliverable:

```
Complete concerto-rs System Architecture

Input paths:
  .cto text ───────────► [cto_parser (pest grammar)]
                                    │
  Metamodel JSON ──────► [json_loader (serde_json)]
                                    │
                                    ▼
                             ModelManager
                        (namespace HashMap<String, ModelFile>)
                        (versionless fallback lookup)
                        (import resolution: explicit + wildcard)
                                    │
                    JSON instance ──┤
                    + type_name ────┤
                                    │
                           validate_instance()
                                    │
                           ValidationResult
                          { valid: bool,
                            errors: Vec<ValidationError> }
                          (path + message + ErrorKind)
                                    │
              ┌─────────────────────┼─────────────────────┐
              │                     │                      │
   ┌──────────▼──────────┐  ┌──────▼──────────┐  ┌───────▼──────┐
   │   concerto-wasm     │  │  concerto-ffi   │  │ concerto-cli │
   │   wasm-bindgen      │  │  C ABI cdylib   │  │ clap 4       │
   │   WasmModelManager  │  │  opaque handle  │  │ 4 subcommands│
   │   two API surfaces: │  │  null=ok error  │  │ validate     │
   │   string + JsValue  │  │  convention     │  │ check        │
   └──────────┬──────────┘  └──────┬──────────┘  │ info         │
              │                    │              │ bench        │
   ┌──────────▼──────────┐  ┌──────▼──────────┐  └───────┬──────┘
   │  Browser (ESM pkg)  │  │  Python ctypes  │          │
   │  Node.js (CJS pkg)  │  │  C# P/Invoke    │   stdout │
   │  Deno / edge runtme │  │  Java JNA       │          ▼
   └─────────────────────┘  └─────────────────┘  Terminal output
```

The central design principle: `ModelManager` is the only stateful object. The validator is a pure function `(ModelManager, Value, &str) → ValidationResult`. This means:

- **WASM**: `WasmModelManager` wraps one `ModelManager`. `wasm-bindgen` makes it a JS class. Safe because WASM is single-threaded by default.
- **FFI**: `ConcertoModelManager(ModelManager)` is heap-allocated via `Box::into_raw`. Callers own the lifetime through `concerto_model_manager_free`. Safe because the null-pointer checks at every entry point handle the FFI contract.
- **CLI**: `ModelManager` lives on the stack in `main`. No allocation ceremony.

This also means the real runtime integration with the JS `@accordproject/concerto-core` package is a one-function change: replace the JS `validateInstance` call with a WASM import that calls `WasmModelManager.validateInstance`. The model loading path can stay in JS for now — only the hot path (validation) needs to move to WASM.

---

## 8. Integration Plan — How the POC Becomes Real GSoC Work

The POC proves four things:

1. The `ModelManager` → `validate_instance` architecture works end-to-end on real fixtures.
2. WASM bindings via `wasm-bindgen` are achievable, with a working browser demo.
3. FFI via C ABI is achievable, with a working Python demo.
4. The conformance fixture format is extensible without writing new Rust code.

What changes in the actual GSoC project:

**Scope moves upstream.** The POC lives in a fork. GSoC contributions go directly into the Accord Project organization repository. The first PR will establish the workspace structure.

**Conformance expands from 12 to 30+ scenarios.** The official `accordproject/concerto-conformance` repository has far more scenarios than my fixtures cover. GSoC work maps those to both Cucumber feature files and the existing JSON fixture format.

**Cucumber replaces the custom harness.** The `fixture_scenarios.rs` test runner is fine for a POC. The project requires Cucumber. The JSON scenario format I designed maps cleanly to Cucumber: `name` → scenario title, `model_files` → background steps, `instance_file` + `type_name` → when step, `expect.valid` → then step.

**WASM integrates into `@accordproject/concerto-core`.** The current WASM output is a standalone library. The GSoC deliverable wires it as an optional fast path inside the existing JS package, with a feature flag to fall back to the JS validator.

**Published to crates.io.** `concerto-core` gets proper `Cargo.toml` metadata (`description`, `keywords`, `categories`, `repository`, `license`) and publishes.

---

## 9. Deliverables with Milestones

### Milestone 1 — Core Validator (Weeks 1–4)

| Deliverable | Type |
|---|---|
| `ModelManager` with `add_model_from_json`, all declaration kinds | Required |
| `validate_instance` for all primitive types with type checking | Required |
| String constraint validators: regex and length | Required |
| Numeric constraint validators: Integer, Long, Double bounds | Required |
| Full supertype chain resolution with cycle detection | Required |
| 20+ conformance scenarios passing (Cucumber format) | Required |
| Cross-namespace import resolution (explicit + wildcard) | Required |
| Map declaration and map instance validation | Optional |

### Milestone 2 — Multi-Platform (Weeks 5–8)

| Deliverable | Type |
|---|---|
| `concerto-wasm` compiling and conformance tests running via WASM | Required |
| Working browser demo (`demo/index.html`) | Required |
| `concerto-ffi` C ABI with `concerto.h` | Required |
| Working Python demo with ctypes | Required |
| CLI: `validate`, `check`, `info`, `bench` subcommands | Required |
| Criterion benchmarks with numbers against JS baseline | Required |
| npm package via `wasm-pack` | Optional |
| `cbindgen` auto-generating `concerto.h` | Optional |

### Milestone 3 — Integration and Polish (Weeks 9–12)

| Deliverable | Type |
|---|---|
| WASM integrated as optional fast path in `@accordproject/concerto-core` JS | Required |
| Full conformance suite pass (all scenarios from official repo) | Required |
| `concerto-core` published to crates.io | Required |
| Complete rustdoc API documentation | Required |
| CTO text parser (pest grammar, basic namespace/concept/property) | Optional |
| Blog post for accordproject.org/news | Optional |

---

## 10. Week-by-Week Timeline

```
Timeline: GSoC 2026 (12 weeks)

  Phase 1: Core          Phase 2: Platforms      Phase 3: Integration
  Wks 1–4               Wks 5–8                  Wks 9–12
  │                      │                         │
  ├─ Wk1: Model loading  ├─ Wk5: WASM bindings     ├─ Wk9:  JS integration
  ├─ Wk2: Basic validate ├─ Wk6: FFI + Python demo ├─ Wk10: CTO parser (stretch)
  ├─ Wk3: Constraints    ├─ Wk7: CLI + benchmarks  ├─ Wk11: Cargo publish + docs
  ├─ Wk4: Inheritance    ├─ Wk8: Conformance ext.  └─ Wk12: Final pass + blog
  │
  Midterm eval: end of Wk4        Final eval: end of Wk12
```

**Community Bonding (pre-GSoC)**

- Week −3: Set up contribution environment. Open a small first PR to the Concerto repo (documentation fix or issue triaging) to get the workflow right before the real work starts.
- Week −2: Deep read of `accordproject/concerto-conformance`. Understand the Cucumber feature file format and step definition conventions. Map each existing JSON fixture to its equivalent `.feature` file.
- Week −1: Sync with Ertugrul on the agreed repo structure and API surface. Specifically: whether `concerto-core` lives in a new organization repo or inside `concerto-validate-rs`; what the WASM API should look like for the JS integration target.
- Week 0 (Community Bonding): Attend TWG calls. Finalize week-by-week plan. Resolve any open architecture questions. No code — just alignment.

**Phase 1: Core Validator**

*Week 1 — Model loading*

Tasks: Establish the workspace in the upstream repo. Implement `ModelManager::new()`, `add_model_from_json()`, and all declaration parsing from `json_loader.rs`. Implement `ModelFile`, `Imports`, explicit and wildcard import parsing.

Commit: working model loader with unit tests for all declaration kinds. Passes `cargo clippy -- -D warnings` and `cargo fmt --check`.

Mentor review: architecture and public API surface.

*Week 2 — Basic entity validation + Cucumber*

Tasks: Implement `validate_instance()` for primitive types only (String, Boolean, Integer, Long, Double, DateTime) — type checking and required/optional/unknown field checks. No constraints yet. Set up `concerto-conformance` with Cucumber infrastructure. First 8 scenarios passing.

Commit: working basic entity validator + Cucumber harness.

Mentor review: Cucumber step definition format, error message quality.

*Week 3 — Constraint validators*

Tasks: Implement `StringRegexValidator`, `StringLengthValidator` (with `chars().count()`), `IntegerDomainValidator`, `LongDomainValidator`, `DoubleDomainValidator`. Implement `isArray` handling with per-element validation. Add enum validation via `ObjectRef → EnumDeclaration`. 15 scenarios passing.

Commit: full constraint validation, regex cache in `string_validator.rs`.

*Week 4 — Inheritance, map, scalar, buffer*

Tasks: Full supertype chain resolution with cycle detection (recursive `collect_properties_inner` + visited set). Cross-namespace supertype resolution. `ScalarDeclaration` validation. `MapDeclaration` validation (key type + value type). 20+ scenarios passing.

Commit: complete `validate_instance` covering all Concerto types.

Mentor review: inheritance behavior matches JS validator output. Midterm evaluation prep.

**Phase 2: Multi-Platform**

*Week 5 — WASM*

Tasks: `concerto-wasm` crate with `wasm-bindgen`. Both string and `JsValue` API surfaces. Compile to both web and Node.js targets. Port conformance tests to run against WASM bindings. Working `demo/index.html`.

Commit: WASM bindings + browser demo.

Mentor review: API ergonomics for JS callers.

*Week 6 — FFI*

Tasks: `concerto-ffi` crate with full C ABI. Four functions: `concerto_model_manager_new`, `concerto_model_manager_free`, `concerto_add_model`, `concerto_validate_instance`, `concerto_free_string`. `concerto.h` header (hand-written or via `cbindgen`). Working Python demo.

Commit: FFI layer + Python demo.

*Week 7 — CLI and benchmarks*

Tasks: `concerto-cli` with four subcommands (`validate`, `check`, `info`, `bench`). Criterion benchmarks comparing Rust validator performance to JS baseline. Benchmark numbers in README.

Commit: CLI + benchmark results.

Note: If benchmark numbers show no meaningful improvement over JS for the tested workloads, I will document why honestly and not invent numbers.

*Week 8 — Conformance expansion + buffer*

Tasks: Extend conformance suite to 30+ scenarios covering cases from the official `concerto-conformance` repository. Fix any cross-platform issues found on Linux vs macOS. Buffer for Phase 2 overflow.

Commit: extended test suite. Second evaluation prep.

**Phase 3: Integration and Polish**

*Week 9 — JS runtime integration*

Tasks: Wire the WASM module as an optional fast path in `@accordproject/concerto-core`. The integration point is the `validateInstance` function in the JS package. When the WASM module is available, delegate validation to it; fall back to the JS implementation otherwise. Write integration tests that run both paths on the same fixtures and compare results.

Commit: PR to `@accordproject/concerto-core`.

*Week 10 — CTO parser (stretch) or conformance depth*

Tasks: If the stretch goal is in scope, start a `pest` grammar for `.cto` files covering namespace declaration, concept/asset/participant declarations, and primitive property types. If the schedule does not allow it, spend the week on full conformance suite coverage instead.

Commit: CTO parser stub with basic tests, or extended conformance scenarios.

*Week 11 — Publishing and documentation*

Tasks: Add `Cargo.toml` publishing metadata to `concerto-core` (`description`, `keywords = ["concerto", "accord-project", "schema", "validation"]`, `categories`, `repository`, `license = "Apache-2.0"`). Run `cargo doc` and fill any missing `///` comments. Publish to crates.io. Publish WASM as npm package via `wasm-pack publish`.

Blog post draft for accordproject.org/news describing what was built, how to use it, and what comes next.

*Week 12 — Final pass*

Tasks: Full conformance suite pass — every scenario must pass. Final mentor review session. Submit work product report. Publish blog post.

---

## 11. Related Work

**`concerto-validate-rs` (existing POC).** Validates Concerto model ASTs structurally against the metamodel. Does not validate user instances. Inheritance walk is one level deep. Reports first error only. No WASM, no FFI. My work extends what this POC proved about the JSON loading approach.

**`concerto-rust` (existing POC).** Has a model manager and declaration structures. Instance validator is not implemented. My POC picks up where this stops.

**`concerto-dotnet`.** The .NET binding for Concerto embeds the Jint JavaScript interpreter to execute the JS validator. This is exactly the problem this project solves: Jint is a dependency, adds startup cost, and does not give you native performance. A C FFI binding into a Rust `.dll` eliminates both.

**`jsonschema-rs`.** Demonstrates that JSON schema validation in Rust compiled to WASM is a proven pattern at production scale. Used by Biome (formerly Rome). The Concerto type system is more expressive than JSON Schema in some dimensions (Concerto has Relationships, Scalars, Maps) and less in others. The architecture lessons transfer directly.

---

## 12. About Me

I am 19, a sophomore in B.Tech CSE at IIIT Sonepat. I started learning Rust seven months ago, coming from a background in Java and C++. The borrow checker took about a week to click — the moment it did, I stopped fighting it and started using it as a design tool. I reach for Rust now when I want the compiler to tell me about ownership bugs before they happen at runtime.

My two open source contributions are both to large, production Rust codebases. For `rust-clippy` PR #16549, I implemented the `clear_instead_of_new` lint. That went through four rounds of review from Samuel Tardieu. Each round meant reading reviewer comments carefully, understanding why my first approach was wrong, and rewriting the relevant piece. Getting all 1729 UI tests passing required understanding how the clippy test infrastructure works, not just the lint logic. For `cargo` PR #16653, I improved the error message for unsupported edition declarations. Smaller change, but it meant reading the cargo resolver to understand where the error originated.

Both contributions taught me the same thing: reading a large codebase carefully before writing a single line is faster than writing first and debugging later. I read the Concerto JS runtime, the two existing Rust POCs, and the TWG call transcript before writing the first struct in this POC.

I care about this project specifically because entity validation in Concerto is essentially a type-checker for data — the same class of problem as compiler type inference, just over JSON instead of source code. I have been spending time in compiler design materials and have a personal project in static analysis. The Concerto validator is where those interests connect to a real deployment problem that legal platforms actually have.

The mentor said the scope and structure of the runtime is what they want to see in proposals. The POC is my answer to that: not a plan for a thing I might build, but a working sketch of a thing I already built, with the gaps documented honestly.

---

## 13. Time Commitment and Availability

I will be available 30–35 hours per week during the GSoC period. IST is UTC+5:30, which overlaps with European morning hours and US evening hours — both reasonable windows for mentor sync calls.

I have no other internship or major employment commitment during GSoC. My semester exams fall in late May (pre-GSoC) and I will note specific exam weeks during the community bonding period so the mentor can plan review cycles around them.

I will attend the Accord Project TWG calls for the duration of the project. I treat mentor feedback as the highest priority input — the four rounds of clippy review taught me that reviewer time is the scarcest resource in open source and I do not want to waste it by submitting half-finished work.

The codebase for this proposal is at [github.com/accordproject/concerto-validate-rs](https://github.com/accordproject/concerto-validate-rs). Every technical claim in this proposal is verifiable against the source. If something does not match the code, that is a bug in the proposal and I want to know.

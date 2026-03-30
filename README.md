# concerto-rs

My GSoC 2026 POC for porting Concerto's entity validation to Rust.

Concerto is a schema language used around legal contracts and related data. The
runtime today is mostly JavaScript, which is fine in Node, but less fun when a
Python or C# app just wants to validate some JSON and suddenly needs a whole
Node process hanging around too.

Our mentor called entity validation the hotspot, so that's what my repo
focuses on. CTO parsing is not part of this POC. I left it out intentionally so the prototype could focus on entity validation first.

Repository: [accordproject/concerto-validate-rs](https://github.com/accordproject/concerto-validate-rs)

## Why

The GSoC 2026 idea list says "Create a new Concerto runtime for multi platform
deployment". The concrete parts I took from that were:

- move validation logic into Rust
- keep checking behavior against conformance-style scenarios
- make the same core callable from JS through WASM

That is basically the shape of this workspace.

## What Works

- load Concerto metamodel JSON into a Rust model registry
- validate JSON instances against named Concerto types
- collect all validation errors, not just the first one
- required fields, unknown fields, and `$class` checks
- primitive property types: `String`, `Integer`, `Long`, `Double`, `Boolean`, `DateTime`
- string regex and length validators
- numeric lower/upper bounds for integer, long, and double
- inheritance across the full supertype chain
- enum validation
- relationship URI validation
- scalar declarations
- map declarations
- import-aware type resolution for loaded models
- CLI wrapper
- WASM wrapper for browser/Node-style callers
- C FFI wrapper plus Python demo
- regression tests plus fixture-based conformance scenarios

## What Does Not Work Yet

- CTO text parsing. Right now the real path is JSON metamodel only
- upstream JS runtime integration in the actual Concerto repo
- upstream cucumber-based conformance wiring
- `@openapi` decorator behavior
- broader runtime features outside entity validation

## Build And Run

All of these were tested from the workspace root.

```bash
cargo build --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo fmt --check
```

### CLI

```bash
cargo run -p concerto-cli -- validate \
  --model ./model.json \
  --instance ./instance.json \
  --type "org.example@1.0.0.Person"
```

Other subcommands:

```bash
cargo run -p concerto-cli -- check --model ./model.json
cargo run -p concerto-cli -- info --model ./model.json
cargo run -p concerto-cli -- bench \
  --model ./model.json \
  --instance ./instance.json \
  --type "org.example@1.0.0.Person" \
  --iterations 1000
```

### WASM

```bash
cd concerto-wasm
wasm-pack build --target web
cd demo
python3 -m http.server 8080
```

Open [http://localhost:8080](http://localhost:8080).

There is also a Node smoke path:

```bash
cd concerto-wasm
wasm-pack build --target nodejs --out-dir pkg-node
node scripts/node_smoke.mjs
```

### FFI

```bash
cargo build --release -p concerto-ffi
python3 concerto-ffi/demo/demo.py
```

## Architecture

```text
                 +----------------------+
                 |   concerto-cli       |
                 |   debug / bench      |
                 +----------+-----------+
                            |
                            |
+-------------------+       v        +----------------------+
| concerto-wasm     |--------------->|   concerto-core      |
| browser / JS FFI  |                |   model + validator  |
+-------------------+                +----------------------+
                            ^
                            |
                            |
                 +----------+-----------+
                 |   concerto-ffi       |
                 |   C ABI / Python     |
                 +----------------------+

                 +----------------------+
                 | concerto-conformance  |
                 | fixture/regression    |
                 +----------------------+
```

## Notes From Building This

**Inheritance was the hardest part.** First version only walked one supertype
up. Looked fine on small examples, then immediately broke on deeper chains.
Had to rewrite it to recurse properly and keep a visited set for circular
inheritance. Took most of a day.

**`$class` resolution was sneakier than I expected.** There are versioned names
like `org.example@1.0.0.Person` and older-looking unversioned ones like
`org.example.Person`. Both show up in fixtures. If only one format works, the
runtime feels random.

**`serde_json::Number` is annoying here.** Rust obviously cares about integer vs
float. JSON kind of does, kind of doesn't. JS mostly doesn't. Turns out the
right compatibility move was: Integer/Long require `is_i64()`, Double accepts
any JSON number.

**String length had a unicode footgun.** `.len()` counts bytes. Needed
`.chars().count()`. Caught that while checking non-ASCII strings and it would
have been a very embarrassing bug to leave in.

**The model manager got bigger than planned.** I expected it to just hold a
`HashMap`. Then imports, versionless lookups, and requested-type vs instance
type checks all piled into the same place. In hindsight that's fine. Better
one file owns the weird name resolution rules.

**What I'd probably change next:** build a type index when models are loaded.
Some lookups still scan linearly. Fine for a POC. Not what I'd ship if this
turns into the real runtime.

## Benchmarks

Run this on your own machine:

```bash
cargo bench --package concerto-core --bench validation_bench -- --noplot
```

I am not putting fake numbers in the README. Whatever I measured on one laptop
last week is not a useful benchmark table for everyone else.

## Author

Yash Kumar - IIIT Sonepat  
GitHub: [@yash-kumarx](https://github.com/yash-kumarx)  
Mentors: Ertugrul Karademir ([@ekarademir](https://github.com/ekarademir)) & Jamie Shorten ([@jamieshorten](https://github.com/jamieshorten))

# WASM Integration Design

## Overview
This document outlines the integration of WebAssembly (WASM) support for ReConf, enabling a browser-based playground for writing, evaluating, and debugging ReConf configurations.

---

## Project Structure
```
reconf/
├── Cargo.toml              # Updated with WASM dependencies
├── src/
│   ├── lib.rs              # NEW: WASM entry point (re-exports cli.rs logic)
│   ├── cli.rs              # Existing CLI (unchanged)
│   ├── error.rs            # Existing error handling
│   ├── diagnostic.rs       # Existing diagnostic logic
│   └── ...                 # Other existing modules
│
├── wasm/                   # NEW: WASM-specific files
│   ├── Cargo.toml          # WASM workspace member
│   ├── src/
│   │   └── lib.rs          # WASM bindings (re-exports ../src/lib.rs)
│   └── build.sh            # Script to build WASM
│
├── web/                    # NEW: Web demo
│   ├── package.json        # Frontend dependencies (Monaco, Svelte/React)
│   ├── public/
│   │   └── index.html      # HTML entry point
│   ├── src/
│   │   ├── main.js         # Frontend entry point (loads WASM)
│   │   ├── editor.js       # Monaco editor setup
│   │   └── styles.css      # Basic styling
│   └── vite.config.js      # Build config (Vite/Rollup)
│
├── examples/               # Existing examples (reused in web demo)
│   ├── simple.reconf
│   └── modules/
│       ├── main.reconf
│       └── lib.reconf
│
└── scripts/
    └── build_wasm.sh       # Script to build WASM and copy to web/public
```

---

## Core Components

### 1. WASM Interface
The WASM module exposes two main functions:

#### `check_reconf(code: &str, filename: &str) -> Result<(), JsValue>`
- Validates ReConf code for syntax and type errors.
- Returns `Ok(())` if valid, or `Err(JsValue)` with serialized diagnostics.

#### `eval_reconf(code: &str, filename: &str, format: &str) -> JsValue`
- Evaluates ReConf code and returns the result in the specified format (`json` or `reconf`).
- Returns a JSON-serialized `EvalResult`:
  ```rust
  struct EvalResult {
      success: bool,
      output: Option<String>,  // JSON/ReConf output on success
      diagnostics: Vec<Diagnostic>,  // Errors/warnings
  }
  ```

#### `Diagnostic` Structure
```rust
#[wasm_bindgen]
pub struct Diagnostic {
    pub code: String,       // e.g., "E_REFINE_004"
    pub message: String,    // Human-readable error
    pub line: usize,        // 1-based line number
    pub column: usize,      // 1-based column number
    pub length: usize,      // Span length in bytes
    pub label: String,      // Short annotation for UI
}
```

---

### 2. Frontend Integration
The web demo uses:
- **Monaco Editor**: For syntax highlighting and editing.
- **Vite**: For bundling and hot-reloading.
- **WASM Worker**: Optional Web Worker for heavy computations.

#### Key Files
- `web/src/main.js`: Initializes WASM and sets up the editor.
- `web/src/editor.js`: Configures Monaco with ReConf syntax highlighting.
- `web/vite.config.js`: Build configuration.

---

## Implementation Steps

### 1. Update `Cargo.toml`
Add WASM dependencies:
```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wasm-bindgen = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6"
console_log = "1.0"
```

### 2. Create `src/lib.rs`
Re-export CLI logic for WASM:
```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn check_reconf(code: &str, filename: &str) -> Result<(), JsValue> {
    // Reuse `eval_path` but skip evaluation
    // Return diagnostics as JsValue
}

#[wasm_bindgen]
pub fn eval_reconf(code: &str, filename: &str, format: &str) -> JsValue {
    // Return EvalResult as JsValue
}
```

### 3. Set Up `wasm/` Directory
- `wasm/Cargo.toml`: WASM workspace member.
- `wasm/src/lib.rs`: Re-exports `../src/lib.rs`.
- `wasm/build.sh`: Build script for WASM.

### 4. Create Web Demo
- Initialize a Vite project in `web/`.
- Configure Monaco with ReConf syntax highlighting.
- Load and interact with the WASM module.

---

## Build Workflow

### 1. Build WASM
```sh
cd wasm
wasm-pack build --target web
cp pkg/* ../web/public/wasm
```

### 2. Build Web Demo
```sh
cd web
npm install
npm run build
```

### 3. Serve Locally
```sh
cd web
npm run dev
```

---

## Error Handling
Errors are serialized to JSON for JavaScript interop. The frontend uses these to:
- Highlight syntax errors in Monaco.
- Show tooltips with error messages.
- Display refinement failures.

---

## Testing
Verify the following scenarios:
1. **Syntax Errors**: Unclosed braces, invalid tokens.
2. **Type Errors**: Mismatched types, unknown fields.
3. **Refinement Failures**: Values violating predicates (e.g., `port: 80` for `Port > 1024`).
4. **Successful Evaluation**: Valid configurations output JSON/ReConf.

---

## Next Steps
1. Implement `src/lib.rs` with WASM bindings.
2. Set up the `wasm/` directory and build script.
3. Create the web demo skeleton with Monaco and Vite.
4. Integrate WASM with the frontend.

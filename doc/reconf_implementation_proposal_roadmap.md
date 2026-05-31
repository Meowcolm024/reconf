# ReConf Implementation Proposal and Roadmap

## 1. Executive Summary

ReConf is specified as a small, configuration-first language with a simply typed lambda-calculus core, refinement types, deterministic normalization, modules, records, lists, options, string interpolation, and bidirectional type checking. The repository is currently in a specification-heavy state: the design documents describe the intended language and compiler pipeline, while the Rust implementation is still effectively a scaffold.

This proposal recommends building ReConf as a **spec-driven Rust compiler/interpreter** with a deliberately small MVP, a conformance-test-first workflow, and a staged path from parsing to type checking, normalization, refinement validation, module loading, diagnostics, and CLI polish.

The central implementation principle should be:

> Treat ReConf as a typed normalizing configuration evaluator, not as a general-purpose programming language.

That means the implementation should prioritize predictable behavior, crisp errors, deterministic output, a small core calculus, and a test suite that encodes the language spec before broad feature expansion.

## 2. Current Repository Assessment

### 2.1 What already exists

The repository contains:

- `README.md`, describing ReConf’s goals, project status, high-level language features, and compiler pipeline.
- `doc/DESIGN.md`, the main language-design document, including source-file structure, surface syntax, core syntax, static semantics, refinement checking, normalization, errors, and ABNF grammar.
- `doc/DRAFT.md`, a shorter narrative draft with examples for modules, literal unions, optional fields, implicit `some`, record refinements, lambdas, and interpolation.
- `Cargo.toml`, defining a Rust package named `reconf`, currently with no dependencies.
- `src/main.rs`, currently only a hello-world program.

### 2.2 Key design commitments to preserve

The implementation should preserve the following commitments from the current design:

1. **Configuration first.** ReConf should produce normalized configuration data, not arbitrary executable programs.
2. **Surface/core split.** User-facing sugar should be lowered into a smaller core language.
3. **Bidirectional shape checking before refinement checking.** Ordinary type checking establishes shape first; refinement validation happens afterward on normalized values.
4. **Purity and termination.** Refinement predicates must be pure, deterministic, and terminating.
5. **Closed records.** Record values checked against record types must not contain extra fields.
6. **Non-recursive declarations and lets.** Recursive values and recursive type aliases are out of scope.
7. **Functions are internal only.** Functions may participate in checking and normalization, but cannot escape into emitted output.
8. **Deterministic module resolution.** Imports should be resolved by canonical file path; cycles and invalid imports should be rejected.
9. **Contextual elaboration.** Optional-field omission and implicit `some` should be handled during checking, because both require an expected type.

## 3. Implementation Goals

### 3.1 MVP goal

Build a command-line ReConf implementation that can:

```sh
reconf check path/to/file.reconf
reconf eval path/to/file.reconf --format json
```

For the MVP, `check` should parse, resolve, lower, type-check, normalize, and validate refinements. `eval` should additionally print normalized data.

### 3.2 Success criteria

The MVP should be considered successful when it can:

- Parse the ABNF-compatible surface language for the MVP subset.
- Resolve imports and exported top-level declarations across multiple files.
- Lower literal unions, method calls, and string interpolation into core syntax.
- Elaborate omitted option fields and implicit `some` using expected types.
- Type-check base types, options, lists, records, functions, annotations, and simple refinements.
- Normalize expressions deterministically.
- Reject function-valued final outputs.
- Evaluate refinement predicates to `true` or `false`, and reject unknown predicates.
- Emit deterministic normalized JSON for data values.
- Provide source-span diagnostics for common errors.
- Pass a conformance suite of positive and negative `.reconf` files.

### 3.3 Non-goals for the first implementation

The first implementation should **not** include:

- Recursive functions or recursive type aliases.
- General union types beyond string literal unions.
- User-defined effects, IO, environment variables, network access, or filesystem reads beyond imports.
- Dependent types beyond first-order refinement predicates over normalized values.
- Constraint solving or SMT-backed refinement verification.
- Subtyping beyond the minimal structural/shape relation required by checking.
- YAML/TOML emitters, unless JSON output is already stable.
- Package management or remote module imports.
- IDE/LSP integration.

## 4. Proposed Architecture

The implementation should be organized as a small compiler pipeline with explicit intermediate representations.

```text
source files
   |
   v
lexer / parser  ---> surface AST
   |
   v
module resolver ---> resolved module graph
   |
   v
syntax-directed lowering
   |
   v
core AST with unresolved contextual sugar removed where possible
   |
   v
bidirectional type checker + type-directed elaborator
   |
   v
fully elaborated core AST
   |
   v
normalizer
   |
   v
refinement validator
   |
   v
normalized data output
```

### 4.1 Suggested Rust module layout

```text
src/
  main.rs                 # thin CLI entry point
  lib.rs                  # public compiler API
  cli.rs                  # argument parsing and command dispatch
  source.rs               # source files, file IDs, spans, interner
  diagnostic.rs           # structured errors and reporting
  syntax/
    mod.rs
    token.rs              # lexer tokens
    lexer.rs
    parser.rs
    surface.rs            # surface AST
  core/
    mod.rs
    ast.rs                # core expr/type/value definitions
    pretty.rs             # debug printing and snapshots
  resolve/
    mod.rs
    modules.rs            # import graph, exports, cycle detection
    names.rs              # name scopes and binding IDs
  lower/
    mod.rs
    desugar.rs            # literal unions, method syntax, interpolation
  typeck/
    mod.rs
    env.rs
    wf.rs                 # well-formedness of types
    bidir.rs              # synth/check judgments
    elaborate.rs          # omitted fields and implicit some
    unify.rs              # structural equality / compatibility
  eval/
    mod.rs
    value.rs
    normalize.rs
    builtins.rs
  refine/
    mod.rs
    validate.rs
  emit/
    mod.rs
    json.rs
  tests/
    fixtures.rs           # test harness helpers, if kept in-tree
```

For a small initial implementation, these can live in one crate. If the project grows, split into crates later:

```text
reconf-syntax
reconf-core
reconf-compiler
reconf-cli
```

Do not split too early; a single crate will keep iteration faster while the spec is evolving.

## 5. Core Data Model

### 5.1 Surface AST

The surface AST should preserve syntax that matters for diagnostics and later lowering:

- imports and exports
- declarations
- type aliases
- string literal unions
- interpolated strings
- method calls
- optional type annotations
- source spans on every node

Surface AST nodes should be close to the grammar and should not attempt type-directed elaboration.

### 5.2 Core AST

The core AST should remove purely syntactic conveniences:

- no import/export syntax
- no string literal unions
- no method syntax
- no interpolated string syntax
- no omitted optional fields after elaboration
- no implicit `some` after elaboration

A representative core type model:

```rust
pub enum Ty {
    Int,
    Float,
    Bool,
    String,
    Option(Box<Ty>),
    List(Box<Ty>),
    Record(Vec<FieldTy>),
    Refine {
        binder: Name,
        base: Box<Ty>,
        pred: ExprId,
    },
    Fun(Box<Ty>, Box<Ty>),
    Alias(Name), // optional before expansion; avoid after normalization
}
```

A representative core expression model:

```rust
pub enum Expr {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Var(BindingId),
    None,
    Some(ExprId),
    List(Vec<ExprId>),
    Record(Vec<FieldExpr>),
    Field(ExprId, Name),
    If { cond: ExprId, then_: ExprId, else_: ExprId },
    Let { name: Name, ann: Option<TyId>, value: ExprId, body: ExprId },
    Lam { param: Name, param_ty: TyId, body: ExprId },
    App(ExprId, ExprId),
    Ascribe(ExprId, TyId),
    Prim(PrimId),
}
```

Use IDs or arenas rather than deeply nested boxes if diagnostics and sharing become cumbersome. For the first pass, boxed ASTs are acceptable, but arenas often make source-span tracking and snapshots cleaner.

### 5.3 Runtime values

Values should distinguish data from functions:

```rust
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    None,
    Some(Box<Value>),
    List(Vec<Value>),
    Record(Vec<FieldValue>),
    Closure(Closure),
    Prim(PrimId),
    PrimPartial(PrimId, Vec<Value>),
}
```

The emitter must reject `Closure`, `Prim`, and `PrimPartial` at the final output boundary.

## 6. Semantic Strategy

### 6.1 Type aliases

Type aliases should be transparent. The checker should expand aliases when comparing types or checking well-formedness. Recursive aliases should be rejected during alias expansion using an expansion stack.

### 6.2 Refinement shape erasure

Refinements should be treated as ordinary base shape plus a validation predicate. During ordinary type checking:

```text
erase({ x : T | p }) = erase(T)
```

Then refinement validation runs after the value is normalized.

This avoids accidentally turning the MVP into an SMT-based dependent type system.

### 6.3 Bidirectional checking

Implement two mutually recursive judgments:

```text
synth(ctx, expr) -> Ty
check(ctx, expr, expected_ty) -> ElaboratedExpr
```

Use `check` when there is an expected type:

- annotated top-level lets
- annotated local lets
- expression ascriptions
- lambda parameters and bodies when checking against function types
- record literals checked against record types
- option values checked against option types
- list elements checked against known list element types
- values checked against refinement types

Use `synth` for literals, variables, field access, applications, ascriptions, and annotated lambdas.

### 6.4 Option elaboration

When checking against `T?`:

1. `none` elaborates to `none : T?`.
2. `some e` checks `e` against `T`.
3. An expression that synthesizes `T?` is accepted directly.
4. Otherwise, check the expression against `T` and elaborate to `some e`.

This exactly captures the intended contextual nature of implicit `some`.

### 6.5 Optional record fields

When checking a record literal against a known record type:

- reject duplicate fields
- reject unknown fields
- check provided fields against expected field types
- insert omitted fields only when their expected type is `T?`
- reject missing non-option fields
- preserve deterministic field order according to the record type

This deterministic field ordering should also drive output emission.

### 6.6 Refinement validation

For a value `v` checked against `{ x : T | p }`:

1. Check `v` against `T`.
2. Normalize `v`.
3. Bind `x` to the normalized value.
4. Normalize `p` under that environment.
5. Accept iff the predicate normalizes to `true`.
6. Reject iff it normalizes to `false`.
7. Emit `unknown predicate` iff it cannot normalize to a boolean.

The MVP should avoid symbolic reasoning. It should only evaluate pure predicates over concrete normalized values.

### 6.7 Evaluation and normalization

A straightforward big-step evaluator is enough for the first implementation:

```text
eval(env, expr) -> Value
```

Because recursion is absent and built-ins operate over finite values, evaluation should terminate unless a bug or future feature violates the language restriction.

The evaluator should:

- evaluate `if` only after the condition normalizes to `Bool`
- evaluate field access only on records
- evaluate function application using closures or built-ins
- evaluate list operations over finite lists
- preserve deterministic record field order
- fail cleanly on stuck internal states

### 6.8 Built-ins

Start with the smallest useful standard environment:

| Category | MVP built-ins |
|---|---|
| Arithmetic | `+`, `-`, `*`, `/`, `%` for `Int`; optionally `Float` separately |
| Comparison | `==`, `!=`, `<`, `<=`, `>`, `>=` |
| Boolean | `&&`, `||`, `!` |
| String | `++`, `contains`, `startsWith`, `endsWith`, `show` |
| List | `length`, `contains`, `all`, `any` |
| Option | `isSome`, `isNone`, `unwrapOr` |

Defer `map` and `filter` until function values and higher-order built-ins are stable. They are useful, but they increase evaluator, type-checker, and diagnostics complexity.

### 6.9 Output format

Use JSON as the initial emitted format because the normalized data model maps cleanly to JSON:

| ReConf value | JSON representation |
|---|---|
| `Int` | number |
| `Float` | number, rejecting NaN and infinity |
| `Bool` | boolean |
| `String` | string |
| `none` | `null` |
| `some v` | representation of `v` |
| list | array |
| record | object |

The `some v` representation should be documented clearly. For configuration output, unwrapping `some` to `v` and `none` to `null` is ergonomic, but an explicit tagged representation may be better for round-tripping. I recommend the ergonomic default plus a future `--preserve-options` flag if needed.

## 7. Diagnostic Strategy

Diagnostics should be designed early because they shape every compiler module.

Each error should contain:

- a stable error code, e.g. `E_PARSE_001`, `E_TYPE_004`
- a primary source span
- a concise message
- optional secondary labels
- optional notes and suggestions

Initial diagnostic categories:

| Category | Examples |
|---|---|
| Parse | unexpected token, unterminated string, invalid interpolation |
| Name resolution | unknown identifier, duplicate binding, unknown type |
| Module resolution | unknown import, unexported import, cyclic import |
| Type checking | type mismatch, applying non-function, field access on non-record |
| Records | missing field, unknown field, duplicate field |
| Refinements | predicate not Bool, refinement failed, unknown predicate |
| Output | function escaped into output |
| Runtime/internal | division by zero, unsupported built-in argument |

The test suite should snapshot diagnostics to keep them stable.

## 8. Test Strategy

The project should become conformance-test-first. Every feature should land with positive and negative fixtures.

### 8.1 Test fixture layout

```text
tests/
  fixtures/
    parse_ok/
    parse_err/
    type_ok/
    type_err/
    eval_ok/
    refine_ok/
    refine_err/
    module_ok/
    module_err/
    output_ok/
  snapshots/
```

### 8.2 Test types

| Test type | Purpose |
|---|---|
| Parser golden tests | source -> surface AST snapshots |
| Lowering golden tests | surface -> core snapshots |
| Type-checking tests | accepted/rejected programs |
| Elaboration tests | omitted fields and implicit `some` become explicit core terms |
| Evaluation tests | source -> normalized value |
| Refinement tests | refinement success, failure, and unknown predicate |
| Module tests | import/export, duplicate imports, cycle detection |
| Diagnostic snapshots | stable, readable errors |
| Property tests | parser robustness, normalization determinism |
| Fuzz tests | parser crash resistance |

### 8.3 Initial conformance corpus

Start with the examples already present in `README.md`, `doc/DESIGN.md`, and `doc/DRAFT.md`. Convert each example into one or more executable fixtures.

Recommended first fixtures:

1. A simple port refinement that accepts `8080`.
2. The same port refinement rejecting `80`.
3. A literal union accepting `"localhost"`.
4. A literal union rejecting `"remote"`.
5. Optional field omission elaborating to `none`.
6. Implicit `some` elaborating from `8080 : Int?`.
7. Record refinement relating `ty` and `addr`.
8. String interpolation with a local `let`.
9. Lambda application inside interpolation.
10. Importing an exported value.
11. Rejecting an unexported import.
12. Rejecting a cyclic import.
13. Rejecting a function-valued final output.
14. Rejecting an unknown field in a closed record.
15. Rejecting a missing non-option record field.

## 9. Roadmap

### Phase 0 — Spec stabilization and acceptance tests

**Goal:** Freeze the first implementable subset and encode it as tests before writing major implementation logic.

**Deliverables:**

- `doc/MVP.md` describing the exact v0 subset.
- `tests/fixtures` directory with at least 25 positive and negative examples.
- A table mapping each language feature to test fixtures.
- A clear decision log for unresolved semantic questions.

**Exit criteria:**

- The MVP subset can be described without contradiction.
- Every example in the docs is either accepted into the MVP or explicitly deferred.
- Expected normalized outputs and expected errors are written down.

### Phase 1 — Project scaffolding and CLI shell

**Goal:** Turn the hello-world Rust project into a compiler skeleton.

**Deliverables:**

- `reconf check <file>` command.
- `reconf eval <file>` command.
- Source-file loading with file IDs and spans.
- Structured diagnostic type.
- Initial test harness that can run fixture files.

**Exit criteria:**

- CLI accepts files and reports placeholder diagnostics.
- Test harness can discover fixtures and compare expected outputs.
- CI runs formatting, linting, and tests.

### Phase 2 — Lexer and parser

**Goal:** Parse the surface language into a span-rich surface AST.

**Deliverables:**

- Lexer with comments, whitespace, identifiers, literals, punctuation, operators, and string modes.
- Parser for files, declarations, types, and expressions.
- Correct precedence for application, unary operators, arithmetic, comparisons, boolean operators, and ascription.
- Initial string interpolation parsing.
- Parse-error diagnostics.

**Exit criteria:**

- All `parse_ok` fixtures produce stable AST snapshots.
- All `parse_err` fixtures produce stable diagnostics.
- The parser rejects malformed strings, invalid declarations, and malformed refinements.

### Phase 3 — Name and module resolution

**Goal:** Resolve imports, exports, declarations, aliases, and local names deterministically.

**Deliverables:**

- Module graph loader using canonical paths.
- Export table per module.
- Import validation.
- Duplicate-name detection.
- Cycle detection.
- Binding IDs for values and type aliases.

**Exit criteria:**

- Imported exported names resolve across files.
- Unexported imports are rejected.
- Duplicate imports and duplicate declarations are rejected.
- Cyclic imports are rejected with useful diagnostics.

### Phase 4 — Core lowering

**Goal:** Lower syntax-directed sugar into core syntax.

**Deliverables:**

- Surface-to-core lowering pass.
- Literal unions lowered to string refinements.
- Method syntax lowered to function application.
- String interpolation lowered to concatenation and `show` calls.
- Core AST pretty-printer for snapshots.

**Exit criteria:**

- Lowering snapshots are stable.
- Lowered programs contain no method calls, literal union nodes, interpolated string nodes, or module syntax.
- Lowering preserves source spans enough for later diagnostics.

### Phase 5 — Bidirectional type checker and elaborator

**Goal:** Implement ordinary shape checking and type-directed elaboration.

**Deliverables:**

- Well-formedness checker for types.
- Alias expansion and recursive-alias rejection.
- Type environment for values and type aliases.
- `synth` and `check` judgments.
- Function type checking and application checking.
- Record checking with closed records.
- List checking.
- Option checking.
- Omitted optional field elaboration.
- Implicit `some` elaboration.
- Refined type shape checking without validation.

**Exit criteria:**

- Shape-correct programs type-check.
- Shape-incorrect programs fail with stable diagnostics.
- Elaborated core snapshots show explicit `none`, `some`, and complete record fields.
- No refinement predicate is validated yet, but predicates are checked to have type `Bool`.

### Phase 6 — Normalization and evaluator

**Goal:** Normalize elaborated core expressions to values.

**Deliverables:**

- Big-step evaluator.
- Closure representation.
- Primitive/built-in application.
- Deterministic record and list normalization.
- Function escape detection at output boundary.
- Runtime checks for impossible or guarded cases, such as division by zero.

**Exit criteria:**

- Pure expressions normalize deterministically.
- Lambdas can be applied internally.
- Final data values emit internally as normalized `Value` trees.
- Final functions are rejected.

### Phase 7 — Refinement validation

**Goal:** Validate refined values after normalization.

**Deliverables:**

- Refinement collector or validation hooks from the checker.
- Predicate evaluation with binder substitution/environment extension.
- `refinement failed` diagnostic.
- `unknown predicate` diagnostic.
- Tests for scalar, record, option, and list refinements.

**Exit criteria:**

- Refined values accepted only when predicates normalize to `true`.
- Failed refinements show the value, predicate span, and type context.
- Predicates that do not normalize to booleans fail clearly.

### Phase 8 — Built-ins and standard environment hardening

**Goal:** Complete the MVP built-in set and specify edge behavior.

**Deliverables:**

- Built-in type signatures.
- Built-in evaluator implementations.
- Equality semantics for all comparable data values.
- String `show` behavior.
- Method-syntax coverage for supported methods.
- Clear errors for unsupported built-in use.

**Exit criteria:**

- Built-ins used in the docs work.
- Invalid built-in calls fail at type-checking time when possible.
- Runtime edge cases are documented and tested.

### Phase 9 — JSON emitter and CLI polish

**Goal:** Make ReConf useful as a command-line configuration normalizer.

**Deliverables:**

- JSON emitter.
- `--format json` flag.
- `--pretty` / `--compact` option.
- `--no-color` option for diagnostics.
- `--explain <error-code>` optional diagnostic help.

**Exit criteria:**

- `reconf eval examples/config.reconf --format json` prints deterministic JSON.
- Diagnostics are readable in terminals and CI logs.
- The README contains installation and usage examples.

### Phase 10 — Hardening, documentation, and v0.1 release

**Goal:** Prepare a credible experimental release.

**Deliverables:**

- Expanded conformance suite.
- Parser fuzzing target.
- Property tests for deterministic normalization.
- Language reference generated or checked against tests.
- Examples directory.
- Release checklist.
- License decision.

**Exit criteria:**

- All examples in docs are executable.
- The test suite covers every MVP feature.
- Known limitations are documented.
- `cargo test` and CLI smoke tests pass reliably.

## 10. Recommended Milestones

| Milestone | Scope | Result |
|---|---|---|
| M0: Executable spec | MVP doc + fixtures | Agreement on what v0 implements |
| M1: Parse-only compiler | CLI + parser + AST snapshots | ReConf files can be parsed and diagnosed |
| M2: Resolved core | modules + lowering | Surface programs become resolved core programs |
| M3: Shape checker | bidirectional checker + elaboration | Programs are accepted/rejected by ordinary types |
| M4: Normalizer | evaluator + values | Well-typed programs normalize to data or function errors |
| M5: Refinement checker | predicate validation | ReConf’s core validation story works |
| M6: Usable CLI | JSON output + diagnostics | Users can check and evaluate configuration files |
| M7: Experimental v0.1 | docs + tests + release checklist | Project is usable as a research prototype |

## 11. Implementation Order Within Each Feature

For every language feature, use this order:

1. Add or update spec text.
2. Add positive and negative fixtures.
3. Generate or implement parser support.
4. Add lowering support if needed.
5. Add type-checking support.
6. Add evaluation/validation support if needed.
7. Add diagnostic snapshots.
8. Update docs and examples.

This keeps the implementation aligned with the language design and prevents silent semantic drift.

## 12. AI-Assisted Implementation Workflow

The README states that implementation code is intended to be generated through AI-assisted development, with humans focusing on design, specification, review, testing, and iteration. The engineering process should make that experiment measurable.

### 12.1 Proposed workflow

For each implementation ticket:

1. Human writes or approves a short spec delta.
2. Human writes tests or at least test intent.
3. AI generates the implementation patch.
4. Human reviews the patch against a checklist.
5. Tests, snapshots, formatter, and linter must pass.
6. Human records what prompt/spec was used and what manual corrections were needed.

### 12.2 Review checklist

Every generated patch should be reviewed for:

- semantic alignment with `doc/DESIGN.md`
- source-span preservation
- deterministic output
- non-recursive behavior
- no hidden IO or side effects in evaluation/refinement checking
- clear diagnostics
- test coverage for success and failure cases
- no broad feature creep beyond the ticket

### 12.3 Prompt template for implementation tickets

```text
Implement [feature] in the ReConf compiler.

Relevant spec:
[paste exact MVP spec section]

Current module boundaries:
[paste relevant file/module summaries]

Acceptance tests:
[paste positive and negative fixtures]

Constraints:
- preserve source spans
- do not add recursion
- do not add side effects
- keep diagnostics structured
- keep output deterministic
- prefer small focused changes

Expected output:
- code patch
- explanation of semantic choices
- tests added or updated
```

## 13. Main Risks and Mitigations

| Risk | Why it matters | Mitigation |
|---|---|---|
| Refinements become too powerful too early | Could require SMT solving or dependent typing | Keep MVP refinement validation concrete and normalization-based |
| Implicit `some` complicates type checking | Requires expected types and careful elaboration | Implement only in `check`, never in `synth`; snapshot elaborated core |
| Optional fields hide missing-data bugs | Could surprise users if over-applied | Insert omitted fields only when checking against known record type |
| Method syntax conflicts with field access | `x.foo` could mean method or field | Resolve syntax-directed method calls carefully; keep field access and method calls distinguishable in AST |
| String interpolation parser complexity | Recursive expression parsing inside strings can be fragile | Implement a string-mode lexer and test nested/escaped braces heavily |
| Float semantics are subtle | NaN/infinity break equality and JSON emission | Reject non-finite floats; document division behavior |
| AI-generated code drifts from spec | Project explicitly relies on generated implementation code | Require fixture-first development and human semantic review |
| Diagnostics added too late | Retrofitting spans is expensive | Attach spans to AST nodes from Phase 1 onward |
| Module resolution security | Imports touch the filesystem | Canonicalize paths; initially restrict imports to local files under the project root or current file tree |
| Feature creep | The language is intentionally small | Maintain `doc/MVP.md` and defer non-MVP features aggressively |

## 14. Open Design Questions to Resolve Early

These questions should be settled in `doc/MVP.md` before or during Phase 0:

1. **Output representation for options:** Should `some v` emit as `v`, or as a tagged value?
2. **Record field ordering:** Should emitted JSON follow type order, source order, or lexical order?
3. **Numeric semantics:** Are integers fixed-width? Are floats allowed in refinements? How are division by zero and modulo defined?
4. **Equality semantics:** Which types support `==` and `!=`? Are functions incomparable?
5. **List built-ins:** Are `map` and `filter` in the MVP, or deferred until higher-order built-ins are mature?
6. **Method syntax:** Is every method just prefix application, or is there a curated method namespace?
7. **Import root:** Can imports escape the current directory? Should absolute paths be allowed?
8. **Type alias namespace:** Do type names and value names share a namespace or separate namespaces?
9. **Predicate failure messages:** Should failed refinements print the normalized value and predicate, or only the source span?
10. **License:** The README says license is TBD; choose one before any public release.

## 15. Suggested MVP Feature Cut

### Include in MVP

- Base values: `Int`, `Float`, `Bool`, `String`
- Records, lists, and options
- Closed record checking
- Top-level `type` and `let`
- Local `let`
- Non-recursive lambdas
- Function application
- Type annotations and ascriptions
- Literal string unions via lowering to refinements
- Simple string interpolation
- Method syntax for option predicates and selected string/list operations
- Module imports/exports
- Bidirectional checking
- Refinement validation by normalization
- JSON emission

### Defer until after MVP

- `map` and `filter`, unless needed by examples
- Rich standard library
- Multiple output formats
- IDE support
- Formatter
- Package manager
- Remote imports
- SMT integration
- Incremental compilation
- Error recovery beyond basic parse diagnostics

## 16. Example v0 Behavior

Input:

```reconf
type Port = { x : Int | x > 1024 && x < 65535 };

type Config = {
  port : Port,
  host : String?,
};

let config = {
  port = 8080,
} : Config;

config
```

Elaborated conceptual core:

```reconf
let config = {
  port = 8080,
  host = none,
} : Config;

config
```

Normalized JSON output:

```json
{
  "port": 8080,
  "host": null
}
```

Refinement failure example:

```reconf
type Port = { x : Int | x > 1024 && x < 65535 };
let bad = 80 : Port;
bad
```

Expected diagnostic shape:

```text
error[E_REFINE_001]: refinement failed
  --> bad.reconf:2:11
   |
 2 | let bad = 80 : Port;
   |           ^^ value does not satisfy Port
   |
   = note: expected predicate `x > 1024 && x < 65535` to evaluate to true
   = note: normalized value was `80`
```

## 17. Near-Term Backlog

### Highest priority

- Create `doc/MVP.md`.
- Convert documentation examples into fixtures.
- Build CLI skeleton.
- Implement source manager and diagnostics.
- Implement lexer/parser for the MVP grammar.

### Medium priority

- Implement module graph resolution.
- Implement core AST and lowering.
- Implement bidirectional type checker.
- Implement option and record elaboration.
- Implement evaluator.

### Later priority

- Implement full built-in set.
- Add parser fuzzing.
- Add formatter or pretty-printer.
- Add LSP prototype.
- Add alternate emitters.

## 18. Recommended First Pull Requests

1. **PR 1: Project skeleton**
   - Add `lib.rs`, `cli.rs`, `source.rs`, `diagnostic.rs`.
   - Implement `reconf check` and `reconf eval` stubs.
   - Add CI and test harness.

2. **PR 2: MVP spec and fixtures**
   - Add `doc/MVP.md`.
   - Add initial positive and negative `.reconf` fixtures.
   - Add expected output/error files.

3. **PR 3: Lexer**
   - Tokenize identifiers, keywords, literals, comments, punctuation, and operators.
   - Add lexer snapshots and errors.

4. **PR 4: Parser**
   - Parse declarations, types, expressions, records, lists, options, lambdas, and ascriptions.
   - Add AST snapshots.

5. **PR 5: Core AST and lowering skeleton**
   - Define core AST.
   - Lower literal unions, method syntax, and interpolation.

6. **PR 6: Shape type checker skeleton**
   - Implement type environments, alias expansion, `synth`, and `check` for base constructs.

7. **PR 7: Records/options elaboration**
   - Implement omitted optional fields and implicit `some`.
   - Snapshot elaborated core.

8. **PR 8: Evaluator and JSON output**
   - Normalize base values, records, lists, options, lets, lambdas, and applications.
   - Emit JSON for normalized data.

9. **PR 9: Refinements**
   - Check predicate type.
   - Validate normalized values.
   - Add failure diagnostics.

10. **PR 10: Modules**
    - Implement import/export resolution, canonical paths, and cycle detection.

## 19. Conclusion

ReConf is well suited to a staged, spec-driven implementation because the design already separates surface syntax, core syntax, bidirectional shape checking, normalization, and refinement validation. The strongest path forward is to make the specification executable through fixtures, then implement each compiler stage with stable intermediate representations and diagnostic snapshots.

The recommended roadmap deliberately delays broad standard-library growth and advanced features until the MVP proves the core thesis: a small typed configuration language can treat schema, normalization, and validation as first-class language features while remaining predictable and easy to reason about.

## 20. Source Notes

This proposal is based on the repository contents available at the time of review:

- `README.md`: project goals, status, language feature list, and compiler pipeline.
- `doc/DESIGN.md`: detailed language design, source-file model, types, expressions, lowering, static semantics, normalization, errors, and ABNF grammar.
- `doc/DRAFT.md`: worked examples for modules, literal unions, optional fields, implicit `some`, refinements, lambdas, interpolation, and a complete example.
- `Cargo.toml`: Rust package metadata.
- `src/main.rs`: current implementation scaffold.

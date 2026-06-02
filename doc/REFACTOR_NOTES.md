# ReConf Code Refactor Plan

This document is the working plan for the next large code refactor. It is based
on the current repository, not on the old roadmap. Parser work in this refactor
means improving the current parser's code contracts, AST output, spans, and
diagnostics.

The purpose of the refactor is to make the codebase more robust and scalable by
decoupling compiler phases, introducing explicit intermediate representations,
centralizing pipeline orchestration, and splitting the project into crates with
clear ownership boundaries.

## Current Baseline

The repository is now a Cargo workspace:

- `crates/core/`
  - Source infrastructure, diagnostics, syntax/surface AST, resolved
    artifacts, core AST, lowering, elaboration/type checking,
    evaluation/normalization, refinement validation, native registry, and the
    bundled prelude source.
- `crates/compiler/`
  - Compiler pipeline/session orchestration, module loading/evaluation policy,
    prelude compilation, output validation, and data emitters.
- `crates/cli/`
  - Binary entrypoint, CLI argument parsing, REPL UI, terminal reporter setup,
    and host-facing filesystem wiring.
- `crates/wasm/`
  - Minimal placeholder crate for target-specific bindings in the separate WASM
    branch.
- `tests/` and `examples/`
  - Shared fixture corpus consumed by crate-local integration tests.
  - Currently returns runtime `Value`.
- `crates/core/src/eval/`
  - Runtime `Value`.
  - Big-step evaluation over surface expressions.
  - Native builtin registry and native application.
  - Prelude loading.
  - ReConf-style value printing.
- `crates/core/src/refine/`
  - Concrete refinement validation by evaluating predicates.
- `crates/compiler/src/emit/`
  - Data-only JSON and ReConf output.
- `crates/core/src/diagnostic.rs` and `crates/core/src/error.rs`
  - Error codes, miette integration, and best-effort source-span attachment.
- `crates/cli/src/cli.rs`
  - CLI command parsing.
  - Filesystem-backed compiler invocation.
  - Emitter selection.
- `crates/cli/src/repl/`
  - Reedline UI.
  - Syntax highlighting.
  - Input validation.
  - Compiler-session evaluation.
  - Diagnostic reporting setup.
- `tests/`
  - Fixture harness.
  - CLI tests.
  - Determinism tests.
  - REPL tests.
  - Duplicated parse/lower/eval helpers.

This baseline should remain behaviorally stable while the internal structure is
changed.

## Primary Objectives

### Generic Design Over Ad Hoc Paths

This is a core design principle for the whole project. ReConf code should be
designed around generic contracts, reusable data representations, and clear
ownership boundaries, not around special cases for one frontend, test harness,
output format, or deployment target.

This also means using Rust's encapsulation and abstraction tools deliberately.
Avoid C-style bags of unrelated free functions that manipulate shared data from
the outside. Prefer cohesive structs, traits, impl blocks, and narrow methods
when a phase has state, policy, or a meaningful behavioral boundary. Free
functions are acceptable for small pure utilities, but compiler phases such as
lowering, adaptation, source loading, emitting, resolving, checking, and
normalizing should be modeled as services or capabilities with explicit
contracts. The goal is not object-oriented ceremony; the goal is ownership,
local reasoning, substitutability, and decoupling.

This principle applies across all crates:

- Prefer target-neutral abstractions such as source providers, compiler inputs,
  diagnostics, typed core, and emitters.
- Encapsulate phase behavior behind Rust types and traits instead of spreading
  phase logic across flat helper functions.
- Keep frontend-specific behavior at the frontend boundary.
- Keep output-specific behavior at the emitter boundary.
- Keep test support outside production phase logic.
- Do not add special internal paths for CLI, REPL, tests, or future host
  targets when a generic capability is the real requirement.
- If one caller needs unusual behavior, model the underlying capability
  generically before adding caller-specific code.
- Temporary migration code must be explicit. Mark it near the implementation
  with `TEMP(refactor-stage-*)`, state what replaces it, and remove it in the
  cleanup stage instead of letting compatibility scaffolding become invisible
  design.
- Keep `reconf-core` clean: no terminal UI assumptions, no process/filesystem
  policy, no line-editor state, and no output-format policy in core phases.

### Decoupling

The largest architectural problem is that many modules know too much about each
other. The refactor should reduce cross-phase knowledge:

- The resolver should not evaluate modules.
- The evaluator should not call the type checker.
- Emitters should not receive function-capable runtime values.
- CLI and REPL should not reconstruct compiler internals.
- Tests should not duplicate private pipeline logic.
- Diagnostics should not be inferred from display strings.

### Scalability

The code should support growth without turning every new backend, output format,
or host environment into a cross-cutting edit. In particular:

- CLI, REPL, tests, and other hosts should share one compiler API.
- Source loading should be pluggable.
- Output emitters should be pluggable.
- Core representations should be independent of UI concerns.
- Crates should make dependency direction obvious.

### Explicit Compiler Representations

The current implementation uses `syntax::surface::{FileAst, Expr, Type}` across
too many phases. The refactor should introduce explicit representations for:

- Parsed surface syntax.
- Resolved/lowered core syntax.
- Typed and elaborated core syntax.
- Normalized runtime values.
- Data-only output values.

Each representation should have a clear owner and a clear set of consumers.

## Non-Goals

- Do not add language features as part of this refactor.
- Do not change user-facing MVP behavior unless a current behavior is clearly a
  bug.
- Do not perform the crate split before phase boundaries are clear enough to
  move safely.

## Current Coupling Problems

### Surface AST Acts As Compiler IR

Current code:

- `crates/core/src/syntax/surface.rs` defines `FileAst`, `Decl`, `Type`, `Expr`, and
  `StrPart`.
- `crates/core/src/syntax/parser.rs` converts Pest pairs into this unspanned surface AST.
- `crates/core/src/lower/desugar.rs` lowers `FileAst` into core syntax.
- Type checking, normalization, and refinement validation now operate on core
  syntax.
- `crates/cli/src/repl/semantic.rs` walks `FileAst` for semantic highlighting.

Problem:

Surface syntax is doing the job of parsed syntax, lowered syntax, typed syntax,
and evaluator input. This makes later phases depend on user-facing syntax
details and prevents a clean core-language boundary.

Refactor direction:

- Keep surface AST as parsed syntax.
- Let parser output preserve source provenance required by later phases.
- Add `CoreExpr` and `CoreType` for lowered/resolved compiler syntax.
- Add `TypedExpr` or `ElaboratedExpr` for type-directed elaboration output.
- Make evaluator consume typed/elaborated core.
- Make refinement validation consume core predicates and normalized values.

### Type Checking Produces Runtime Values

Current code:

- `check_expr` returns `Result<Value>`.
- `synth_expr` returns `Result<Value>`.
- `check_value_against` validates a runtime value against a type.
- Option elaboration and omitted option fields appear in returned `Value`, not
  in an explicit checked term.

Problem:

The type checker cannot be reused as a static phase because its result is
already a normalized runtime value. This collapses checking, elaboration, and
evaluation into one path.

Refactor direction:

- Split checking from evaluation.
- Type checking returns typed/elaborated core and a type.
- Runtime evaluation happens after checking.
- Existing `check_value_against` logic can be preserved temporarily as an
  output/data validation helper, but it should not be the core checker result.

### Evaluator Depends On Type Checker

Current code:

- `eval::eval` calls `check_expr` for annotated `let` expressions and
  ascriptions.
- `eval` accepts aliases so it can handle type-directed behavior at runtime.

Problem:

The evaluator is not a pure evaluator of already checked terms. It owns part of
the static semantics, which makes it harder to test and harder to reuse.

Refactor direction:

- Move all ascription and annotated-let checking into the elaboration phase.
- Remove type checker calls from `eval`.
- Evaluate only typed/elaborated core.
- Runtime should not need type aliases except possibly for debug metadata.

### Resolver Owns Pipeline Orchestration

Current code:

- `compiler::loader::ModuleLoader` owns source-provider access for both entry
  inputs and imports. It exposes intention-specific loading methods instead of
  leaking mutable access to the provider, then delegates frontend compilation,
  caching, cycle detection, and module compilation through narrower services.
- `resolve::resolved::ResolvedProgram`, `ResolvedModule`, and
  `ResolvedExports` are the first resolved artifacts. The module evaluator
  imports through this narrowed resolved boundary instead of receiving an entire
  imported `Module`.
- `ResolvedModule` carries its module path identity, and
  `ModuleLoader::load_resolved` is the explicit import-facing module loading
  boundary.
- `ResolvedModuleBody` carries explicit `ResolvedImport` entries and
  `ResolvedDecl` declarations. Value-bearing declarations carry `GlobalRef`
  binding ids before elaboration/evaluation, and same-module value references
  are rewritten to `CoreExpr::Global` at this boundary.
- `ModuleCompiler` and `ModuleEvaluator` consume `ResolvedModuleBody`, so module
  evaluation now enters through the resolved artifact instead of the raw lowered
  `CoreModule`.
- `ModuleGraph` caches `ResolvedModule` values directly and records them in the
  `ResolvedProgram`. Evaluated `Module` values are transient artifacts used only
  to compute the resolved export table while a module is being loaded.
- Requested import selection and missing/duplicate requested-name validation
  now live on resolved import/export artifacts through `ResolvedImport` and
  `ResolvedImportSelector`; module evaluation loads the target module and asks
  the import artifact to select from its exports.
- `compiler::module::ModuleEvaluator` inserts context definitions, evaluates
  imports, checks declarations, evaluates output, and builds exports.

Problem:

Module graph behavior has moved out of `resolve`, and import selection now goes
through a resolved export table. Imported files are still evaluated to compute
runtime exports, but the graph cache and import-facing API store and return
`ResolvedModule` artifacts directly.

Refactor direction:

- Resolver should eventually own resolved module/name data structures rather
  than evaluated module values.
- Pipeline orchestration should stay in the compiler pipeline layer.
- Prelude setup should be explicit in compiler context construction.
- Output validation should stay in the compiler/output boundary.

### CLI, REPL, And Tests Duplicate Pipeline Logic

Current code:

- `crates/compiler/src/compiler.rs` exposes `Compiler`, `CompilerOptions`, `CompileInput`,
  `CheckOutput`, and `EvalOutput`.
- `crates/compiler/src/compiler/pipeline.rs` owns shared parse/lower/entry-compile
  orchestration for `Compiler` and `CompilerSession`.
- CLI and integration tests call `Compiler::check` or `Compiler::eval`, while
  REPL evaluation calls `CompilerSession`; none of these callers reconstruct
  parser/lowerer/loader/evaluator wiring.
- `CompileInput` adapts path-backed and source-backed inputs into one pipeline
  entry shape.
- `CheckOutput` and `EvalOutput` expose accessors and owned conversion methods
  instead of public result fields, so callers do not couple to compiler result
  storage.
- `EvalOutput` owns data-output validation, so emitters receive `DataValue`.
- Syntax/core modules can represent declaration-only inputs, so the REPL no
  longer injects a sentinel output expression.
- REPL declaration and expression inputs call the compiler-owned session API.

Problem:

Pipeline behavior can drift across hosts. Adding new outputs or changing module
loading requires edits in several places.

Refactor direction:

- Continue consolidating behavior behind the shared compiler pipeline API.
- Host-specific code provides sources and output preferences only.
- REPL state is now represented by `compiler::session::CompilerSession`, which
  reuses the shared compiler pipeline.
- Accepted declaration artifacts are owned by `SessionArtifacts`, a private
  session-state service that commits frontend outputs and composes declaration
  or expression inputs as paired surface/core artifacts. `CompilerSession`
  orchestrates compilation instead of managing parallel artifact vectors.

### Diagnostics Are Coupled To Message Text

Current code:

- `Error` stores structured labels and notes.
- `DiagnosticSource` is the explicit compiler-boundary source context used when
  a producer emits structured labels without an embedded source object.
- Parser duplicate-field, empty-interpolation, and unterminated-interpolation
  diagnostics attach producer-owned labels.
- Runtime division-by-zero, core type alias, and refinement failure diagnostics
  attach labels from carried origin metadata.
- Parser errors are still converted to `Error` early, so later code does not
  receive structured parse error kinds.

Problem:

Error messages are serving as structured data. This is fragile and makes
diagnostics hard to improve.

Refactor direction:

- Compiler phases should produce structured diagnostics directly.
- Parser diagnostics should be structured diagnostics from the beginning.
- Diagnostics should support multiple labels and notes.
- Error code metadata remains centralized.
- CLI/reporter converts structured diagnostics to `miette`.
- Tests can assert diagnostic structure without depending only on rendered
  terminal output.

### Builtins And Prelude Metadata Are Split

Current code:

- `eval/prelude.reconf` declares native signatures.
- `eval/builtins.rs` separately defines declared names, arity, and runtime
  behavior.
- Runtime behavior accepts some shapes beyond declared signatures.

Problem:

Native function metadata can drift. Type signatures, arity, and implementation
are not one coherent registry.

Refactor direction:

- Introduce a structured native registry.
- Keep user-visible prelude definitions explicit.
- Tie native name, type, arity, and implementation together.
- Make runtime errors from native calls structured and phase-owned.

### Output Validation Is Scattered

Current code:

- `compiler::output::OutputValidator` owns function-output rejection.
- `Compiler::eval` validates the final output through `OutputValidator` into
  `DataValue`.
- Emitters consume `DataValue`.
- `emit::EmitterRegistry` owns a collection of `Emitter` implementations and
  centralizes output-format lookup, so hosts do not match directly on concrete
  emitter implementations.

Problem:

Every output path must remember to reject functions. This will get worse when
more emitters are added.

Refactor direction:

- Add a data-only output validation step.
- Emitters consume `DataValue` or `CheckedOutput`, not arbitrary `Value`.
- Function escape is reported once by the output validation phase.

## Current Generic-Design Violations

This section records concrete current-code locations that violate the project
principle "generic design over ad hoc paths". These are not all bugs. Most are
reasonable MVP shortcuts, but they are the places the refactor should unwind.

### Resolver Owns Host Policy And Full Evaluation

Files:

- Old location: `src/resolve/modules.rs`
- Current transition location: `crates/compiler/src/compiler/loader.rs`, `crates/compiler/src/compiler/module.rs`,
  and `crates/compiler/src/compiler/module/`

Current violation:

- `ModuleLoader` keeps `SourceProvider` private and exposes loader-level methods
  for entry-source and import loading. Source text to surface/core compilation is
  delegated to `compiler::front::FrontendCompiler`.
- `compiler::loader::graph::ModuleGraph` owns resolved-module caching,
  in-progress module tracking, cycle detection, and `ResolvedProgram`
  recording. The in-progress `LoadingModule` token carries the
  `ResolvedModuleBuilder`, so resolved-body provenance is no longer stored in a
  loose graph side table.
- `ModuleLoader` no longer exposes a normal evaluated-module `load` API for
  imports. Import loading goes through `load_resolved`, and evaluated modules
  remain a private cache detail.
- `compiler::front::FrontendCompiler` owns the parse/lower frontend step and
  attaches frontend diagnostics for entry compilation, imported modules, and
  bundled prelude compilation.
- `ModuleCompiler` is now the policy boundary between module graph loading and
  module compilation. `ContextualModuleCompiler` evaluates modules with an
  explicit `ModuleContext`; `ContextualModuleCompiler::with_prelude` requests a
  context from `compiler::prelude::PreludeCompiler`. `EvaluatingModuleCompiler`
  evaluates with an empty context for bundled prelude construction and targeted
  tests.
- `ResolvedProgram`, `ResolvedModule`, and `ResolvedExports` are the first
  resolved artifacts; `ResolvedModule` / `ResolvedExports` are the
  import-facing artifacts used by the evaluator.
- `ResolvedModule` now carries a path identity, so module identity is not only
  an external map key.
- `ResolvedModuleBody` exposes explicit `ResolvedImport` data and
  `ResolvedDecl` declarations. `ResolvedDecl::Native` and `ResolvedDecl::Let`
  carry `GlobalRef` value binding ids, so module elaboration no longer invents
  declaration-local global ids. Same-module value references in declaration
  bodies and module output are resolved against those ids before elaboration.
- Compiler module exports use private module-owned `ValueExport` metadata.
  `ResolvedValueExport` is created only when `Module::resolved_exports` projects
  module exports across the compiler/resolver boundary.
- `ResolvedModuleBuilder` now carries the path and body before evaluation
  finishes. `ModuleGraph` keeps that builder on the `LoadingModule` token and
  finalizes it with `ResolvedExports`, instead of keeping loose resolved-body
  side tables.
  `compiler::module::Module` exposes only a `resolved_exports` projection for
  this boundary.
- `resolve::names::NameScope` now provides a reusable value/type namespace
  abstraction for local-name collision detection without import-specific
  diagnostics. `compiler::module` uses it through a narrow import binder instead
  of open-coding import collision checks.
- `compiler::module::Module` is no longer a public bag of environment maps.
  Compiler/session callers retrieve output through methods, while module
  evaluation mutates values, type aliases, value types, and exports through
  module-owned operations.
- Selected import application is a module-state operation:
  `ModuleImportBinder` validates requested names and collisions, then delegates
  selected export insertion to `Module::import_selection` through the
  `ResolvedImportTarget` capability.
- `compiler::module::ModuleEvaluator` receives an explicit `ModuleContext` and
  is the public module-evaluation service. It orchestrates import binding and
  elaboration, while
  `ModuleDeclEvaluator` evaluates elaborated declarations/output and builds
  module-owned exports.
- `crates/compiler/src/compiler/module/` now separates module state, import binding, and module
  evaluation into `state.rs`, `imports.rs`, and `eval.rs`; `module.rs` is the
  narrow facade for public module-facing names.

Why this is ad hoc:

- Module graph loading is no longer tied to filesystem policy or the `resolve`
  module, frontend parsing/lowering has moved behind a compiler service, and
  graph/cache/cycle state is encapsulated by `ModuleGraph`. Loaded imports are
  cached as `ResolvedModule` artifacts and recorded in a `ResolvedProgram`.
  Module evaluation remains a private export-computation step instead of the
  public artifact exposed to import resolution.
- Import selection and local-name collision detection have moved behind
  resolved and name-scope abstractions. Module import binding still translates
  collisions into import diagnostics, while selected export insertion is now
  owned by module state.
- Prelude setup is no longer hidden behind a boolean in module evaluation.
  It is an explicit `ModuleContext` supplied through `ContextualModuleCompiler`,
  and bundled prelude construction is owned by `PreludeCompiler`.
- Module evaluation has been split into import binding, elaboration
  orchestration, and declaration execution services. Resolved modules are now
  the graph cache and import-facing output; evaluated modules remain internal to
  export computation.

Target:

- Source loading moves behind `SourceProvider`.
- Introduce resolved module/program structures for import/export checks and name
  resolution.
- Compiler pipeline owns parse/lower/check/eval/refine/output orchestration.
- Continue reducing eager prelude construction after the explicit
  `ModuleContext` boundary is stable.
- Output validation becomes a separate phase.

### CLI, REPL, And Tests Rebuild The Pipeline

Files:

- `crates/cli/src/cli.rs`
- `crates/cli/src/repl/eval.rs`
- `crates/compiler/tests/fixtures.rs`
- `crates/compiler/tests/determinism.rs`

Current violation:

- Shared CLI, REPL, and integration paths now call the compiler pipeline rather
  than rebuilding parse/lower/load/evaluate wiring.
- `Compiler` and `CompilerSession` share the internal `CompilerPipeline`
  service for source parsing, lowering, diagnostic source context, and entry
  compilation.
- CLI `check` now calls `Compiler::check`; output validation and data emission
  remain on the eval/output path.
- REPL declaration state now lives in `CompilerSession` and its private
  `SessionArtifacts` store instead of frontend source-text accumulation.
- CLI, REPL, fixtures, determinism checks, language tests, and compiler
  integration tests route normal output formatting through `EmitterRegistry`.
- Tests use production internals directly instead of a stable compiler API.

Why this is ad hoc:

- Frontends duplicate compiler behavior.
- Any pipeline change must be edited in multiple callers.
- REPL behavior becomes a special compiler path instead of frontend state that
  calls a generic compiler capability.

Target:

- Add one compiler pipeline API.
- Frontends provide inputs and options.
- Tests use the public pipeline API except when testing individual phases.
- Frontend-specific state remains outside core compiler phases.

### Type Checker Returns Runtime Values

Files:

- Removed `src/typeck/bidir.rs`

Current violation:

- The old surface bidirectional checker has been removed.
- Contextual elaboration is represented in core before runtime checking.
- The old surface-type/runtime-value compatibility module has been removed.

Why this is ad hoc:

- Static checking, elaboration, normalization, refinement validation, and output
  shaping are collapsed into one path.
- There is no typed/elaborated core artifact that other phases can reuse.
- It is hard to inspect or test elaboration independently.

Target:

- Type checker consumes core and returns typed/elaborated core.
- Runtime `Value` is produced only by evaluation/normalization.
- Type compatibility/equality is separated from data-output validation.
- Implicit `some` and omitted optional fields become explicit elaboration nodes.

### Evaluator Depends On Surface Syntax And Type Checker

Files:

- `crates/core/src/eval/mod.rs`

Current violation:

- Runtime closures store core expressions.
- The surface evaluator has been removed.
- Runtime function application is exposed through a core applicator service.

Why this is ad hoc:

- Checked-normalization callers now compose `CoreElaborator` with
  evaluator-owned `PreparedCoreNormalizer` explicitly. The production
  `typeck::CheckedCoreNormalizer` bridge has been removed, so typeck no longer
  imports evaluator code just to provide a convenience path.
- Runtime closure application uses prepared-core normalization and does not
  re-enter the checked-normalization boundary.
- Checked ascriptions are erased during elaboration; prepared normalization
  rejects any remaining ascription as unelaborated input instead of invoking
  type checking.

Target:

- Evaluator consumes typed/elaborated core.
- Closures store core expression bodies.
- Evaluation does not call the type checker.
- ReConf data printing moves to an emitter.
- Function-output rejection moves to output validation.

### Surface AST Is Used As Every IR

Files:

- `crates/core/src/syntax/surface.rs`
- `crates/core/src/lower/desugar.rs`
- `crates/core/src/core/ast.rs`
- Removed `crates/core/src/core/pretty.rs`

Current violation:

- Surface `Expr` and `Type` are used by parser, lowerer, checker, evaluator,
  refinement validator, REPL semantic tracking, and tests.
- `lower_file` returns another `FileAst`.
- `core::ast` re-exports surface AST and runtime `Value`.
- The old `core::pretty` helper has been removed.

Why this is ad hoc:

- The project has no real core boundary.
- User syntax leaks into every compiler phase.
- Deterministic output concerns are mixed into parsed syntax through `BTreeMap`
  records.

Target:

- Keep surface AST as parsed/user syntax.
- Add real core AST and core type modules.
- Lowering produces core.
- Type checking produces typed/elaborated core.
- Output ordering belongs in checked core or data output, not parsed syntax.

### Diagnostics Are Recovered From Strings

Files:

- `crates/core/src/diagnostic.rs`
- `crates/core/src/syntax/parser.rs`
- `crates/core/src/error.rs`

Current violation:

- `DiagnosticSource` still attaches source text at compiler entry boundaries for
  already-labeled errors whose spans are byte offsets into that source.
- Parser diagnostics now build producer-owned labels separately from placeholder
  source text; `DiagnosticSource` owns renaming placeholder sources to real
  compiler-boundary names.
- `Error` exposes separate source and label APIs; the old combined
  source-span helper has been removed so producers do not hide source
  attachment inside label construction.
- Surface and core expressions now have a transparent `Spanned` wrapper for
  source-origin propagation. Runtime division-by-zero diagnostics use that
  origin instead of message-text recovery.
- Surface and core types now have a transparent `Spanned` wrapper for
  source-origin propagation. Unknown and recursive type diagnostics use that
  origin instead of message-text recovery.
- Refinement failures now use checked-expression origin metadata instead of
  message-text recovery.
- Parser empty-interpolation, unterminated-interpolation, duplicate-field, and
  top-level unterminated-string diagnostics use producer-owned spans instead of
  whole-source string heuristics.
- `Error` supports multiple structured labels and notes.

Why this is ad hoc:

- Source attachment still happens after the producing phase, so diagnostics do
  not yet carry stable source ids through every phase.
- Diagnostics are easier to compose now that labels and notes are structured,
  but source identity is still attached by boundary adapters.
- Runtime division-by-zero is no longer part of this fallback, but other
  producers still need to attach labels directly.
- Unknown and recursive type diagnostics are no longer part of this fallback.
- Refinement failures are no longer part of this fallback.

Target:

- Phase-owned structured diagnostics.
- Keep producer labels independent from rendering/source attachment.
- Thread stable source ids/spans through phase outputs.
- Move `miette` conversion policy to CLI/reporter code.
- Replace compiler-boundary source attachment after diagnostics carry stable
  source ids directly.
- Parser constructs structured parse diagnostics directly.
- Error values carry multiple labels and notes.
- CLI/reporter handles `miette` rendering.

### Native Metadata Is Duplicated

Files:

- `crates/core/src/eval/builtins.rs`
- `crates/compiler/src/compiler/prelude.rs`
- `crates/core/src/eval/prelude.reconf`

Current violation:

- Native names, arities, and implementations now live in `NativeRegistry`.
- Native registry entries carry core type metadata and expose name, arity, and
  type through accessors instead of public storage fields.
- Runtime behavior is selected through a private `NativeImplementation` service
  enum and executed by `NativeCall`, rather than public callback fields and a
  flat pile of `native_*` functions.
- Prelude signatures are listed in `prelude.reconf`.
- Tests verify that exported prelude native names and types match registry
  entries.
- Prelude module construction lives in the compiler layer and evaluates through
  module logic without hidden filesystem loading.
- `PreludeCompiler` owns bundled prelude compilation and can produce either a
  `Module` or the `ModuleContext` used by normal compiler construction.

Why this is ad hoc:

- Name, arity, type, and runtime implementation can drift.
- Some runtime behavior is broader than the exposed prelude signatures.
- Prelude setup is still an eager compiler-context construction path, but the
  construction behavior is now encapsulated by a compiler-layer service.

Target:

- Continue moving prelude setup toward explicit compiler context construction.
- Decide whether broader runtime behavior than prelude signatures is intended
  or temporary.

### Emitters Receive Runtime Values

Files:

- `crates/compiler/src/emit/json.rs`
- `crates/core/src/eval/mod.rs`
- `crates/compiler/src/compiler.rs`

Current violation:

- JSON emitter consumes raw `Value`.
- JSON emitter rejects functions.
- Resolver rejects functions.
- ReConf value printer rejects functions.

Why this is ad hoc:

- Every output path repeats output-validity checks.
- Emitters know about runtime-only values such as closures and natives.
- Adding another output format repeats the same validation concern.

Target:

- Add output validation phase.
- Convert `Value` to `DataValue` or `CheckedOutput`.
- Emitters consume data-only output.

## Target Pipeline

The code should move toward this pipeline:

```text
source provider
  -> parse existing surface syntax
  -> resolve modules and names
  -> lower surface to core
  -> bidirectionally shape-check and elaborate core
  -> normalize/evaluate elaborated core
  -> validate refinements on normalized values
  -> validate data-only output at the output boundary
  -> emit output
```

The important refactor is the phase separation after parsing. Each phase should
consume the previous phase's output and produce a well-defined result.

This order intentionally follows the language design: ordinary shape checking
comes before refinement validation, and refinement predicates are checked and
validated as concrete normalized computations rather than solved symbolically.

## Dependency Direction

Dependencies should flow in one direction:

```text
reconf-core -> reconf-compiler -> host crates
```

Within `reconf-core`, dependencies should also move forward through the
compiler:

```text
source/diagnostic
  -> syntax
  -> resolve
  -> lower/core
  -> typeck/elaborate
  -> eval/refine
```

Lower layers must not depend on CLI flags, REPL line editing, terminal
reporting, filesystem policy, or concrete output formatting. Output formatting
belongs in `reconf-compiler`, not in evaluator or resolver code.

## Target Representations

### Surface Syntax

Owned by syntax code.

Purpose:

- Represent user-written syntax.
- Preserve source-level concepts needed by diagnostics and REPL semantic
  tooling.
- Preserve enough source provenance for later phases to report precise errors.

Common wrapper:

```rust
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}
```

This is not a parser redesign. It is source infrastructure that later compiler
phases need so they do not recover spans from rendered error messages.

Parser-facing requirements for the current Pest parser:

- Keep Pest as the parser implementation.
- Convert Pest spans into `Span`/`Spanned<T>` during AST construction.
- Preserve spans for declarations, type nodes, expression nodes, import items,
  record fields, and interpolation fragments where practical.
- Keep duplicate fields representable long enough to report the duplicate field
  span precisely.
- Preserve source order in surface records and declarations. Deterministic
  output ordering belongs in checked core/data output, not in parsed syntax.

Should not be consumed by:

- Evaluator.
- Emitters.
- Final refinement validator.
- Output validation.

### Resolved Program

Owned by resolver.

Current:

- `resolve::resolved::ResolvedProgram`, `ResolvedModule`, and
  `ResolvedExports` exist as the first resolved artifacts.
- Import loading returns `ResolvedModule`, not a full evaluated compiler
  `Module`.
- `ResolvedProgram` currently indexes resolved modules by the path identity
  stored on each `ResolvedModule`.
- `ModuleLoader::resolved_program` exposes the resolved modules loaded so far.
- `compiler::loader::graph::ModuleGraph` caches `ResolvedModule` values
  directly rather than evaluated/resolved module pairs.
- `ResolvedImport` owns requested-name selection against `ResolvedExports`
  through `select_from`, backed by `ResolvedImportSelector`. The resulting
  selection applies through `ResolvedImportTarget`, so callers do not inspect
  its storage or iterate its internal map directly.
- `ResolvedModule` carries exports and a `ResolvedModuleBody`.
- `ResolvedModuleBody` carries explicit `ResolvedImport` data and
  `ResolvedDecl` declarations. Value-bearing declarations carry `GlobalRef`
  binding ids, and same-module references are rewritten to `CoreExpr::Global`
  before module evaluation.
- Module compilation/evaluation consumes `ResolvedModuleBody`; raw lowered core
  is converted by compiler pipeline/front-loader entry boundaries before module
  compilation begins.
- Resolved value exports carry `ResolvedValueExport` metadata with the runtime
  value and optional core type metadata. Compiler module state keeps private
  `ValueExport` metadata and projects it to resolved exports at the boundary.
  Annotated and native exports provide type metadata; unannotated exports remain
  untyped until synthesis is complete.
- `ResolvedExportsBuilder` owns resolved export construction so callers define
  value/type exports by capability rather than mutating the raw export map.
- Resolved module construction from compiler-produced exports is owned by
  `ResolvedModuleBuilder::finish`; the compiler module facade does not define
  resolved-module constructors.
- Resolved value declaration bindings are in place. Remaining resolver work is
  to resolve all value references and imports against those bindings before
  elaboration, rather than relying on name lookup during elaboration.

Purpose:

- Represent loaded modules.
- Resolve import/export relationships.
- Resolve name references where possible.
- Preserve module boundaries and source provenance.

Should not:

- Evaluate declarations.
- Emit output.
- Know CLI policy.

### Core Syntax

Owned by core/lowering code.

Purpose:

- Represent the compiler language after surface conveniences are removed.
- Be independent of import/export syntax.
- Be suitable for type checking and normalization.

Core should represent:

- Base, option, list, record, function, and refinement types.
- Explicit variable references by symbol or binding reference before
  elaboration.
- Explicit native references.
- Explicit field projection.
- Explicit `some` and `none`.
- Explicit records and lists.
- Lambdas, application, `let`, and `if`.
- Ascription only if keeping it improves diagnostics after elaboration.
- Records and lists in deterministic checked order.

Core should not represent:

- Module imports/exports.
- Method syntax.
- String interpolation syntax.
- Literal union syntax unless it has intentionally been represented as a
  refinement/core predicate.
- Omitted optional fields.
- Implicit option construction.
- Raw unresolved user-name strings for resolved bindings.

Local binding representation:

- Lowered core may keep user-facing names for diagnostics and because imports
  and declarations have not necessarily been resolved yet.
- Elaborated/evaluator core should not use raw strings for local variables.
  Lambda/let locals now use `LocalRef`, a small typed De Bruijn-index wrapper,
  instead of a naked `usize` or raw user-name string.
- Elaborated/evaluator core should not use raw strings for known global values.
  Module declarations, imported values, and native values now use `GlobalRef`,
  a stable value binding id allocated by module elaboration/module state.
- Resolved/core type aliases should not rely only on raw type-name strings after
  name resolution. Same-module aliases now use `TypeAliasRef` and
  `CoreType::ResolvedAlias`; `CoreType::Alias(String)` remains the
  lowered/unresolved syntax form.
- Do not force globals into De Bruijn form unless module resolution naturally
  creates a global environment model that benefits from it.
- The current implementation uses indices because evaluator stack lookup is the
  dominant operation. Revisit levels only if later phases need stable references
  while extending the local context.
- Preserve source-origin/name metadata separately for diagnostics, rather than
  using display names as semantic identity.

De Bruijn decision:

- `CoreExpr::Local(LocalRef)` currently uses De Bruijn indices in prepared /
  evaluator core. This matches the current evaluator model, where local lookup
  is stack-relative and happens after elaboration has already fixed binder
  identity.
- Do not expose naked `usize` indices outside the local-binding service. Keep
  `LocalRef` as the semantic handle so a later switch from indices to levels
  remains a localized core/evaluator change instead of a cross-project rewrite.
- Prefer De Bruijn levels only if later elaboration, normalization, or
  diagnostics need local references that remain stable under context extension.
  That would be a real design change, not a formatting cleanup.
- Keep global/module/native references separate from local De Bruijn
  references. `GlobalRef` and `TypeAliasRef` are resolved symbol identities, not
  stack addresses.

### Typed/Elaborated Core

Owned by type checker/elaborator.

Purpose:

- Carry checked expression/type relationships.
- Make contextual rewrites explicit.
- Provide evaluator input for ordinary normalization.

Must make explicit:

- Inserted `some`.
- Inserted omitted optional fields.
- Checked record field order.
- Expanded or resolved aliases where needed.
- Native and user binding types.
- Local binder references as `LocalRef` De Bruijn indices.
- Known module/global/native value references as `GlobalRef` binding ids.

Should not:

- Treat refinement predicates as SMT constraints.
- Validate refinement predicates during ordinary shape checking.
- Invent refined types during synthesis.
- Carry unresolved local-variable strings into evaluator input.

Suggested artifacts:

```rust
pub struct TypedExpr {
    pub expr: CoreExprId,
    pub ty: CoreType,
}

pub struct ElaboratedModule {
    pub declarations: Vec<ElaboratedDecl>,
    pub output: TypedExpr,
    pub exports: ExportTable,
}
```

Use arenas or stable ids if span mapping, sharing, or diagnostics become
awkward with boxed trees. The important rule is that phases after elaboration
should not inspect surface AST variants.

Implemented local-reference shape:

```rust
pub struct LocalRef {
    index: usize,
}

pub struct BinderInfo {
    pub debug_name: Option<Symbol>,
    pub origin: Option<Span>,
}
```

`LocalRef` currently stores a De Bruijn index because runtime lookup uses a
stack of local values. A future arena/context representation may switch to
levels if stable references under context extension become more valuable than
stack lookup. The design constraint should remain: semantic local identity is
structural, while user names are diagnostic metadata.

Current transition:

- `CoreModuleElaborator` is the module-level elaboration service.
- Annotated and unannotated declarations are elaborated into typed core.
- Unannotated literals, known value references, records, non-empty homogeneous
  lists, field projection, `if`, `let`, lambdas, application, unary operators,
  and supported binary operators now synthesize `TypedCoreExpr` using a
  value-type context.
- Bare `none`, empty lists, unknown identifiers, and other unsynthesizable
  expressions now fail in the elaboration phase with structured type errors
  instead of falling through to checked normalization.
- The temporary `ElaboratedExpr::Prepared` module-evaluation path has been
  removed.
- Lambda/let-local references in synthesized expressions are rewritten to
  `CoreExpr::Local(LocalRef)`. Checked refinement validation also accepts a
  `CheckedRefinementPredicate` and evaluates predicates through the local stack.
- Known module/global/native value references in synthesized expressions are
  rewritten to `CoreExpr::Global(GlobalRef)`. `CoreExpr::Var(String)` remains
  the unresolved/lowered-core name form and is still accepted by direct
  evaluator paths used below the elaboration boundary.
- `ResolvedDecl` now carries value binding ids before elaboration/evaluation.
  Same-module value references are resolved to those ids in `ResolvedModuleBody`;
  imported/context value references are resolved after import/context binding
  through the `ResolvedValueBindings` capability. Module-local value bindings
  are rebased against already-bound context/import bindings before elaboration,
  so `GlobalRef` ids share one runtime namespace. Elaboration no longer rewrites
  global names into global ids.

### Runtime Value

Owned by evaluator.

Purpose:

- Represent normalized computation results.
- Represent closures and native functions internally.
- Keep runtime environment state encapsulated behind an environment type instead
  of exposing map/reference-counting details to callers.

Should not be:

- The primary type checker output.
- The direct emitter input before output validation.

### Data Output Value

Owned by compiler/output validation.

Purpose:

- Represent values that are safe to emit.
- Exclude functions and other non-output runtime values.

Emitters should consume this representation.

## Trait And API Boundaries

Traits should be introduced only where there is a real boundary.

### Source Provider

Use a source provider to decouple compiler logic from filesystem access.

```rust
pub trait SourceProvider {
    fn load(&mut self, path: &SourcePath) -> Result<SourceFile>;
}
```

Implementations:

- Filesystem-backed provider.
- In-memory provider.
- Virtual or generated-source provider.
- Any other provider needed by a frontend or embedding host.

The provider interface is capability-based. Core compiler phases should depend
on "load this source" rather than on why the source exists or which caller
requested it.

The parser can remain a concrete implementation detail of the syntax layer. A
parser trait is not required unless there is more than one parser capability to
abstract. The refactor may still change parser output types and diagnostic
construction.

### Phase Contracts

Some compiler phases may be traits, or they may be concrete services with these
same input/output contracts. The important part is the contract, not trait use
for its own sake.

```rust
pub trait ModuleResolver {
    fn resolve(&mut self, entry: ModuleId) -> Result<ResolvedProgram>;
}

pub trait TypeChecker {
    fn check_module(&mut self, module: CoreModule) -> Result<ElaboratedModule>;
}

pub trait Normalizer {
    fn normalize(&self, expr: CoreExprId, env: &RuntimeEnv) -> Result<Value>;
}
```

These contracts should prevent phases from reaching backward into source
loading, CLI options, or output formatting.

### Compiler Pipeline

Expose one public pipeline API.

```rust
pub struct Compiler<P> {
    pub sources: P,
    pub options: CompilerOptions,
}

impl<P: SourceProvider> Compiler<P> {
    pub fn check(&mut self, input: CompileInput) -> Result<CheckOutput>;
    pub fn eval(&mut self, input: CompileInput) -> Result<EvalOutput>;
}
```

This API should be used by:

- Any frontend or embedding host.
- Integration tests through the same public entry points.

The pipeline API should not expose caller-specific methods. Caller-specific
state should be adapted into `CompileInput`, source providers, compiler options,
or emit options.

### Emitter

Emitters should be independent of checking/evaluation.

```rust
pub trait Emitter {
    fn format(&self) -> OutputFormat;
    fn emit(&self, value: &DataValue, options: &EmitOptions) -> Result<String>;
}
```

Implementations:

- JSON.
- ReConf data syntax.
- Additional output formats when added.

### Native Registry

Native functions should be described by structured registry entries.

```rust
pub struct NativeSpec {
    name: Symbol,
    ty: CoreType,
    arity: usize,
    implementation: NativeImplementation,
}
```

`NativeSpec` should expose narrow accessors and an apply boundary; callers
should not reach into registry storage or callback details.

## Crate Split

The crate split should happen after the boundaries above exist internally. The
goal is to move coherent modules, not to split first and then chase compile
errors.

### Workspace Layout

Target layout:

```text
Cargo.toml
crates/
  core/       # package: reconf-core
  compiler/   # package: reconf-compiler
  cli/        # package: reconf-cli
  wasm/       # package: reconf-wasm
tests/
examples/
doc/
```

### `reconf-core`

Owns language internals and compiler phases that do not depend on terminal UI
or output format policy.

Move or create:

- Source map, source ids, spans.
- Structured diagnostics and error codes.
- Syntax modules and surface AST.
- Resolver data structures.
- Core AST.
- Lowering.
- Type checking and elaboration.
- Evaluator/normalizer.
- Refinement validation.
- Builtin/native registry.
- Prelude definitions.

Must not depend on:

- `clap`.
- `reedline`.
- Terminal-specific reporting.
- CLI argument structures.
- Output format policy.

### `reconf-compiler`

Owns orchestration and output-facing compiler APIs.

Move or create:

- `Compiler`.
- `CompilerOptions`.
- `CompileInput`.
- `CheckOutput`.
- `EvalOutput`.
- Output validation.
- Emitters.

Depends on:

- `reconf-core`.

Must not depend on:

- CLI.
- REPL UI.
- Terminal reporter setup.

### `reconf-cli`

Owns user interfaces.

Move:

- `main.rs`.
- `cli.rs`.
- `repl/`.
- Terminal reporter setup.
- Filesystem provider wiring.

Responsibilities:

- Parse command-line flags.
- Choose emitter/options.
- Build filesystem-backed compiler.
- Render diagnostics for terminal output.
- Run REPL UI.

Must not:

- Reimplement parse/lower/typecheck/eval.
- Reach into resolver internals.
- Own compiler phase logic.

### `reconf-wasm`

Placeholder crate for the separate WASM branch.

Initial state:

- Minimal crate with documented intent.
- May be excluded from default workspace members if necessary.
- Should not block native CLI refactor work.

Future responsibilities:

- Bindings for the WASM branch.
- Target-specific source provider.
- Serializable diagnostics.
- Calls into `reconf-compiler`.

## Module-Level Refactor Plan

### Workspace Crate Roots

Current:

- The root `Cargo.toml` is a workspace manifest and no longer defines a
  compatibility package.
- `crates/core/src/lib.rs` exposes language-phase modules and common
  diagnostics/error aliases for `reconf-core`.
- `crates/compiler/src/lib.rs` exposes the supported compiler-facing API:
  `Compiler`, `CompileInput`, check/eval outputs, `DataValue`, emitter
  facades, and common error aliases.
- `crates/cli/src/lib.rs` exposes only the host entrypoint and REPL modules
  needed by CLI/REPL tests. The binary is `crates/cli/src/main.rs`.
- `crates/wasm/src/lib.rs` is a documented placeholder.

Target:

- Keep old single-crate compatibility re-exports removed.
- Continue shrinking visibility where a module is only an implementation detail.
- Keep compiler-facing API in `reconf-compiler`; host UI API belongs in
  `reconf-cli`.
- Do not add target-specific code to `reconf-core`.

### `crates/cli/src/cli.rs`

Current:

- Parses CLI.
- Reads source files.
- Parses and lowers.
- Creates loader.
- Evaluates file.
- Selects emitter.

Target:

- Parse CLI only.
- Build compiler input and options.
- Build or select a filesystem-backed source provider.
- Call compiler pipeline.
- Render diagnostics.
- Print output.

Concrete cleanup:

- `eval_path` has been removed from the compiler API.
- Remove direct `parse`, `lower_file`, old `Loader`, and free-function module
  evaluation usage.
- Keep `check` on the check-only compiler path; do not validate data-output
  constraints for `reconf check`.
- Adapt CLI formatting flags into `OutputStyle` at the host boundary; emitters
  receive structured options, not CLI flag booleans.

### `src/resolve/modules.rs`

Current:

- Removed as a live module.
- The old loader behavior is now in `crates/compiler/src/compiler/loader.rs` as
  `ModuleLoader`, but source-provider access is private to the loader and routed
  through intention-specific methods.
- `compiler::loader::graph::ModuleGraph` owns resolved-module cache state,
  cycle tracking, loading tokens with resolved-module builders, and
  resolved-program recording.
- `src/resolve/resolved.rs` now owns `ResolvedProgram`, `ResolvedModule`, and
  `ResolvedExports`.
- `ModuleLoader` delegates resolved-program recording to `ModuleGraph`, whose
  cache entries carry resolved module artifacts directly.
- `ModuleLoader::load_resolved` is now the import-facing load path; evaluated
  module cache access stays private to graph loading.
- `compiler::front::FrontendCompiler` owns parse/lower orchestration for
  `CompilerPipeline`, imported module loading, bundled prelude compilation, and
  session inputs. `SessionArtifacts` owns accepted declaration storage and
  composes frontend outputs after the shared frontend service has parsed and
  lowered them.
- `ResolvedImport` owns requested import selection against a target export table;
  `ResolvedImportSelector` owns the unexported or duplicate requested-name
  diagnostics behind that capability.
- `ResolvedModuleBody` exists with explicit import data and `ResolvedDecl`
  declarations, and is the module-evaluation input. Value declarations already
  carry `GlobalRef` ids; same-module expression references are rewritten to
  `CoreExpr::Global`, and imported/context references are rewritten through
  `ResolvedValueBindings` before elaboration. Module-local value bindings are
  rebased after imports/context are bound to avoid `GlobalRef` collisions.

Target:

- Rebuild `resolve` around explicit resolved module/program data structures.
- Keep concrete source reads delegated to `SourceProvider`, with provider access
  encapsulated behind loader/pipeline APIs rather than exposed as mutable state.
- Keep parsing/lowering/checking/evaluation in compiler pipeline phases.
- Keep prelude setup as explicit compiler context policy.

Potential split:

- `loader/graph.rs`: loading cache, resolved-program recording, and cycles.
- `imports.rs`: import/export validation.
- `names.rs`: binding ids and scopes.
- `resolved.rs`: resolved module/program structures.

### `src/resolve/names.rs`

Current:

- Contains `BindingId` as a `GlobalRef` alias, a `BindingIds` allocator,
  explicit `Namespace`, and `NameScope`.
- `NameScope` tracks value/type namespaces and reports generic name collisions.
  Module import binding translates those collisions into import diagnostics.
- `ResolvedModuleBody` delegates binding-id assignment, same-module
  value/type-name resolution, external value/type-name resolution, and
  binding-id rebasing to `ResolvedBodyResolver` / `BindingRebaser` services
  instead of exposing those transformations as loose helper chains.
- Local shadow tracking during value-name resolution is owned by
  `LexicalShadowScope`, so lambda/let shadowing policy is not represented as an
  ad hoc `Vec<String>` threaded through recursive helpers.

Target:

- Grow into real symbol/binding infrastructure across resolved modules.
- Avoid raw strings for all resolved value references after name resolution.
- Keep local binders represented as `LocalRef` in evaluator input, while
  preserving user names as diagnostics metadata.
- Keep module/global/native value references as `GlobalRef` binding ids rather
  than overloading local De Bruijn references.
- Keep same-module, imported, prelude, and context type alias references as
  `TypeAliasRef` / `ResolvedAlias` after resolved-body construction.
  `CoreType::Alias(String)` should only represent lowered or intentionally
  unresolved syntax that will be diagnosed by core type services.
- Resolve external type names through a narrow binding capability
  (`ResolvedTypeBindings`) rather than passing module maps into core/type
  phases.
- Use explicit namespace scopes for all name-definition checks instead of
  scattered map membership tests in later compiler phases.

### `crates/core/src/lower/desugar.rs`

Current:

- Recursively transforms `FileAst` into `CoreModule`.
- Lowers interpolation into `show` and `++`.
- Keeps the public `SurfaceToCoreLowerer` as an orchestration facade while
  delegating module, declaration, type, expression, and interpolation lowering
  to narrow internal services with explicit methods and owned collaborators.

Target:

- Produce core syntax.
- Own syntax-directed desugaring only.
- Keep new lowering rules attached to the smallest service that owns the
  corresponding surface construct instead of adding broad `lower_*` helper
  methods to the facade.
- Do not do type-directed option insertion.
- Do not evaluate.
- Preserve source-origin metadata for diagnostics.

### Removed `src/typeck/bidir.rs`

Current:

- Removed as live code.
- Core elaboration and runtime value checking now own the behavior that used to
  live in the surface checker.

Target:

- Check core expressions.
- Return typed/elaborated core.
- Make contextual elaboration explicit.
- Separate shape checking from normalization/refinement validation.
- Check refinement predicates for ordinary shape, including `Bool` predicate
  type, but leave predicate truth validation to the refinement phase.
- Produce structured diagnostics with spans.

Potential split:

- `check.rs`: bidirectional judgments.
- `elaborate.rs`: explicit elaboration forms and helpers.
- `types.rs`: type equality/compatibility.
- `aliases.rs`: alias expansion and recursion checks.
- `records.rs`: closed record logic.

### Removed `src/typeck/unify.rs`

Current:

- Removed as live code.
- Its old responsibilities are now covered by core type environments,
  `CoreTypeValidator`, `CoreElaborator`, and `CoreValueChecker`.

Target:

- Do not reintroduce surface-type/runtime-value compatibility logic.
- Keep type expansion/equality in core-oriented type services.
- Use richer expected/actual type diagnostics.

### Removed `src/typeck/wf.rs`

Current:

- Removed as live code.
- Surface-type well-formedness is no longer a separate checker path.
- Core type alias validation now lives in `CoreTypeValidator` and module
  elaboration.
- `CoreTypeEnv` stores aliases by name and by `TypeAliasRef`, so
  `CoreType::ResolvedAlias` can be validated and expanded without relying on a
  display name.

Target:

- Keep well-formedness checks core-native.
- Do not reintroduce surface `Type` validation after lowering.
- Route alias expansion, unknown alias diagnostics, and recursive alias
  diagnostics through core type services.

### Removed `src/typeck/env.rs`

Current:

- Removed as live code.
- The old aliases mixed surface `Type` aliases with runtime `Value`
  environments and were no longer used by the core-oriented checker.

Target:

- Keep runtime environments in evaluator/runtime modules.
- Keep type alias environments in core/type services.
- Do not add shared environment bags that mix phase-owned data.

### `crates/core/src/eval/mod.rs`

Current:

- Contains runtime values, primitive operations, core normalization/evaluation
  modules, and builtin/prelude modules.
- Synthesized and checked module expressions can evaluate `CoreExpr` directly.
- `CoreEvaluator` is strict and rejects checked syntax that should have been
  handled by normalization/elaboration.
- Checked normalization validates runtime values against `CoreType` through a
  dedicated core value-checking service.
- Core refinement predicates are evaluated by a dedicated
  `CoreRefinementValidator`.
- Refinement predicate binder-name preparation is owned by
  `refine::validate::CheckedRefinementPredicateBuilder`; `eval::core` no
  longer carries a temporary predicate-preparer implementation or depends on
  type checking for this rewrite.
- Module type aliases are stored as `CoreType` through `CoreTypeEnv`.
- Resolved same-module, imported, prelude, and context type alias references
  use `CoreType::ResolvedAlias` and expand through `CoreTypeEnv::alias_by_ref`.
- Runtime closures store `CoreExpr`.
- Runtime function application is exposed through a core-side applicator service;
  builtin higher-order functions no longer need to construct the legacy surface
  evaluator to apply closures.
- The old `CheckedCoreNormalizer` bridge has been removed. Callers that need a
  checked path elaborate through `CoreElaborator` or `CoreModuleElaborator`,
  then normalize through `PreparedCoreNormalizer`.
- `PreparedCoreNormalizer` owns prepared-core normalization and delegates
  runtime value validation to `CoreValueChecker`.
- Runtime closure application evaluates prepared core directly instead of
  re-entering checked normalization.
- Runtime environments are represented by `Env`, which owns construction,
  lookup, and extension; callers no longer construct or clone raw environment
  maps through helper functions.
- Module state exposes a runtime environment capability through `runtime_env`;
  declaration evaluation does not inspect or clone the module's raw value map.
- Checked ascriptions are erased before prepared normalization, so the prepared
  normalizer no longer has an ascription-to-type-checking path.
- Module evaluation now receives an `ElaboratedModule` from
  `CoreModuleElaborator` before evaluating declarations and output.
- Checked elaborated expressions are evaluated through `PreparedCoreNormalizer`.
- Module values now track known value types separately from runtime values, and
  typed value exports preserve that metadata across imports.
- Compiler module export storage is private and module-owned; the resolved layer
  receives `ResolvedExports` only through the explicit projection boundary.
- Module state owns selected import application so import binding does not match
  directly on resolved export variants.
- `ResolvedImportSelection` owns traversal of selected exports and applies them
  through `ResolvedImportTarget`, avoiding map-shaped selection APIs.
- Module value-type lookup is exposed to elaboration through a
  `CoreValueTypeContext` adapter, not through the module's internal type map.
- Arbitrary `BTreeMap<String, CoreType>` values are no longer treated as
  semantic value-type contexts; callers provide an explicit context object when
  they need that capability.
- `ModuleDeclEvaluator` owns execution of `ElaboratedDecl` values and final
  output evaluation, so declaration execution is separated from module import
  loading and core elaboration orchestration.
- Compiler module execution is split across `compiler/module/state.rs`,
  `compiler/module/imports.rs`, and `compiler/module/eval.rs`, with
  `compiler/module.rs` acting as the facade.

Target:

- Evaluate typed/elaborated core.
- Remove dependency on type checker.
- Keep runtime environment and closures.
- Move output validation out.
- Move ReConf value printing into emitter.
- Keep primitive operation behavior deterministic and tested.
- Remove all evaluator-related `TEMP(refactor-stage-*)` markers before
  considering this stage complete.

Potential split:

- `value.rs`: runtime values.
- `env.rs`: runtime environments.
- `eval.rs`: evaluator.
- `ops.rs`: primitive operations.
- `native.rs`: native application bridge.

Temporary cleanup targets:

- Keep focused evaluator tests on explicit typed/elaborated core artifacts
  rather than reintroducing a production checked-normalizer bridge.
- The surface evaluator and `typeck::bidir` checker bridge have been removed.
- Reverse core-to-surface lowering has been removed. Keep lowering one-way:
  surface syntax enters the compiler once, then later phases operate on core.

### `crates/core/src/eval/builtins.rs`

Current:

- Defines `NativeRegistry`, `NativeSpec`, native arity, and runtime
  implementation selection.
- Native specs carry type metadata and module evaluation rejects mismatched
  native declarations.
- `NativeSpec` encapsulates metadata behind accessors, and `NativeFunction`
  applies through registry metadata.
- `NativeImplementation` and `NativeCall` keep runtime implementation dispatch
  private to the builtin module.
- Tests verify registry entries and exported prelude native names/types agree.

Target:

- Keep native runtime behavior cohesive and testable behind registry-owned
  implementation services.
- Report structured native-call diagnostics.
- Decide whether broader runtime behavior than prelude signatures is intended
  or temporary.

### `crates/compiler/src/compiler/prelude.rs` And `crates/core/src/eval/prelude.reconf`

Current:

- Prelude is parsed and evaluated through compiler module logic with an empty
  `ModuleContext`, using `PreludeCompiler`.
- `PreludeCompiler` returns a `ModuleContext` for normal compiler construction,
  so loader policy does not call raw prelude module construction directly.
- `prelude::source` exposes bundled source for registry/prelude agreement
  checks.

Target:

- Prelude setup is part of compiler context.
- Native registry and prelude declarations should be checked against each other.
- Avoid hidden filesystem/module behavior for bundled prelude.

### `crates/core/src/refine/validate.rs`

Current:

- Contains `CoreRefinementValidator` for checked core predicate expressions and
  normalized values.
- `CoreRefinementValidator` is a concrete service over an owned runtime
  environment; unused lifetime/phantom scaffolding has been removed.
- `CheckedRefinementPredicate` is the predicate validation input and can carry
  either a borrowed already-checked predicate or an owned predicate prepared by
  typeck.
- `refine::validate::CheckedRefinementPredicateBuilder` prepares source-style
  refinement binder names into `LocalRef` while respecting nested lambda/let
  shadowing.
- `eval::core` asks the typeck-owned builder for checked predicate input, then
  delegates concrete predicate truth validation to `CoreRefinementValidator`.

Target:

- Validate core predicate expressions after normalization.
- Receive checked core predicate metadata and normalized value.
- Return structured refinement diagnostics.
- Keep concrete evaluation semantics.
- Move refinement predicates into typed core type artifacts when the core type
  representation is ready, so `CoreType::Refinement` no longer stores raw
  source-style predicate expressions.

### `crates/compiler/src/emit/json.rs`

Current:

- Converts `DataValue` to JSON.
- No longer inspects runtime closures or native functions.

Target:

- Move to `reconf-compiler`.
- Consume `DataValue`.
- Keep JSON ordering deterministic.
- Keep formatting as `EmitOptions { style: OutputStyle }`.

### `crates/core/src/core/`

Current:

- Owns the real core AST and core type environment.
- `CoreTypeEnv` and `CoreTypeValidator` provide the core-native alias context
  used by module evaluation and runtime normalization.
- `CoreTypeEnv` exposes alias-definition and generic alias-name visiting
  capabilities, not raw map iteration, storage conversion, or resolver-specific
  scope types.
- The old `core::pretty` helper has been removed because it depended upward on
  emitters and runtime output validation.

Target:

- Define ids/symbols if they are not owned by resolver.
- Provide core-only debug helpers if needed.
- Do not depend on evaluator, emitters, terminal UI, or output-format policy.

### `crates/core/src/source.rs`

Current:

- Minimal source map that is not integrated with most phases.

Target:

- Become central source infrastructure.
- Track source ids, paths, text, and spans.
- Support filesystem and memory-backed source loading through providers.
- Make diagnostics refer to source ids instead of embedding source text early.

### `crates/core/src/diagnostic.rs` And `crates/core/src/error.rs`

Current:

- `ErrorCode` table is useful.
- `Error` stores structured labels and notes.
- Parser duplicate-field diagnostics attach producer-owned labels.
- Parser empty-interpolation diagnostics attach producer-owned labels.
- Parser unterminated-interpolation diagnostics attach producer-owned labels.
- Unknown core type diagnostics use `ErrorCode::TypeUnknown` rather than the
  uncategorized fallback code.
- The parser no longer classifies top-level parse failures as empty
  interpolation by scanning the whole source for `{}`.
- Top-level unterminated-string classification is tied to the Pest error
  location and local quote state instead of a whole-file quote count.
- Span attachment is message-driven.

Target:

- Keep `ErrorCode` as the code registry.
- Continue moving phase producers to structured diagnostic data.
- Move `miette` rendering conversion to CLI/reporter layer.
- Remove message-based span recovery.

### `crates/cli/src/repl/`

Current:

- UI code and evaluator wrapper are in the same module tree.
- `ReplEvaluator` delegates persistent declaration state to
  `compiler::session::CompilerSession`.
- Declaration-only and expression REPL inputs compile through
  `CompilerSession`.
- The old sentinel output expression has been removed from REPL evaluation.

Target:

- Keep UI, highlighter, validator, prompt, and reporter in CLI crate.
- Make REPL evaluation call shared compiler API.
- Use compiler-owned session state and pluggable source providers.
- Keep semantic highlighting independent from compiler internals.
- Grow compiler sessions toward reusable module/type/evaluation state without
  reintroducing frontend-owned source accumulation.

### `tests/`

Current:

- Fixture and determinism tests still have local corpus traversal/evaluation
  helpers, but normal output rendering now uses `EmitterRegistry`.
- CLI tests call binary.
- REPL tests call REPL evaluator internals.

Target:

- Shared test support around compiler API.
- Phase-specific unit tests for core lowering, type elaboration, diagnostics,
  and output validation.
- Existing fixture outputs remain the main behavior guard.

## Diagnostics Refactor

Diagnostics should move from display-message-based errors to structured
compiler diagnostics.

Target shape:

```rust
pub struct Diagnostic {
    pub code: ErrorCode,
    pub message: String,
    pub labels: Vec<DiagnosticLabel>,
    pub notes: Vec<String>,
}

pub struct DiagnosticLabel {
    pub span: Span,
    pub message: String,
    pub kind: LabelKind,
}
```

Principles:

- The phase that detects the error owns the diagnostic.
- Display text is not parsed by later code.
- `ErrorCode` remains the single source for code strings and explanations.
- CLI/reporter converts diagnostics to `miette`.
- Tests can assert code and label spans directly.

Migration:

- Keep current `Error` wrapper initially.
- `Error` now stores structured labels and notes.
- Update high-value diagnostics first:
  - recursive aliases;
  - unknown type;
  - missing fields;
  - unknown fields;
  - refinement failures;
  - module import errors.
- Parser duplicate-field diagnostics already carry producer-owned labels.
- Parser empty-interpolation diagnostics already carry producer-owned labels.
- Parser unterminated-interpolation diagnostics already carry producer-owned
  labels.
- Parser top-level error-code selection no longer uses `{}` or whole-file quote
  count heuristics.
- Unknown type diagnostics now have a dedicated error code.
- Unknown and recursive type diagnostics now attach producer-owned labels from
  core type origins instead of message-text recovery.
- Division-by-zero diagnostics now attach producer-owned labels from core
  expression origins instead of message-text recovery.
- Refinement failure diagnostics now attach producer-owned labels from checked
  expression origins instead of message-text recovery.
- Replace compiler-boundary source attachment after diagnostics carry stable
  source ids directly.

## Output And Emitters

Current output behavior is spread across resolver, evaluator, and JSON emitter.
The refactor should make output a dedicated compiler phase.

Target:

```text
Value
  -> output validation
  -> DataValue
  -> Emitter
  -> String
```

`DataValue` should support:

- Int.
- Float.
- Bool.
- String.
- Null/none.
- Lists.
- Records in deterministic order.

It should exclude:

- Closures.
- Native functions.
- Any future non-data runtime values.

Emitters:

- JSON emitter.
- ReConf data emitter.
- `EmitterRegistry` chooses an emitter by `OutputFormat` from registered
  `Emitter` implementations rather than hard-coding every concrete emitter in
  host code.
- Additional format emitters when added.

Benefits:

- One function-output check.
- Emitters become simpler.
- Future formats do not touch evaluator/type checker.

## Builtin And Native Registry Refactor

Current native metadata is split between:

- `prelude.reconf`;
- `prelude.rs`;
- `NativeRegistry` entries.

Target registry:

```rust
pub struct NativeSpec {
    name: Symbol,
    ty: CoreType,
    arity: usize,
    implementation: NativeImplementation,
}

pub struct NativeRegistry {
    specs: BTreeMap<Symbol, NativeSpec>,
}
```

Goals:

- One place to enumerate native functions.
- One source for arity.
- One source for runtime implementation lookup.
- Checked relationship between native declaration type and implementation.
- Better unsupported-argument diagnostics.

This should remain conservative. Do not introduce full polymorphic type schemes
unless that becomes an explicit language decision.

## Source Loading And Module Resolution

Source loading should not be hardcoded into resolver internals.

Target:

```rust
pub trait SourceProvider {
    fn load(&mut self, path: &SourcePath) -> Result<SourceFile>;
}
```

Concrete providers:

- Filesystem provider.
- In-memory provider.
- Virtual/generated-source provider.
- Frontend-specific providers implemented outside core compiler phases.

Resolver target responsibilities:

- Canonical module identity.
- Import graph traversal.
- Cycle detection.
- Export visibility.
- Duplicate import/name checks.
- Resolved binding tables.

Resolver should not:

- Print diagnostics.
- Emit output.
- Evaluate final configuration.
- Know CLI flags.

## Workspace Migration

Do the workspace split after internal APIs are stable enough to move.

### Target Workspace

```text
Cargo.toml
crates/
  core/       # package: reconf-core
  compiler/   # package: reconf-compiler
  cli/        # package: reconf-cli
  wasm/       # package: reconf-wasm
examples/
tests/
doc/
```

### `reconf-core`

Owns:

- Source infrastructure.
- Diagnostics data model and error codes.
- Syntax and surface AST.
- Resolver.
- Core AST and symbols.
- Lowering.
- Type checking and elaboration.
- Evaluation/normalization.
- Refinement validation.
- Native registry and prelude internals.

Should not depend on:

- `clap`.
- `reedline`.
- Terminal UI.
- CLI reporter setup.
- Output format policy.

### `reconf-compiler`

Owns:

- Public compiler pipeline.
- Compiler options.
- Check/eval outputs.
- Output validation.
- Emitters.
- Test-friendly API.

Depends on:

- `reconf-core`.

Should not depend on:

- CLI.
- REPL UI.
- Terminal renderer setup.

### `reconf-cli`

Owns:

- Binary entry point.
- Command-line parsing.
- REPL UI.
- Terminal reporting.
- Filesystem provider wiring.

Depends on:

- `reconf-compiler`.

Should not own:

- Resolver internals.
- Type checker internals.
- Evaluation orchestration.

### `reconf-wasm`

Owns later bindings for the WASM branch.

For this refactor:

- Add placeholder crate or documented workspace placeholder.
- Keep it minimal.
- Do not block native build/test workflow.

## Migration Stages

### Stage 1: Centralize Pipeline Inside Current Crate

Before moving crates, remove duplicated orchestration.

Tasks:

- `crates/compiler/src/compiler.rs`, `crates/compiler/src/compiler/`, and the internal `CompilerPipeline`
  service are in place.
- `CompileInput`, `CompilerOptions`, `CheckOutput`, and `EvalOutput` are in
  in place; compiler result objects expose methods rather than public storage
  fields.
- Shared check/eval orchestration has moved out of caller-specific code.
- Current CLI, REPL, and integration callers use the compiler pipeline.
- Keep existing fixtures unchanged.

Exit criteria:

- Current frontends and integration tests use one pipeline path.
- No behavior changes.

### Stage 2: Add Source Provider Abstraction

Tasks:

- Introduce `SourceProvider`.
- Implement filesystem provider.
- Implement in-memory provider.
- Make pipeline use providers.
- Remove ad hoc file reads from CLI/tests/module loader where possible.

Exit criteria:

- Filesystem-backed callers use the filesystem provider.
- Tests and other callers can evaluate sources through the same provider-based
  API.
- Resolver no longer owns raw filesystem policy.

### Stage 3: Introduce Core AST

Tasks:

- Define `CoreType`, `CoreExpr`, and module-level core structures.
- Make lowering produce core syntax.
- Keep temporary adapters only when needed to preserve behavior during the
  migration, and mark each one with `TEMP(refactor-stage-*)`.
- Move method/interpolation/literal-union lowering into core-lowering path.
- Preserve current behavior in fixtures.

Exit criteria:

- Later phases can start consuming core.
- Surface AST is no longer the only compiler IR.

### Stage 4: Add Typed/Elaborated Core

Tasks:

- Define typed/elaborated expression structures.
- Define a module-level elaboration service.
- Refactor type checker to return typed core.
- Implement the local-reference representation for elaborated core:
  `LocalRef` De Bruijn indices are in place for lambda/let locals and checked
  refinement predicate validation.
- Implement the global-reference representation for elaborated core:
  `GlobalRef` binding ids are in place for known module/global/native value
  references, and value declaration ids now live in `ResolvedDecl` before
  elaboration/evaluation. Same-module and imported/context value references are
  rewritten before elaboration.
- Make implicit `some` explicit.
- Make omitted option fields explicit.
- Keep closed-record behavior.
- Keep refinement shape checks.
- Check refinement predicates have type `Bool`, but do not validate predicate
  truth in this stage.
- Keep any temporary prepared-but-untyped path explicit and marked.

Exit criteria:

- Type checker result is not runtime `Value`.
- Elaboration can be inspected/tested separately.
- Evaluator input no longer uses raw local-variable strings as semantic
  identity for lambda/let locals.
- Module evaluation consumes `ElaboratedModule`, not raw `CoreModule`
  declarations.

### Stage 5: Refactor Evaluator

Tasks:

- Make evaluator consume typed/elaborated core.
- Remove evaluator dependency on type checker.
- Move ascription/annotation handling fully into type checking.
  - Checked ascriptions are erased during elaboration and rejected if they reach
    prepared normalization.
- Keep runtime `Value` and closure behavior.
- Move ReConf value printing out of evaluator.
- Keep checked-normalization orchestration explicit: elaborate first, then pass
  typed/prepared core to the evaluator.

Exit criteria:

- `eval` does not accept surface `Expr`.
- `eval` does not call `check_expr`.

### Stage 6: Refactor Output Validation And Emitters

Tasks:

- Add `DataValue` or `CheckedOutput`.
- Move function-output rejection into output validation.
- Make JSON emitter consume data output.
- Move ReConf output printing into emitter layer.
- Route CLI, REPL, and tests through `EmitterRegistry` unless they are testing a
  concrete emitter directly.

Exit criteria:

- Emitters do not inspect closures/native functions.
- Hosts no longer instantiate concrete emitters for normal output selection.
- Output validation owns function-escape diagnostics.

### Stage 7: Refactor Diagnostics

Tasks:

- Add structured diagnostic labels and notes.
- Thread source ids/spans through phase outputs.
- Convert important diagnostics phase by phase.
- Update tests to assert structured diagnostics where appropriate.
- Replace compiler-boundary source attachment after diagnostics carry stable
  source ids.

Exit criteria:

- `attach_best_effort_span` and `attach_source_to_labeled_error` are gone.
- Remaining source attachment is explicit compiler-boundary context, not
  message-text span recovery.

### Stage 8: Refactor Builtins And Prelude

Tasks:

- Introduce native registry.
- Centralize name, arity, type, and implementation.
- Make prelude setup part of compiler context.
- Add registry tests.

Exit criteria:

- Native metadata is not duplicated across unrelated functions.

### Stage 9: Split Workspace Crates

Tasks:

- Create workspace root.
- Move core internals to `reconf-core`.
- Move pipeline/output to `reconf-compiler`.
- Move CLI/REPL to `reconf-cli`.
- Add `reconf-wasm` placeholder.
- Update imports and public APIs.
- Drop obsolete re-exports instead of preserving backward API compatibility.

Exit criteria:

- Workspace builds.
- CLI binary works.
- Integration tests still pass.

### Stage 10: Cleanup

Tasks:

- Remove adapters and compatibility modules.
- Remove all `TEMP(refactor-stage-*)` code paths.
- Make internal modules private.
- Remove duplicated test helpers.
- Update examples and docs that mention crate layout.
- Run full quality gate.

Exit criteria:

- Architecture matches this document.
- Transitional code is removed.

## Testing Requirements

Keep existing coverage:

- Positive fixture JSON outputs.
- Negative fixture error-code assertions.
- Selected rendered diagnostic snapshots.
- CLI tests.
- Determinism tests.
- REPL tests.

Add refactor-focused coverage:

- Pipeline API tests.
- Source provider tests.
- Core lowering tests.
- Type elaboration tests.
- Output validation tests.
- Native registry tests.
- Structured diagnostic tests.

Testing rule:

Each migration stage should preserve the existing fixture corpus. When a stage
changes internal representations, add focused tests for the new representation
before removing the old path.

## Success Criteria

The refactor is complete when:

- The project is a workspace with:
  - `reconf-core`;
  - `reconf-compiler`;
  - `reconf-cli`;
  - `reconf-wasm` placeholder.
- Surface AST and core AST are distinct.
- Type checking produces typed/elaborated core.
- Evaluation consumes typed/elaborated core, not surface AST.
- Runtime `Value` is not the primary type checker output.
- Module resolution does not own full pipeline evaluation.
- Frontends, integration tests, and embedding hosts use the shared compiler API.
- Source loading is provider-based.
- Emitters consume data-only output.
- Native metadata is centralized.
- Structured diagnostics replace message-based span recovery.
- Current MVP behavior remains covered by fixtures and examples.
- The standard quality gate passes:

```sh
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

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

The repository currently contains a working single-crate MVP:

- `src/syntax/`
  - Pest grammar and parser.
  - Surface AST in `surface.rs`.
- `src/lower/`
  - Syntax-directed lowering for interpolation and recursive AST traversal.
  - Currently returns another `FileAst`, not a separate core representation.
- `src/resolve/`
  - Module loading.
  - Import/export validation.
  - Cycle detection.
  - Prelude insertion.
  - Calls type checking/evaluation as part of module evaluation.
- `src/typeck/`
  - Bidirectional checking entry points.
  - Shape validation, contextual option behavior, record checks, alias
    expansion, and refinement checks.
  - Currently returns runtime `Value`.
- `src/eval/`
  - Runtime `Value`.
  - Big-step evaluation over surface expressions.
  - Native builtin registry and native application.
  - Prelude loading.
  - ReConf-style value printing.
- `src/refine/`
  - Concrete refinement validation by evaluating predicates.
- `src/emit/`
  - JSON output.
- `src/diagnostic.rs` and `src/error.rs`
  - Error codes, miette integration, and best-effort source-span attachment.
- `src/cli.rs`
  - CLI command parsing.
  - File loading.
  - Parse/lower/eval orchestration.
  - Emitter selection.
- `src/repl/`
  - Reedline UI.
  - Syntax highlighting.
  - Input validation.
  - Accumulated-source evaluation.
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

This principle applies across all crates:

- Prefer target-neutral abstractions such as source providers, compiler inputs,
  diagnostics, typed core, and emitters.
- Keep frontend-specific behavior at the frontend boundary.
- Keep output-specific behavior at the emitter boundary.
- Keep test support outside production phase logic.
- Do not add special internal paths for CLI, REPL, tests, or future host
  targets when a generic capability is the real requirement.
- If one caller needs unusual behavior, model the underlying capability
  generically before adding caller-specific code.
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

- `src/syntax/surface.rs` defines `FileAst`, `Decl`, `Type`, `Expr`, and
  `StrPart`.
- `src/syntax/parser.rs` converts Pest pairs into this unspanned surface AST.
- `src/lower/desugar.rs` takes `FileAst` and returns `FileAst`.
- `src/typeck/bidir.rs` checks `Expr` and `Type`.
- `src/eval/mod.rs` evaluates `Expr`.
- `src/refine/validate.rs` evaluates refinement predicate `Expr`.
- `src/repl/semantic.rs` walks `FileAst` for semantic highlighting.

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

- `resolve::modules::Loader::load` reads imported files, parses, lowers, and
  calls `eval_file`.
- `eval_file` inserts prelude definitions, evaluates imports, checks
  declarations, evaluates output, and validates function escape.

Problem:

Resolver is responsible for module graph behavior and for most of the compiler
pipeline. This makes it hard to use the same pipeline from CLI, REPL, tests, or
other hosts without re-entering resolver internals.

Refactor direction:

- Resolver should own module graph loading, import/export validation, and name
  resolution.
- Pipeline orchestration should move to a compiler pipeline layer.
- Prelude setup should be explicit in compiler context construction.
- Output validation should move out of resolver.

### CLI, REPL, And Tests Duplicate Pipeline Logic

Current code:

- `src/cli.rs::eval_path` reads files, parses, lowers, creates a loader, and
  extracts `$output`.
- `src/repl/eval.rs` builds synthetic accumulated source, parses, lowers,
  creates a loader, evaluates, and emits.
- `tests/fixtures.rs` and `tests/determinism.rs` repeat similar helpers.

Problem:

Pipeline behavior can drift across hosts. Adding new outputs or changing module
loading requires edits in several places.

Refactor direction:

- Add a shared compiler pipeline API.
- CLI, REPL, and tests call the same API.
- Host-specific code provides sources and output preferences only.

### Diagnostics Are Coupled To Message Text

Current code:

- `Error` contains one optional span and label.
- `diagnostic::attach_best_effort_span` inspects error messages such as
  `unknown type`, `division by zero`, and `recursive type alias`.
- Some parser error codes are selected from source-text heuristics.
- Parser errors are converted to `Error` early, so later code does not receive
  structured parse error kinds.

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

- `resolve::modules::eval_file` checks `contains_function` on `$output`.
- `emit/json.rs` rejects functions again.
- `eval::emit` also rejects functions.

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

- `src/resolve/modules.rs`

Current violation:

- `Loader::load` canonicalizes filesystem paths, reads files from disk, parses,
  lowers, attaches source spans, recurses through imports, evaluates modules,
  and caches evaluated `Module` values.
- `eval_file_inner` inserts the prelude, checks declarations, evaluates values,
  builds exports, evaluates final output, and performs function-output
  validation.

Why this is ad hoc:

- Module resolution is tied to filesystem policy.
- Resolver owns compiler orchestration that should be generic pipeline logic.
- The prelude is a special path hidden inside module evaluation.
- Output validation is mixed into module loading.

Target:

- Source loading moves behind `SourceProvider`.
- Resolver owns module graph, import/export checks, and name resolution only.
- Compiler pipeline owns parse/lower/check/eval/refine/output orchestration.
- Prelude setup becomes compiler context construction.
- Output validation becomes a separate phase.

### CLI, REPL, And Tests Rebuild The Pipeline

Files:

- `src/cli.rs`
- `src/repl/eval.rs`
- `tests/fixtures.rs`
- `tests/determinism.rs`

Current violation:

- Each path performs its own read/parse/lower/load/evaluate/extract-output
  sequence.
- REPL constructs synthetic source with a sentinel output expression.
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

- `src/typeck/bidir.rs`
- `src/typeck/unify.rs`

Current violation:

- `check_expr` and `synth_expr` return `Value`.
- The checker evaluates expressions while checking.
- Contextual elaboration is represented only by returned runtime values.
- Runtime value/type compatibility lives next to type expansion logic.

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

- `src/eval/mod.rs`

Current violation:

- `Value::Closure` stores surface `Expr`.
- `eval` consumes surface `Expr`.
- `eval` accepts type aliases.
- `eval` calls `check_expr` for annotated local lets and ascriptions.
- `eval::emit` prints ReConf data syntax and rejects functions.

Why this is ad hoc:

- The evaluator owns part of static semantics.
- Runtime closures depend on surface AST.
- Output formatting is mixed into runtime evaluation.

Target:

- Evaluator consumes typed/elaborated core.
- Closures store core expression bodies.
- Evaluation does not call the type checker.
- ReConf data printing moves to an emitter.
- Function-output rejection moves to output validation.

### Surface AST Is Used As Every IR

Files:

- `src/syntax/surface.rs`
- `src/lower/desugar.rs`
- `src/core/ast.rs`
- `src/core/pretty.rs`

Current violation:

- Surface `Expr` and `Type` are used by parser, lowerer, checker, evaluator,
  refinement validator, REPL semantic tracking, and tests.
- `lower_file` returns another `FileAst`.
- `core::ast` re-exports surface AST and runtime `Value`.
- `core::pretty` delegates to runtime value printing.

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

- `src/diagnostic.rs`
- `src/syntax/parser.rs`
- `src/error.rs`

Current violation:

- `attach_best_effort_span` inspects rendered error messages to infer meaning.
- Parser error codes use source-text heuristics such as empty interpolation and
  unmatched quote checks.
- `Error` supports only one optional label.

Why this is ad hoc:

- Display text acts as structured diagnostic data.
- Source spans are rediscovered after the producing phase has already lost
  context.
- Diagnostics are hard to compose across phases.

Target:

- Phase-owned structured diagnostics.
- Parser constructs structured parse diagnostics directly.
- Error values carry multiple labels and notes.
- CLI/reporter handles `miette` rendering.

### Native Metadata Is Duplicated

Files:

- `src/eval/builtins.rs`
- `src/eval/prelude.rs`
- `src/eval/prelude.reconf`

Current violation:

- Native names are listed in `declared`.
- Native arities are listed separately in `arity`.
- Runtime behavior is listed separately in `call`.
- Prelude signatures are listed in `prelude.reconf`.
- Prelude module construction parses/lowers/evaluates through resolver internals.

Why this is ad hoc:

- Name, arity, type, and runtime implementation can drift.
- Some runtime behavior is broader than the exposed prelude signatures.
- Prelude setup is another special compiler path.

Target:

- Add `NativeRegistry`.
- Tie native name, arity, type, and implementation together.
- Make prelude setup part of compiler context.
- Add tests that native registry metadata and prelude exposure agree.

### Emitters Receive Runtime Values

Files:

- `src/emit/json.rs`
- `src/eval/mod.rs`
- `src/resolve/modules.rs`

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
  -> type-check and elaborate core
  -> normalize/evaluate elaborated core
  -> validate refinements
  -> validate data-only output
  -> emit output
```

The important refactor is the phase separation after parsing. Each phase should
consume the previous phase's output and produce a well-defined result.

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
- Explicit variable references by symbol or binding id.
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

### Typed/Elaborated Core

Owned by type checker/elaborator.

Purpose:

- Carry checked expression/type relationships.
- Make contextual rewrites explicit.
- Provide evaluator input.

Must make explicit:

- Inserted `some`.
- Inserted omitted optional fields.
- Checked record field order.
- Expanded or resolved aliases where needed.
- Native and user binding types.

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

### Runtime Value

Owned by evaluator.

Purpose:

- Represent normalized computation results.
- Represent closures and native functions internally.

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
    pub name: Symbol,
    pub ty: CoreType,
    pub arity: usize,
    pub implementation: NativeImpl,
}
```

This should replace scattered native metadata.

## Crate Split

The crate split should happen after the boundaries above exist internally. The
goal is to move coherent modules, not to split first and then chase compile
errors.

### Workspace Layout

Target layout:

```text
Cargo.toml
crates/
  reconf-core/
  reconf-compiler/
  reconf-cli/
  reconf-wasm/
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

### `src/lib.rs`

Current:

- Re-exports many implementation modules.
- Exposes `run`, `emit_json`, and error types directly.

Target:

- During transition, re-export compatibility APIs only where needed.
- Long term, public API should come from `reconf-compiler`.
- Internal modules should become private when moved to crates.

### `src/cli.rs`

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

- Remove `eval_path`.
- Remove direct `parse`, `lower_file`, `Loader`, and `eval_file` usage.
- Replace `pretty`/`compact` booleans with one output style enum if more output
  modes are added.

### `src/resolve/modules.rs`

Current:

- `Loader` owns cache and cycle detection.
- Loads files from filesystem.
- Parses and lowers imported files.
- Evaluates modules.
- Inserts prelude.
- Builds output value.

Target:

- Resolver owns module graph and import/export semantics.
- Source loading is delegated to `SourceProvider`.
- Parsing/lowering/checking/evaluation are pipeline phases.
- Prelude setup moves into compiler context.
- Output validation moves into compiler/output layer.

Potential split:

- `module_graph.rs`: loading/caching/cycles.
- `imports.rs`: import/export validation.
- `symbols.rs`: binding ids and scopes.
- `resolved.rs`: resolved module/program structures.

### `src/resolve/names.rs`

Current:

- Contains only `BindingId`.

Target:

- Grow into real symbol/binding infrastructure or move binding ids into core.
- Avoid raw strings for all resolved references after name resolution.
- Track value/type namespaces explicitly.

### `src/lower/desugar.rs`

Current:

- Recursively transforms `FileAst` into another `FileAst`.
- Lowers interpolation into `show` and `++`.

Target:

- Produce core syntax.
- Own syntax-directed desugaring only.
- Do not do type-directed option insertion.
- Do not evaluate.
- Preserve source-origin metadata for diagnostics.

### `src/typeck/bidir.rs`

Current:

- Checks surface expressions.
- Evaluates expressions while checking.
- Returns `Value`.
- Handles literal unions and refinements by validating runtime values.

Target:

- Check core expressions.
- Return typed/elaborated core.
- Make contextual elaboration explicit.
- Separate shape checking from normalization/refinement validation.
- Produce structured diagnostics with spans.

Potential split:

- `check.rs`: bidirectional judgments.
- `elaborate.rs`: explicit elaboration forms and helpers.
- `types.rs`: type equality/compatibility.
- `aliases.rs`: alias expansion and recursion checks.
- `records.rs`: closed record logic.

### `src/typeck/unify.rs`

Current:

- Expands aliases.
- Checks runtime value/type compatibility.
- Produces coarse type/value names.

Target:

- Move runtime value compatibility out of type unification.
- Keep type expansion/equality in typeck/core.
- Use richer expected/actual type diagnostics.

### `src/typeck/wf.rs`

Current:

- Checks type well-formedness and alias recursion.

Target:

- Keep as a separate phase.
- Return structured diagnostics.
- Use resolved type symbols instead of raw alias strings after name resolution.

### `src/eval/mod.rs`

Current:

- Evaluates surface `Expr`.
- Calls type checker for annotations/ascriptions.
- Contains `Value`, `Env`, closures, binary operations, function-output search,
  and ReConf value printing.

Target:

- Evaluate typed/elaborated core.
- Remove dependency on type checker.
- Keep runtime environment and closures.
- Move output validation out.
- Move ReConf value printing into emitter.
- Keep primitive operation behavior deterministic and tested.

Potential split:

- `value.rs`: runtime values.
- `env.rs`: runtime environments.
- `eval.rs`: evaluator.
- `ops.rs`: primitive operations.
- `native.rs`: native application bridge.

### `src/eval/builtins.rs`

Current:

- Defines native function names.
- Defines arity.
- Implements runtime calls.

Target:

- Replace name/arity duplication with registry entries.
- Keep implementation functions small and testable.
- Report structured native-call diagnostics.
- Decide whether broader runtime behavior than prelude signatures is intended
  or temporary.

### `src/eval/prelude.rs` And `src/eval/prelude.reconf`

Current:

- Prelude is parsed and evaluated through module logic.

Target:

- Prelude setup is part of compiler context.
- Native registry and prelude declarations should be checked against each other.
- Avoid hidden filesystem/module behavior for bundled prelude.

### `src/refine/validate.rs`

Current:

- Evaluates predicate expressions using runtime env and aliases.

Target:

- Validate core predicate expressions after normalization.
- Receive typed/core predicate and normalized value.
- Return structured refinement diagnostics.
- Keep concrete evaluation semantics.

### `src/emit/json.rs`

Current:

- Converts runtime `Value` to JSON.
- Rejects functions.

Target:

- Move to `reconf-compiler`.
- Consume `DataValue`.
- Keep JSON ordering deterministic.
- Keep pretty/compact formatting as emitter options.

### `src/core/`

Current:

- Mostly compatibility wrappers around `eval::Value` and surface AST.

Target:

- Become the real core AST and core type home.
- Define ids/symbols if they are not owned by resolver.
- Provide core pretty/debug helpers.
- Avoid depending on evaluator where possible.

### `src/source.rs`

Current:

- Minimal source map that is not integrated with most phases.

Target:

- Become central source infrastructure.
- Track source ids, paths, text, and spans.
- Support filesystem and memory-backed source loading through providers.
- Make diagnostics refer to source ids instead of embedding source text early.

### `src/diagnostic.rs` And `src/error.rs`

Current:

- `ErrorCode` table is useful.
- `Error` supports one optional label.
- Span attachment is message-driven.

Target:

- Keep `ErrorCode` as the code registry.
- Add structured diagnostic data.
- Support multiple labels and notes.
- Move `miette` rendering conversion to CLI/reporter layer.
- Remove message-based span recovery.

### `src/repl/`

Current:

- UI code and evaluator wrapper are in the same module tree.
- `ReplEvaluator` accumulates source text and re-runs the file pipeline.
- It manually parses, lowers, creates loader, and emits.

Target:

- Keep UI, highlighter, validator, prompt, and reporter in CLI crate.
- Make REPL evaluation call shared compiler API.
- Use memory/session source provider.
- Keep semantic highlighting independent from compiler internals.
- Avoid output suppression based on sentinel output value `"0"` leaking into the
  compiler API.

### `tests/`

Current:

- Fixture and determinism tests duplicate evaluation helpers.
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
- Add structured labels to `Error`.
- Update high-value diagnostics first:
  - recursive aliases;
  - unknown type;
  - duplicate fields;
  - missing fields;
  - unknown fields;
  - refinement failures;
  - division by zero;
  - module import errors.
- Remove `attach_best_effort_span` after producers carry spans directly.

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
- Additional format emitters when added.

Benefits:

- One function-output check.
- Emitters become simpler.
- Future formats do not touch evaluator/type checker.

## Builtin And Native Registry Refactor

Current native metadata is split between:

- `prelude.reconf`;
- `prelude.rs`;
- `builtins::declared`;
- `builtins::arity`;
- `builtins::call`.

Target registry:

```rust
pub struct NativeSpec {
    pub name: Symbol,
    pub ty: CoreType,
    pub arity: usize,
    pub implementation: NativeImpl,
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
  reconf-core/
  reconf-compiler/
  reconf-cli/
  reconf-wasm/
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

- Add `src/compiler/` or `src/pipeline.rs`.
- Define `CompileInput`, `CompilerOptions`, `CheckOutput`, `EvalOutput`.
- Move shared check/eval orchestration out of caller-specific code.
- Update all current callers to use the pipeline.
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
- Keep temporary adapters if needed.
- Move method/interpolation/literal-union lowering into core-lowering path.
- Preserve current behavior in fixtures.

Exit criteria:

- Later phases can start consuming core.
- Surface AST is no longer the only compiler IR.

### Stage 4: Add Typed/Elaborated Core

Tasks:

- Define typed/elaborated expression structures.
- Refactor type checker to return typed core.
- Make implicit `some` explicit.
- Make omitted option fields explicit.
- Keep closed-record behavior.
- Keep refinement shape checks.

Exit criteria:

- Type checker result is not runtime `Value`.
- Elaboration can be inspected/tested separately.

### Stage 5: Refactor Evaluator

Tasks:

- Make evaluator consume typed/elaborated core.
- Remove evaluator dependency on type checker.
- Move ascription/annotation handling fully into type checking.
- Keep runtime `Value` and closure behavior.
- Move ReConf value printing out of evaluator.

Exit criteria:

- `eval` does not accept surface `Expr`.
- `eval` does not call `check_expr`.

### Stage 6: Refactor Output Validation And Emitters

Tasks:

- Add `DataValue` or `CheckedOutput`.
- Move function-output rejection into output validation.
- Make JSON emitter consume data output.
- Move ReConf output printing into emitter layer.

Exit criteria:

- Emitters do not inspect closures/native functions.
- Output validation owns function-escape diagnostics.

### Stage 7: Refactor Diagnostics

Tasks:

- Add structured diagnostic labels and notes.
- Thread source ids/spans through phase outputs.
- Convert important diagnostics phase by phase.
- Update tests to assert structured diagnostics where appropriate.
- Remove message-based span recovery after replacement.

Exit criteria:

- `attach_best_effort_span` is gone or only a temporary fallback with no main
  diagnostics depending on it.

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
- Keep compatibility re-exports only if they reduce migration risk.

Exit criteria:

- Workspace builds.
- CLI binary works.
- Integration tests still pass.

### Stage 10: Cleanup

Tasks:

- Remove adapters and compatibility modules.
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

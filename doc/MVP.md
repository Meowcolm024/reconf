# ReConf MVP

This document fixes the first implementable ReConf subset. It follows
`doc/DESIGN.md` and the implementation roadmap while deferring features that
would make the first compiler depend on inference, effects, or symbolic
reasoning.

## Commands

The MVP command line is:

```sh
reconf check path/to/file.reconf
reconf eval path/to/file.reconf --format json
```

`check` parses, resolves imports, type-checks, normalizes declarations, validates
refinements, and rejects function-valued outputs. `eval` does the same work and
prints normalized JSON.

## Included Syntax

- Top-level `import`, `export type`, `export let`, `type`, and `let`.
- One final output expression per file.
- Base values: `Int`, `Float`, `Bool`, and `String`.
- Records, lists, options, local `let`, `if`, lambdas, function application, and
  expression ascription.
- String interpolation with `{ expression }`.
- Literal string unions as a type form.
- Refinement types `{ x : T | predicate }`.
- Field access and method syntax for the MVP built-ins.

## Type Checking

Type checking is bidirectional. Annotated declarations and ascriptions check
against the annotation. Unannotated declarations must synthesize a type.

Refinements are erased during ordinary shape checking and validated after the
checked expression normalizes to a concrete value. The MVP performs no SMT or
symbolic reasoning.

Records are closed. Unknown fields, duplicate fields, and missing non-option
fields are errors. When a record literal is checked against a known record type,
omitted option fields are inserted as `none` in type-field order.

When checking an expression against `T?`, `none` and `some expr` are accepted
directly. A value of type `T` is elaborated to `some value`.

## Built-Ins

The MVP supports:

- Arithmetic operators for matching `Int` or `Float`: `+`, `-`, `*`, `/`, `%`
  (`%` is `Int` only).
- Comparison operators: `==`, `!=`, `<`, `<=`, `>`, `>=`.
- Boolean operators: `&&`, `||`, `!`.
- String concatenation: `++`.
- Built-ins and matching methods: `show`, `isSome`, `isNone`, `length`,
  `contains`, `startsWith`, `endsWith`, and `unwrapOr`.

Functions may be used internally but are rejected at the output boundary.

## JSON Output

Normalized data emits as JSON:

- `Int`, `Float`, `Bool`, and `String` map to JSON scalars.
- `none` maps to `null`.
- `some value` maps to `value`.
- Lists map to arrays.
- Records map to objects in deterministic checked field order.

Non-finite floats and functions are not valid JSON outputs.

## Deferred

- Recursive values and recursive type aliases.
- General unions beyond string literal unions.
- User effects, environment access, remote imports, or package management.
- SMT-backed refinement solving.
- `map`, `filter`, and richer higher-order library functions.
- YAML/TOML output, formatter, LSP, and incremental compilation.

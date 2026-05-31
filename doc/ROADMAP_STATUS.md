# ReConf Roadmap Status

This status file tracks the implementation against
`doc/reconf_implementation_proposal_roadmap.md`.

## Completed Or Mostly Complete

- Phase 0: MVP subset is documented in `doc/MVP.md`.
- Phase 1: CLI shell, source loading, spans, diagnostics, and fixture harness.
- Phase 2: Lexer and parser for the MVP grammar.
- Phase 3: Local module loading, exported imports, duplicate-name checks, and
  cycle detection.
- Phase 5: Bidirectional shape checking for MVP types and expressions,
  contextual option elaboration, closed-record checking, and recursive-alias
  rejection.
- Phase 6: Big-step evaluator, closures, primitive application, deterministic
  records/lists, and function-output rejection.
- Phase 7: Concrete refinement validation by normalization.
- Phase 8: MVP built-ins for arithmetic, comparison, booleans, strings, lists,
  and options.
- Phase 9: JSON emitter, pretty/compact output, `--no-color`, and diagnostic
  code explanations.

## Partially Complete

- Phase 4: Surface conveniences are lowered during checking/evaluation rather
  than through a separate persisted core AST snapshot pipeline.
- Diagnostics: errors have codes and spans, and selected rendered diagnostics
  are snapshot-tested. More snapshots should be added as messages stabilize.
- Phase 10: Examples, CI, and release checklist exist. Parser fuzzing and
  property tests are not yet implemented.

## Deferred Beyond MVP

- General unions beyond string literal unions.
- Recursive values and recursive type aliases.
- Effects, environment access, remote imports, and package management.
- SMT-backed refinement solving.
- `map`, `filter`, richer higher-order library functions, formatter, LSP, and
  incremental compilation.

## Current Quality Gate

```sh
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

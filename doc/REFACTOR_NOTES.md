# ReConf Refactor Notes

This file tracks implementation debt that is visible in the current MVP. These
are not feature requests; they are places where the design should be tightened
before the codebase grows.

## Diagnostics

- `ErrorCode` is now the single source for diagnostic code strings and short
  explanations. CLI `--explain` must keep using `ErrorCode::from_code` and
  `ErrorCode::info` rather than adding another string table.
- `diagnostic::attach_best_effort_span` is still message-driven. It parses text
  such as `unknown type ...`, `division by zero`, and `recursive type alias ...`
  to recover spans. This should become structured diagnostic context carried by
  the error producer, not inferred from display messages.
- `Error` supports only one labeled span. Several diagnostics will eventually
  need primary and secondary labels, notes, and help text.
- Parser errors still classify some codes with heuristics such as checking the
  source for `{}` or unmatched quotes. These should come from parser-specific
  error constructors or structured parse error kinds.

## Source Locations

- Surface AST nodes do not carry spans. That forces later phases to rediscover
  source locations from text. Add spans to declarations, types, and expressions
  at parse time, then preserve or map them through lowering.
- The module loader reparses imported files but does not thread source names and
  source text through all diagnostics. Imported-file errors should report the
  imported file path directly without outer callers guessing spans.

## CLI

- `--compact` currently exists as an explicit alias for the default compact JSON
  mode. If more output modes are added, replace the `pretty`/`compact` booleans
  with a single output-style enum to avoid invalid flag combinations.
- `--explain` currently prints one-line explanations. If diagnostic metadata
  grows, move formatting into a dedicated reporter instead of expanding CLI code.

## Builtins And Prelude

- The prelude declares native functions and the runtime registry implements
  them. Keep that split: names should be exposed through `prelude.reconf`, while
  Rust should only register opaque native implementations.
- Native signatures are still specialized and partly duplicated between prelude
  declarations and Rust behavior. Longer term, add a registry entry type that
  ties name, arity, implementation, and expected runtime shape together.

## Tests

- Fixture tests now assert `.err` codes and compare `.stderr` snapshots, but only
  selected diagnostics have snapshots. Add snapshots for any diagnostic whose
  user-facing location or wording is important.
- Test helpers are duplicated between fixture and determinism tests. Extract a
  shared integration-test support module if this grows further.

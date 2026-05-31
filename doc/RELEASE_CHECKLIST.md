# ReConf v0.1 Release Checklist

Use this checklist before tagging an experimental v0.1 release.

## Required

- `cargo fmt --check`
- `cargo test`
- `cargo clippy -- -D warnings`
- CLI smoke test: `reconf check examples/simple.reconf`
- CLI smoke test: `reconf eval examples/simple.reconf --format json`
- Review `doc/MVP.md` for semantic drift.
- Review `doc/ROADMAP_STATUS.md` for honest status.
- Confirm all examples have expected `.json` outputs.
- Choose and add a license.

## Recommended

- Add more rendered diagnostic snapshots for common user errors.
- Add parser fuzzing or at least parser crash-resistance tests.
- Add property tests for deterministic normalization.
- Review import-path policy and document whether imports may escape the current
  directory tree.
- Decide whether the first release ships binaries or source-only instructions.

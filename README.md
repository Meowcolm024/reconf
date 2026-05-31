# ReConf

ReConf is an experimental configuration language built around a simply typed lambda calculus with refinement types.

The goal is to explore what a small, principled configuration language might look like when configuration, validation, normalization, and schema checking are treated as first-class language features.

## Project Status

⚠️ Experimental

This project is primarily an experiment in vibe coding a programming language implementation.

A core rule of the project is:

> Not a single line of implementation code will be handwritten.

All implementation code is intended to be generated through AI-assisted development workflows. Human effort is focused on language design, specification, review, testing, and iteration rather than directly writing source code.

The purpose of the experiment is to evaluate how far modern AI systems can be pushed when building a language implementation from a reasonably detailed specification.

## What Is ReConf?

ReConf is a small configuration language with:

- Static typing
- Refinement types
- Module imports and exports
- Records, lists, and options
- String interpolation
- Bidirectional type checking
- Deterministic normalization
- Validation through refinement predicates

A ReConf program ultimately evaluates to normalized configuration data.

Example:

```reconf
type Port = { x : Int | x > 1024 && x < 65535 };

let config = {
  port = 8080,
} : { port : Port };

config
``` 

## Design Principles

### Configuration First

The language is designed for describing configuration data, not for building general-purpose applications.

### Types Before Validation

Ordinary type checking verifies shape and structure first.

Refinement checking happens afterward on normalized values.

### Pure Evaluation

Refinement predicates must be:

- Pure
- Deterministic
- Terminating

Validation should never depend on side effects.

### Small Core Language

The user-facing syntax contains conveniences such as:

- Literal unions
- String interpolation
- Method syntax
- Optional-field omission
- Implicit option construction

These features are lowered into a smaller core language before type checking and evaluation.

## Compiler Pipeline

1. Parse source files
2. Resolve imports and exports
3. Lower surface syntax into core syntax
4. Perform bidirectional type checking
5. Normalize expressions
6. Validate refinement predicates
7. Emit normalized output

## Current State

The language specification is still evolving.

Current design work focuses on:

- Surface syntax
- Core language definition
- Type system rules
- Refinement checking
- Module semantics
- Normalization behavior

Implementation details may change as the experiment progresses.

## Inspiration

ReConf draws inspiration from:

- Typed functional languages
- Configuration languages
- Refinement type systems
- Normalization-based evaluation

while intentionally remaining small and easy to reason about.

## License

TBD

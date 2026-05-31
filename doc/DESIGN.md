# ReConf Design

ReConf is a small configuration language with a typed expression core. The surface
language is meant to feel like writing data, while the type system gives schema,
normalization, and validation a precise shape.

The compiler pipeline is:

1. Parse the surface syntax.
2. Resolve imports and exports into a module graph.
3. Lower surface syntax into a smaller core syntax.
4. Bidirectionally type-check ordinary shapes while ignoring refinements.
5. Normalize the final expression and every refinement predicate.
6. Check refinements on normalized values.
7. Emit the normalized final value.

Refinements are validation rules, not effects. All expressions used by the
checker must be pure, terminating, and deterministic.

## Source Files

A ReConf file contains zero or more imports and declarations followed by one
output expression. Top-level `type` and `let` declarations may be prefixed with
`export`, making them available to other files.

```reconf
import "./lib.reconf": default_config;

type Port = { x : Int | x > 1024 && x < 65535 };

let out = {
  addr = { ty = "localhost" },
  port = 8080,
  dir = default_config.dir,
  msg = "{hello false} world!",
} : Config;

out
```

Declarations are evaluated in order. Later declarations may refer to earlier
declarations. Recursive `let` bindings and recursive type aliases are not part
of the language. Type annotations on `let` declarations are optional; when an
annotation is present, the right-hand side is checked against it, and when it is
absent, the right-hand side must synthesize a type.

Imports bind exported names from another ReConf file into the current file:

```reconf
import "./lib.reconf": default_config;
```

Only exported names can be imported:

```reconf
export type Config = { port : Int, dir : String };
export let default_config = { port = 8080, dir = "." } : Config;
```

## Types

```txt
Int
Float
Bool
String
T?
[T]
{ field : T, ... }
{ x : T | predicate }
"literal-a" | "literal-b"
```

Base types describe scalar values. `T?` is the option type and `[T]` is the list
type. Records are closed: a value of record type must not contain fields that
are absent from the type.

Literal unions are the only union form. They are shorthand for string
refinements:

```reconf
type AddrTy = "localhost" | "fixed";
```

desugars to:

```reconf
type AddrTy = { x : String | x == "localhost" || x == "fixed" };
```

Refinement types bind a value name and a base shape:

```reconf
type Port = { x : Int | x > 1024 && x < 65535 };
```

The binder is in scope only inside the predicate. Refinements may wrap any
non-function type, including records and lists.

Function types exist for annotations on `let` bindings and lambdas, but they are
not valid output types. A final configuration must normalize to data.

```reconf
let hello : Bool -> String = (g : Bool) =>
  if g then "Hallo" else "Hello";
```

## Values And Expressions

The expression language is a simply typed lambda calculus plus configuration
data:

```txt
42
3.14
true
"hello"
none
some 1
[1, 2, 3]
{ port = 8080, dir = "." }
if cond then a else b
let x = e in body
(x : T) => body
f x
e : T
```

Lambdas are non-recursive. Application is left-associative, so `f x y` means
`(f x) y`.

Local `let` expressions are also non-recursive. Their type annotation is
optional, with the same checking rule as top-level `let` declarations.

```reconf
let msg =
  let greeting = "Hello" in
  "{greeting} world!";
```

Records use `=` in expressions and `:` in types. Field access uses dot syntax:

```reconf
config.addr.ty
```

Option fields may be omitted in record literals. If a record type requires
`addr : String?` and the literal omits `addr`, the field is inserted as `none`.
This omission sugar is available only when the record literal is checked against
a known record type.

```reconf
type AddrSchema = { ty : AddrTy, addr : String? };

let local : AddrSchema = { ty = "localhost" };
```

The `some` constructor may also be omitted when the expected type is known to be
an option. When checking an expression against `T?`, an expression that has type
`T` is elaborated to `some expression`.

```reconf
let maybe_port : Int? = 8080;  # elaborates to some 8080
```

This sugar is contextual. Without an expected option type, `8080` synthesizes
`Int`, not `Int?`.

Type ascription gives an expression an expected type locally:

```reconf
8080 : Int?
```

The expression above checks `8080` against `Int?`, so it elaborates to
`some 8080` and synthesizes `Int?`.

## Strings

Strings support interpolation with `{ expression }`.

```reconf
let msg = "{hello false} world!";
```

Interpolation is desugared to string concatenation after parsing. Each embedded
expression must have type `String`, or a built-in `show` conversion must be
available for its type. The initial language provides `show` for `Int`, `Float`,
`Bool`, and `String`.

To write literal braces inside a string, escape them:

```reconf
"\{not interpolation\}"
```

## Built-Ins

The initial standard environment contains pure functions and operators:

```txt
Arithmetic:   + - * / %
Comparison:   == != < <= > >=
Boolean:      && || !
String:       ++ contains startsWith endsWith
List:         length contains map filter all any
Option:       isSome isNone unwrapOr map
```

Method syntax is shorthand for function application:

```reconf
x.isSome
xs.contains x
```

desugar to:

```reconf
isSome x
contains xs x
```

Refinement predicates must have type `Bool`. They may call built-ins and earlier
non-recursive `let` bindings whose normalized value is a function or data.

## Compiler Design

The parsed language is the surface syntax. It keeps conveniences that are nice
to write but noisy to check directly:

```txt
imports and exports
literal union types
method calls
string interpolation
omitted optional fields
implicit some
```

The compiler resolves modules before type checking. Each file is loaded at most
once by canonical path. An import names one or more exported bindings from the
target file:

```reconf
import "./lib.reconf": default_config, Config;
```

Imported names enter the same namespace as local declarations. A local
declaration cannot duplicate an imported name, and two imports cannot bind the
same name unless they refer to the same exported definition. Cyclic imports are
rejected.

After module resolution, surface syntax lowers to a smaller core syntax:

```txt
core type CT ::=
    Int | Float | Bool | String
  | CT?
  | [CT]
  | { field : CT, ... }
  | { x : CT | CE }
  | CT -> CT

core expr CE ::=
    literal
  | variable
  | none
  | some CE
  | [CE, ...]
  | { field = CE, ... }
  | CE.field
  | if CE then CE else CE
  | let x [: CT] = CE in CE
  | (x : CT) => CE
  | CE CE
  | CE : CT
  | primitive
```

The core language has no import/export forms, no literal unions, no method
syntax, no string interpolation, and no omitted constructors or fields. Export
information is module metadata, not a core expression form. Type ascription is
preserved as a core annotation because it guides bidirectional checking.

Lowering is split into syntax-directed desugaring and type-directed elaboration:

```txt
"a" | "b"              => { x : String | x == "a" || x == "b" }
x.isSome               => isSome x
xs.contains x          => contains xs x
"hi {name}"            => "hi " ++ show name
```

Omitted option fields and implicit `some` are elaborated during bidirectional
checking, because both need an expected type. After elaboration, the checked core
term contains explicit `none`, `some`, and all record fields.

## Static Semantics

Shape checking happens before refinement checking, and it is bidirectional.
Expressions either synthesize a type or are checked against an expected type.

For `let name : T = expr;`, `expr` is checked against `T`. For
`let name = expr;`, `expr` must synthesize an ordinary type. Inference does not
invent refinements, so a declaration whose intended type is refined should use
either a `let` annotation or an expression ascription.

For `expr : T`, `T` must be well formed. The expression is checked against `T`,
and the ascribed expression synthesizes `T`.

When checking an expression against an option type `T?`, the checker accepts
four cases:

1. `none`, elaborated as `none : T?`.
2. `some expr`, where `expr` is checked against `T`.
3. An expression that synthesizes `T?`, accepted as-is.
4. Any other expression, checked against `T` and elaborated as `some expr`.

When synthesizing, `some expr` synthesizes `T?` if `expr` synthesizes `T`.
`none` does not synthesize a useful type on its own and must be checked against
an expected option type or ascribed.

For `let x : T = value in body`, `value` is checked against `T`, then `body` is
checked or synthesized in an environment extended with `x : T`. For
`let x = value in body`, `value` must synthesize a type, and that type is used
for `x` while checking `body`.

For `type Name = T;`, `T` must be well formed. Type aliases are transparent:
using `Name` is the same as using its expanded type.

For `{ x : T | p }`, the predicate `p` is checked in an environment where
`x : T`. The predicate must have type `Bool`.

For a value `v` checked against `{ x : T | p }`:

1. Check `v : T`.
2. Normalize `v`.
3. Substitute the normalized value for `x` in `p`.
4. Normalize the predicate.
5. Accept only if the predicate normalizes to `true`.

If the predicate cannot be fully normalized to `true` or `false`, refinement
checking fails with an "unknown predicate" error.

## Normal Form

Output normalization removes all computation:

```txt
numbers, booleans, strings
none and some normalized-value
lists of normalized values
records with normalized field values
```

Functions may exist during type checking and normalization, but they cannot
appear in the emitted output.

## Errors

Implementations should report errors with source spans and one primary reason:

```txt
parse error
unknown identifier
unknown type
unknown import
unexported import
duplicate import
cyclic import
type mismatch
missing field
unknown field
duplicate field
non-terminating recursion
refinement failed
unknown predicate
function escaped into output
```

## ABNF Syntax

The grammar below follows RFC 5234 ABNF. Lexical whitespace and comments are
modeled with `ws`. Parser implementations may use a conventional lexer instead
of applying this grammar directly. Keywords are case-sensitive.

```abnf
file            = ws *(top-decl ws) expr ws

top-decl        = import-decl / export-decl / type-decl / top-let-decl
import-decl     = "import" ws1 string-lit ws ":" ws import-item
                  *(ws "," ws import-item) ws ";"
import-item     = ident / type-name
export-decl     = "export" ws1 (type-decl / top-let-decl)
type-decl       = "type" ws1 type-name ws "=" ws type ws ";"
top-let-decl    = "let" ws1 ident [ws ":" ws type] ws "=" ws expr ws ";"

type            = fun-type
fun-type        = postfix-type [ws "->" ws fun-type]
postfix-type    = primary-type ["?"]
primary-type    = base-type / type-name / list-type / record-type
                / refinement-type / literal-union / paren-type
base-type       = "Int" / "Float" / "Bool" / "String"
list-type       = "[" ws type ws "]"
record-type     = "{" ws [field-type *(ws "," ws field-type) [ws ","]] ws "}"
field-type      = ident ws ":" ws type
refinement-type = "{" ws ident ws ":" ws type ws "|" ws expr ws "}"
literal-union   = string-lit *(ws "|" ws string-lit)
paren-type      = "(" ws type ws ")"

expr            = let-expr / ascription-expr
let-expr        = "let" ws1 ident [ws ":" ws type] ws "=" ws expr
                  ws1 "in" ws1 expr
ascription-expr = plain-expr [ws ":" ws type]
plain-expr      = lambda-expr / if-expr / logic-or
lambda-expr     = "(" ws ident ws ":" ws type ws ")" ws "=>" ws expr
if-expr         = "if" ws1 expr ws1 "then" ws1 expr ws1 "else" ws1 expr

logic-or        = logic-and *(ws "||" ws logic-and)
logic-and       = equality *(ws "&&" ws equality)
equality        = relation *(ws ("==" / "!=") ws relation)
relation        = additive *(ws ("<=" / "<" / ">=" / ">") ws additive)
additive        = multiplicative *(ws ("+" / "-" / "++") ws multiplicative)
multiplicative  = unary *(ws ("*" / "/" / "%") ws unary)
unary           = [("!" / "-") ws] application
application     = postfix *(ws1 postfix)
postfix         = primary *(method-call / field-access)
field-access    = "." ident
method-call     = "." ident *(ws1 postfix)

primary         = literal / option-expr / ident / list-expr / record-expr
                / paren-expr
option-expr     = "none" / ("some" ws1 expr)
list-expr       = "[" ws [expr *(ws "," ws expr) [ws ","]] ws "]"
record-expr     = "{" ws [field-expr *(ws "," ws field-expr) [ws ","]] ws "}"
field-expr      = ident ws "=" ws expr
paren-expr      = "(" ws expr ws ")"

literal         = float-lit / int-lit / bool-lit / string-lit
bool-lit        = "true" / "false"

ident           = ident-start *ident-rest
type-name       = upper-start *ident-rest
ident-start     = lower-start / "_"
ident-rest      = ident-start / upper-start / digit / "-"
lower-start     = %x61-7A
upper-start     = %x41-5A
digit           = %x30-39

int-lit         = ["-"] 1*digit
float-lit       = ["-"] 1*digit "." 1*digit

string-lit      = DQUOTE *string-char DQUOTE
string-char     = escaped-char / interpolation / normal-char
escaped-char    = "\" (%x22 / "\" / "n" / "r" / "t" / "{" / "}")
interpolation   = "{" ws expr ws "}"
normal-char     = %x20-21 / %x23-5B / %x5D-7A / %x7C / %x7E

ws              = *(wsp / comment)
ws1             = 1*(wsp / comment)
wsp             = 1*(SP / HTAB / CR / LF)
comment         = "#" *(HTAB / %x20-7E) [LF]

DQUOTE          = %x22
SP              = %x20
HTAB            = %x09
CR              = %x0D
LF              = %x0A
```

### ABNF Notes

The ABNF intentionally allows syntactic forms that are later rejected by the
type checker, such as applying a non-function.

`literal-union` is a type form only. The expression grammar does not contain a
general union expression.

`import-decl` uses `string-lit` for grammar reuse, but import paths must be
literal strings without interpolation.

`ascription-expr` has the lowest expression precedence. Use parentheses when
ascribing only a subexpression inside a larger expression.

Omitting `some` is not a syntax form in the grammar. It is a type-directed
elaboration performed while checking an expression against `T?`.

Identifiers must not be one of the reserved words used by the grammar: `import`,
`export`, `type`, `let`, `in`, `if`, `then`, `else`, `true`, `false`, `none`,
or `some`.

`method-call` is parsed as postfix sugar and desugared before type checking.
Implementations may instead parse `x.isSome` as field syntax and resolve it as a
method during elaboration.

`string-lit` includes interpolation recursively for readability. Practical
lexers usually tokenize strings with a string mode and hand embedded expression
text back to the expression parser.

# ReConf

A silly completely vibe-coded configuration language based on simply typed
lambda calculus with refinement types.

## Shape

ReConf files define types and values, then end with one output expression.
Top-level `let` annotations are optional; add one when you want to check against
a specific type, or use expression ascription with `expr : Type`.

```reconf
type Port = { x : Int | x > 1024 && x < 65535 };

let default_port = 8080;
let checked_port = default_port : Port;

checked_port
```

The first type-checking stage is bidirectional: expressions either synthesize a
type, or are checked against an expected type. Refinements are checked after
normalization.

## Modules

Use `export` to make a top-level type or value available to another file.

```reconf
# lib.reconf
export type BaseConfig = {
  dir : String,
  retries : Int,
};

export let default_config = {
  dir = ".",
  retries = 3,
} : BaseConfig;

default_config
```

Import exported names with a file path and a name list.

```reconf
import "./lib.reconf": default_config, BaseConfig;

let config = default_config : BaseConfig;

config
```

## Literal Unions

String literal unions are the only union type. They are shorthand for a string
refinement.

```reconf
type AddrTy = "localhost" | "fixed";

let local = "localhost" : AddrTy;
```

Conceptually, `AddrTy` means:

```reconf
{ x : String | x == "localhost" || x == "fixed" }
```

## Records And Optional Fields

Records are closed. Optional fields can be omitted when the record is checked
against a known record type.

```reconf
type AddrSchema = {
  ty : AddrTy,
  addr : String?,
};

let local_addr = {
  ty = "localhost",
} : AddrSchema;
```

Here `addr` is inserted as `none`.

## Skipping `some`

When the expected type is known to be `T?`, a value of type `T` is automatically
wrapped with `some`.

```reconf
let explicit : Int? = some 8080;
let implicit : Int? = 8080;

let also_implicit = 8080 : Int?;
```

All three values above have type `Int?`. Without an expected option type,
`8080` still has type `Int`.

## Refining Records

Refinements can relate fields inside a record.

```reconf
type Addr =
  { a : AddrSchema
  | (a.ty == "localhost" && a.addr.isNone)
    || (a.ty == "fixed" && a.addr.isSome)
  };

let localhost = {
  ty = "localhost",
} : Addr;

let fixed = {
  ty = "fixed",
  addr = "127.0.0.1",
} : Addr;
```

The `fixed.addr` field is checked as `String?`, so `"127.0.0.1"` elaborates to
`some "127.0.0.1"`.

## Lambdas And Interpolation

Non-recursive lambdas are allowed. String interpolation evaluates expressions
inside braces.

```reconf
let hello = (g : Bool) =>
  if g then "Hallo" else "Hello";

let msg =
  let greeting = hello false in
  "{greeting} world!";

msg
```

Local `let x = e in body` expressions are non-recursive. Their type annotation
is optional, just like top-level `let`.

## Complete Example

```reconf
import "./lib.reconf": default_config;

type Port = { x : Int | x > 1024 && x < 65535 };
type AddrTy = "localhost" | "fixed";
type AddrSchema = { ty : AddrTy, addr : String? };

type Addr =
  { a : AddrSchema
  | (a.ty == "localhost" && a.addr.isNone)
    || (a.ty == "fixed" && a.addr.isSome)
  };

type Config = {
  addr : Addr,
  port : Port,
  dir : String,
  msg : String,
};

export let hello = (g : Bool) =>
  if g then "Hallo" else "Hello";

let out = {
  addr = { ty = "localhost" },
  port = 8080,
  dir = default_config.dir,
  msg =
    let greeting = hello false in
    "{greeting} world!",
} : Config;

out
```

This checks the ordinary shape first, normalizes the terms, then validates the
refinements on `addr` and `port`.

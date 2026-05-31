use std::path::Path;

use reconf::eval::emit;
use reconf::lower::lower_file;
use reconf::resolve::modules::{Loader, eval_file};
use reconf::syntax::parser::parse;

fn eval_src(src: &str) -> reconf::Result<String> {
    let ast = lower_file(parse(src)?);
    let mut loader = Loader::default();
    let module = eval_file(&mut loader, Path::new("."), ast)?;
    emit(module.values.get("$output").unwrap())
}

#[test]
fn checks_refinement() {
    let out = eval_src(
        r#"
        type Port = { x : Int | x > 1024 && x < 65535 };
        let checked_port = 8080 : Port;
        checked_port
        "#,
    )
    .unwrap();
    assert_eq!(out, "8080");
}

#[test]
fn fills_optional_fields_and_wraps_some() {
    let out = eval_src(
        r#"
        type AddrTy = "localhost" | "fixed";
        type AddrSchema = { ty : AddrTy, addr : String? };
        let local_addr = { ty = "localhost" } : AddrSchema;
        local_addr
        "#,
    )
    .unwrap();
    assert_eq!(out, r#"{ addr = none, ty = "localhost" }"#);
}

#[test]
fn supports_lambdas_and_interpolation() {
    let out = eval_src(
        r#"
        let hello = (g : Bool) =>
          if g then "Hallo" else "Hello";
        let msg =
          let greeting = hello false in
          "{greeting} world!";
        msg
        "#,
    )
    .unwrap();
    assert_eq!(out, r#""Hello world!""#);
}

#[test]
fn rejects_failed_refinement() {
    let err = eval_src(
        r#"
        type Port = { x : Int | x > 1024 && x < 65535 };
        80 : Port
        "#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("refinement failed"));
}

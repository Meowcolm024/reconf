use reconf_compiler::compiler::{CompileInput, Compiler, SourceInput};
use reconf_compiler::emit::{EmitOptions, EmitterRegistry, OutputFormat};

fn eval_src(src: &str) -> reconf_compiler::Result<String> {
    let compiled = Compiler::new().eval(CompileInput::from(SourceInput::new("test", ".", src)))?;
    EmitterRegistry::new().emit(
        OutputFormat::Reconf,
        compiled.data_output(),
        &EmitOptions::default(),
    )
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

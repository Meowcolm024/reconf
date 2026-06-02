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

fn expect_refinement_failure(src: &str) {
    let error = eval_src(src).unwrap_err();
    assert_eq!(error.code().as_str(), "E_REFINE_004");
}

#[test]
#[ignore = "counterexample: field access does not synthesize through refined record alias"]
fn field_access_after_nested_refined_field_ascription() {
    let out = eval_src(
        r#"
        type Positive = { x : Int | x > 0 };
        type Config = { port : Positive };

        let config = { port = (8080 : Positive) } : Config;

        config.port
        "#,
    )
    .unwrap();
    assert_eq!(out, "8080");
}

#[test]
#[ignore = "counterexample: refined ascription in a plain Int argument is erased"]
fn refined_ascription_in_lambda_argument() {
    expect_refinement_failure(
        r#"
        ((x : Int) => x) (0 : { v : Int | v > 0 })
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined ascription in a named function argument is erased"]
fn refined_ascription_in_named_lambda_argument() {
    expect_refinement_failure(
        r#"
        let id = (x : Int) => x;

        id (0 : { v : Int | v > 0 })
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined ascription in a nested application argument is erased"]
fn refined_ascription_in_nested_application_argument() {
    expect_refinement_failure(
        r#"
        ((x : Int) => x) (((y : Int) => y) (0 : { v : Int | v > 0 }))
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined body ascription is erased when lambda is checked as Int -> Int"]
fn refined_ascription_in_higher_order_lambda_body() {
    expect_refinement_failure(
        r#"
        ((f : Int -> Int) => f 0) ((x : Int) => x : { v : Int | v > 0 })
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined body ascription is erased by function annotation"]
fn refined_ascription_in_annotated_lambda_body() {
    expect_refinement_failure(
        r#"
        let f : Int -> Int = (x : Int) => x : { v : Int | v > 0 };

        f 0
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined ascription is erased by plain Int let annotation"]
fn refined_ascription_in_annotated_let_value() {
    expect_refinement_failure(
        r#"
        let value : Int = 0 : { v : Int | v > 0 };

        value
        "#,
    );
}

#[test]
#[ignore = "counterexample: nested refined body ascription is erased by outer Int ascription"]
fn refined_ascription_in_lambda_body_with_outer_plain_ascription() {
    expect_refinement_failure(
        r#"
        ((x : Int) => (x : { v : Int | v > 0 }) : Int) 0
        "#,
    );
}

#[test]
#[ignore = "counterexample: field access does not synthesize through refined record alias"]
fn field_access_after_refined_alias_record_ascription() {
    let out = eval_src(
        r#"
        type Positive = { x : Int | x > 0 };
        type Config = { port : Positive };

        let config = { port = 8080 } : Config;

        config.port
        "#,
    )
    .unwrap();
    assert_eq!(out, "8080");
}

#[test]
#[ignore = "counterexample: inline field access does not synthesize through refined record alias"]
fn field_access_after_inline_refined_alias_record_ascription() {
    let out = eval_src(
        r#"
        type Positive = { x : Int | x > 0 };
        type Config = { port : Positive };

        ({ port = 8080 } : Config).port
        "#,
    )
    .unwrap();
    assert_eq!(out, "8080");
}

#[test]
#[ignore = "counterexample: returned record alias cannot be used for field access"]
fn field_access_after_refined_alias_record_returned_from_lambda() {
    let out = eval_src(
        r#"
        type Positive = { x : Int | x > 0 };
        type Config = { port : Positive };

        let make = (port : Positive) => { port = port } : Config;
        let config = make (8080 : Positive);

        config.port
        "#,
    )
    .unwrap();
    assert_eq!(out, "8080");
}

#[test]
#[ignore = "counterexample: lambda bodies cannot access fields on refined record aliases"]
fn field_access_on_refined_alias_record_lambda_parameter() {
    let out = eval_src(
        r#"
        type Positive = { x : Int | x > 0 };
        type Config = { port : Positive };

        let get_port = (config : Config) => config.port;

        get_port ({ port = 8080 } : Config)
        "#,
    )
    .unwrap();
    assert_eq!(out, "8080");
}

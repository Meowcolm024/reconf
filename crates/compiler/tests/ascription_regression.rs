mod common;

use common::{eval_src, expect_refinement_failure};

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

mod common;

use common::expect_refinement_failure;

#[test]
#[ignore = "counterexample: refined option payload ascription is erased by Int? annotation"]
fn refined_ascription_in_annotated_optional_value() {
    expect_refinement_failure(
        r#"
        let value : Int? = 0 : { v : Int | v > 0 };

        value
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined option payload ascription is erased by outer Int? ascription"]
fn refined_ascription_in_optional_value_with_outer_plain_ascription() {
    expect_refinement_failure(
        r#"
        let value = (0 : { v : Int | v > 0 }) : Int?;

        value
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined explicit some payload ascription is erased by Int? annotation"]
fn refined_ascription_in_annotated_some_payload() {
    expect_refinement_failure(
        r#"
        let value : Int? = some (0 : { v : Int | v > 0 });

        value
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined option argument is erased when checked against Int?"]
fn refined_ascription_in_plain_optional_lambda_argument() {
    expect_refinement_failure(
        r#"
        type Positive = { v : Int | v > 0 };

        ((value : Int?) => true) (0 : Positive)
        "#,
    );
}

mod common;

use common::expect_refinement_failure;

#[test]
#[ignore = "counterexample: refined list element ascription is erased by [Int] annotation"]
fn refined_ascription_in_annotated_list_element() {
    expect_refinement_failure(
        r#"
        let xs : [Int] = [0 : { v : Int | v > 0 }];

        xs
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined list element ascription is erased by outer [Int] ascription"]
fn refined_ascription_in_list_with_outer_plain_ascription() {
    expect_refinement_failure(
        r#"
        let xs = [0 : { v : Int | v > 0 }] : [Int];

        xs
        "#,
    );
}

#[test]
#[ignore = "counterexample: refined list argument is erased when checked against [Int]"]
fn refined_ascription_in_plain_list_lambda_argument() {
    expect_refinement_failure(
        r#"
        type Positive = { v : Int | v > 0 };

        ((xs : [Int]) => 1) ([0 : Positive] : [Int])
        "#,
    );
}

use reconf_core::core::{CoreExpr, CoreType, EmptyCoreTypeContext, LocalRef, TypedCoreExpr};
use reconf_core::error::ErrorCode;
use reconf_core::eval::core::PreparedCoreNormalizer;
use reconf_core::eval::{Env, Value};
use reconf_core::refine::validate::{
    CheckedRefinementPredicate, CheckedRefinementPredicateBuilder, CoreRefinementValidator,
    RefinementValidationOptions,
};

#[test]
fn core_refinement_validator_accepts_true_predicate() {
    let env = Env::empty();
    let pred = CoreExpr::Binary(
        ">".to_string(),
        Box::new(CoreExpr::Local(LocalRef::new(0))),
        Box::new(CoreExpr::Int(1024)),
    );

    let value = CoreRefinementValidator::new(env)
        .validate_checked(Value::Int(8080), CheckedRefinementPredicate::new(&pred))
        .unwrap();

    assert!(matches!(value, Value::Int(8080)));
}

#[test]
fn core_refinement_validator_rejects_false_predicate() {
    let env = Env::empty();
    let pred = CoreExpr::Binary(
        ">".to_string(),
        Box::new(CoreExpr::Local(LocalRef::new(0))),
        Box::new(CoreExpr::Int(1024)),
    );

    let error = CoreRefinementValidator::new(env)
        .validate_checked(Value::Int(80), CheckedRefinementPredicate::new(&pred))
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::RefineFailed);
}

#[test]
fn core_refinement_validator_labels_value_origin() {
    let env = Env::empty();
    let pred = CoreExpr::Binary(
        ">".to_string(),
        Box::new(CoreExpr::Local(LocalRef::new(0))),
        Box::new(CoreExpr::Int(1024)),
    );

    let error = CoreRefinementValidator::new(env)
        .validate_checked_with_options(
            Value::Int(80),
            CheckedRefinementPredicate::new(&pred),
            RefinementValidationOptions {
                value_span: Some(7..9),
                ..Default::default()
            },
            None,
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::RefineFailed);
    assert_eq!(error.diagnostic_labels()[0].span, 7..9);
    assert_eq!(
        error.diagnostic_labels()[0].message,
        "value does not satisfy refinement"
    );
}

#[test]
fn core_refinement_validator_named_binder_api_adapts_to_local_ref() {
    let env = Env::empty();
    let pred = CoreExpr::Binary(
        ">".to_string(),
        Box::new(CoreExpr::Local(LocalRef::new(0))),
        Box::new(CoreExpr::Int(1024)),
    );

    let value = CoreRefinementValidator::new(env)
        .validate(Value::Int(8080), "x", &pred)
        .unwrap();

    assert!(matches!(value, Value::Int(8080)));
}

#[test]
fn checked_list_validation_runs_element_refinements() {
    let env = Env::empty();
    let types = EmptyCoreTypeContext;
    let pred = CoreExpr::Binary(
        ">".to_string(),
        Box::new(CoreExpr::Var("x".to_string())),
        Box::new(CoreExpr::Int(10)),
    );
    let typed = TypedCoreExpr {
        expr: CoreExpr::List(vec![CoreExpr::Int(12), CoreExpr::Int(3)]),
        ty: CoreType::List(Box::new(CoreType::Refinement {
            binder: "x".to_string(),
            base: Box::new(CoreType::Int),
            pred: Box::new(pred),
        })),
    };

    let error = PreparedCoreNormalizer::new(env, &types)
        .evaluate_typed(typed)
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::RefineFailed);
}

#[test]
fn checked_refinement_predicate_binder_is_not_captured_by_nested_lambda() {
    let env = Env::empty();
    let types = EmptyCoreTypeContext;
    let pred = CoreExpr::Apply(
        Box::new(CoreExpr::Lambda(
            "x".to_string(),
            CoreType::Int,
            Box::new(CoreExpr::Bool(true)),
        )),
        Box::new(CoreExpr::Int(0)),
    );
    let typed = TypedCoreExpr {
        expr: CoreExpr::Int(3),
        ty: CoreType::Refinement {
            binder: "x".to_string(),
            base: Box::new(CoreType::Int),
            pred: Box::new(pred),
        },
    };

    let value = PreparedCoreNormalizer::new(env, &types)
        .evaluate_typed(typed)
        .unwrap();

    assert!(matches!(value, Value::Int(3)));
}

#[test]
fn checked_refinement_predicate_builder_resolves_binder_to_local_ref() {
    let pred = CoreExpr::Binary(
        ">".to_string(),
        Box::new(CoreExpr::Var("x".to_string())),
        Box::new(CoreExpr::Int(1024)),
    );
    let checked = CheckedRefinementPredicateBuilder::new("x").build(&pred);

    let value = CoreRefinementValidator::new(Env::empty())
        .validate_checked(Value::Int(8080), checked)
        .unwrap();

    assert!(matches!(value, Value::Int(8080)));
}

#[test]
fn checked_refinement_predicate_builder_respects_nested_shadowing() {
    let pred = CoreExpr::Apply(
        Box::new(CoreExpr::Lambda(
            "x".to_string(),
            CoreType::Int,
            Box::new(CoreExpr::Bool(true)),
        )),
        Box::new(CoreExpr::Int(0)),
    );
    let checked = CheckedRefinementPredicateBuilder::new("x").build(&pred);

    let value = CoreRefinementValidator::new(Env::empty())
        .validate_checked(Value::Int(3), checked)
        .unwrap();

    assert!(matches!(value, Value::Int(3)));
}

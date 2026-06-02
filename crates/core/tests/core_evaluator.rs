use std::collections::BTreeMap;

use reconf_core::core::{
    CoreExpr, CoreType, CoreTypeEnv, EmptyCoreTypeContext, GlobalRef, LocalRef,
};
use reconf_core::eval::core::{
    CoreEvaluator, PreparedCoreNormalizer, RuntimeValueApplicator, ValueApplicator,
};
use reconf_core::eval::{Env, Value};
use reconf_core::syntax::surface::Type;
use reconf_core::typeck::CoreElaborator;

#[test]
fn core_evaluator_evaluates_core_lambda_without_surface_adapter() {
    let expr = CoreExpr::Apply(
        Box::new(CoreExpr::Lambda(
            "x".to_string(),
            CoreType::Int,
            Box::new(CoreExpr::Binary(
                "+".to_string(),
                Box::new(CoreExpr::Var("x".to_string())),
                Box::new(CoreExpr::Int(1)),
            )),
        )),
        Box::new(CoreExpr::Int(41)),
    );
    let env = Env::empty();

    let value = CoreEvaluator::new().eval(expr, &env).unwrap();

    assert!(matches!(value, Value::Int(42)));
}

#[test]
fn prepared_core_normalizer_evaluates_global_refs() {
    let binding = GlobalRef::new(0);
    let env = Env::from_bindings(
        [(binding, Value::Int(41))].into_iter().collect(),
        BTreeMap::new(),
    );
    let expr = CoreExpr::Binary(
        "+".to_string(),
        Box::new(CoreExpr::Global(binding)),
        Box::new(CoreExpr::Int(1)),
    );

    let value = PreparedCoreNormalizer::new(env, &EmptyCoreTypeContext)
        .synthesize(expr)
        .unwrap();

    assert!(matches!(value, Value::Int(42)));
}

#[test]
fn core_evaluator_rejects_unelaborated_checked_let() {
    let expr = CoreExpr::Let(
        "port".to_string(),
        Some(CoreType::Option(Box::new(CoreType::Int))),
        Box::new(CoreExpr::Int(8080)),
        Box::new(CoreExpr::Var("port".to_string())),
    );
    let env = Env::empty();

    let error = CoreEvaluator::new().eval(expr, &env).unwrap_err();

    assert!(
        error.message().contains("unelaborated annotated let"),
        "{}",
        error.message()
    );
}

#[test]
fn core_evaluator_rejects_unelaborated_ascription() {
    let expr = CoreExpr::Ascribe(Box::new(CoreExpr::Int(1)), CoreType::Int);
    let env = Env::empty();

    let error = CoreEvaluator::new().eval(expr, &env).unwrap_err();

    assert!(
        error.message().contains("unelaborated ascription"),
        "{}",
        error.message()
    );
}

#[test]
fn explicit_checked_path_elaborates_before_strict_evaluation() {
    let expr = CoreExpr::Let(
        "port".to_string(),
        Some(CoreType::Option(Box::new(CoreType::Int))),
        Box::new(CoreExpr::Int(8080)),
        Box::new(CoreExpr::Var("port".to_string())),
    );
    let env = Env::empty();
    let types = EmptyCoreTypeContext;

    let value = TestCheckedNormalizer::new(env, &types)
        .synthesize(expr)
        .unwrap();

    assert!(matches!(value, Value::Some(_)));
}

#[test]
fn explicit_checked_path_checks_refinements_after_evaluation() {
    let ty = CoreType::Refinement {
        binder: "x".to_string(),
        base: Box::new(CoreType::Int),
        pred: Box::new(CoreExpr::Binary(
            ">".to_string(),
            Box::new(CoreExpr::Var("x".to_string())),
            Box::new(CoreExpr::Int(1024)),
        )),
    };
    let env = Env::empty();
    let types = EmptyCoreTypeContext;

    let value = TestCheckedNormalizer::new(env, &types)
        .check(CoreExpr::Int(8080), ty)
        .unwrap();

    assert!(matches!(value, Value::Int(8080)));
}

#[test]
fn prepared_core_normalizer_evaluates_typed_core_without_elaboration() {
    let env = Env::empty();
    let types = EmptyCoreTypeContext;
    let typed = reconf_core::core::TypedCoreExpr {
        expr: CoreExpr::Int(8080),
        ty: CoreType::Int,
    };

    let value = PreparedCoreNormalizer::new(env, &types)
        .evaluate_typed(typed)
        .unwrap();

    assert!(matches!(value, Value::Int(8080)));
}

#[test]
fn prepared_core_normalizer_evaluates_local_refs_from_elaborated_core() {
    let env = Env::empty();
    let types = EmptyCoreTypeContext;
    let typed = reconf_core::core::TypedCoreExpr {
        expr: CoreExpr::Let(
            "x".to_string(),
            None,
            Box::new(CoreExpr::Int(42)),
            Box::new(CoreExpr::Local(LocalRef::new(0))),
        ),
        ty: CoreType::Int,
    };

    let value = PreparedCoreNormalizer::new(env, &types)
        .evaluate_typed(typed)
        .unwrap();

    assert!(matches!(value, Value::Int(42)));
}

#[test]
fn prepared_core_normalizer_rejects_unelaborated_ascription() {
    let env = Env::empty();
    let types = EmptyCoreTypeContext;
    let typed = reconf_core::core::TypedCoreExpr {
        expr: CoreExpr::Ascribe(Box::new(CoreExpr::Int(8080)), CoreType::Int),
        ty: CoreType::Int,
    };

    let error = PreparedCoreNormalizer::new(env, &types)
        .evaluate_typed(typed)
        .unwrap_err();

    assert!(
        error.message().contains("unelaborated ascription"),
        "{}",
        error.message()
    );
}

#[test]
fn explicit_checked_path_expands_aliases_through_type_context() {
    let mut aliases = BTreeMap::new();
    aliases.insert(
        "Port".to_string(),
        Type::Refinement {
            binder: "x".to_string(),
            base: Box::new(Type::Int),
            pred: Box::new(reconf_core::syntax::surface::Expr::Binary(
                ">".to_string(),
                Box::new(reconf_core::syntax::surface::Expr::Var("x".to_string())),
                Box::new(reconf_core::syntax::surface::Expr::Int(1024)),
            )),
        },
    );
    let mut types = CoreTypeEnv::default();
    for (name, ty) in aliases {
        types.define(
            name,
            reconf_core::lower::SurfaceToCoreLowerer::new().lower_type(ty),
        );
    }
    let env = Env::empty();

    let value = TestCheckedNormalizer::new(env, &types)
        .check(CoreExpr::Int(8080), CoreType::Alias("Port".to_string()))
        .unwrap();

    assert!(matches!(value, Value::Int(8080)));
}

#[test]
fn runtime_value_applicator_applies_core_closure_without_surface_evaluator() {
    let env = Env::empty();
    let function = Value::CoreClosure {
        param: "x".to_string(),
        body: CoreExpr::Binary(
            "+".to_string(),
            Box::new(CoreExpr::Var("x".to_string())),
            Box::new(CoreExpr::Int(1)),
        ),
        env,
    };

    let value = RuntimeValueApplicator::without_type_context()
        .apply(function, Value::Int(41))
        .unwrap();

    assert!(matches!(value, Value::Int(42)));
}

#[test]
fn runtime_value_applicator_does_not_elaborate_closure_body() {
    let env = Env::empty();
    let function = Value::CoreClosure {
        param: "x".to_string(),
        body: CoreExpr::Let(
            "port".to_string(),
            Some(CoreType::Option(Box::new(CoreType::Int))),
            Box::new(CoreExpr::Int(8080)),
            Box::new(CoreExpr::Var("port".to_string())),
        ),
        env,
    };

    let error = RuntimeValueApplicator::without_type_context()
        .apply(function, Value::Int(0))
        .unwrap_err();

    assert!(
        error.message().contains("unelaborated annotated let"),
        "{}",
        error.message()
    );
}

struct TestCheckedNormalizer<'a> {
    env: Env,
    types: &'a dyn reconf_core::core::CoreTypeContext,
}

impl<'a> TestCheckedNormalizer<'a> {
    fn new(env: Env, types: &'a dyn reconf_core::core::CoreTypeContext) -> Self {
        Self { env, types }
    }

    fn synthesize(&self, expr: CoreExpr) -> reconf_core::Result<Value> {
        let expr = CoreElaborator::new().prepare_expr(expr)?;
        PreparedCoreNormalizer::new(self.env.clone(), self.types).synthesize(expr)
    }

    fn check(&self, expr: CoreExpr, ty: CoreType) -> reconf_core::Result<Value> {
        let typed = CoreElaborator::new().check_expr(expr, &ty)?;
        PreparedCoreNormalizer::new(self.env.clone(), self.types).evaluate_typed(typed)
    }
}

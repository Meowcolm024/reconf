use reconf_core::core::{CoreExpr, CoreType, EmptyCoreTypeContext};
use reconf_core::eval::core::PreparedCoreNormalizer;
use reconf_core::eval::{Env, Value};
use reconf_core::typeck::CoreElaborator;

#[test]
fn core_normalizer_routes_annotation_through_checked_boundary() {
    let expr = CoreExpr::Let(
        "x".to_string(),
        Some(CoreType::Option(Box::new(CoreType::Int))),
        Box::new(CoreExpr::Int(1)),
        Box::new(CoreExpr::Var("x".to_string())),
    );
    let env = Env::empty();
    let types = EmptyCoreTypeContext;

    let expr = CoreElaborator::new().prepare_expr(expr).unwrap();
    let value = PreparedCoreNormalizer::new(env, &types)
        .synthesize(expr)
        .unwrap();

    assert!(matches!(value, Value::Some(_)));
}

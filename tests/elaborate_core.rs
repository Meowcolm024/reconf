use std::collections::BTreeMap;

use reconf::core::{CoreExpr, CoreType};
use reconf::typeck::CoreElaborator;

#[test]
fn elaborator_inserts_implicit_some_for_expected_option() {
    let typed = CoreElaborator::new()
        .check_expr(CoreExpr::Int(1), &CoreType::Option(Box::new(CoreType::Int)))
        .unwrap();

    assert_eq!(typed.expr, CoreExpr::Some(Box::new(CoreExpr::Int(1))));
}

#[test]
fn elaborator_inserts_omitted_optional_record_fields() {
    let mut fields = BTreeMap::new();
    fields.insert("host".to_string(), CoreType::String);
    fields.insert(
        "port".to_string(),
        CoreType::Option(Box::new(CoreType::Int)),
    );

    let mut input = BTreeMap::new();
    input.insert(
        "host".to_string(),
        CoreExpr::String("localhost".to_string()),
    );

    let typed = CoreElaborator::new()
        .check_expr(CoreExpr::Record(input), &CoreType::Record(fields))
        .unwrap();

    let CoreExpr::Record(output) = typed.expr else {
        panic!("expected elaborated record");
    };

    assert_eq!(output.get("port"), Some(&CoreExpr::None));
}

#[test]
fn elaborator_removes_nested_checked_lets() {
    let expr = CoreExpr::Let(
        "x".to_string(),
        Some(CoreType::Option(Box::new(CoreType::Int))),
        Box::new(CoreExpr::Int(1)),
        Box::new(CoreExpr::Var("x".to_string())),
    );

    let typed = CoreElaborator::new()
        .check_expr(expr, &CoreType::Int)
        .unwrap();

    let CoreExpr::Let(_, annotation, value, _) = typed.expr else {
        panic!("expected let");
    };
    assert!(annotation.is_none());
    assert_eq!(*value, CoreExpr::Some(Box::new(CoreExpr::Int(1))));
}

#[test]
fn elaborator_erases_checked_ascription_context() {
    let expr = CoreExpr::Ascribe(
        Box::new(CoreExpr::Int(1)),
        CoreType::Option(Box::new(CoreType::Int)),
    );

    let typed = CoreElaborator::new()
        .check_expr(expr, &CoreType::Option(Box::new(CoreType::Int)))
        .unwrap();

    assert_eq!(typed.expr, CoreExpr::Some(Box::new(CoreExpr::Int(1))));
}

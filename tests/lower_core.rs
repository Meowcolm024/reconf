use reconf::core::{CoreExpr, CoreModule};
use reconf::lower::SurfaceToCoreLowerer;
use reconf::syntax::parser::parse;

#[test]
fn lowering_removes_surface_only_interpolation_syntax() {
    let surface = parse(
        r#"
        let port = 8080;
        "port={port}"
        "#,
    )
    .unwrap();

    let core = SurfaceToCoreLowerer::new().lower_file(surface);
    let output = core.output.as_ref().expect("expected output");

    assert_no_surface_only_exprs(&core);
    assert!(contains_var(output, "show"));
    assert!(contains_binary_op(output, "++"));
}

#[test]
fn lowering_removes_surface_only_method_call_syntax() {
    let surface = parse(
        r#"
        let xs = [1, 2, 3];
        xs.contains 2
        "#,
    )
    .unwrap();

    let core = SurfaceToCoreLowerer::new().lower_file(surface);
    let output = core.output.as_ref().expect("expected output");

    assert_no_surface_only_exprs(&core);
    assert!(contains_field(output, "contains"));
    assert!(contains_apply(output));
}

fn assert_no_surface_only_exprs(module: &CoreModule) {
    for decl in &module.decls {
        match decl {
            reconf::core::CoreDecl::Native { .. } | reconf::core::CoreDecl::Type { .. } => {}
            reconf::core::CoreDecl::Let { expr, .. } => assert_expr(expr),
        }
    }
    if let Some(output) = &module.output {
        assert_expr(output);
    }
}

fn assert_expr(expr: &CoreExpr) {
    match expr {
        CoreExpr::Spanned(expr, _) => assert_expr(expr),
        CoreExpr::Int(_)
        | CoreExpr::Float(_)
        | CoreExpr::Bool(_)
        | CoreExpr::String(_)
        | CoreExpr::None
        | CoreExpr::Var(_)
        | CoreExpr::Global(_)
        | CoreExpr::Local(_) => {}
        CoreExpr::Some(expr) | CoreExpr::Field(expr, _) | CoreExpr::Unary(_, expr) => {
            assert_expr(expr)
        }
        CoreExpr::List(items) => {
            for item in items {
                assert_expr(item);
            }
        }
        CoreExpr::Record(fields) => {
            for value in fields.values() {
                assert_expr(value);
            }
        }
        CoreExpr::If(cond, then_expr, else_expr) => {
            assert_expr(cond);
            assert_expr(then_expr);
            assert_expr(else_expr);
        }
        CoreExpr::Let(_, _, value, body) => {
            assert_expr(value);
            assert_expr(body);
        }
        CoreExpr::Lambda(_, _, body) => assert_expr(body),
        CoreExpr::Apply(function, arg) | CoreExpr::Binary(_, function, arg) => {
            assert_expr(function);
            assert_expr(arg);
        }
        CoreExpr::Ascribe(expr, _) => assert_expr(expr),
    }
}

fn contains_var(expr: &CoreExpr, needle: &str) -> bool {
    match expr {
        CoreExpr::Spanned(expr, _) => contains_var(expr, needle),
        CoreExpr::Var(name) => name == needle,
        CoreExpr::Global(_) => false,
        CoreExpr::Local(_) => false,
        CoreExpr::Some(expr)
        | CoreExpr::Field(expr, _)
        | CoreExpr::Unary(_, expr)
        | CoreExpr::Ascribe(expr, _) => contains_var(expr, needle),
        CoreExpr::List(items) => items.iter().any(|item| contains_var(item, needle)),
        CoreExpr::Record(fields) => fields.values().any(|value| contains_var(value, needle)),
        CoreExpr::If(cond, then_expr, else_expr) => {
            contains_var(cond, needle)
                || contains_var(then_expr, needle)
                || contains_var(else_expr, needle)
        }
        CoreExpr::Let(_, _, value, body) => {
            contains_var(value, needle) || contains_var(body, needle)
        }
        CoreExpr::Lambda(_, _, body) => contains_var(body, needle),
        CoreExpr::Apply(function, arg) | CoreExpr::Binary(_, function, arg) => {
            contains_var(function, needle) || contains_var(arg, needle)
        }
        CoreExpr::Int(_)
        | CoreExpr::Float(_)
        | CoreExpr::Bool(_)
        | CoreExpr::String(_)
        | CoreExpr::None => false,
    }
}

fn contains_binary_op(expr: &CoreExpr, needle: &str) -> bool {
    match expr {
        CoreExpr::Spanned(expr, _) => contains_binary_op(expr, needle),
        CoreExpr::Binary(op, left, right) => {
            op == needle || contains_binary_op(left, needle) || contains_binary_op(right, needle)
        }
        CoreExpr::Some(expr)
        | CoreExpr::Field(expr, _)
        | CoreExpr::Unary(_, expr)
        | CoreExpr::Ascribe(expr, _) => contains_binary_op(expr, needle),
        CoreExpr::List(items) => items.iter().any(|item| contains_binary_op(item, needle)),
        CoreExpr::Record(fields) => fields
            .values()
            .any(|value| contains_binary_op(value, needle)),
        CoreExpr::If(cond, then_expr, else_expr) => {
            contains_binary_op(cond, needle)
                || contains_binary_op(then_expr, needle)
                || contains_binary_op(else_expr, needle)
        }
        CoreExpr::Let(_, _, value, body) => {
            contains_binary_op(value, needle) || contains_binary_op(body, needle)
        }
        CoreExpr::Lambda(_, _, body) => contains_binary_op(body, needle),
        CoreExpr::Apply(function, arg) => {
            contains_binary_op(function, needle) || contains_binary_op(arg, needle)
        }
        CoreExpr::Int(_)
        | CoreExpr::Float(_)
        | CoreExpr::Bool(_)
        | CoreExpr::String(_)
        | CoreExpr::None
        | CoreExpr::Var(_)
        | CoreExpr::Global(_)
        | CoreExpr::Local(_) => false,
    }
}

fn contains_field(expr: &CoreExpr, needle: &str) -> bool {
    match expr {
        CoreExpr::Spanned(expr, _) => contains_field(expr, needle),
        CoreExpr::Field(expr, name) => name == needle || contains_field(expr, needle),
        CoreExpr::Some(expr) | CoreExpr::Unary(_, expr) | CoreExpr::Ascribe(expr, _) => {
            contains_field(expr, needle)
        }
        CoreExpr::List(items) => items.iter().any(|item| contains_field(item, needle)),
        CoreExpr::Record(fields) => fields.values().any(|value| contains_field(value, needle)),
        CoreExpr::If(cond, then_expr, else_expr) => {
            contains_field(cond, needle)
                || contains_field(then_expr, needle)
                || contains_field(else_expr, needle)
        }
        CoreExpr::Let(_, _, value, body) => {
            contains_field(value, needle) || contains_field(body, needle)
        }
        CoreExpr::Lambda(_, _, body) => contains_field(body, needle),
        CoreExpr::Apply(function, arg) | CoreExpr::Binary(_, function, arg) => {
            contains_field(function, needle) || contains_field(arg, needle)
        }
        CoreExpr::Int(_)
        | CoreExpr::Float(_)
        | CoreExpr::Bool(_)
        | CoreExpr::String(_)
        | CoreExpr::None
        | CoreExpr::Var(_)
        | CoreExpr::Global(_)
        | CoreExpr::Local(_) => false,
    }
}

fn contains_apply(expr: &CoreExpr) -> bool {
    match expr {
        CoreExpr::Spanned(expr, _) => contains_apply(expr),
        CoreExpr::Apply(_, _) => true,
        CoreExpr::Some(expr)
        | CoreExpr::Field(expr, _)
        | CoreExpr::Unary(_, expr)
        | CoreExpr::Ascribe(expr, _) => contains_apply(expr),
        CoreExpr::List(items) => items.iter().any(contains_apply),
        CoreExpr::Record(fields) => fields.values().any(contains_apply),
        CoreExpr::If(cond, then_expr, else_expr) => {
            contains_apply(cond) || contains_apply(then_expr) || contains_apply(else_expr)
        }
        CoreExpr::Let(_, _, value, body) => contains_apply(value) || contains_apply(body),
        CoreExpr::Lambda(_, _, body) => contains_apply(body),
        CoreExpr::Binary(_, left, right) => contains_apply(left) || contains_apply(right),
        CoreExpr::Int(_)
        | CoreExpr::Float(_)
        | CoreExpr::Bool(_)
        | CoreExpr::String(_)
        | CoreExpr::None
        | CoreExpr::Var(_)
        | CoreExpr::Global(_)
        | CoreExpr::Local(_) => false,
    }
}

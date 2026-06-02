use std::collections::BTreeMap;

use reconf_core::core::{
    CoreDecl, CoreExpr, CoreModule, CoreType, CoreTypeEnv, ElaboratedDecl, ElaboratedExpr,
    GlobalRef,
};
use reconf_core::resolve::resolved::{ResolvedModuleBody, ResolvedValueBindings};
use reconf_core::typeck::CoreModuleElaborator;
use reconf_core::typeck::CoreValueTypeContext;

#[test]
fn module_elaborator_checks_annotated_declarations_before_evaluation() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: vec![CoreDecl::Let {
            export: false,
            name: "port".to_string(),
            annotation: Some(CoreType::Option(Box::new(CoreType::Int))),
            expr: CoreExpr::Int(8080),
        }],
        output: Some(CoreExpr::Var("port".to_string())),
    };
    let types = CoreTypeEnv::default();
    let body = ResolvedModuleBody::from_core(module);
    let (_, decls, output) = body.into_core_parts();

    let elaborated = CoreModuleElaborator::new(&types)
        .elaborate_resolved_module(decls, output)
        .unwrap();

    match &elaborated.decls[0] {
        ElaboratedDecl::Let {
            expr: ElaboratedExpr::Checked(typed),
            ..
        } => {
            assert!(matches!(typed.ty, CoreType::Option(_)));
            assert!(matches!(typed.expr, CoreExpr::Some(_)));
        }
        _ => panic!("expected checked let declaration"),
    }
}

#[test]
fn module_elaborator_synthesizes_unannotated_literal_output() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Int(1)),
    };
    let types = CoreTypeEnv::default();
    let body = ResolvedModuleBody::from_core(module);
    let (_, decls, output) = body.into_core_parts();

    let elaborated = CoreModuleElaborator::new(&types)
        .elaborate_resolved_module(decls, output)
        .unwrap();

    assert!(matches!(
        elaborated.output,
        Some(ElaboratedExpr::Checked(reconf_core::core::TypedCoreExpr {
            expr: CoreExpr::Int(1),
            ty: CoreType::Int
        }))
    ));
}

#[test]
fn module_elaborator_rejects_bare_none_without_expected_type() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::None),
    };
    let types = CoreTypeEnv::default();

    let error = CoreModuleElaborator::new(&types)
        .elaborate_module(module)
        .unwrap_err();

    assert_eq!(
        error.code(),
        reconf_core::error::ErrorCode::TypeNoneNeedsExpected
    );
}

#[test]
fn module_elaborator_resolves_earlier_type_aliases_in_later_declarations() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: vec![
            CoreDecl::Type {
                export: false,
                name: "Port".to_string(),
                ty: CoreType::Int,
            },
            CoreDecl::Type {
                export: false,
                name: "Config".to_string(),
                ty: CoreType::Record(
                    [("port".to_string(), CoreType::Alias("Port".to_string()))]
                        .into_iter()
                        .collect(),
                ),
            },
        ],
        output: Some(CoreExpr::Int(1)),
    };
    let types = CoreTypeEnv::default();
    let body = ResolvedModuleBody::from_core(module);
    let (_, decls, output) = body.into_core_parts();

    let elaborated = CoreModuleElaborator::new(&types)
        .elaborate_resolved_module(decls, output)
        .unwrap();

    assert_eq!(elaborated.decls.len(), 2);
}

#[test]
fn module_elaborator_synthesizes_local_value_references() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: vec![CoreDecl::Let {
            export: false,
            name: "answer".to_string(),
            annotation: None,
            expr: CoreExpr::Int(42),
        }],
        output: Some(CoreExpr::Var("answer".to_string())),
    };
    let types = CoreTypeEnv::default();
    let body = ResolvedModuleBody::from_core(module);
    let (_, decls, output) = body.into_core_parts();

    let elaborated = CoreModuleElaborator::new(&types)
        .elaborate_resolved_module(decls, output)
        .unwrap();

    assert!(matches!(
        elaborated.output,
        Some(ElaboratedExpr::Checked(reconf_core::core::TypedCoreExpr {
            expr: CoreExpr::Global(_),
            ty: CoreType::Int
        }))
    ));
}

#[test]
fn module_elaborator_synthesizes_imported_value_references() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Var("imported".to_string())),
    };
    let types = CoreTypeEnv::default();
    let values = TestValueTypes::new([("imported".to_string(), CoreType::String)]);
    let body = ResolvedModuleBody::from_core(module).resolve_external_values(&values);
    let (_, decls, output) = body.into_core_parts();

    let elaborated = CoreModuleElaborator::with_context(
        &types,
        &values,
        reconf_core::typeck::CoreElaborator::new(),
    )
    .elaborate_resolved_module(decls, output)
    .unwrap();

    assert!(matches!(
        elaborated.output,
        Some(ElaboratedExpr::Checked(reconf_core::core::TypedCoreExpr {
            expr: CoreExpr::Global(_),
            ty: CoreType::String
        }))
    ));
}

#[test]
fn module_elaborator_resolves_expression_locals_to_local_refs() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Let(
            "x".to_string(),
            None,
            Box::new(CoreExpr::Int(41)),
            Box::new(CoreExpr::Lambda(
                "y".to_string(),
                CoreType::Int,
                Box::new(CoreExpr::Binary(
                    "+".to_string(),
                    Box::new(CoreExpr::Var("x".to_string())),
                    Box::new(CoreExpr::Var("y".to_string())),
                )),
            )),
        )),
    };
    let types = CoreTypeEnv::default();

    let elaborated = CoreModuleElaborator::new(&types)
        .elaborate_module(module)
        .unwrap();

    let Some(ElaboratedExpr::Checked(reconf_core::core::TypedCoreExpr {
        expr: CoreExpr::Let(_, _, _, body),
        ..
    })) = elaborated.output
    else {
        panic!("expected checked let");
    };
    let CoreExpr::Lambda(_, _, body) = *body else {
        panic!("expected lambda body");
    };
    let CoreExpr::Binary(_, left, right) = *body else {
        panic!("expected binary body");
    };

    assert!(matches!(*left, CoreExpr::Local(local) if local.index() == 1));
    assert!(matches!(*right, CoreExpr::Local(local) if local.index() == 0));
}

struct TestValueTypes {
    values: BTreeMap<String, (GlobalRef, CoreType)>,
}

impl TestValueTypes {
    fn new(values: impl IntoIterator<Item = (String, CoreType)>) -> Self {
        Self {
            values: values
                .into_iter()
                .enumerate()
                .map(|(index, (name, ty))| (name, (GlobalRef::new(index), ty)))
                .collect(),
        }
    }
}

impl CoreValueTypeContext for TestValueTypes {
    fn value_type(&self, name: &str) -> Option<&CoreType> {
        self.values.get(name).map(|(_, ty)| ty)
    }

    fn global_value(&self, name: &str) -> Option<(GlobalRef, &CoreType)> {
        self.values.get(name).map(|(binding, ty)| (*binding, ty))
    }

    fn global_type(&self, binding: GlobalRef) -> Option<&CoreType> {
        self.values
            .values()
            .find(|(candidate, _)| *candidate == binding)
            .map(|(_, ty)| ty)
    }
}

impl ResolvedValueBindings for TestValueTypes {
    fn value_binding(&self, name: &str) -> Option<GlobalRef> {
        self.values.get(name).map(|(binding, _)| *binding)
    }
}

#[test]
fn module_elaborator_synthesizes_structural_expressions() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Record(
            [
                ("items".to_string(), CoreExpr::List(vec![CoreExpr::Int(1)])),
                (
                    "flag".to_string(),
                    CoreExpr::Binary(
                        "&&".to_string(),
                        Box::new(CoreExpr::Bool(true)),
                        Box::new(CoreExpr::Bool(false)),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        )),
    };
    let types = CoreTypeEnv::default();

    let elaborated = CoreModuleElaborator::new(&types)
        .elaborate_module(module)
        .unwrap();

    let Some(ElaboratedExpr::Checked(typed)) = elaborated.output else {
        panic!("expected checked output");
    };
    let CoreType::Record(fields) = typed.ty else {
        panic!("expected record type");
    };
    assert!(matches!(fields.get("items"), Some(CoreType::List(_))));
    assert!(matches!(fields.get("flag"), Some(CoreType::Bool)));
}

#[test]
fn module_elaborator_synthesizes_let_lambda_apply_and_if() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Let(
            "inc".to_string(),
            None,
            Box::new(CoreExpr::Lambda(
                "x".to_string(),
                CoreType::Int,
                Box::new(CoreExpr::Binary(
                    "+".to_string(),
                    Box::new(CoreExpr::Var("x".to_string())),
                    Box::new(CoreExpr::Int(1)),
                )),
            )),
            Box::new(CoreExpr::If(
                Box::new(CoreExpr::Bool(true)),
                Box::new(CoreExpr::Apply(
                    Box::new(CoreExpr::Var("inc".to_string())),
                    Box::new(CoreExpr::Int(41)),
                )),
                Box::new(CoreExpr::Int(0)),
            )),
        )),
    };
    let types = CoreTypeEnv::default();

    let elaborated = CoreModuleElaborator::new(&types)
        .elaborate_module(module)
        .unwrap();

    assert!(matches!(
        elaborated.output,
        Some(ElaboratedExpr::Checked(reconf_core::core::TypedCoreExpr {
            ty: CoreType::Int,
            ..
        }))
    ));
}

#[test]
fn module_elaborator_synthesizes_record_field_projection() {
    let module = CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Field(
            Box::new(CoreExpr::Record(
                [("port".to_string(), CoreExpr::Int(8080))]
                    .into_iter()
                    .collect(),
            )),
            "port".to_string(),
        )),
    };
    let types = CoreTypeEnv::default();

    let elaborated = CoreModuleElaborator::new(&types)
        .elaborate_module(module)
        .unwrap();

    assert!(matches!(
        elaborated.output,
        Some(ElaboratedExpr::Checked(reconf_core::core::TypedCoreExpr {
            ty: CoreType::Int,
            ..
        }))
    ));
}

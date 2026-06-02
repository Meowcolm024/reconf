use std::path::PathBuf;

use reconf_core::core::{
    CoreDecl, CoreExpr, CoreImport, CoreModule, CoreType, GlobalRef, TypeAliasRef,
};
use reconf_core::error::ErrorCode;
use reconf_core::eval::Value;
use reconf_core::resolve::names::{NameScope, Namespace};
use reconf_core::resolve::resolved::{
    ResolvedDecl, ResolvedExport, ResolvedExports, ResolvedImportSelector, ResolvedImportTarget,
    ResolvedModule, ResolvedModuleBody, ResolvedModuleBuilder, ResolvedProgram,
    ResolvedTypeBindings, ResolvedTypeExport, ResolvedValueBindings, ResolvedValueExport,
};

fn type_export(id: usize, ty: CoreType) -> ResolvedTypeExport {
    ResolvedTypeExport::new(TypeAliasRef::new(id), ty)
}

#[test]
fn resolved_exports_expose_values_and_types_by_name() {
    let mut exports = ResolvedExports::builder();
    exports.define_value(
        "answer".to_string(),
        ResolvedValueExport::new(Value::Int(42), CoreType::Int),
    );
    exports.define_type("Port".to_string(), type_export(0, CoreType::Int));
    let exports = exports.finish();

    match exports.get("answer") {
        Some(ResolvedExport::Value(value)) => {
            assert!(matches!(value.value(), Value::Int(42)));
            assert!(matches!(value.ty(), Some(CoreType::Int)));
        }
        _ => panic!("expected value export"),
    }

    match exports.get("Port") {
        Some(ResolvedExport::Type(ty)) => assert!(matches!(ty.ty(), CoreType::Int)),
        _ => panic!("expected type export"),
    }
}

#[test]
fn resolved_import_selector_validates_requested_names() {
    let mut exports = ResolvedExports::builder();
    exports.define_value(
        "answer".to_string(),
        ResolvedValueExport::new(Value::Int(42), CoreType::Int),
    );
    exports.define_type("Port".to_string(), type_export(0, CoreType::Int));
    let exports = exports.finish();

    let selection = ResolvedImportSelector::new(&exports)
        .select(["answer", "Port"])
        .unwrap();
    let mut names = SelectedNames::default();
    selection.apply_to(&mut names);

    assert_eq!(names.names, ["type:Port", "value:answer"]);
}

#[test]
fn resolved_import_selects_requested_exports_from_module_exports() {
    let mut exports = ResolvedExports::builder();
    exports.define_value(
        "answer".to_string(),
        ResolvedValueExport::new(Value::Int(42), CoreType::Int),
    );
    exports.define_type("Port".to_string(), type_export(0, CoreType::Int));
    let exports = exports.finish();
    let import = reconf_core::resolve::resolved::ResolvedImport::new(CoreImport {
        path: "lib.reconf_core".to_string(),
        names: vec!["answer".to_string(), "Port".to_string()],
    });

    let selection = import.select_from(&exports).unwrap();
    let mut names = SelectedNames::default();
    selection.apply_to(&mut names);

    assert_eq!(names.names, ["type:Port", "value:answer"]);
}

#[derive(Default)]
struct SelectedNames {
    names: Vec<String>,
}

impl ResolvedImportTarget for SelectedNames {
    fn import_value(&mut self, name: &str, _: &ResolvedValueExport) {
        self.names.push(format!("value:{name}"));
    }

    fn import_type(&mut self, name: &str, _: &ResolvedTypeExport) {
        self.names.push(format!("type:{name}"));
    }
}

#[test]
fn resolved_import_selector_rejects_duplicate_requested_names() {
    let mut exports = ResolvedExports::builder();
    exports.define_type("answer".to_string(), type_export(0, CoreType::Int));
    let exports = exports.finish();

    let error = match ResolvedImportSelector::new(&exports).select(["answer", "answer"]) {
        Ok(_) => panic!("expected duplicate import error"),
        Err(error) => error,
    };

    assert_eq!(error.code(), ErrorCode::NameDuplicateImport);
}

#[test]
fn resolved_import_selector_rejects_unexported_requested_names() {
    let exports = ResolvedExports::default();

    let error = match ResolvedImportSelector::new(&exports).select(["missing"]) {
        Ok(_) => panic!("expected unexported import error"),
        Err(error) => error,
    };

    assert_eq!(error.code(), ErrorCode::ModuleUnexportedImport);
}

#[test]
fn name_scope_tracks_value_and_type_namespaces() {
    let mut scope = NameScope::new();
    scope.define(Namespace::Value, "answer");
    scope.define(Namespace::Type, "Port");

    assert!(scope.contains(Namespace::Value, "answer"));
    assert!(scope.contains(Namespace::Type, "Port"));
    assert!(!scope.contains(Namespace::Type, "answer"));
    assert!(scope.contains_any("answer"));
    assert!(scope.contains_any("Port"));
}

#[test]
fn name_scope_rejects_imports_that_collide_with_existing_names() {
    let mut scope = NameScope::new();
    scope.define(Namespace::Value, "answer");

    let collision = scope
        .first_collision(["missing", "answer"])
        .expect("expected name collision");

    assert_eq!(collision.name(), "answer");
    assert_eq!(collision.namespace(), Namespace::Value);
}

#[test]
fn resolved_module_exposes_resolved_exports() {
    let mut exports = ResolvedExports::builder();
    exports.define_value(
        "answer".to_string(),
        ResolvedValueExport::new(Value::Int(42), CoreType::Int),
    );
    let exports = exports.finish();

    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Int(42)),
    });
    let module = ResolvedModule::new(PathBuf::from("main.reconf_core"), body, exports);

    assert_eq!(module.path(), PathBuf::from("main.reconf_core"));
    assert!(matches!(module.body().output(), Some(CoreExpr::Int(42))));
    match module.exports().get("answer") {
        Some(ResolvedExport::Value(value)) => assert!(matches!(value.value(), Value::Int(42))),
        _ => panic!("expected value export"),
    }
}

#[test]
fn resolved_module_builder_finalizes_with_exports() {
    let mut exports = ResolvedExports::builder();
    exports.define_type("Port".to_string(), type_export(0, CoreType::Int));
    let exports = exports.finish();
    let path = PathBuf::from("lib.reconf_core");
    let body = ResolvedModuleBody::default();
    let builder = ResolvedModuleBuilder::new(path.clone(), body);
    let module = builder.finish(exports);

    assert_eq!(module.path(), path);
    assert!(matches!(
        module.exports().get("Port"),
        Some(ResolvedExport::Type(ty)) if matches!(ty.ty(), CoreType::Int)
    ));
}

#[test]
fn resolved_program_indexes_resolved_modules_by_path() {
    let mut exports = ResolvedExports::builder();
    exports.define_value(
        "answer".to_string(),
        ResolvedValueExport::new(Value::Int(42), CoreType::Int),
    );
    let exports = exports.finish();
    let path = PathBuf::from("main.reconf_core");
    let module = ResolvedModule::new(path.clone(), ResolvedModuleBody::default(), exports);

    let mut program = ResolvedProgram::new();
    program.insert_module(module);

    match program
        .module(&path)
        .and_then(|module| module.exports().get("answer"))
    {
        Some(ResolvedExport::Value(value)) => assert!(matches!(value.value(), Value::Int(42))),
        _ => panic!("expected resolved module export"),
    }
}

#[test]
fn resolved_module_body_preserves_lowered_module_shape() {
    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: vec![CoreImport {
            path: "lib.reconf_core".to_string(),
            names: vec!["answer".to_string()],
        }],
        decls: vec![CoreDecl::Type {
            export: false,
            name: "Port".to_string(),
            ty: CoreType::Int,
        }],
        output: Some(CoreExpr::Bool(true)),
    });

    assert_eq!(body.imports()[0].path(), "lib.reconf_core");
    assert_eq!(body.imports()[0].names(), ["answer"]);
    assert!(matches!(&body.decls()[0], ResolvedDecl::Type { name, .. } if name == "Port"));
    assert!(matches!(body.output(), Some(CoreExpr::Bool(true))));
}

#[test]
fn resolved_module_body_rewrites_same_module_type_aliases() {
    let body = ResolvedModuleBody::from_core(CoreModule {
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
        output: None,
    });

    let port = body.decls()[0].type_alias().expect("Port alias");
    let ResolvedDecl::Type { ty, .. } = &body.decls()[1] else {
        panic!("expected Config type alias");
    };
    let CoreType::Record(fields) = ty else {
        panic!("expected record type");
    };

    assert!(matches!(
        fields.get("port"),
        Some(CoreType::ResolvedAlias(alias)) if *alias == port
    ));
}

#[test]
fn resolved_module_body_assigns_value_binding_ids() {
    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: Vec::new(),
        decls: vec![
            CoreDecl::Let {
                export: false,
                name: "answer".to_string(),
                annotation: None,
                expr: CoreExpr::Int(42),
            },
            CoreDecl::Native {
                export: false,
                name: "show".to_string(),
                ty: CoreType::Function(Box::new(CoreType::Int), Box::new(CoreType::String)),
            },
            CoreDecl::Type {
                export: false,
                name: "Port".to_string(),
                ty: CoreType::Int,
            },
        ],
        output: None,
    });

    let first = body.decls()[0].binding().expect("let binding");
    let second = body.decls()[1].binding().expect("native binding");

    assert_ne!(first, second);
    assert!(body.decls()[2].binding().is_none());
}

#[test]
fn resolved_module_body_rewrites_same_module_value_references() {
    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: Vec::new(),
        decls: vec![
            CoreDecl::Let {
                export: false,
                name: "answer".to_string(),
                annotation: None,
                expr: CoreExpr::Int(42),
            },
            CoreDecl::Let {
                export: false,
                name: "next".to_string(),
                annotation: None,
                expr: CoreExpr::Binary(
                    "+".to_string(),
                    Box::new(CoreExpr::Var("answer".to_string())),
                    Box::new(CoreExpr::Int(1)),
                ),
            },
        ],
        output: Some(CoreExpr::Var("next".to_string())),
    });

    let answer = body.decls()[0].binding().expect("answer binding");
    let next = body.decls()[1].binding().expect("next binding");

    let ResolvedDecl::Let { expr, .. } = &body.decls()[1] else {
        panic!("expected second let");
    };
    let CoreExpr::Binary(_, left, _) = expr else {
        panic!("expected binary");
    };

    assert!(matches!(**left, CoreExpr::Global(binding) if binding == answer));
    assert!(matches!(body.output(), Some(CoreExpr::Global(binding)) if *binding == next));
}

#[test]
fn resolved_module_body_does_not_capture_shadowed_locals() {
    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: Vec::new(),
        decls: vec![CoreDecl::Let {
            export: false,
            name: "answer".to_string(),
            annotation: None,
            expr: CoreExpr::Int(42),
        }],
        output: Some(CoreExpr::Let(
            "answer".to_string(),
            None,
            Box::new(CoreExpr::Int(1)),
            Box::new(CoreExpr::Var("answer".to_string())),
        )),
    });

    let Some(CoreExpr::Let(_, _, _, body)) = body.output() else {
        panic!("expected let output");
    };

    assert!(matches!(**body, CoreExpr::Var(ref name) if name == "answer"));
}

#[test]
fn resolved_module_body_rewrites_external_value_references() {
    let imported = GlobalRef::new(99);
    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Var("imported".to_string())),
    })
    .resolve_external_values(&TestResolvedBindings::new([("imported", imported)]));

    assert!(matches!(body.output(), Some(CoreExpr::Global(binding)) if *binding == imported));
}

#[test]
fn resolved_module_body_rewrites_external_type_alias_references() {
    let port = TypeAliasRef::new(99);
    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: Vec::new(),
        decls: vec![CoreDecl::Let {
            export: false,
            name: "port".to_string(),
            annotation: Some(CoreType::Alias("Port".to_string())),
            expr: CoreExpr::Int(8080),
        }],
        output: None,
    })
    .resolve_external_types(&TestResolvedTypes::new([("Port", port)]));

    let ResolvedDecl::Let {
        annotation: Some(annotation),
        ..
    } = &body.decls()[0]
    else {
        panic!("expected annotated let");
    };

    assert!(matches!(annotation, CoreType::ResolvedAlias(alias) if *alias == port));
}

#[test]
fn resolved_module_body_keeps_unknown_external_type_alias_references_unresolved() {
    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: Vec::new(),
        decls: vec![CoreDecl::Let {
            export: false,
            name: "port".to_string(),
            annotation: Some(CoreType::Alias("Port".to_string())),
            expr: CoreExpr::Int(8080),
        }],
        output: None,
    })
    .resolve_external_types(&TestResolvedTypes::new([]));

    let ResolvedDecl::Let {
        annotation: Some(annotation),
        ..
    } = &body.decls()[0]
    else {
        panic!("expected annotated let");
    };

    assert!(matches!(annotation, CoreType::Alias(name) if name == "Port"));
}

#[test]
fn resolved_module_body_rebases_value_bindings_and_references() {
    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: Vec::new(),
        decls: vec![
            CoreDecl::Let {
                export: false,
                name: "answer".to_string(),
                annotation: None,
                expr: CoreExpr::Int(42),
            },
            CoreDecl::Let {
                export: false,
                name: "next".to_string(),
                annotation: None,
                expr: CoreExpr::Var("answer".to_string()),
            },
        ],
        output: Some(CoreExpr::Var("next".to_string())),
    })
    .rebase_value_bindings_from(10);

    let answer = body.decls()[0].binding().expect("answer binding");
    let next = body.decls()[1].binding().expect("next binding");

    assert_eq!(answer.id(), 10);
    assert_eq!(next.id(), 11);

    let ResolvedDecl::Let { expr, .. } = &body.decls()[1] else {
        panic!("expected second let");
    };

    assert!(matches!(expr, CoreExpr::Global(binding) if *binding == answer));
    assert!(matches!(body.output(), Some(CoreExpr::Global(binding)) if *binding == next));
}

#[test]
fn resolved_module_body_external_rewrite_respects_shadowed_locals() {
    let imported = GlobalRef::new(99);
    let body = ResolvedModuleBody::from_core(CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Lambda(
            "imported".to_string(),
            CoreType::Int,
            Box::new(CoreExpr::Var("imported".to_string())),
        )),
    })
    .resolve_external_values(&TestResolvedBindings::new([("imported", imported)]));

    let Some(CoreExpr::Lambda(_, _, body)) = body.output() else {
        panic!("expected lambda output");
    };

    assert!(matches!(**body, CoreExpr::Var(ref name) if name == "imported"));
}

struct TestResolvedBindings {
    bindings: Vec<(&'static str, GlobalRef)>,
}

impl TestResolvedBindings {
    fn new(bindings: impl IntoIterator<Item = (&'static str, GlobalRef)>) -> Self {
        Self {
            bindings: bindings.into_iter().collect(),
        }
    }
}

impl ResolvedValueBindings for TestResolvedBindings {
    fn value_binding(&self, name: &str) -> Option<GlobalRef> {
        self.bindings
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, binding)| *binding)
    }
}

struct TestResolvedTypes {
    bindings: Vec<(&'static str, TypeAliasRef)>,
}

impl TestResolvedTypes {
    fn new(bindings: impl IntoIterator<Item = (&'static str, TypeAliasRef)>) -> Self {
        Self {
            bindings: bindings.into_iter().collect(),
        }
    }
}

impl ResolvedTypeBindings for TestResolvedTypes {
    fn type_binding(&self, name: &str) -> Option<TypeAliasRef> {
        self.bindings
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, binding)| *binding)
    }
}

use std::collections::BTreeSet;

use reconf::compiler::prelude;
use reconf::compiler::prelude::PreludeCompiler;
use reconf::core::CoreTypeEquivalence;
use reconf::eval::Value;
use reconf::eval::builtins::{NativeFunction, NativeRegistry};
use reconf::lower::SurfaceToCoreLowerer;
use reconf::syntax::parser::parse;
use reconf::syntax::surface::Decl;

#[test]
fn native_registry_is_single_source_for_known_native_metadata() {
    let show = NativeRegistry::get("show").unwrap();
    let map = NativeRegistry::get("map").unwrap();

    assert_eq!(show.name(), "show");
    assert_eq!(show.arity(), 1);
    assert_eq!(map.name(), "map");
    assert_eq!(map.arity(), 2);
    assert!(NativeRegistry::declared("filter"));
    assert!(NativeRegistry::get("missing").is_err());
}

#[test]
fn native_function_uses_registry_arity_and_implementation() {
    let partial = NativeFunction::new("contains")
        .apply(Value::String("hello".to_string()))
        .unwrap();
    let Value::Native(partial) = partial else {
        panic!("expected partial native application");
    };

    let value = partial.apply(Value::String("ell".to_string())).unwrap();

    assert!(matches!(value, Value::Bool(true)));
}

#[test]
fn prelude_exported_natives_match_registry_entries() {
    let file = parse(prelude::source()).unwrap();
    let prelude_names = file
        .decls
        .into_iter()
        .filter_map(|decl| match decl {
            Decl::Native {
                export: true, name, ..
            } => Some(name),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let registry_names = NativeRegistry::all()
        .iter()
        .map(|spec| spec.name().to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(prelude_names, registry_names);
}

#[test]
fn prelude_native_types_match_registry_metadata() {
    let file = parse(prelude::source()).unwrap();
    let mut lowerer = SurfaceToCoreLowerer::new();

    for decl in file.decls {
        let Decl::Native {
            export: true,
            name,
            ty,
        } = decl
        else {
            continue;
        };
        let declared_ty = lowerer.lower_type(ty);
        let registry_ty = NativeRegistry::get(&name).unwrap().ty().to_core();

        assert!(
            CoreTypeEquivalence::equivalent(&declared_ty, &registry_ty),
            "native `{name}`"
        );
    }
}

#[test]
fn prelude_compiler_builds_prelude_module() {
    let module = PreludeCompiler::new().compile_module().unwrap();

    assert!(matches!(module.value("show"), Some(Value::Native(_))));
}

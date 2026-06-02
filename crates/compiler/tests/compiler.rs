use reconf_compiler::compiler::output::OutputValidator;
use reconf_compiler::compiler::session::CompilerSession;
use reconf_compiler::compiler::{CompileInput, Compiler, CompilerOptions, SourceInput};
use reconf_compiler::emit::{DataValue, EmitOptions, EmitterRegistry, OutputFormat, OutputStyle};
use reconf_core::core::{CoreExpr, CoreModule};
use reconf_core::eval::Value;
use reconf_core::eval::builtins::NativeFunction;
use reconf_core::resolve::resolved::{ResolvedExport, ResolvedModuleBody};
use reconf_core::source::{LoadedSource, MemorySourceProvider, SourceProvider};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

#[test]
fn compiler_can_load_imports_from_memory_source_provider() {
    let sources = MemorySourceProvider::new()
        .with_file(
            "lib.reconf",
            r#"
            export type Port = { x : Int | x > 1024 };
            export let default_port : Port = 8080;
            default_port
            "#,
        )
        .with_file(
            "main.reconf",
            r#"
            import "./lib.reconf": Port, default_port;
            { port = default_port : Port }
            "#,
        );

    let value = Compiler::with_sources(sources, CompilerOptions)
        .eval(CompileInput::from(Path::new("main.reconf")))
        .unwrap()
        .into_data_output();
    let output = EmitterRegistry::new()
        .emit(
            OutputFormat::Json,
            &value,
            &EmitOptions {
                style: OutputStyle::Compact,
            },
        )
        .unwrap();

    assert_eq!(output, r#"{"port":8080}"#);
}

#[test]
fn module_loader_records_loaded_modules_in_resolved_program() {
    let sources = MemorySourceProvider::new().with_file(
        "lib.reconf",
        r#"
        export type Port = { x : Int | x > 1024 };
        export let default_port : Port = 8080;
        default_port
        "#,
    );
    let mut loader = reconf_compiler::compiler::loader::ModuleLoader::with_sources(
        sources,
        reconf_compiler::compiler::loader::ContextualModuleCompiler::with_prelude(),
    );

    let resolved = loader.load_resolved(Path::new("lib.reconf")).unwrap();
    let indexed = loader
        .resolved_program()
        .module(Path::new("lib.reconf"))
        .unwrap();

    assert_eq!(resolved.path(), Path::new("lib.reconf"));
    assert_eq!(indexed.path(), resolved.path());
    assert!(matches!(
        indexed.exports().get("Port"),
        Some(ResolvedExport::Type(_))
    ));
    assert!(matches!(
        indexed.exports().get("default_port"),
        Some(ResolvedExport::Value(_))
    ));
}

#[test]
fn module_loader_reuses_cached_resolved_modules_without_reloading_sources() {
    let load_count = Rc::new(RefCell::new(0));
    let sources = CountingSourceProvider {
        inner: MemorySourceProvider::new().with_file(
            "lib.reconf",
            r#"
            export let answer = 42;
            answer
            "#,
        ),
        load_count: load_count.clone(),
    };
    let mut loader = reconf_compiler::compiler::loader::ModuleLoader::with_sources(
        sources,
        reconf_compiler::compiler::loader::ContextualModuleCompiler::with_prelude(),
    );

    let first = loader.load_resolved(Path::new("lib.reconf")).unwrap();
    let second = loader.load_resolved(Path::new("lib.reconf")).unwrap();

    assert_eq!(*load_count.borrow(), 1);
    assert_eq!(first.path(), second.path());
    assert!(matches!(
        second.exports().get("answer"),
        Some(ResolvedExport::Value(_))
    ));
}

#[test]
fn module_loader_frontend_errors_keep_import_source_labels() {
    let sources = MemorySourceProvider::new().with_file("broken.reconf", r#""prefix {} suffix""#);
    let mut loader = reconf_compiler::compiler::loader::ModuleLoader::with_sources(
        sources,
        reconf_compiler::compiler::loader::ContextualModuleCompiler::with_prelude(),
    );

    let error = match loader.load_resolved(Path::new("broken.reconf")) {
        Ok(_) => panic!("expected parse error"),
        Err(error) => error,
    };

    assert_eq!(
        error.code(),
        reconf_core::error::ErrorCode::ParseEmptyInterpolation
    );
    assert_eq!(error.source_name(), Some("broken.reconf"));
    assert_eq!(error.diagnostic_labels().len(), 1);
}

struct CountingSourceProvider {
    inner: MemorySourceProvider,
    load_count: Rc<RefCell<usize>>,
}

impl SourceProvider for CountingSourceProvider {
    fn load_source(&mut self, path: &Path) -> reconf_compiler::Result<LoadedSource> {
        *self.load_count.borrow_mut() += 1;
        self.inner.load_source(path)
    }
}

#[test]
fn compiler_session_keeps_successful_declarations_without_source_accumulation() {
    let mut session = CompilerSession::with_sources("session", ".", MemorySourceProvider::new());

    session
        .check_declarations("type Port = { x : Int | x > 1024 };")
        .unwrap();
    session.check_declarations("let p : Port = 8080;").unwrap();
    let output = session.eval_expression("p").unwrap();

    assert_eq!(output.data_output(), &DataValue::Int(8080));
    assert_eq!(output.checked().core().decls.len(), 2);
}

#[test]
fn compiler_session_does_not_commit_failed_declarations() {
    let mut session = CompilerSession::with_sources("session", ".", MemorySourceProvider::new());

    let error = match session.check_declarations("let broken : Missing = 1;") {
        Ok(_) => panic!("expected declaration check to fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), reconf_core::error::ErrorCode::TypeUnknown);

    let output = session.eval_expression("1").unwrap();
    assert_eq!(output.checked().core().decls.len(), 0);
}

#[test]
fn compiler_entry_uses_prelude_context() {
    let value = Compiler::with_sources(MemorySourceProvider::new(), CompilerOptions)
        .eval(CompileInput::from(SourceInput::new(
            "test",
            ".",
            r#"show 1"#,
        )))
        .unwrap()
        .into_data_output();

    assert_eq!(value, DataValue::String("1".to_string()));
}

#[test]
fn empty_module_compiler_does_not_inject_prelude_context() {
    let mut loader = reconf_compiler::compiler::loader::ModuleLoader::with_sources(
        MemorySourceProvider::new(),
        reconf_compiler::compiler::loader::EvaluatingModuleCompiler,
    );
    let module = CoreModule {
        imports: Vec::new(),
        decls: Vec::new(),
        output: Some(CoreExpr::Var("show".to_string())),
    };

    let error = match loader.compile_entry(Path::new("."), ResolvedModuleBody::from_core(module)) {
        Ok(_) => panic!("expected missing prelude identifier"),
        Err(error) => error,
    };

    assert_eq!(error.message(), "unknown identifier `show`");
}

#[test]
fn output_validation_happens_when_requesting_data_output() {
    let checked = Compiler::new()
        .check(CompileInput::from(SourceInput::new(
            "test",
            ".",
            r#"
            let id = (x : Int) => x;
            id
            "#,
        )))
        .unwrap();

    assert!(checked.output().is_ok());

    let error = checked.data_output().unwrap_err();
    assert_eq!(error.code(), reconf_core::error::ErrorCode::OutputFunction);
}

#[test]
fn checked_module_exposes_output_without_public_environment_maps() {
    let checked = Compiler::with_sources(MemorySourceProvider::new(), CompilerOptions)
        .check(CompileInput::from(SourceInput::new("test", ".", "41 + 1")))
        .unwrap();

    assert!(matches!(checked.output().unwrap(), Value::Int(42)));
    assert!(matches!(
        checked.module().value("$output"),
        Some(Value::Int(42))
    ));
}

#[test]
fn module_into_output_consumes_checked_module() {
    let checked = Compiler::with_sources(MemorySourceProvider::new(), CompilerOptions)
        .check(CompileInput::from(SourceInput::new("test", ".", r#""ok""#)))
        .unwrap();

    assert!(matches!(
        checked.into_output().unwrap(),
        Value::String(value) if value == "ok"
    ));
}

#[test]
fn output_validator_is_the_runtime_to_data_boundary() {
    let value = Value::List(vec![
        Value::Int(1),
        Value::Some(Box::new(Value::String("ok".to_string()))),
    ]);

    let data = OutputValidator::new().validate(&value).unwrap();

    assert_eq!(
        data,
        DataValue::List(vec![
            DataValue::Int(1),
            DataValue::Some(Box::new(DataValue::String("ok".to_string()))),
        ])
    );
}

#[test]
fn output_validator_rejects_non_data_runtime_values() {
    let error = OutputValidator::new()
        .validate(&Value::Native(NativeFunction::new("show")))
        .unwrap_err();

    assert_eq!(error.code(), reconf_core::error::ErrorCode::OutputFunction);
}

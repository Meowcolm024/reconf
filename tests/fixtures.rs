use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

use reconf::compiler::{CompileInput, Compiler};
use reconf::emit::{DataValue, EmitOptions, EmitterRegistry, OutputFormat, OutputStyle};
use reconf::syntax::parser::parse;
use reconf::syntax::surface::Decl;
use reconf::{Error, Result};

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

fn collect_expected_files(dir: &Path, extension: &str, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("fixture directory should be readable") {
        let entry = entry.expect("fixture entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_expected_files(&path, extension, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            files.push(path);
        }
    }
}

fn collect_reconf_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("fixture directory should be readable") {
        let entry = entry.expect("fixture entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_reconf_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("reconf") {
            files.push(path);
        }
    }
}

fn source_for_expected(expected: &Path) -> PathBuf {
    expected.with_extension("reconf")
}

fn eval_source_file(path: &Path) -> Result<DataValue> {
    Ok(Compiler::new()
        .eval(CompileInput::from(path))?
        .into_data_output())
}

fn render_error(error: &Error) -> String {
    static INIT_REPORTER: Once = Once::new();
    INIT_REPORTER.call_once(reconf::repl::reporter::init_reporter);
    format!("{:?}", miette::Report::new(error.clone()))
}

fn normalize_diagnostic(output: &str) -> String {
    strip_ansi(output).replace(env!("CARGO_MANIFEST_DIR"), "$RECONF")
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }

        if chars.next_if_eq(&'[').is_none() {
            out.push(ch);
            continue;
        }

        for ch in chars.by_ref() {
            if ch.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

fn expected_code(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        .trim()
        .to_string()
}

fn assert_json_eq(actual: &str, expected: &str, source_path: &Path) {
    let actual: serde_json::Value = serde_json::from_str(actual)
        .unwrap_or_else(|err| panic!("{} emitted invalid JSON: {err}", source_path.display()));
    let expected: serde_json::Value = serde_json::from_str(expected)
        .unwrap_or_else(|err| panic!("{} expected invalid JSON: {err}", source_path.display()));
    assert_eq!(actual, expected, "{}", source_path.display());
}

trait FixtureTarget {
    const EXTENSION: &'static str;

    fn run(expected_path: &Path, source_path: &Path);
}

struct JsonTarget;

impl FixtureTarget for JsonTarget {
    const EXTENSION: &'static str = "json";

    fn run(expected_path: &Path, source_path: &Path) {
        let value = eval_source_file(source_path).unwrap_or_else(|error| {
            panic!(
                "{} failed:\n{}",
                source_path.display(),
                render_error(&error)
            );
        });
        let actual = EmitterRegistry::new()
            .emit(
                OutputFormat::Json,
                &value,
                &EmitOptions {
                    style: OutputStyle::Pretty,
                },
            )
            .expect("fixture output should be JSON data");
        let expected = fs::read_to_string(expected_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", expected_path.display()));
        assert_json_eq(&actual, &expected, source_path);
    }
}

struct ErrorCodeTarget;

impl FixtureTarget for ErrorCodeTarget {
    const EXTENSION: &'static str = "err";

    fn run(expected_path: &Path, source_path: &Path) {
        let error = eval_source_file(source_path)
            .expect_err(&format!("{} unexpectedly passed", source_path.display()));
        assert_eq!(
            error.code().as_str(),
            expected_code(expected_path),
            "{}",
            source_path.display()
        );
        let rendered = render_error(&error);
        assert!(!rendered.trim().is_empty(), "{}", source_path.display());
    }
}

struct DiagnosticSnapshotTarget;

impl FixtureTarget for DiagnosticSnapshotTarget {
    const EXTENSION: &'static str = "stderr";

    fn run(expected_path: &Path, source_path: &Path) {
        let error = eval_source_file(source_path)
            .expect_err(&format!("{} unexpectedly passed", source_path.display()));
        assert_eq!(
            error.code().as_str(),
            expected_code(&expected_path.with_extension("err")),
            "{}",
            source_path.display()
        );
        let rendered = render_error(&error);
        assert!(
            rendered.contains(error.code().as_str()),
            "{}",
            source_path.display()
        );
        let expected = fs::read_to_string(expected_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", expected_path.display()));
        assert_eq!(
            normalize_diagnostic(&rendered),
            expected,
            "{}",
            expected_path.display()
        );
    }
}

fn direct_expected_sources(root: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    for extension in extensions {
        let mut expected_files = Vec::new();
        collect_expected_files(root, extension, &mut expected_files);
        sources.extend(
            expected_files
                .into_iter()
                .map(|path| source_for_expected(&path)),
        );
    }
    sources.sort();
    sources.dedup();
    sources
}

macro_rules! fixture_case {
    ($target:ty, $root:expr, $name:ident, $case:literal) => {
        #[test]
        fn $name() {
            let root = $root;
            let expected_path = root.join(format!(
                "{}.{}",
                $case,
                <$target as crate::FixtureTarget>::EXTENSION
            ));
            let source_path = root.join(format!("{}.reconf", $case));
            <$target as crate::FixtureTarget>::run(&expected_path, &source_path);
        }
    };
}

fn imported_sources(path: &Path) -> Vec<PathBuf> {
    let Ok(src) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(ast) = parse(&src) else {
        return Vec::new();
    };
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    ast.decls
        .into_iter()
        .filter_map(|decl| match decl {
            Decl::Import { path, .. } => Some(base_dir.join(path)),
            _ => None,
        })
        .collect()
}

fn normalize_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

#[test]
fn every_fixture_and_example_source_is_covered() {
    let fixture_sources = direct_expected_sources(&fixture_root(), &["json", "err", "stderr"]);
    let example_sources = direct_expected_sources(&examples_root(), &["json"]);

    assert!(
        !fixture_sources.is_empty(),
        "expected fixture sources to be discovered"
    );
    assert!(
        !example_sources.is_empty(),
        "expected example sources to be discovered"
    );

    for source in fixture_sources.iter().chain(example_sources.iter()) {
        assert!(
            source.exists(),
            "expected source for fixture target: {}",
            source.display()
        );
    }

    let mut covered = fixture_sources
        .iter()
        .chain(example_sources.iter())
        .cloned()
        .map(normalize_path)
        .collect::<std::collections::BTreeSet<_>>();

    for source in fixture_sources.iter().chain(example_sources.iter()) {
        covered.extend(imported_sources(source).into_iter().map(normalize_path));
    }

    let mut all_sources = Vec::new();
    collect_reconf_files(&fixture_root(), &mut all_sources);
    collect_reconf_files(&examples_root(), &mut all_sources);
    all_sources.sort();

    for source in all_sources {
        assert!(
            covered.contains(&normalize_path(source.clone())),
            "{} has no target fixture and is not imported by a targeted source",
            source.display()
        );
    }
}

mod eval_ok {
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        arithmetic_bool,
        "eval_ok/arithmetic_bool"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        bool_list_builtins,
        "eval_ok/bool_list_builtins"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        float_and_show,
        "eval_ok/float_and_show"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        implicit_some,
        "eval_ok/implicit_some"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        interpolation,
        "eval_ok/interpolation"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        list_and_methods,
        "eval_ok/list_and_methods"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        literal_union,
        "eval_ok/literal_union"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        module_lib,
        "eval_ok/module_lib"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        module_main,
        "eval_ok/module_main"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        option_predicates,
        "eval_ok/option_predicates"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        record_refinement,
        "eval_ok/record_refinement"
    );
    fixture_case!(
        super::JsonTarget,
        super::fixture_root(),
        simple_config,
        "eval_ok/simple_config"
    );
}

mod examples {
    fixture_case!(
        super::JsonTarget,
        super::examples_root(),
        modules_main,
        "modules/main"
    );
    fixture_case!(super::JsonTarget, super::examples_root(), simple, "simple");
}

mod module_err {
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        cycle_a,
        "module_err/cycle_a"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        duplicate_imports,
        "module_err/duplicate_imports"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        missing_import_path,
        "module_err/missing_import_path"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        unexported_main,
        "module_err/unexported_main"
    );
}

mod output_err {
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        function_field,
        "output_err/function_field"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        function_output,
        "output_err/function_output"
    );
}

mod parse_err {
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        empty_interpolation,
        "parse_err/empty_interpolation"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        unterminated_string,
        "parse_err/unterminated_string"
    );
}

mod refine_err {
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        bad_literal_union,
        "refine_err/bad_literal_union"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        bad_port,
        "refine_err/bad_port"
    );
}

mod runtime_err {
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        division_by_zero,
        "runtime_err/division_by_zero"
    );
}

mod type_err {
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        apply_non_function,
        "type_err/apply_non_function"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        bad_interpolation,
        "type_err/bad_interpolation"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        duplicate_field,
        "type_err/duplicate_field"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        missing_field,
        "type_err/missing_field"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        none_without_expected_type,
        "type_err/none_without_expected_type"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        recursive_alias,
        "type_err/recursive_alias"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        type_mismatch,
        "type_err/type_mismatch"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        unknown_field,
        "type_err/unknown_field"
    );
    fixture_case!(
        super::ErrorCodeTarget,
        super::fixture_root(),
        unsupported_builtin_arg,
        "type_err/unsupported_builtin_arg"
    );
}

mod diagnostic {
    fixture_case!(
        super::DiagnosticSnapshotTarget,
        super::fixture_root(),
        parse_err_empty_interpolation,
        "parse_err/empty_interpolation"
    );
    fixture_case!(
        super::DiagnosticSnapshotTarget,
        super::fixture_root(),
        refine_err_bad_port,
        "refine_err/bad_port"
    );
    fixture_case!(
        super::DiagnosticSnapshotTarget,
        super::fixture_root(),
        runtime_err_division_by_zero,
        "runtime_err/division_by_zero"
    );
    fixture_case!(
        super::DiagnosticSnapshotTarget,
        super::fixture_root(),
        type_err_recursive_alias,
        "type_err/recursive_alias"
    );
}

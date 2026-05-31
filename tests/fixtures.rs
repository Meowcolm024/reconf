use std::fs;
use std::path::{Path, PathBuf};

use reconf::emit::json::emit_json;
use reconf::eval::Value;
use reconf::lower::lower_file;
use reconf::repl::diagnostics::attach_best_effort_span;
use reconf::resolve::modules::{Loader, eval_file};
use reconf::syntax::parser::parse;
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

fn source_for_expected(expected: &Path) -> PathBuf {
    expected.with_extension("reconf")
}

fn eval_source_file(path: &Path) -> Result<Value> {
    let src = fs::read_to_string(path)
        .map_err(|error| Error::new(format!("unknown import `{}`: {error}", path.display())))?;
    let name = path.display().to_string();
    let ast = lower_file(parse(&src).map_err(|error| attach_best_effort_span(error, &name, &src))?);
    let mut loader = Loader::default();
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let module = eval_file(&mut loader, base_dir, ast)
        .map_err(|error| attach_best_effort_span(error, &name, &src))?;
    module
        .values
        .get("$output")
        .cloned()
        .ok_or_else(|| Error::new("internal error: missing output"))
}

fn render_error(error: Error) -> String {
    format!("{:?}", miette::Report::new(error))
}

fn assert_json_eq(actual: &str, expected: &str, source_path: &Path) {
    let actual: serde_json::Value = serde_json::from_str(actual)
        .unwrap_or_else(|err| panic!("{} emitted invalid JSON: {err}", source_path.display()));
    let expected: serde_json::Value = serde_json::from_str(expected)
        .unwrap_or_else(|err| panic!("{} expected invalid JSON: {err}", source_path.display()));
    assert_eq!(actual, expected, "{}", source_path.display());
}

#[test]
fn eval_fixtures_match_json() {
    let mut expected_files = Vec::new();
    collect_expected_files(&fixture_root(), "json", &mut expected_files);
    expected_files.sort();

    assert!(
        !expected_files.is_empty(),
        "expected at least one eval fixture"
    );

    for expected_path in expected_files {
        let source_path = source_for_expected(&expected_path);
        let value = eval_source_file(&source_path).unwrap_or_else(|error| {
            panic!("{} failed:\n{}", source_path.display(), render_error(error));
        });
        let actual = emit_json(&value, true).expect("fixture output should be JSON data");
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", expected_path.display()));
        assert_json_eq(&actual, &expected, &source_path);
    }
}

#[test]
fn examples_match_json() {
    let mut expected_files = Vec::new();
    collect_expected_files(&examples_root(), "json", &mut expected_files);
    expected_files.sort();

    assert!(
        !expected_files.is_empty(),
        "expected at least one runnable example"
    );

    for expected_path in expected_files {
        let source_path = source_for_expected(&expected_path);
        let value = eval_source_file(&source_path).unwrap_or_else(|error| {
            panic!("{} failed:\n{}", source_path.display(), render_error(error));
        });
        let actual = emit_json(&value, true).expect("example output should be JSON data");
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", expected_path.display()));
        assert_json_eq(&actual, &expected, &source_path);
    }
}

#[test]
fn error_fixtures_still_fail() {
    let mut expected_files = Vec::new();
    collect_expected_files(&fixture_root(), "err", &mut expected_files);
    expected_files.sort();

    assert!(
        !expected_files.is_empty(),
        "expected at least one error fixture"
    );

    for expected_path in expected_files {
        let source_path = source_for_expected(&expected_path);
        let error = eval_source_file(&source_path)
            .expect_err(&format!("{} unexpectedly passed", source_path.display()));
        let rendered = render_error(error);
        assert!(!rendered.trim().is_empty(), "{}", source_path.display());
    }
}

#[test]
fn diagnostic_snapshots_still_fail_with_rendered_errors() {
    let mut expected_files = Vec::new();
    collect_expected_files(&fixture_root(), "stderr", &mut expected_files);
    expected_files.sort();

    assert!(
        !expected_files.is_empty(),
        "expected at least one diagnostic snapshot"
    );

    for expected_path in expected_files {
        let source_path = source_for_expected(&expected_path);
        let error = eval_source_file(&source_path)
            .expect_err(&format!("{} unexpectedly passed", source_path.display()));
        let rendered = render_error(error);
        assert!(
            rendered.contains("reconf::error"),
            "{}",
            source_path.display()
        );
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use reconf::{Compiler, emit_json};

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
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

fn normalize_diagnostic(text: &str) -> String {
    let root = fixture_root();
    text.replace(&root.display().to_string(), "$FIXTURES")
        .trim_end()
        .to_string()
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
        let mut compiler = Compiler::new();
        let value = compiler.eval_file(&source_path).unwrap_or_else(|err| {
            panic!(
                "{} failed:\n{}",
                source_path.display(),
                compiler.render(err)
            );
        });
        let actual = emit_json(&value, true).expect("fixture output should be JSON data");
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", expected_path.display()));
        assert_eq!(
            actual.trim_end(),
            expected.trim_end(),
            "{}",
            source_path.display()
        );
    }
}

#[test]
fn error_fixtures_match_codes() {
    let mut expected_files = Vec::new();
    collect_expected_files(&fixture_root(), "err", &mut expected_files);
    expected_files.sort();

    assert!(
        !expected_files.is_empty(),
        "expected at least one error fixture"
    );

    for expected_path in expected_files {
        let source_path = source_for_expected(&expected_path);
        let mut compiler = Compiler::new();
        let err = compiler
            .check_file(&source_path)
            .expect_err(&format!("{} unexpectedly passed", source_path.display()));
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", expected_path.display()));
        assert_eq!(err.code(), expected.trim(), "{}", source_path.display());
    }
}

#[test]
fn diagnostic_snapshots_match() {
    let mut expected_files = Vec::new();
    collect_expected_files(&fixture_root(), "stderr", &mut expected_files);
    expected_files.sort();

    assert!(
        !expected_files.is_empty(),
        "expected at least one diagnostic snapshot"
    );

    for expected_path in expected_files {
        let source_path = source_for_expected(&expected_path);
        let mut compiler = Compiler::new();
        let err = compiler
            .check_file(&source_path)
            .expect_err(&format!("{} unexpectedly passed", source_path.display()));
        let actual = normalize_diagnostic(&compiler.render(err));
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", expected_path.display()));
        assert_eq!(actual, expected.trim_end(), "{}", source_path.display());
    }
}

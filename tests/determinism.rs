use std::fs;
use std::path::{Path, PathBuf};

use reconf::diagnostic::attach_best_effort_span;
use reconf::emit::json::emit_json;
use reconf::eval::Value;
use reconf::lower::lower_file;
use reconf::resolve::modules::{Loader, eval_file};
use reconf::syntax::parser::parse;
use reconf::{Error, Result};

fn collect_json_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("directory should be readable") {
        let entry = entry.expect("directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
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

fn eval_json(source_path: &Path) -> String {
    let value = eval_source_file(source_path).unwrap_or_else(|error| {
        panic!(
            "{} failed:\n{:?}",
            source_path.display(),
            miette::Report::new(error)
        );
    });
    emit_json(&value, true).expect("output should be JSON data")
}

#[test]
fn normalization_is_deterministic_for_all_positive_corpus() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut expected_files = Vec::new();
    collect_json_files(
        &manifest.join("tests").join("fixtures"),
        &mut expected_files,
    );
    collect_json_files(&manifest.join("examples"), &mut expected_files);
    expected_files.sort();

    assert!(
        !expected_files.is_empty(),
        "expected at least one positive JSON corpus file"
    );

    for expected_path in expected_files {
        let source_path = expected_path.with_extension("reconf");
        let first = eval_json(&source_path);
        let second = eval_json(&source_path);
        assert_eq!(first, second, "{}", source_path.display());
    }
}

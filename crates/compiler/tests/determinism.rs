use std::fs;
use std::path::{Path, PathBuf};

use reconf_compiler::Result;
use reconf_compiler::compiler::{CompileInput, Compiler};
use reconf_compiler::emit::{DataValue, EmitOptions, EmitterRegistry, OutputFormat, OutputStyle};

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

fn eval_source_file(path: &Path) -> Result<DataValue> {
    Ok(Compiler::new()
        .eval(CompileInput::from(path))?
        .into_data_output())
}

fn eval_json(source_path: &Path) -> String {
    let value = eval_source_file(source_path).unwrap_or_else(|error| {
        panic!(
            "{} failed:\n{:?}",
            source_path.display(),
            miette::Report::new(error)
        );
    });
    EmitterRegistry::new()
        .emit(
            OutputFormat::Json,
            &value,
            &EmitOptions {
                style: OutputStyle::Pretty,
            },
        )
        .expect("output should be JSON data")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn normalization_is_deterministic_for_all_positive_corpus() {
    let manifest = workspace_root();
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

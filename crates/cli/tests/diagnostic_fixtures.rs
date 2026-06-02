use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

use reconf_compiler::compiler::{CompileInput, Compiler};
use reconf_compiler::emit::DataValue;
use reconf_compiler::{Error, Result};

fn fixture_root() -> PathBuf {
    workspace_root().join("tests").join("fixtures")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should be canonicalizable")
}

fn eval_source_file(path: &Path) -> Result<DataValue> {
    Ok(Compiler::new()
        .eval(CompileInput::from(path))?
        .into_data_output())
}

fn render_error(error: &Error) -> String {
    static INIT_REPORTER: Once = Once::new();
    INIT_REPORTER.call_once(reconf_cli::repl::reporter::init_reporter);
    format!("{:?}", miette::Report::new(error.clone()))
}

fn normalize_diagnostic(output: &str) -> String {
    strip_ansi(output).replace(&workspace_root().display().to_string(), "$RECONF")
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

trait FixtureTarget {
    const EXTENSION: &'static str;

    fn run(expected_path: &Path, source_path: &Path);
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

fixture_case!(
    DiagnosticSnapshotTarget,
    fixture_root(),
    parse_err_empty_interpolation,
    "parse_err/empty_interpolation"
);
fixture_case!(
    DiagnosticSnapshotTarget,
    fixture_root(),
    refine_err_bad_port,
    "refine_err/bad_port"
);
fixture_case!(
    DiagnosticSnapshotTarget,
    fixture_root(),
    runtime_err_division_by_zero,
    "runtime_err/division_by_zero"
);
fixture_case!(
    DiagnosticSnapshotTarget,
    fixture_root(),
    type_err_recursive_alias,
    "type_err/recursive_alias"
);

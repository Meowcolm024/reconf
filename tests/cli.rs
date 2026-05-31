use std::process::Command;

#[test]
fn explain_prints_error_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_reconf"))
        .args(["--explain", "E_REFINE_004"])
        .output()
        .expect("reconf binary should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("E_REFINE_004"));
    assert!(stdout.contains("refinement predicate"));
}

#[test]
fn check_accepts_no_color_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_reconf"))
        .args([
            "check",
            "tests/fixtures/eval_ok/simple_config.reconf",
            "--no-color",
        ])
        .output()
        .expect("reconf binary should run");

    assert!(output.status.success());
}

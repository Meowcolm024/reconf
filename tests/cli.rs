use std::fs;
use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_reconf").to_string()
}

#[test]
fn eval_emits_json() {
    let dir = tempfile_dir();
    let file = dir.join("config.reconf");
    fs::write(
        &file,
        r#"
        type Port = { x : Int | x > 1024 && x < 65535 };
        type Config = { port : Port, host : String? };
        let config = { port = 8080 } : Config;
        config
        "#,
    )
    .unwrap();

    let output = Command::new(bin())
        .args(["eval", file.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        r#"{"host":null,"port":8080}"#
    );
}

#[test]
fn check_rejects_bad_refinement_with_miette_report() {
    let dir = tempfile_dir();
    let file = dir.join("bad.reconf");
    fs::write(
        &file,
        r#"
        type Port = { x : Int | x > 1024 && x < 65535 };
        80 : Port
        "#,
    )
    .unwrap();

    let output = Command::new(bin())
        .args(["check", file.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("refinement failed"));
}

fn tempfile_dir() -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "reconf-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

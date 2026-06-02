use reconf_compiler::emit::{DataValue, EmitOptions, EmitterRegistry, OutputFormat};

#[test]
fn emitter_registry_selects_json_emitter() {
    let output = EmitterRegistry::new()
        .emit(
            OutputFormat::Json,
            &DataValue::String("ok".to_string()),
            &EmitOptions::default(),
        )
        .unwrap();

    assert_eq!(output, r#""ok""#);
}

#[test]
fn emitter_registry_selects_reconf_emitter() {
    let output = EmitterRegistry::new()
        .emit(
            OutputFormat::Reconf,
            &DataValue::Some(Box::new(DataValue::Int(42))),
            &EmitOptions::default(),
        )
        .unwrap();

    assert_eq!(output, "some 42");
}

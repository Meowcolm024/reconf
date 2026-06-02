use reconf::Error;
use reconf::diagnostic::DiagnosticSource;
use reconf::error::ErrorCode;
use reconf::syntax::parser::parse;

#[test]
fn error_keeps_structured_labels_and_notes() {
    let error = Error::with_code(ErrorCode::TypeMismatch, "type mismatch")
        .with_source("input.reconf", "let x = 1")
        .with_label(4..5, "binding")
        .with_note("expected Bool");

    assert_eq!(error.code(), ErrorCode::TypeMismatch);
    assert_eq!(error.diagnostic_labels().len(), 1);
    assert_eq!(error.diagnostic_labels()[0].span, 4..5);
    assert_eq!(error.diagnostic_labels()[0].message, "binding");
    assert_eq!(error.notes(), &["expected Bool".to_string()]);
}

#[test]
fn error_can_hold_multiple_structured_labels() {
    let error = Error::new("duplicate name")
        .with_source("input.reconf", "let x = 1\nlet x = 2")
        .with_label(4..5, "first")
        .with_label(14..15, "second");

    let labels = error.diagnostic_labels();

    assert_eq!(labels.len(), 2);
    assert_eq!(labels[0].message, "first");
    assert_eq!(labels[1].message, "second");
}

#[test]
fn diagnostic_source_attaches_boundary_source_to_labeled_error() {
    let error =
        Error::with_code(ErrorCode::TypeMismatch, "type mismatch").with_label(4..5, "binding");
    let error = DiagnosticSource::new("input.reconf", "let x = 1").attach(error);

    assert_eq!(error.source_name(), Some("input.reconf"));
    assert_eq!(error.diagnostic_labels()[0].span, 4..5);
}

#[test]
fn diagnostic_source_renames_placeholder_source() {
    let error = Error::with_code(ErrorCode::TypeMismatch, "type mismatch")
        .with_label(4..5, "binding")
        .with_placeholder_source("let x = 1");
    let error = DiagnosticSource::new("input.reconf", "let x = 1").attach(error);

    assert_eq!(error.source_name(), Some("input.reconf"));
    assert_eq!(error.diagnostic_labels()[0].span, 4..5);
}

#[test]
fn parser_duplicate_field_diagnostic_has_producer_owned_label() {
    let source = "{ port = 1, port = 2 }";
    let error = parse(source).unwrap_err();

    assert_eq!(error.code(), ErrorCode::RecordDuplicateField);
    assert_eq!(error.diagnostic_labels().len(), 1);
    assert_eq!(&source[error.diagnostic_labels()[0].span.clone()], "port");
    assert_eq!(
        error.diagnostic_labels()[0].message,
        "duplicate field `port`"
    );
}

#[test]
fn parser_empty_interpolation_diagnostic_has_producer_owned_label() {
    let source = r#""prefix {} suffix""#;
    let error = parse(source).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ParseEmptyInterpolation);
    assert_eq!(error.diagnostic_labels().len(), 1);
    assert_eq!(&source[error.diagnostic_labels()[0].span.clone()], "{}");
    assert_eq!(error.diagnostic_labels()[0].message, "empty interpolation");
}

#[test]
fn parser_unterminated_interpolation_diagnostic_has_producer_owned_label() {
    let source = r#""prefix {value""#;
    let error = parse(source).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ParseUnterminatedString);
    assert_eq!(error.diagnostic_labels().len(), 1);
    assert_eq!(&source[error.diagnostic_labels()[0].span.clone()], "{value");
    assert_eq!(
        error.diagnostic_labels()[0].message,
        "unterminated interpolation"
    );
}

#[test]
fn parser_does_not_classify_regular_parse_error_as_empty_interpolation() {
    let source = "let text = ; # {}";
    let error = parse(source).unwrap_err();

    assert_ne!(error.code(), ErrorCode::ParseEmptyInterpolation);
}

#[test]
fn parser_unterminated_string_error_is_classified_without_source_wide_quote_counting() {
    let source = r#"{ closed = "ok", broken = "not closed }"#;
    let error = parse(source).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ParseUnterminatedString);
}

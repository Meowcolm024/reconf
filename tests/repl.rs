use reconf::repl::eval_for_test::ReplEvaluator;
use reconf::repl::highlighter::{
    semantic_type_spans_for_test, syntax_definition_source, theme_for_test,
};
use reconf::repl::is_complete_reconf_input;
use reconf::repl::semantic::SemanticState;
use reconf::syntax::parser::parse;

#[test]
fn validator_accepts_complete_input() {
    assert!(is_complete_reconf_input("let a = (1 + 2)"));
    assert!(is_complete_reconf_input(r#"let a = "{not structural}""#));
    assert!(is_complete_reconf_input("let a = { x = 1 } # {"));
    assert!(is_complete_reconf_input("[{ port = 8080 }]"));
}

#[test]
fn validator_rejects_incomplete_input() {
    assert!(!is_complete_reconf_input("let a = ("));
    assert!(!is_complete_reconf_input("let a = { x = 1"));
    assert!(!is_complete_reconf_input(r#"let a = "{unterminated"#));
    assert!(!is_complete_reconf_input("]"));
}

#[test]
fn syntax_definition_keeps_builtins_and_types_out_of_keywords() {
    let syntax = syntax_definition_source();
    let keyword_section = syntax
        .split("keywords:")
        .nth(1)
        .unwrap()
        .split("literals:")
        .next()
        .unwrap();

    for ordinary_identifier in ["startsWith", "contains", "isSome", "unwrapOr", "length"] {
        assert!(
            !keyword_section.contains(ordinary_identifier),
            "{ordinary_identifier} should be highlighted as an identifier, not a keyword"
        );
    }

    for type_name in ["Int", "Float", "Bool", "String"] {
        assert!(
            !keyword_section.contains(type_name),
            "{type_name} should be highlighted by the type-name rule, not as a keyword"
        );
    }

    assert!(syntax.contains("entity.name.type.reconf"));
    assert!(syntax.contains("variable.other.reconf"));
}

#[test]
fn repl_theme_loads_globals_and_font_styles() {
    let theme = theme_for_test();

    assert!(theme.settings.background.is_some());
    assert!(theme.settings.caret.is_some());
    assert!(theme.settings.selection.is_some());
    assert!(theme.settings.line_highlight.is_some());
    assert!(
        theme
            .scopes
            .iter()
            .any(|item| item.style.font_style.is_some()),
        "theme should preserve scoped font styles"
    );
}

#[test]
fn semantic_highlighting_tracks_custom_types() {
    let semantics = SemanticState::default();
    let file = parse(
        r#"
        import "./schema.reconf": ImportedTy;
        type Port = { x : Int | x > 1024 };
        let p : Port = 8080;
        p
        "#,
    )
    .unwrap();
    semantics.learn_file(&file);

    let line = "let x : Port = (8080 : Int); let y : ImportedTy = x; startsWith";
    let spans = semantic_type_spans_for_test(line, &semantics);
    let highlighted: Vec<&str> = spans.iter().map(|(a, b)| &line[*a..*b]).collect();

    assert!(highlighted.contains(&"Port"));
    assert!(highlighted.contains(&"Int"));
    assert!(highlighted.contains(&"ImportedTy"));
    assert!(!highlighted.contains(&"startsWith"));
}

#[test]
fn repl_evaluator_keeps_successful_declarations() {
    let mut evaluator = ReplEvaluator::new(SemanticState::default());

    assert_eq!(
        evaluator
            .eval("type Port = { x : Int | x > 1024 };")
            .unwrap(),
        "0"
    );
    assert_eq!(evaluator.eval("let p : Port = 8080;").unwrap(), "0");
    assert_eq!(evaluator.eval("p").unwrap(), "8080");
}

#[test]
fn repl_errors_carry_source_labels() {
    let mut evaluator = ReplEvaluator::new(SemanticState::default());

    let error = evaluator
        .eval("let config = { port = 8080 } : { port : Port };")
        .unwrap_err();
    let report = format!("{:?}", miette::Report::new(error));

    assert!(report.contains("unknown type `Port`"));
    assert!(report.contains("let config"));
    assert!(report.contains("Port"));
}

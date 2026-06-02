use miette::highlighters::SyntectHighlighter;
use reedline::StyledText;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, Style, Theme};
use syntect::parsing::{SyntaxDefinition, SyntaxSet, SyntaxSetBuilder};

use crate::repl::semantic::SemanticState;
use crate::repl::theme::ParseSettings;

const SYNTAX_YAML: &str = include_str!("reconf.sublime-syntax.yml");
const THEME_JSON: &str = include_str!("theme.json");

pub struct ReconfHighlighter {
    pub syntax: SyntaxSet,
    pub theme: Theme,
    semantics: SemanticState,
    type_style: Style,
}

impl Default for ReconfHighlighter {
    fn default() -> Self {
        Self::new(SemanticState::default())
    }
}

impl ReconfHighlighter {
    pub fn new(semantics: SemanticState) -> Self {
        let syntax_definition = SyntaxDefinition::load_from_str(SYNTAX_YAML, true, None)
            .expect("failed to load ReConf syntax definition");
        let theme = theme_for_test();
        let mut syntax_set = SyntaxSetBuilder::new();
        syntax_set.add(syntax_definition);
        Self {
            syntax: syntax_set.build(),
            type_style: semantic_type_style(&theme),
            theme,
            semantics,
        }
    }
}

impl From<ReconfHighlighter> for SyntectHighlighter {
    fn from(highlighter: ReconfHighlighter) -> Self {
        SyntectHighlighter::new(highlighter.syntax, highlighter.theme, false)
    }
}

impl reedline::Highlighter for ReconfHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let semantic_spans = semantic_type_spans(line, &self.semantics);
        let syntax = self
            .syntax
            .find_syntax_by_name("ReConf")
            .expect("ReConf syntax must be registered");
        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let ranges: Vec<(Style, &str)> = highlighter
            .highlight_line(line, &self.syntax)
            .expect("failed to highlight ReConf input");
        StyledText {
            buffer: apply_semantic_spans(ranges, &semantic_spans, self.type_style)
                .into_iter()
                .map(|(style, piece)| (ansi_style(style.foreground), piece))
                .collect(),
        }
    }
}

pub fn syntax_definition_source() -> &'static str {
    SYNTAX_YAML
}

#[doc(hidden)]
pub fn theme_for_test() -> Theme {
    let theme_settings =
        serde_json::from_str(THEME_JSON).expect("failed to parse ReConf REPL theme");
    Theme::parse_settings(theme_settings).expect("failed to load ReConf theme")
}

pub fn semantic_type_spans_for_test(line: &str, semantics: &SemanticState) -> Vec<(usize, usize)> {
    semantic_type_spans(line, semantics)
}

fn semantic_type_style(_theme: &Theme) -> Style {
    let foreground = Color {
        r: 79,
        g: 201,
        b: 176,
        a: 255,
    };
    Style {
        foreground,
        ..Style::default()
    }
}

fn apply_semantic_spans(
    ranges: Vec<(Style, &str)>,
    semantic_spans: &[(usize, usize)],
    type_style: Style,
) -> Vec<(Style, String)> {
    let mut out = Vec::new();
    let mut offset = 0usize;

    for (syntax_style, piece) in ranges {
        let piece_start = offset;
        let piece_end = piece_start + piece.len();
        let mut cursor = piece_start;

        for &(span_start, span_end) in semantic_spans {
            if span_end <= piece_start || span_start >= piece_end {
                continue;
            }

            let start = span_start.max(piece_start);
            let end = span_end.min(piece_end);
            if cursor < start {
                out.push((
                    syntax_style,
                    piece[cursor - piece_start..start - piece_start].to_string(),
                ));
            }
            out.push((
                type_style,
                piece[start - piece_start..end - piece_start].to_string(),
            ));
            cursor = end;
        }

        if cursor < piece_end {
            out.push((
                syntax_style,
                piece[cursor - piece_start..piece_end - piece_start].to_string(),
            ));
        }
        offset = piece_end;
    }

    out
}

fn semantic_type_spans(line: &str, semantics: &SemanticState) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut cursor = 0usize;

    while cursor < line.len() {
        let Some((relative, current)) = line[cursor..].char_indices().next() else {
            break;
        };
        cursor += relative;

        if in_string {
            if escaped {
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == '"' {
                in_string = false;
            }
            cursor += current.len_utf8();
            continue;
        }

        if current == '#' {
            break;
        }
        if current == '"' {
            in_string = true;
            cursor += current.len_utf8();
            continue;
        }

        if is_identifier_start(current) {
            let start = cursor;
            cursor += current.len_utf8();
            while cursor < line.len() {
                let Some(next) = line[cursor..].chars().next() else {
                    break;
                };
                if !is_identifier_continue(next) {
                    break;
                }
                cursor += next.len_utf8();
            }

            let ident = &line[start..cursor];
            if semantics.contains_type(ident) {
                spans.push((start, cursor));
            }
            continue;
        }

        cursor += current.len_utf8();
    }

    spans
}

fn is_identifier_start(value: char) -> bool {
    value == '_' || value.is_ascii_alphabetic()
}

fn is_identifier_continue(value: char) -> bool {
    value == '_' || value == '-' || value.is_ascii_alphanumeric()
}

fn ansi_style(color: Color) -> nu_ansi_term::Style {
    nu_ansi_term::Style::new().fg(nu_ansi_term::Color::Rgb(color.r, color.g, color.b))
}

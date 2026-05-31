use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub(crate) file_id: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Span {
    pub(crate) fn new(file_id: usize, start: usize, end: usize) -> Self {
        Self {
            file_id,
            start,
            end,
        }
    }

    pub(crate) fn empty(file_id: usize, pos: usize) -> Self {
        Self {
            file_id,
            start: pos,
            end: pos,
        }
    }

    pub(crate) fn join(self, other: Span) -> Self {
        Self {
            file_id: self.file_id,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) span: Span,
    pub(crate) notes: Vec<String>,
}

impl Diagnostic {
    pub(crate) fn new(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            message: message.into(),
            span,
            notes: Vec::new(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub(crate) fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn explain_code(code: &str) -> Option<&'static str> {
        match code {
            "E_PARSE_006" => Some("An interpolation was present but contained no expression."),
            "E_PARSE_008" => {
                Some("A string literal reached a line break before its closing quote.")
            }
            "E_MODULE_002" => Some(
                "The import graph contains a cycle. Break the cycle by moving shared definitions into a separate module.",
            ),
            "E_MODULE_004" => {
                Some("An import path could not be resolved relative to the importing file.")
            }
            "E_MODULE_005" => {
                Some("The imported name exists only if the target module exports it with `export`.")
            }
            "E_NAME_005" => Some("The expression references a value that is not in scope."),
            "E_RECORD_004" => Some(
                "Record literals are closed when checked against a record type; remove the unknown field or add it to the type.",
            ),
            "E_RECORD_005" => {
                Some("A required record field is missing. Only option-typed fields may be omitted.")
            }
            "E_REFINE_004" => Some(
                "A normalized value did not satisfy the refinement predicate for its annotated type.",
            ),
            "E_RUNTIME_016" => Some("Integer division by zero is rejected during normalization."),
            "E_TYPE_002" => {
                Some("A type alias refers to itself. Recursive type aliases are outside the MVP.")
            }
            "E_TYPE_006" => {
                Some("The `none` literal needs an expected option type such as `Int?`.")
            }
            "E_TYPE_008" => Some(
                "Function application requires the left-hand expression to have a function type.",
            ),
            "E_TYPE_014" => Some("A built-in was applied to an unsupported argument type."),
            "E_TYPE_017" => {
                Some("The expression's synthesized type does not match the expected type.")
            }
            "E_OUTPUT_001" => Some(
                "The final normalized configuration contains a function. ReConf outputs must be data.",
            ),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct SourceFile {
    path: PathBuf,
    text: String,
    line_starts: Vec<usize>,
}

#[derive(Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub(crate) fn add(&mut self, path: PathBuf, text: String) -> usize {
        let mut line_starts = vec![0];
        for (idx, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(idx + 1);
            }
        }
        let file_id = self.files.len();
        self.files.push(SourceFile {
            path,
            text,
            line_starts,
        });
        file_id
    }

    fn file(&self, id: usize) -> &SourceFile {
        &self.files[id]
    }

    fn line_col(&self, span: Span) -> (usize, usize) {
        let file = self.file(span.file_id);
        let line_idx = match file.line_starts.binary_search(&span.start) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let col = span.start.saturating_sub(file.line_starts[line_idx]) + 1;
        (line_idx + 1, col)
    }

    pub(crate) fn render(&self, diagnostic: Diagnostic) -> String {
        if self.files.get(diagnostic.span.file_id).is_none() {
            let mut out = String::new();
            let _ = writeln!(out, "error[{}]: {}", diagnostic.code, diagnostic.message);
            for note in diagnostic.notes {
                let _ = writeln!(out, "     = note: {note}");
            }
            return out;
        }
        let file = self.file(diagnostic.span.file_id);
        let (line, col) = self.line_col(diagnostic.span);
        let mut out = String::new();
        let _ = writeln!(out, "error[{}]: {}", diagnostic.code, diagnostic.message);
        let _ = writeln!(out, "  --> {}:{line}:{col}", file.path.display());

        let line_start = file.line_starts[line - 1];
        let line_end = file
            .text
            .get(line_start..)
            .and_then(|tail| tail.find('\n').map(|offset| line_start + offset))
            .unwrap_or(file.text.len());
        let source_line = &file.text[line_start..line_end];
        let _ = writeln!(out, "{line:>4} | {source_line}");

        let caret_start = diagnostic
            .span
            .start
            .saturating_sub(line_start)
            .min(source_line.len());
        let caret_end = diagnostic
            .span
            .end
            .max(diagnostic.span.start + 1)
            .saturating_sub(line_start)
            .min(source_line.len().max(caret_start + 1));
        let underline_len = caret_end.saturating_sub(caret_start).max(1);
        let _ = writeln!(
            out,
            "     | {}{}",
            " ".repeat(caret_start),
            "^".repeat(underline_len)
        );
        for note in diagnostic.notes {
            let _ = writeln!(out, "     = note: {note}");
        }
        out
    }
}

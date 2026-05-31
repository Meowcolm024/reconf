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

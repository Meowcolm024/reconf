use crate::source::Span;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Option<Span>,
}

impl Diagnostic {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            span: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

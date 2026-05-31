use std::sync::Arc;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("{message}")]
#[diagnostic(code(reconf::error))]
pub struct Error {
    message: String,
    #[source_code]
    source_code: Option<Arc<miette::NamedSource<String>>>,
    #[label("{label}")]
    span: Option<miette::SourceSpan>,
    label: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source_code: None,
            span: None,
            label: String::new(),
        }
    }

    pub fn with_source_span(
        mut self,
        name: impl AsRef<str>,
        source: impl Into<String>,
        span: std::ops::Range<usize>,
        label: impl Into<String>,
    ) -> Self {
        if self.source_code.is_none() && span.start <= span.end {
            self.source_code = Some(Arc::new(miette::NamedSource::new(name, source.into())));
            self.span = Some((span.start, span.end - span.start).into());
            self.label = label.into();
        }
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Clone for Error {
    fn clone(&self) -> Self {
        Self {
            message: self.message.clone(),
            source_code: self.source_code.clone(),
            span: self.span,
            label: self.label.clone(),
        }
    }
}

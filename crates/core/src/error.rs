use std::sync::Arc;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCodeInfo {
    pub code: &'static str,
    pub explanation: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ParseEmptyInterpolation,
    ParseUnterminatedString,
    ModuleCycle,
    ModuleMissingImport,
    ModuleUnexportedImport,
    NameDuplicateImport,
    OutputFunction,
    RefineLiteralUnion,
    RefineFailed,
    RuntimeDivisionByZero,
    TypeBadInterpolation,
    TypeApplyNonFunction,
    TypeNoneNeedsExpected,
    TypeRecursiveAlias,
    TypeUnknown,
    TypeMismatch,
    TypeUnsupportedBuiltinArg,
    RecordDuplicateField,
    RecordMissingField,
    RecordUnknownField,
    Reconf,
}

pub const ERROR_CODES: &[ErrorCode] = &[
    ErrorCode::ParseEmptyInterpolation,
    ErrorCode::ParseUnterminatedString,
    ErrorCode::ModuleCycle,
    ErrorCode::ModuleMissingImport,
    ErrorCode::ModuleUnexportedImport,
    ErrorCode::NameDuplicateImport,
    ErrorCode::OutputFunction,
    ErrorCode::RefineLiteralUnion,
    ErrorCode::RefineFailed,
    ErrorCode::RuntimeDivisionByZero,
    ErrorCode::TypeBadInterpolation,
    ErrorCode::TypeApplyNonFunction,
    ErrorCode::TypeNoneNeedsExpected,
    ErrorCode::TypeRecursiveAlias,
    ErrorCode::TypeUnknown,
    ErrorCode::TypeMismatch,
    ErrorCode::TypeUnsupportedBuiltinArg,
    ErrorCode::RecordDuplicateField,
    ErrorCode::RecordMissingField,
    ErrorCode::RecordUnknownField,
    ErrorCode::Reconf,
];

impl ErrorCode {
    pub fn from_code(code: &str) -> Option<Self> {
        ERROR_CODES
            .iter()
            .copied()
            .find(|candidate| candidate.as_str() == code)
    }

    pub fn info(self) -> ErrorCodeInfo {
        ErrorCodeInfo {
            code: self.as_str(),
            explanation: self.explanation(),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::ParseEmptyInterpolation => "E_PARSE_006",
            ErrorCode::ParseUnterminatedString => "E_PARSE_008",
            ErrorCode::ModuleCycle => "E_MODULE_002",
            ErrorCode::ModuleMissingImport => "E_MODULE_004",
            ErrorCode::ModuleUnexportedImport => "E_MODULE_005",
            ErrorCode::NameDuplicateImport => "E_NAME_002",
            ErrorCode::OutputFunction => "E_OUTPUT_001",
            ErrorCode::RefineLiteralUnion => "E_REFINE_002",
            ErrorCode::RefineFailed => "E_REFINE_004",
            ErrorCode::RuntimeDivisionByZero => "E_RUNTIME_016",
            ErrorCode::TypeBadInterpolation => "E_TYPE_005",
            ErrorCode::TypeApplyNonFunction => "E_TYPE_008",
            ErrorCode::TypeNoneNeedsExpected => "E_TYPE_006",
            ErrorCode::TypeRecursiveAlias => "E_TYPE_002",
            ErrorCode::TypeUnknown => "E_TYPE_003",
            ErrorCode::TypeMismatch => "E_TYPE_017",
            ErrorCode::TypeUnsupportedBuiltinArg => "E_TYPE_014",
            ErrorCode::RecordDuplicateField => "E_RECORD_003",
            ErrorCode::RecordMissingField => "E_RECORD_005",
            ErrorCode::RecordUnknownField => "E_RECORD_004",
            ErrorCode::Reconf => "reconf::error",
        }
    }

    pub fn explanation(self) -> &'static str {
        match self {
            ErrorCode::ParseEmptyInterpolation => "empty string interpolation",
            ErrorCode::ParseUnterminatedString => "unterminated string or interpolation",
            ErrorCode::ModuleCycle => "cyclic module import",
            ErrorCode::ModuleMissingImport => "missing module import path",
            ErrorCode::ModuleUnexportedImport => "imported name is not exported by the module",
            ErrorCode::NameDuplicateImport => "duplicate imported name",
            ErrorCode::OutputFunction => "function value escaped into output",
            ErrorCode::RefineLiteralUnion => "literal union refinement failed",
            ErrorCode::RefineFailed => "refinement predicate evaluated to false",
            ErrorCode::RuntimeDivisionByZero => "division by zero",
            ErrorCode::TypeBadInterpolation => "invalid string interpolation expression",
            ErrorCode::TypeApplyNonFunction => "attempted to apply a non-function value",
            ErrorCode::TypeNoneNeedsExpected => "none needs an expected option type",
            ErrorCode::TypeRecursiveAlias => "recursive type alias",
            ErrorCode::TypeUnknown => "unknown type",
            ErrorCode::TypeMismatch => "type mismatch",
            ErrorCode::TypeUnsupportedBuiltinArg => "unsupported native function argument",
            ErrorCode::RecordDuplicateField => "duplicate record field",
            ErrorCode::RecordMissingField => "missing record field",
            ErrorCode::RecordUnknownField => "unknown record field",
            ErrorCode::Reconf => "uncategorized ReConf error",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    code: ErrorCode,
    message: String,
    source_code: Option<Arc<miette::NamedSource<String>>>,
    labels: Vec<DiagnosticLabel>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    pub span: std::ops::Range<usize>,
    pub message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self::with_code(ErrorCode::Reconf, message)
    }

    pub fn with_code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source_code: None,
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn with_label(mut self, span: std::ops::Range<usize>, label: impl Into<String>) -> Self {
        if span.start <= span.end {
            self.labels.push(DiagnosticLabel {
                span,
                message: label.into(),
            });
        }
        self
    }

    pub fn with_source(mut self, name: impl AsRef<str>, source: impl Into<String>) -> Self {
        self.source_code = Some(Arc::new(miette::NamedSource::new(name, source.into())));
        self
    }

    pub fn with_placeholder_source(self, source: impl Into<String>) -> Self {
        self.with_source("<source>", source)
    }

    pub fn with_source_name(mut self, name: impl AsRef<str>) -> Self {
        if let Some(source) = self.source_code.as_ref() {
            let contents = source.inner().clone();
            self.source_code = Some(Arc::new(miette::NamedSource::new(name, contents)));
        }
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn diagnostic_labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }

    pub fn has_source_code(&self) -> bool {
        self.source_code.is_some()
    }

    pub fn source_name(&self) -> Option<&str> {
        self.source_code.as_ref().map(|source| source.name())
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }
}

impl miette::Diagnostic for Error {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(self.code))
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.source_code
            .as_ref()
            .map(|source| source as &dyn miette::SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        (!self.labels.is_empty()).then(|| {
            Box::new(self.labels.iter().map(|label| {
                miette::LabeledSpan::new_with_span(
                    Some(label.message.clone()),
                    (label.span.start, label.span.end - label.span.start),
                )
            })) as Box<dyn Iterator<Item = miette::LabeledSpan>>
        })
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        (!self.notes.is_empty()).then(|| Box::new(self.notes.join("\n")) as Box<_>)
    }
}

impl Clone for Error {
    fn clone(&self) -> Self {
        Self {
            code: self.code,
            message: self.message.clone(),
            source_code: self.source_code.clone(),
            labels: self.labels.clone(),
            notes: self.notes.clone(),
        }
    }
}

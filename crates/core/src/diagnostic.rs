use crate::Error;

pub struct DiagnosticSource<'a> {
    name: &'a str,
    source: &'a str,
}

impl<'a> DiagnosticSource<'a> {
    pub fn new(name: &'a str, source: &'a str) -> Self {
        Self { name, source }
    }

    pub fn attach(&self, error: Error) -> Error {
        if error.diagnostic_labels().is_empty() {
            return error;
        }
        if error.has_source_code() {
            return match error.source_name() {
                Some("<source>") => error.with_source_name(self.name),
                _ => error,
            };
        }
        error.with_source(self.name, self.source)
    }
}

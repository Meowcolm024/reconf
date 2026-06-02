mod data;
mod json;
mod reconf;

pub use crate::emit::data::DataValue;
pub use crate::emit::json::JsonEmitter;
pub use crate::emit::reconf::ReconfEmitter;

use reconf_core::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Reconf,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmitOptions {
    pub style: OutputStyle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputStyle {
    #[default]
    Compact,
    Pretty,
}

pub trait Emitter {
    fn format(&self) -> OutputFormat;

    fn emit(&self, value: &DataValue, options: &EmitOptions) -> Result<String>;
}

#[derive(Default)]
pub struct EmitterRegistry {
    emitters: Vec<Box<dyn Emitter>>,
}

impl EmitterRegistry {
    pub fn new() -> Self {
        Self::with_emitters([
            Box::new(JsonEmitter) as Box<dyn Emitter>,
            Box::new(ReconfEmitter),
        ])
    }

    pub fn with_emitters(emitters: impl IntoIterator<Item = Box<dyn Emitter>>) -> Self {
        Self {
            emitters: emitters.into_iter().collect(),
        }
    }

    pub fn emit(
        &self,
        format: OutputFormat,
        value: &DataValue,
        options: &EmitOptions,
    ) -> Result<String> {
        if let Some(emitter) = self
            .emitters
            .iter()
            .find(|emitter| emitter.format() == format)
        {
            return emitter.emit(value, options);
        }

        Err(reconf_core::error::Error::new(format!(
            "no emitter registered for {format:?}"
        )))
    }
}

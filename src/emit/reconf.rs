use crate::emit::{DataValue, EmitOptions, Emitter, OutputFormat};
use crate::error::Result;

#[derive(Clone, Copy, Debug, Default)]
pub struct ReconfEmitter;

impl Emitter for ReconfEmitter {
    fn format(&self) -> OutputFormat {
        OutputFormat::Reconf
    }

    fn emit(&self, value: &DataValue, _options: &EmitOptions) -> Result<String> {
        ReconfFormatter::new().format_value(value)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ReconfFormatter;

impl ReconfFormatter {
    fn new() -> Self {
        Self
    }

    fn format_value(&self, value: &DataValue) -> Result<String> {
        Ok(match value {
            DataValue::Int(value) => value.to_string(),
            DataValue::Float(value) => value.to_string(),
            DataValue::Bool(value) => value.to_string(),
            DataValue::String(value) => format!("{value:?}"),
            DataValue::None => "none".to_string(),
            DataValue::Some(value) => format!("some {}", self.format_value(value)?),
            DataValue::List(items) => self.format_list(items)?,
            DataValue::Record(fields) => self.format_record(fields)?,
        })
    }

    fn format_list(&self, items: &[DataValue]) -> Result<String> {
        let parts = items
            .iter()
            .map(|item| self.format_value(item))
            .collect::<Result<Vec<_>>>()?;
        Ok(format!("[{}]", parts.join(", ")))
    }

    fn format_record(
        &self,
        fields: &std::collections::BTreeMap<String, DataValue>,
    ) -> Result<String> {
        let parts = fields
            .iter()
            .map(|(name, value)| Ok(format!("{name} = {}", self.format_value(value)?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(format!("{{ {} }}", parts.join(", ")))
    }
}

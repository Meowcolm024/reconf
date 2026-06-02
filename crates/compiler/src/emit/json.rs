use std::collections::BTreeMap;

use serde_json::{Map, Number, Value as JsonValue};

use crate::emit::{DataValue, EmitOptions, Emitter, OutputFormat, OutputStyle};
use reconf_core::error::{Error, Result};

#[derive(Clone, Copy, Debug, Default)]
pub struct JsonEmitter;

impl Emitter for JsonEmitter {
    fn format(&self) -> OutputFormat {
        OutputFormat::Json
    }

    fn emit(&self, value: &DataValue, options: &EmitOptions) -> Result<String> {
        let json = JsonEncoder::new().encode(value)?;
        match options.style {
            OutputStyle::Compact => serde_json::to_string(&json),
            OutputStyle::Pretty => serde_json::to_string_pretty(&json),
        }
        .map_err(|error| Error::new(format!("failed to emit JSON: {error}")))
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct JsonEncoder;

impl JsonEncoder {
    fn new() -> Self {
        Self
    }

    fn encode(&self, value: &DataValue) -> Result<JsonValue> {
        Ok(match value {
            DataValue::Int(value) => JsonValue::Number(Number::from(*value)),
            DataValue::Float(value) if value.is_finite() => Number::from_f64(*value)
                .map(JsonValue::Number)
                .ok_or_else(|| Error::new("non-finite float cannot be emitted as JSON"))?,
            DataValue::Float(_) => {
                return Err(Error::new("non-finite float cannot be emitted as JSON"));
            }
            DataValue::Bool(value) => JsonValue::Bool(*value),
            DataValue::String(value) => JsonValue::String(value.clone()),
            DataValue::None => JsonValue::Null,
            DataValue::Some(value) => self.encode(value)?,
            DataValue::List(items) => JsonValue::Array(
                items
                    .iter()
                    .map(|item| self.encode(item))
                    .collect::<Result<Vec<JsonValue>>>()?,
            ),
            DataValue::Record(fields) => self.encode_record(fields)?,
        })
    }

    fn encode_record(&self, fields: &BTreeMap<String, DataValue>) -> Result<JsonValue> {
        let mut object = Map::new();
        for (name, value) in fields {
            object.insert(name.clone(), self.encode(value)?);
        }
        Ok(JsonValue::Object(object))
    }
}

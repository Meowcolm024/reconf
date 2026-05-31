use serde_json::{Map, Number, Value as JsonValue};

use crate::error::{Error, Result};
use crate::eval::Value;

pub fn emit_json(value: &Value, pretty: bool) -> Result<String> {
    let json = to_json(value)?;
    if pretty {
        serde_json::to_string_pretty(&json)
    } else {
        serde_json::to_string(&json)
    }
    .map_err(|error| Error::new(format!("failed to emit JSON: {error}")))
}

fn to_json(value: &Value) -> Result<JsonValue> {
    Ok(match value {
        Value::Int(value) => JsonValue::Number(Number::from(*value)),
        Value::Float(value) if value.is_finite() => Number::from_f64(*value)
            .map(JsonValue::Number)
            .ok_or_else(|| Error::new("non-finite float cannot be emitted as JSON"))?,
        Value::Float(_) => return Err(Error::new("non-finite float cannot be emitted as JSON")),
        Value::Bool(value) => JsonValue::Bool(*value),
        Value::String(value) => JsonValue::String(value.clone()),
        Value::None => JsonValue::Null,
        Value::Some(value) => to_json(value)?,
        Value::List(items) => JsonValue::Array(
            items
                .iter()
                .map(to_json)
                .collect::<Result<Vec<JsonValue>>>()?,
        ),
        Value::Record(fields) => {
            let mut object = Map::new();
            for (name, value) in fields {
                object.insert(name.clone(), to_json(value)?);
            }
            JsonValue::Object(object)
        }
        Value::Closure { .. } | Value::Native(_) => {
            return Err(Error::new("function escaped into output"));
        }
    })
}

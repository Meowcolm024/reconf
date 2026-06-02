use std::collections::BTreeMap;

use crate::emit::DataValue;
use crate::error::{Error, ErrorCode, Result};
use crate::eval::Value;

#[derive(Default)]
pub struct OutputValidator;

impl OutputValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(&self, value: &Value) -> Result<DataValue> {
        Ok(match value {
            Value::Int(value) => DataValue::Int(*value),
            Value::Float(value) => DataValue::Float(*value),
            Value::Bool(value) => DataValue::Bool(*value),
            Value::String(value) => DataValue::String(value.clone()),
            Value::None => DataValue::None,
            Value::Some(value) => DataValue::Some(Box::new(self.validate(value)?)),
            Value::List(items) => DataValue::List(
                items
                    .iter()
                    .map(|item| self.validate(item))
                    .collect::<Result<Vec<_>>>()?,
            ),
            Value::Record(fields) => DataValue::Record(
                fields
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), self.validate(value)?)))
                    .collect::<Result<BTreeMap<_, _>>>()?,
            ),
            Value::CoreClosure { .. } | Value::Native(_) => {
                return Err(Error::with_code(
                    ErrorCode::OutputFunction,
                    "function escaped into output",
                ));
            }
        })
    }
}

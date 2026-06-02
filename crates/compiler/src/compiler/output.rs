use std::collections::BTreeMap;

use crate::emit::DataValue;
use reconf_core::error::{Error, ErrorCode, Result};
use reconf_core::eval::Value;

#[derive(Default)]
pub struct OutputValidator;

impl OutputValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(&self, value: &Value) -> Result<DataValue> {
        Self::validate_value(value)
    }

    fn validate_value(value: &Value) -> Result<DataValue> {
        Ok(match value {
            Value::Int(value) => DataValue::Int(*value),
            Value::Float(value) => DataValue::Float(*value),
            Value::Bool(value) => DataValue::Bool(*value),
            Value::String(value) => DataValue::String(value.clone()),
            Value::None => DataValue::None,
            Value::Some(value) => DataValue::Some(Box::new(Self::validate_value(value)?)),
            Value::List(items) => DataValue::List(
                items
                    .iter()
                    .map(Self::validate_value)
                    .collect::<Result<Vec<_>>>()?,
            ),
            Value::Record(fields) => DataValue::Record(
                fields
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), Self::validate_value(value)?)))
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

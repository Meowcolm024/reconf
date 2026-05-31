use crate::error::Result;
use crate::eval::{Value, emit};

pub fn value(value: &Value) -> Result<String> {
    emit(value)
}

use crate::error::{Error, Result};
use crate::eval::Value;

pub fn emit_json(value: &Value, pretty: bool) -> Result<String> {
    let mut out = String::new();
    write_value(value, &mut out, pretty, 0)?;
    Ok(out)
}

fn write_value(value: &Value, out: &mut String, pretty: bool, indent: usize) -> Result<()> {
    match value {
        Value::Int(value) => out.push_str(&value.to_string()),
        Value::Float(value) if value.is_finite() => out.push_str(&value.to_string()),
        Value::Float(_) => return Err(Error::new("non-finite float cannot be emitted as JSON")),
        Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Value::String(value) => write_json_string(value, out),
        Value::None => out.push_str("null"),
        Value::Some(value) => write_value(value, out, pretty, indent)?,
        Value::List(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                if pretty {
                    out.push('\n');
                    out.push_str(&" ".repeat(indent + 2));
                }
                write_value(item, out, pretty, indent + 2)?;
            }
            if pretty && !items.is_empty() {
                out.push('\n');
                out.push_str(&" ".repeat(indent));
            }
            out.push(']');
        }
        Value::Record(fields) => {
            out.push('{');
            for (index, (name, value)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                if pretty {
                    out.push('\n');
                    out.push_str(&" ".repeat(indent + 2));
                }
                write_json_string(name, out);
                out.push(':');
                if pretty {
                    out.push(' ');
                }
                write_value(value, out, pretty, indent + 2)?;
            }
            if pretty && !fields.is_empty() {
                out.push('\n');
                out.push_str(&" ".repeat(indent));
            }
            out.push('}');
        }
        Value::Closure { .. } | Value::Native(_) => {
            return Err(Error::new("function escaped into output"));
        }
    }
    Ok(())
}

fn write_json_string(value: &str, out: &mut String) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

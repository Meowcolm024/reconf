use std::fmt::Write as _;

use crate::diagnostic::{Diagnostic, Span};
use crate::eval::Value;

pub fn emit_json(value: &Value, pretty: bool) -> Result<String, Diagnostic> {
    let mut out = String::new();
    write_json(value, pretty, 0, &mut out, Span::empty(0, 0))?;
    Ok(out)
}

fn write_json(
    value: &Value,
    pretty: bool,
    indent: usize,
    out: &mut String,
    span: Span,
) -> Result<(), Diagnostic> {
    match value {
        Value::Int(value) => write!(out, "{value}").unwrap(),
        Value::Float(value) if value.is_finite() => write!(out, "{value}").unwrap(),
        Value::Float(_) => {
            return Err(Diagnostic::new(
                "E_OUTPUT_002",
                "cannot emit non-finite float as JSON",
                span,
            ));
        }
        Value::Bool(value) => write!(out, "{value}").unwrap(),
        Value::String(value) => write_json_string(value, out),
        Value::None => out.push_str("null"),
        Value::Some(value) => write_json(value, pretty, indent, out, span)?,
        Value::List(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                if pretty {
                    out.push('\n');
                    out.push_str(&" ".repeat(indent + 2));
                }
                write_json(item, pretty, indent + 2, out, span)?;
            }
            if pretty && !items.is_empty() {
                out.push('\n');
                out.push_str(&" ".repeat(indent));
            }
            out.push(']');
        }
        Value::Record(fields) => {
            out.push('{');
            for (idx, (name, value)) in fields.iter().enumerate() {
                if idx > 0 {
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
                write_json(value, pretty, indent + 2, out, span)?;
            }
            if pretty && !fields.is_empty() {
                out.push('\n');
                out.push_str(&" ".repeat(indent));
            }
            out.push('}');
        }
        Value::Closure(_) | Value::Builtin { .. } => {
            return Err(Diagnostic::new(
                "E_OUTPUT_003",
                "function escaped into output",
                span,
            ));
        }
    }
    Ok(())
}

fn write_json_string(value: &str, out: &mut String) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

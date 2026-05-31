use crate::Error;

pub fn attach_best_effort_span(error: Error, name: &str, source: &str) -> Error {
    if let Some(type_name) = unknown_type_name(error.message()).map(str::to_string)
        && let Some(span) = find_word(source, &type_name)
    {
        return error.with_source_span(name, source, span, format!("unknown type `{type_name}`"));
    }

    if error.message().contains("refinement failed")
        && let Some(span) = find_refinement_value(source)
    {
        return error.with_source_span(name, source, span, "value does not satisfy refinement");
    }

    error
}

fn unknown_type_name(message: &str) -> Option<&str> {
    message.strip_prefix("unknown type `")?.strip_suffix('`')
}

fn find_word(source: &str, word: &str) -> Option<std::ops::Range<usize>> {
    let mut offset = 0;
    while let Some(index) = source[offset..].find(word) {
        let start = offset + index;
        let end = start + word.len();
        let before = source[..start].chars().next_back();
        let after = source[end..].chars().next();
        if !is_ident_continue(before) && !is_ident_continue(after) {
            return Some(start..end);
        }
        offset = end;
    }
    None
}

fn find_refinement_value(source: &str) -> Option<std::ops::Range<usize>> {
    let colon = source.rfind(':')?;
    let value_end = source[..colon].trim_end().len();
    let mut value_start = value_end;
    for (index, ch) in source[..value_end].char_indices().rev() {
        if ch.is_whitespace() || matches!(ch, '=' | ',' | '{' | '[' | '(') {
            break;
        }
        value_start = index;
    }
    (value_start < value_end).then_some(value_start..value_end)
}

fn is_ident_continue(value: Option<char>) -> bool {
    value.is_some_and(|value| value == '_' || value == '-' || value.is_ascii_alphanumeric())
}

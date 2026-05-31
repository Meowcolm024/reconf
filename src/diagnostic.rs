use crate::Error;

pub fn attach_best_effort_span(error: Error, name: &str, source: &str) -> Error {
    if error.message().contains("empty interpolation")
        && let Some(span) = find_empty_interpolation(source)
    {
        return error.with_source_span(name, source, span, "empty interpolation");
    }

    if let Some(type_name) = recursive_alias_name(error.message()).map(str::to_string)
        && let Some(span) = find_type_declaration_name(source, &type_name)
    {
        return error.with_source_span(
            name,
            source,
            span,
            format!("recursive type alias `{type_name}`"),
        );
    }

    if error.message().contains("division by zero")
        && let Some(span) = find_division_by_zero(source)
    {
        return error.with_source_span(name, source, span, "division by zero");
    }

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

fn find_empty_interpolation(source: &str) -> Option<std::ops::Range<usize>> {
    source.find("{}").map(|start| start..start + 2)
}

fn unknown_type_name(message: &str) -> Option<&str> {
    message.strip_prefix("unknown type `")?.strip_suffix('`')
}

fn recursive_alias_name(message: &str) -> Option<&str> {
    message
        .strip_prefix("recursive type alias `")?
        .strip_suffix('`')
}

fn find_type_declaration_name(source: &str, name: &str) -> Option<std::ops::Range<usize>> {
    let mut offset = 0;
    while let Some(index) = source[offset..].find("type") {
        let type_start = offset + index;
        let mut cursor = type_start + "type".len();
        if is_ident_continue(source[..type_start].chars().next_back())
            || !source[cursor..].starts_with(char::is_whitespace)
        {
            offset = cursor;
            continue;
        }

        cursor += source[cursor..]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();

        let name_end = cursor
            + source[cursor..]
                .chars()
                .take_while(|ch| *ch == '_' || *ch == '-' || ch.is_ascii_alphanumeric())
                .map(char::len_utf8)
                .sum::<usize>();
        if &source[cursor..name_end] == name {
            return Some(cursor..name_end);
        }
        offset = name_end.max(cursor + 1);
    }
    None
}

fn find_division_by_zero(source: &str) -> Option<std::ops::Range<usize>> {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'/' {
            index += 1;
            continue;
        }

        let mut cursor = index + 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor < bytes.len() && bytes[cursor] == b'0' {
            cursor += 1;
            if cursor == bytes.len() || !is_ident_continue(source[cursor..].chars().next()) {
                return Some(index..cursor);
            }
        }
        index += 1;
    }
    None
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

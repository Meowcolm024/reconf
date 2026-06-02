use reedline::{ValidationResult, Validator};

pub struct ReconfValidator;

impl Validator for ReconfValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if is_complete_reconf_input(line) {
            ValidationResult::Complete
        } else {
            ValidationResult::Incomplete
        }
    }
}

pub fn is_complete_reconf_input(line: &str) -> bool {
    let mut in_string = false;
    let mut in_comment = false;
    let mut escaped = false;
    let mut parens = 0i32;
    let mut braces = 0i32;
    let mut brackets = 0i32;

    for current in line.chars() {
        if in_comment {
            if current == '\n' {
                in_comment = false;
            }
            continue;
        }

        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match current {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match current {
            '#' => in_comment = true,
            '"' => in_string = true,
            '(' => parens += 1,
            ')' => parens -= 1,
            '{' => braces += 1,
            '}' => braces -= 1,
            '[' => brackets += 1,
            ']' => brackets -= 1,
            _ => {}
        }

        if parens < 0 || braces < 0 || brackets < 0 {
            return false;
        }
    }

    parens == 0 && braces == 0 && brackets == 0 && !in_string
}

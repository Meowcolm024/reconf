use std::collections::BTreeMap;

use pest::Parser as PestParser;
use pest::error::{Error as PestError, ErrorVariant, InputLocation};
use pest::iterators::Pair;
use pest_derive::Parser;

use crate::error::{Error, ErrorCode, Result};
use crate::syntax::surface::{Decl, Expr, FileAst, StrPart, Type};

#[derive(Parser)]
#[grammar = "syntax/reconf.pest"]
struct ReconfParser;

pub fn parse(src: &str) -> Result<FileAst> {
    let mut pairs =
        ReconfParser::parse(Rule::file, src).map_err(|error| parse_error(src, error))?;
    build_file(pairs.next().ok_or_else(|| Error::new("parse error"))?)
}

fn parse_error(src: &str, error: PestError<Rule>) -> Error {
    let code = if is_unterminated_string_error(src, &error) {
        ErrorCode::ParseUnterminatedString
    } else {
        ErrorCode::Reconf
    };

    let message = match &error.variant {
        ErrorVariant::ParsingError {
            positives,
            negatives,
        } => parse_expected_message(positives, negatives),
        ErrorVariant::CustomError { message } => message.clone(),
    };

    let span = match error.location {
        InputLocation::Pos(pos) => pos..pos.saturating_add(1),
        InputLocation::Span((start, end)) => start..end.max(start.saturating_add(1)),
    };

    Error::with_code(code, format!("parse error: {message}"))
        .with_label(span, message)
        .with_placeholder_source(src)
}

fn is_unterminated_string_error(src: &str, error: &PestError<Rule>) -> bool {
    let ErrorVariant::ParsingError { .. } = &error.variant else {
        return false;
    };

    let start = match error.location {
        InputLocation::Pos(pos) => pos,
        InputLocation::Span((start, _)) => start,
    };
    unclosed_quote_before(src, start).is_some()
}

fn unclosed_quote_before(src: &str, position: usize) -> Option<usize> {
    let mut open_quote = None;
    let mut escaped = false;
    let end = position.saturating_add(1).min(src.len());
    for (index, ch) in src[..end].char_indices() {
        match (escaped, ch) {
            (true, _) => escaped = false,
            (false, '\\') => escaped = true,
            (false, '"') => {
                open_quote = match open_quote {
                    Some(_) => None,
                    None => Some(index),
                };
            }
            (false, _) => {}
        }
    }
    open_quote
}

fn parse_expected_message(positives: &[Rule], negatives: &[Rule]) -> String {
    match (positives.is_empty(), negatives.is_empty()) {
        (false, false) => format!(
            "unexpected {}; expected {}",
            rule_list(negatives),
            rule_list(positives)
        ),
        (false, true) => format!("expected {}", rule_list(positives)),
        (true, false) => format!("unexpected {}", rule_list(negatives)),
        (true, true) => "unknown parsing error".to_string(),
    }
}

fn duplicate_field_error(pair: &Pair<'_, Rule>) -> Error {
    let name = pair.as_str();
    let span = pair.as_span();
    Error::with_code(
        ErrorCode::RecordDuplicateField,
        format!("duplicate field `{name}`"),
    )
    .with_label(
        span.start()..span.end(),
        format!("duplicate field `{name}`"),
    )
    .with_placeholder_source(span.get_input())
}

fn rule_list(rules: &[Rule]) -> String {
    let mut names = rules.iter().map(rule_name).collect::<Vec<_>>();
    names.sort();
    names.dedup();
    match names.as_slice() {
        [] => String::new(),
        [one] => (*one).to_string(),
        [first, second] => format!("{first} or {second}"),
        _ => {
            let last = names.pop().unwrap();
            format!("{}, or {last}", names.join(", "))
        }
    }
}

fn rule_name(rule: &Rule) -> &'static str {
    match rule {
        Rule::EOI => "end of input",
        Rule::WHITESPACE => "whitespace",
        Rule::COMMENT => "comment",
        Rule::ident => "identifier",
        Rule::file => "file",
        Rule::decl => "declaration",
        Rule::import_decl => "import",
        Rule::export_native_decl => "export native",
        Rule::export_type_decl => "export type",
        Rule::export_let_decl => "export let",
        Rule::native_decl => "native",
        Rule::type_decl => "type declaration",
        Rule::top_let_decl => "let declaration",
        Rule::ty => "type",
        Rule::fun_ty => "function type",
        Rule::postfix_ty => "postfix type",
        Rule::primary_ty => "type",
        Rule::base_ty => "primitive type",
        Rule::type_alias => "type name",
        Rule::list_ty => "list type",
        Rule::paren_ty => "parenthesized type",
        Rule::record_or_refinement_ty => "record or refinement type",
        Rule::record_fields => "record fields",
        Rule::field_ty => "field type",
        Rule::refinement_body => "refinement body",
        Rule::literal_union_ty => "literal union type",
        Rule::expr => "expression",
        Rule::let_expr => "let expression",
        Rule::ascription_expr => "ascription",
        Rule::if_expr => "if expression",
        Rule::plain_expr => "expression",
        Rule::logic_or => "or expression",
        Rule::logic_and => "and expression",
        Rule::equality => "equality expression",
        Rule::relation => "comparison expression",
        Rule::additive => "additive expression",
        Rule::multiplicative => "multiplicative expression",
        Rule::unary => "unary expression",
        Rule::application => "function application",
        Rule::postfix => "postfix expression",
        Rule::method_call => "method call",
        Rule::field_access => "field access",
        Rule::primary => "expression",
        Rule::option_expr => "option",
        Rule::none_expr => "none",
        Rule::some_expr => "some",
        Rule::ident_expr => "identifier",
        Rule::record_expr => "record",
        Rule::field_expr => "field",
        Rule::list_expr => "list",
        Rule::lambda_expr => "lambda",
        Rule::paren_expr => "parenthesized expression",
        Rule::literal => "literal",
        Rule::bool_lit => "bool",
        Rule::int_lit => "int",
        Rule::float_lit => "float",
        Rule::string_lit => "string",
        Rule::escaped_char => "escaped character",
        Rule::or_op => "`||`",
        Rule::and_op => "`&&`",
        Rule::eq_op => "equality operator",
        Rule::rel_op => "comparison operator",
        Rule::add_op => "additive operator",
        Rule::mul_op => "multiplicative operator",
        Rule::unary_op => "unary operator",
        Rule::name => "name",
        Rule::type_name => "type name",
        Rule::keyword => "keyword",
        Rule::lower => "lowercase letter",
        Rule::upper => "uppercase letter",
    }
}

fn build_file(pair: Pair<'_, Rule>) -> Result<FileAst> {
    let mut decls = Vec::new();
    let mut output = None;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::decl => decls.push(build_decl(only(pair)?)?),
            Rule::import_decl
            | Rule::export_native_decl
            | Rule::export_type_decl
            | Rule::export_let_decl
            | Rule::native_decl
            | Rule::type_decl
            | Rule::top_let_decl => decls.push(build_decl(pair)?),
            Rule::expr => output = Some(build_expr(pair)?),
            Rule::EOI => {}
            _ => {}
        }
    }
    Ok(FileAst { decls, output })
}

fn build_decl(pair: Pair<'_, Rule>) -> Result<Decl> {
    match pair.as_rule() {
        Rule::import_decl => {
            let mut inner = pair.into_inner();
            let path = unquote(inner.next().unwrap().as_str())?;
            let names = inner.map(|p| p.as_str().to_string()).collect();
            Ok(Decl::Import { path, names })
        }
        Rule::export_native_decl => match build_decl(only(pair)?)? {
            Decl::Native { name, ty, .. } => Ok(Decl::Native {
                export: true,
                name,
                ty,
            }),
            _ => unreachable!(),
        },
        Rule::export_type_decl => match build_decl(only(pair)?)? {
            Decl::Type { name, ty, .. } => Ok(Decl::Type {
                export: true,
                name,
                ty,
            }),
            _ => unreachable!(),
        },
        Rule::export_let_decl => match build_decl(only(pair)?)? {
            Decl::Let {
                name,
                annotation,
                expr,
                ..
            } => Ok(Decl::Let {
                export: true,
                name,
                annotation,
                expr,
            }),
            _ => unreachable!(),
        },
        Rule::native_decl => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            let ty = build_type(inner.next().unwrap())?;
            Ok(Decl::Native {
                export: false,
                name,
                ty,
            })
        }
        Rule::type_decl => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            let ty = build_type(inner.next().unwrap())?;
            Ok(Decl::Type {
                export: false,
                name,
                ty,
            })
        }
        Rule::top_let_decl => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            let mut annotation = None;
            let mut expr = None;
            for pair in inner {
                match pair.as_rule() {
                    Rule::ty => annotation = Some(build_type(pair)?),
                    Rule::expr => expr = Some(build_expr(pair)?),
                    _ => {}
                }
            }
            Ok(Decl::Let {
                export: false,
                name,
                annotation,
                expr: expr.ok_or_else(|| Error::new("parse error: missing let expression"))?,
            })
        }
        _ => Err(Error::new("parse error: expected declaration")),
    }
}

fn build_type(pair: Pair<'_, Rule>) -> Result<Type> {
    let span = pair.as_span();
    Ok(Type::Spanned(
        Box::new(build_type_node(pair)?),
        span.start()..span.end(),
    ))
}

fn build_type_node(pair: Pair<'_, Rule>) -> Result<Type> {
    match pair.as_rule() {
        Rule::ty | Rule::primary_ty => build_type(only(pair)?),
        Rule::fun_ty => {
            let mut inner = pair.into_inner();
            let left = build_type(inner.next().unwrap())?;
            if let Some(right) = inner.next() {
                Ok(Type::Function(Box::new(left), Box::new(build_type(right)?)))
            } else {
                Ok(left)
            }
        }
        Rule::postfix_ty => {
            let text = pair.as_str();
            let inner = build_type(pair.into_inner().next().unwrap())?;
            if text.trim_end().ends_with('?') {
                Ok(Type::Option(Box::new(inner)))
            } else {
                Ok(inner)
            }
        }
        Rule::base_ty => Ok(match pair.as_str() {
            "Int" => Type::Int,
            "Float" => Type::Float,
            "Bool" => Type::Bool,
            "String" => Type::String,
            _ => unreachable!(),
        }),
        Rule::type_alias => Ok(Type::Alias(pair.as_str().to_string())),
        Rule::list_ty => Ok(Type::List(Box::new(build_type(only(pair)?)?))),
        Rule::paren_ty => build_type(only(pair)?),
        Rule::record_or_refinement_ty => {
            let Some(inner) = pair.into_inner().next() else {
                return Ok(Type::Record(BTreeMap::new()));
            };
            match inner.as_rule() {
                Rule::record_fields => {
                    let mut fields = BTreeMap::new();
                    for field in inner.into_inner() {
                        let mut items = field.into_inner();
                        let name_pair = items.next().unwrap();
                        let name = name_pair.as_str().to_string();
                        let ty = build_type(items.next().unwrap())?;
                        if fields.insert(name.clone(), ty).is_some() {
                            return Err(duplicate_field_error(&name_pair));
                        }
                    }
                    Ok(Type::Record(fields))
                }
                Rule::refinement_body => {
                    let mut items = inner.into_inner();
                    let binder = items.next().unwrap().as_str().to_string();
                    let base = build_type(items.next().unwrap())?;
                    let pred = build_expr(items.next().unwrap())?;
                    Ok(Type::Refinement {
                        binder,
                        base: Box::new(base),
                        pred: Box::new(pred),
                    })
                }
                _ => unreachable!(),
            }
        }
        Rule::literal_union_ty => {
            let choices = pair
                .into_inner()
                .map(|p| unquote(p.as_str()))
                .collect::<Result<Vec<_>>>()?;
            Ok(Type::LiteralUnion(choices))
        }
        _ => Err(Error::new(format!(
            "parse error: unexpected type rule {:?}",
            pair.as_rule()
        ))),
    }
}

fn build_expr(pair: Pair<'_, Rule>) -> Result<Expr> {
    let span = pair.as_span();
    Ok(Expr::Spanned(
        Box::new(build_expr_node(pair)?),
        span.start()..span.end(),
    ))
}

fn build_expr_node(pair: Pair<'_, Rule>) -> Result<Expr> {
    match pair.as_rule() {
        Rule::expr | Rule::plain_expr | Rule::primary => build_expr(only(pair)?),
        Rule::let_expr => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            let mut annotation = None;
            let mut exprs = Vec::new();
            for pair in inner {
                match pair.as_rule() {
                    Rule::ty => annotation = Some(build_type(pair)?),
                    Rule::expr => exprs.push(build_expr(pair)?),
                    _ => {}
                }
            }
            Ok(Expr::Let(
                name,
                annotation,
                Box::new(exprs.remove(0)),
                Box::new(exprs.remove(0)),
            ))
        }
        Rule::ascription_expr => {
            let mut inner = pair.into_inner();
            let expr = build_expr(inner.next().unwrap())?;
            if let Some(ty) = inner.next() {
                Ok(Expr::Ascribe(Box::new(expr), build_type(ty)?))
            } else {
                Ok(expr)
            }
        }
        Rule::lambda_expr => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            let ty = build_type(inner.next().unwrap())?;
            let body = build_expr(inner.next().unwrap())?;
            Ok(Expr::Lambda(name, ty, Box::new(body)))
        }
        Rule::if_expr => {
            let mut inner = pair.into_inner();
            Ok(Expr::If(
                Box::new(build_expr(inner.next().unwrap())?),
                Box::new(build_expr(inner.next().unwrap())?),
                Box::new(build_expr(inner.next().unwrap())?),
            ))
        }
        Rule::logic_or
        | Rule::logic_and
        | Rule::equality
        | Rule::relation
        | Rule::additive
        | Rule::multiplicative => build_left_assoc(pair),
        Rule::unary => build_unary(pair),
        Rule::application => build_application(pair),
        Rule::postfix => build_postfix(pair),
        Rule::literal => build_expr(only(pair)?),
        Rule::int_lit => Ok(Expr::Int(pair.as_str().parse().unwrap())),
        Rule::float_lit => Ok(Expr::Float(pair.as_str().parse().unwrap())),
        Rule::bool_lit => Ok(Expr::Bool(pair.as_str() == "true")),
        Rule::string_lit => build_string(pair),
        Rule::none_expr => Ok(Expr::None),
        Rule::some_expr => Ok(Expr::Some(Box::new(build_expr(only(pair)?)?))),
        Rule::option_expr => build_expr(only(pair)?),
        Rule::ident_expr => Ok(Expr::Var(only(pair)?.as_str().to_string())),
        Rule::list_expr => pair
            .into_inner()
            .map(build_expr)
            .collect::<Result<Vec<_>>>()
            .map(Expr::List),
        Rule::record_expr => {
            let mut fields = BTreeMap::new();
            for field in pair.into_inner() {
                let mut inner = field.into_inner();
                let name_pair = inner.next().unwrap();
                let name = name_pair.as_str().to_string();
                let value = build_expr(inner.next().unwrap())?;
                if fields.insert(name.clone(), value).is_some() {
                    return Err(duplicate_field_error(&name_pair));
                }
            }
            Ok(Expr::Record(fields))
        }
        Rule::paren_expr => build_expr(only(pair)?),
        _ => Err(Error::new(format!(
            "parse error: unexpected expression rule {:?}",
            pair.as_rule()
        ))),
    }
}

fn build_left_assoc(pair: Pair<'_, Rule>) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let mut expr = build_expr(inner.next().unwrap())?;
    while let Some(op) = inner.next() {
        let rhs = build_expr(inner.next().unwrap())?;
        expr = Expr::Binary(op.as_str().to_string(), Box::new(expr), Box::new(rhs));
    }
    Ok(expr)
}

fn build_unary(pair: Pair<'_, Rule>) -> Result<Expr> {
    let mut ops = Vec::new();
    let mut application = None;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::unary_op => ops.push(pair.as_str().to_string()),
            Rule::application => application = Some(build_expr(pair)?),
            _ => {}
        }
    }
    let mut expr = application.ok_or_else(|| Error::new("parse error: expected expression"))?;
    for op in ops.into_iter().rev() {
        expr = Expr::Unary(op, Box::new(expr));
    }
    Ok(expr)
}

fn build_application(pair: Pair<'_, Rule>) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let mut expr = build_expr(inner.next().unwrap())?;
    for arg in inner {
        expr = Expr::Apply(Box::new(expr), Box::new(build_expr(arg)?));
    }
    Ok(expr)
}

fn build_postfix(pair: Pair<'_, Rule>) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let mut expr = build_expr(inner.next().unwrap())?;
    for pair in inner {
        match pair.as_rule() {
            Rule::field_access => {
                let name = only(pair)?.as_str().to_string();
                expr = Expr::Dot(Box::new(expr), name);
            }
            Rule::method_call => {
                let mut parts = pair.into_inner();
                let name = parts.next().unwrap().as_str().to_string();
                let mut method = Expr::Apply(Box::new(Expr::Var(name)), Box::new(expr));
                for arg in parts {
                    method = Expr::Apply(Box::new(method), Box::new(build_expr(arg)?));
                }
                expr = method;
            }
            _ => {}
        }
    }
    Ok(expr)
}

fn build_string(pair: Pair<'_, Rule>) -> Result<Expr> {
    let span = pair.as_span();
    let raw = pair.as_str();
    let unquoted = unquote(raw)?;
    let mut parts = Vec::new();
    let mut text = String::new();
    let chars: Vec<char> = unquoted.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\u{1}' => {
                text.push('{');
                i += 1;
            }
            '\u{2}' => {
                text.push('}');
                i += 1;
            }
            '{' => {
                if !text.is_empty() {
                    parts.push(StrPart::Text(std::mem::take(&mut text)));
                }
                let start = i + 1;
                let mut depth = 1;
                i += 1;
                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                if depth != 0 {
                    let interpolation_span =
                        unterminated_interpolation_span(span.start(), raw, start, chars.len());
                    return Err(Error::with_code(
                        ErrorCode::ParseUnterminatedString,
                        "parse error: unterminated interpolation",
                    )
                    .with_label(interpolation_span, "unterminated interpolation")
                    .with_placeholder_source(span.get_input()));
                }
                let inner: String = chars[start..i - 1].iter().collect();
                let interpolation_span = interpolation_span(span.start(), raw, start, i - 1);
                parts.push(StrPart::Expr(parse_expr_fragment(
                    &inner,
                    span.get_input(),
                    interpolation_span,
                )?));
            }
            c => {
                text.push(c);
                i += 1;
            }
        }
    }
    if !text.is_empty() {
        parts.push(StrPart::Text(text));
    }
    if parts.len() == 1
        && matches!(parts[0], StrPart::Text(_))
        && let StrPart::Text(text) = parts.remove(0)
    {
        return Ok(Expr::String(text));
    }
    Ok(Expr::Interp(parts))
}

fn parse_expr_fragment(
    src: &str,
    outer_source: &str,
    interpolation_span: std::ops::Range<usize>,
) -> Result<Expr> {
    if src.trim().is_empty() {
        return Err(Error::with_code(
            ErrorCode::ParseEmptyInterpolation,
            "parse error: empty interpolation",
        )
        .with_label(interpolation_span, "empty interpolation")
        .with_placeholder_source(outer_source));
    }
    let mut pairs = ReconfParser::parse(Rule::expr, src).map_err(|e| parse_error(src, e))?;
    build_expr(pairs.next().unwrap())
}

fn interpolation_span(
    string_start: usize,
    raw: &str,
    inner_start: usize,
    inner_end: usize,
) -> std::ops::Range<usize> {
    if inner_start == inner_end
        && let Some(offset) = raw.find("{}")
    {
        return string_start + offset..string_start + offset + 2;
    }

    let content_start = string_start + 1;
    content_start + inner_start.saturating_sub(1)..content_start + inner_end + 1
}

fn unterminated_interpolation_span(
    string_start: usize,
    raw: &str,
    inner_start: usize,
    content_end: usize,
) -> std::ops::Range<usize> {
    let content_start = string_start + 1;
    let start = content_start + inner_start.saturating_sub(1);
    let end = content_start + content_end;

    let raw_content_end = string_start + raw.len().saturating_sub(1);
    start..end.min(raw_content_end).max(start)
}

fn unquote(raw: &str) -> Result<String> {
    let mut chars = raw.chars();
    if chars.next() != Some('"') || chars.next_back() != Some('"') {
        return Err(Error::new("parse error: expected string"));
    }
    let mut out = String::new();
    let mut escape = false;
    for c in chars {
        if escape {
            out.push(match c {
                '"' => '"',
                '\\' => '\\',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '{' => '\u{1}',
                '}' => '\u{2}',
                other => other,
            });
            escape = false;
        } else if c == '\\' {
            escape = true;
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

fn only(pair: Pair<'_, Rule>) -> Result<Pair<'_, Rule>> {
    let mut inner = pair.into_inner();
    let first = inner
        .next()
        .ok_or_else(|| Error::new("parse error: expected inner node"))?;
    if inner.next().is_some() {
        return Err(Error::new("parse error: unexpected extra node"));
    }
    Ok(first)
}

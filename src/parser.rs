use std::collections::BTreeMap;

use pest::Parser as PestParser;
use pest::iterators::Pair;
use pest_derive::Parser;

use crate::ast::{Decl, Expr, FileAst, StrPart, Type};
use crate::error::{Error, Result};

#[derive(Parser)]
#[grammar = "reconf.pest"]
struct ReconfParser;

pub fn parse(src: &str) -> Result<FileAst> {
    let mut pairs = ReconfParser::parse(Rule::file, src)
        .map_err(|e| Error::new(format!("parse error: {e}")))?;
    build_file(pairs.next().ok_or_else(|| Error::new("parse error"))?)
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
    Ok(FileAst {
        decls,
        output: output.ok_or_else(|| Error::new("parse error: missing output expression"))?,
    })
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
                        let name = items.next().unwrap().as_str().to_string();
                        let ty = build_type(items.next().unwrap())?;
                        if fields.insert(name.clone(), ty).is_some() {
                            return Err(Error::new(format!("duplicate field `{name}`")));
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
            let pred = choices
                .into_iter()
                .map(|choice| {
                    Expr::Binary(
                        "==".to_string(),
                        Box::new(Expr::Var("x".to_string())),
                        Box::new(Expr::String(choice)),
                    )
                })
                .reduce(|a, b| Expr::Binary("||".to_string(), Box::new(a), Box::new(b)))
                .unwrap_or(Expr::Bool(false));
            Ok(Type::Refinement {
                binder: "x".to_string(),
                base: Box::new(Type::String),
                pred: Box::new(pred),
            })
        }
        _ => Err(Error::new(format!(
            "parse error: unexpected type rule {:?}",
            pair.as_rule()
        ))),
    }
}

fn build_expr(pair: Pair<'_, Rule>) -> Result<Expr> {
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
        Rule::string_lit => build_string(pair.as_str()),
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
                let name = inner.next().unwrap().as_str().to_string();
                let value = build_expr(inner.next().unwrap())?;
                if fields.insert(name.clone(), value).is_some() {
                    return Err(Error::new(format!("duplicate field `{name}`")));
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

fn build_string(raw: &str) -> Result<Expr> {
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
                    return Err(Error::new("parse error: unterminated interpolation"));
                }
                let inner: String = chars[start..i - 1].iter().collect();
                parts.push(StrPart::Expr(parse_expr_fragment(&inner)?));
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

fn parse_expr_fragment(src: &str) -> Result<Expr> {
    let mut pairs = ReconfParser::parse(Rule::expr, src)
        .map_err(|e| Error::new(format!("parse error: {e}")))?;
    build_expr(pairs.next().unwrap())
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

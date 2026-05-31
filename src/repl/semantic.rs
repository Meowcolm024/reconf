use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

use crate::syntax::surface::{Decl, Expr, FileAst, StrPart, Type};

#[derive(Clone, Debug)]
pub struct SemanticState {
    types: Arc<RwLock<BTreeSet<String>>>,
}

impl Default for SemanticState {
    fn default() -> Self {
        let types = ["Int", "Float", "Bool", "String"]
            .into_iter()
            .map(String::from)
            .collect();
        Self {
            types: Arc::new(RwLock::new(types)),
        }
    }
}

impl SemanticState {
    pub fn contains_type(&self, name: &str) -> bool {
        self.types.read().is_ok_and(|types| types.contains(name))
    }

    pub fn learn_file(&self, file: &FileAst) {
        let mut collector = TypeCollector::default();
        collector.visit_file(file);
        if let Ok(mut types) = self.types.write() {
            types.extend(collector.types);
        }
    }
}

#[derive(Default)]
struct TypeCollector {
    types: BTreeSet<String>,
}

impl TypeCollector {
    fn visit_file(&mut self, file: &FileAst) {
        for decl in &file.decls {
            self.visit_decl(decl);
        }
    }

    fn visit_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Import { names, .. } => {
                for name in names {
                    if starts_with_uppercase(name) {
                        self.types.insert(name.clone());
                    }
                }
            }
            Decl::Native { ty, .. } => self.visit_type(ty),
            Decl::Type { name, ty, .. } => {
                self.types.insert(name.clone());
                self.visit_type(ty);
            }
            Decl::Let {
                annotation, expr, ..
            } => {
                if let Some(ty) = annotation {
                    self.visit_type(ty);
                }
                self.visit_expr(expr);
            }
        }
    }

    fn visit_type(&mut self, ty: &Type) {
        match ty {
            Type::Int | Type::Float | Type::Bool | Type::String | Type::LiteralUnion(_) => {}
            Type::Option(inner) | Type::List(inner) => self.visit_type(inner),
            Type::Record(fields) => {
                for ty in fields.values() {
                    self.visit_type(ty);
                }
            }
            Type::Refinement { base, pred, .. } => {
                self.visit_type(base);
                self.visit_expr(pred);
            }
            Type::Function(param, ret) => {
                self.visit_type(param);
                self.visit_type(ret);
            }
            Type::Alias(name) => {
                self.types.insert(name.clone());
            }
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Int(_)
            | Expr::Float(_)
            | Expr::Bool(_)
            | Expr::String(_)
            | Expr::None
            | Expr::Var(_) => {}
            Expr::Interp(parts) => {
                for part in parts {
                    if let StrPart::Expr(expr) = part {
                        self.visit_expr(expr);
                    }
                }
            }
            Expr::Some(expr) | Expr::Field(expr, _) | Expr::Dot(expr, _) | Expr::Unary(_, expr) => {
                self.visit_expr(expr)
            }
            Expr::List(items) => {
                for item in items {
                    self.visit_expr(item);
                }
            }
            Expr::Record(fields) => {
                for value in fields.values() {
                    self.visit_expr(value);
                }
            }
            Expr::If(cond, then_expr, else_expr) => {
                self.visit_expr(cond);
                self.visit_expr(then_expr);
                self.visit_expr(else_expr);
            }
            Expr::Let(_, annotation, value, body) => {
                if let Some(ty) = annotation {
                    self.visit_type(ty);
                }
                self.visit_expr(value);
                self.visit_expr(body);
            }
            Expr::Lambda(_, ty, body) => {
                self.visit_type(ty);
                self.visit_expr(body);
            }
            Expr::Apply(function, arg) | Expr::Binary(_, function, arg) => {
                self.visit_expr(function);
                self.visit_expr(arg);
            }
            Expr::Ascribe(expr, ty) => {
                self.visit_expr(expr);
                self.visit_type(ty);
            }
        }
    }
}

fn starts_with_uppercase(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
}

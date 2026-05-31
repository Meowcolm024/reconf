use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct FileAst {
    pub decls: Vec<Decl>,
    pub output: Expr,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Import {
        path: String,
        names: Vec<String>,
    },
    Native {
        export: bool,
        name: String,
        ty: Type,
    },
    Type {
        export: bool,
        name: String,
        ty: Type,
    },
    Let {
        export: bool,
        name: String,
        annotation: Option<Type>,
        expr: Expr,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    LiteralUnion(Vec<String>),
    Option(Box<Type>),
    List(Box<Type>),
    Record(BTreeMap<String, Type>),
    Refinement {
        binder: String,
        base: Box<Type>,
        pred: Box<Expr>,
    },
    Function(Box<Type>, Box<Type>),
    Alias(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Interp(Vec<StrPart>),
    None,
    Some(Box<Expr>),
    Var(String),
    List(Vec<Expr>),
    Record(BTreeMap<String, Expr>),
    Field(Box<Expr>, String),
    Dot(Box<Expr>, String),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Let(String, Option<Type>, Box<Expr>, Box<Expr>),
    Lambda(String, Type, Box<Expr>),
    Apply(Box<Expr>, Box<Expr>),
    Ascribe(Box<Expr>, Type),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Text(String),
    Expr(Expr),
}

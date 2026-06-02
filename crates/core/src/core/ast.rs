use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct CoreModule {
    pub imports: Vec<CoreImport>,
    pub decls: Vec<CoreDecl>,
    pub output: Option<CoreExpr>,
}

#[derive(Debug, Clone)]
pub struct ElaboratedModule {
    pub decls: Vec<ElaboratedDecl>,
    pub output: Option<ElaboratedExpr>,
}

#[derive(Debug, Clone)]
pub enum ElaboratedDecl {
    Native {
        export: bool,
        name: String,
        binding: GlobalRef,
        ty: CoreType,
    },
    Type {
        export: bool,
        name: String,
        alias: TypeAliasRef,
        ty: CoreType,
    },
    Let {
        export: bool,
        name: String,
        binding: GlobalRef,
        expr: ElaboratedExpr,
    },
}

#[derive(Debug, Clone)]
pub enum ElaboratedExpr {
    Checked(TypedCoreExpr),
}

impl ElaboratedExpr {
    pub fn ty(&self) -> Option<&CoreType> {
        match self {
            ElaboratedExpr::Checked(expr) => Some(&expr.ty),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypedCoreExpr {
    pub expr: CoreExpr,
    pub ty: CoreType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalRef {
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalRef {
    id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeAliasRef {
    id: usize,
}

impl LocalRef {
    pub fn new(index: usize) -> Self {
        Self { index }
    }

    pub fn index(self) -> usize {
        self.index
    }
}

impl GlobalRef {
    pub fn new(id: usize) -> Self {
        Self { id }
    }

    pub fn id(self) -> usize {
        self.id
    }
}

impl TypeAliasRef {
    pub fn new(id: usize) -> Self {
        Self { id }
    }

    pub fn id(self) -> usize {
        self.id
    }
}

#[derive(Debug, Clone)]
pub struct CoreImport {
    pub path: String,
    pub names: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum CoreDecl {
    Native {
        export: bool,
        name: String,
        ty: CoreType,
    },
    Type {
        export: bool,
        name: String,
        ty: CoreType,
    },
    Let {
        export: bool,
        name: String,
        annotation: Option<CoreType>,
        expr: CoreExpr,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoreType {
    Spanned(Box<CoreType>, std::ops::Range<usize>),
    Int,
    Float,
    Bool,
    String,
    LiteralUnion(Vec<String>),
    Option(Box<CoreType>),
    List(Box<CoreType>),
    Record(BTreeMap<String, CoreType>),
    Refinement {
        binder: String,
        base: Box<CoreType>,
        pred: Box<CoreExpr>,
    },
    Function(Box<CoreType>, Box<CoreType>),
    Alias(String),
    ResolvedAlias(TypeAliasRef),
}

impl CoreType {
    pub fn origin_span(&self) -> Option<std::ops::Range<usize>> {
        match self {
            CoreType::Spanned(_, span) => Some(span.clone()),
            _ => None,
        }
    }

    pub fn as_unspanned(&self) -> &CoreType {
        match self {
            CoreType::Spanned(ty, _) => ty.as_unspanned(),
            ty => ty,
        }
    }

    pub fn alias_origin(&self, needle: &str) -> Option<std::ops::Range<usize>> {
        match self {
            CoreType::Spanned(ty, span) if matches!(ty.as_unspanned(), CoreType::Alias(name) if name == needle) => {
                Some(span.clone())
            }
            CoreType::Spanned(ty, _) => ty.alias_origin(needle),
            CoreType::Option(inner) | CoreType::List(inner) => inner.alias_origin(needle),
            CoreType::Record(fields) => fields.values().find_map(|ty| ty.alias_origin(needle)),
            CoreType::Refinement { base, .. } => base.alias_origin(needle),
            CoreType::Function(input, output) => input
                .alias_origin(needle)
                .or_else(|| output.alias_origin(needle)),
            CoreType::Alias(_)
            | CoreType::Int
            | CoreType::Float
            | CoreType::Bool
            | CoreType::String
            | CoreType::LiteralUnion(_)
            | CoreType::ResolvedAlias(_) => None,
        }
    }

    pub fn resolved_alias_origin(&self, needle: TypeAliasRef) -> Option<std::ops::Range<usize>> {
        match self {
            CoreType::Spanned(ty, span) if matches!(ty.as_unspanned(), CoreType::ResolvedAlias(alias) if *alias == needle) => {
                Some(span.clone())
            }
            CoreType::Spanned(ty, _) => ty.resolved_alias_origin(needle),
            CoreType::Option(inner) | CoreType::List(inner) => inner.resolved_alias_origin(needle),
            CoreType::Record(fields) => fields
                .values()
                .find_map(|ty| ty.resolved_alias_origin(needle)),
            CoreType::Refinement { base, .. } => base.resolved_alias_origin(needle),
            CoreType::Function(input, output) => input
                .resolved_alias_origin(needle)
                .or_else(|| output.resolved_alias_origin(needle)),
            CoreType::ResolvedAlias(_)
            | CoreType::Alias(_)
            | CoreType::Int
            | CoreType::Float
            | CoreType::Bool
            | CoreType::String
            | CoreType::LiteralUnion(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoreExpr {
    Spanned(Box<CoreExpr>, std::ops::Range<usize>),
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    None,
    Some(Box<CoreExpr>),
    Var(String),
    Local(LocalRef),
    Global(GlobalRef),
    List(Vec<CoreExpr>),
    Record(BTreeMap<String, CoreExpr>),
    Field(Box<CoreExpr>, String),
    If(Box<CoreExpr>, Box<CoreExpr>, Box<CoreExpr>),
    Let(String, Option<CoreType>, Box<CoreExpr>, Box<CoreExpr>),
    Lambda(String, CoreType, Box<CoreExpr>),
    Apply(Box<CoreExpr>, Box<CoreExpr>),
    Ascribe(Box<CoreExpr>, CoreType),
    Unary(String, Box<CoreExpr>),
    Binary(String, Box<CoreExpr>, Box<CoreExpr>),
}

impl CoreExpr {
    pub fn origin_span(&self) -> Option<std::ops::Range<usize>> {
        match self {
            CoreExpr::Spanned(_, span) => Some(span.clone()),
            _ => None,
        }
    }
}

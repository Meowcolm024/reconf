use crate::core::{CoreDecl, CoreExpr, CoreImport, CoreModule, CoreType};
use crate::syntax::surface::{Decl, Expr, FileAst, StrPart, Type};

#[derive(Default)]
pub struct SurfaceToCoreLowerer {
    modules: ModuleLowerer,
    types: TypeLowerer,
    exprs: ExprLowerer,
}

impl SurfaceToCoreLowerer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lower_file(&mut self, file: FileAst) -> CoreModule {
        self.modules.lower(file)
    }

    pub fn lower_type(&mut self, ty: Type) -> CoreType {
        self.types.lower(ty)
    }

    pub fn lower_expr(&mut self, expr: Expr) -> CoreExpr {
        self.exprs.lower(expr)
    }
}

#[derive(Default)]
struct ModuleLowerer {
    declarations: DeclLowerer,
    output: ExprLowerer,
}

impl ModuleLowerer {
    fn lower(&mut self, file: FileAst) -> CoreModule {
        let mut imports = Vec::new();
        let mut decls = Vec::new();

        for decl in file.decls {
            match self.declarations.lower(decl) {
                LoweredDecl::Import(import) => imports.push(import),
                LoweredDecl::Decl(decl) => decls.push(decl),
            }
        }

        CoreModule {
            imports,
            decls,
            output: file.output.map(|expr| self.output.lower(expr)),
        }
    }
}

#[derive(Default)]
struct DeclLowerer {
    types: TypeLowerer,
    exprs: ExprLowerer,
}

impl DeclLowerer {
    fn lower(&mut self, decl: Decl) -> LoweredDecl {
        match decl {
            Decl::Import { path, names } => LoweredDecl::Import(CoreImport { path, names }),
            Decl::Native { export, name, ty } => LoweredDecl::Decl(CoreDecl::Native {
                export,
                name,
                ty: self.types.lower(ty),
            }),
            Decl::Type { export, name, ty } => LoweredDecl::Decl(CoreDecl::Type {
                export,
                name,
                ty: self.types.lower(ty),
            }),
            Decl::Let {
                export,
                name,
                annotation,
                expr,
            } => LoweredDecl::Decl(CoreDecl::Let {
                export,
                name,
                annotation: annotation.map(|ty| self.types.lower(ty)),
                expr: self.exprs.lower(expr),
            }),
        }
    }
}

#[derive(Default)]
struct TypeLowerer;

impl TypeLowerer {
    fn lower(&mut self, ty: Type) -> CoreType {
        match ty {
            Type::Spanned(ty, span) => CoreType::Spanned(Box::new(self.lower(*ty)), span),
            Type::Int => CoreType::Int,
            Type::Float => CoreType::Float,
            Type::Bool => CoreType::Bool,
            Type::String => CoreType::String,
            Type::Option(inner) => CoreType::Option(Box::new(self.lower(*inner))),
            Type::List(inner) => CoreType::List(Box::new(self.lower(*inner))),
            Type::Record(fields) => CoreType::Record(
                fields
                    .into_iter()
                    .map(|(name, ty)| (name, self.lower(ty)))
                    .collect(),
            ),
            Type::LiteralUnion(choices) => CoreType::LiteralUnion(choices),
            Type::Refinement { binder, base, pred } => CoreType::Refinement {
                binder,
                base: Box::new(self.lower(*base)),
                pred: Box::new(ExprLowerer::default().lower(*pred)),
            },
            Type::Function(input, output) => {
                CoreType::Function(Box::new(self.lower(*input)), Box::new(self.lower(*output)))
            }
            Type::Alias(name) => CoreType::Alias(name),
        }
    }
}

#[derive(Default)]
struct ExprLowerer {
    types: TypeLowerer,
}

impl ExprLowerer {
    fn lower(&mut self, expr: Expr) -> CoreExpr {
        match expr {
            Expr::Spanned(expr, span) => CoreExpr::Spanned(Box::new(self.lower(*expr)), span),
            Expr::Int(value) => CoreExpr::Int(value),
            Expr::Float(value) => CoreExpr::Float(value),
            Expr::Bool(value) => CoreExpr::Bool(value),
            Expr::String(value) => CoreExpr::String(value),
            Expr::Interp(parts) => InterpolationLowerer::new(self).lower(parts),
            Expr::None => CoreExpr::None,
            Expr::Some(expr) => CoreExpr::Some(Box::new(self.lower(*expr))),
            Expr::Var(name) => CoreExpr::Var(name),
            Expr::List(items) => {
                CoreExpr::List(items.into_iter().map(|item| self.lower(item)).collect())
            }
            Expr::Record(fields) => CoreExpr::Record(
                fields
                    .into_iter()
                    .map(|(name, expr)| (name, self.lower(expr)))
                    .collect(),
            ),
            Expr::Field(expr, name) | Expr::Dot(expr, name) => {
                CoreExpr::Field(Box::new(self.lower(*expr)), name)
            }
            Expr::If(cond, then_expr, else_expr) => CoreExpr::If(
                Box::new(self.lower(*cond)),
                Box::new(self.lower(*then_expr)),
                Box::new(self.lower(*else_expr)),
            ),
            Expr::Let(name, annotation, value, body) => CoreExpr::Let(
                name,
                annotation.map(|ty| self.types.lower(ty)),
                Box::new(self.lower(*value)),
                Box::new(self.lower(*body)),
            ),
            Expr::Lambda(param, ty, body) => {
                CoreExpr::Lambda(param, self.types.lower(ty), Box::new(self.lower(*body)))
            }
            Expr::Apply(function, arg) => {
                CoreExpr::Apply(Box::new(self.lower(*function)), Box::new(self.lower(*arg)))
            }
            Expr::Ascribe(expr, ty) => {
                CoreExpr::Ascribe(Box::new(self.lower(*expr)), self.types.lower(ty))
            }
            Expr::Unary(op, expr) => CoreExpr::Unary(op, Box::new(self.lower(*expr))),
            Expr::Binary(op, left, right) => CoreExpr::Binary(
                op,
                Box::new(self.lower(*left)),
                Box::new(self.lower(*right)),
            ),
        }
    }
}

struct InterpolationLowerer<'a> {
    exprs: &'a mut ExprLowerer,
}

impl<'a> InterpolationLowerer<'a> {
    fn new(exprs: &'a mut ExprLowerer) -> Self {
        Self { exprs }
    }

    fn lower(&mut self, parts: Vec<StrPart>) -> CoreExpr {
        let mut exprs = parts.into_iter().filter_map(|part| match part {
            StrPart::Text(text) if text.is_empty() => None,
            StrPart::Text(text) => Some(CoreExpr::String(text)),
            StrPart::Expr(expr) => Some(CoreExpr::Apply(
                Box::new(CoreExpr::Var("show".to_string())),
                Box::new(self.exprs.lower(expr)),
            )),
        });

        let Some(first) = exprs.next() else {
            return CoreExpr::String(String::new());
        };

        exprs.fold(first, |left, right| {
            CoreExpr::Binary("++".to_string(), Box::new(left), Box::new(right))
        })
    }
}

enum LoweredDecl {
    Import(CoreImport),
    Decl(CoreDecl),
}

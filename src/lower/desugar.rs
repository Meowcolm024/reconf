use crate::syntax::surface::{Decl, Expr, FileAst, StrPart, Type};

pub fn lower_file(file: FileAst) -> FileAst {
    FileAst {
        decls: file.decls.into_iter().map(lower_decl).collect(),
        output: lower_expr(file.output),
    }
}

fn lower_decl(decl: Decl) -> Decl {
    match decl {
        Decl::Import { path, names } => Decl::Import { path, names },
        Decl::Native { export, name, ty } => Decl::Native {
            export,
            name,
            ty: lower_type(ty),
        },
        Decl::Type { export, name, ty } => Decl::Type {
            export,
            name,
            ty: lower_type(ty),
        },
        Decl::Let {
            export,
            name,
            annotation,
            expr,
        } => Decl::Let {
            export,
            name,
            annotation: annotation.map(lower_type),
            expr: lower_expr(expr),
        },
    }
}

fn lower_type(ty: Type) -> Type {
    match ty {
        Type::Option(inner) => Type::Option(Box::new(lower_type(*inner))),
        Type::List(inner) => Type::List(Box::new(lower_type(*inner))),
        Type::Record(fields) => Type::Record(
            fields
                .into_iter()
                .map(|(name, ty)| (name, lower_type(ty)))
                .collect(),
        ),
        Type::Refinement { binder, base, pred } => Type::Refinement {
            binder,
            base: Box::new(lower_type(*base)),
            pred: Box::new(lower_expr(*pred)),
        },
        Type::Function(input, output) => {
            Type::Function(Box::new(lower_type(*input)), Box::new(lower_type(*output)))
        }
        ty => ty,
    }
}

fn lower_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Interp(parts) => lower_interpolation(parts),
        Expr::Some(expr) => Expr::Some(Box::new(lower_expr(*expr))),
        Expr::List(items) => Expr::List(items.into_iter().map(lower_expr).collect()),
        Expr::Record(fields) => Expr::Record(
            fields
                .into_iter()
                .map(|(name, expr)| (name, lower_expr(expr)))
                .collect(),
        ),
        Expr::Field(expr, name) => Expr::Field(Box::new(lower_expr(*expr)), name),
        Expr::Dot(expr, name) => Expr::Dot(Box::new(lower_expr(*expr)), name),
        Expr::If(cond, then_expr, else_expr) => Expr::If(
            Box::new(lower_expr(*cond)),
            Box::new(lower_expr(*then_expr)),
            Box::new(lower_expr(*else_expr)),
        ),
        Expr::Let(name, annotation, value, body) => Expr::Let(
            name,
            annotation.map(lower_type),
            Box::new(lower_expr(*value)),
            Box::new(lower_expr(*body)),
        ),
        Expr::Lambda(param, ty, body) => {
            Expr::Lambda(param, lower_type(ty), Box::new(lower_expr(*body)))
        }
        Expr::Apply(function, arg) => {
            Expr::Apply(Box::new(lower_expr(*function)), Box::new(lower_expr(*arg)))
        }
        Expr::Ascribe(expr, ty) => Expr::Ascribe(Box::new(lower_expr(*expr)), lower_type(ty)),
        Expr::Unary(op, expr) => Expr::Unary(op, Box::new(lower_expr(*expr))),
        Expr::Binary(op, left, right) => Expr::Binary(
            op,
            Box::new(lower_expr(*left)),
            Box::new(lower_expr(*right)),
        ),
        expr => expr,
    }
}

fn lower_interpolation(parts: Vec<StrPart>) -> Expr {
    let mut exprs = parts.into_iter().filter_map(|part| match part {
        StrPart::Text(text) if text.is_empty() => None,
        StrPart::Text(text) => Some(Expr::String(text)),
        StrPart::Expr(expr) => Some(Expr::Apply(
            Box::new(Expr::Var("show".to_string())),
            Box::new(lower_expr(expr)),
        )),
    });

    let Some(first) = exprs.next() else {
        return Expr::String(String::new());
    };

    exprs.fold(first, |left, right| {
        Expr::Binary("++".to_string(), Box::new(left), Box::new(right))
    })
}

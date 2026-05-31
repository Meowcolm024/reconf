use crate::diagnostic::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    Ident(String),
    Int(i64),
    Float(f64),
    String(Vec<StringPart>),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Colon,
    Semi,
    Comma,
    Dot,
    Question,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    PlusPlus,
    Eq,
    EqEq,
    Bang,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AndAnd,
    OrOr,
    Pipe,
    Arrow,
    FatArrow,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
enum StringPart {
    Text(String),
    Expr(String),
}

#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    span: Span,
}

struct Lexer<'a> {
    file_id: usize,
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(file_id: usize, input: &'a str) -> Self {
        Self {
            file_id,
            input,
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn tokenize(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            self.skip_ws_and_comments();
            let start = self.pos;
            let kind = match self.peek() {
                None => TokenKind::Eof,
                Some(b'(') => {
                    self.pos += 1;
                    TokenKind::LParen
                }
                Some(b')') => {
                    self.pos += 1;
                    TokenKind::RParen
                }
                Some(b'{') => {
                    self.pos += 1;
                    TokenKind::LBrace
                }
                Some(b'}') => {
                    self.pos += 1;
                    TokenKind::RBrace
                }
                Some(b'[') => {
                    self.pos += 1;
                    TokenKind::LBracket
                }
                Some(b']') => {
                    self.pos += 1;
                    TokenKind::RBracket
                }
                Some(b':') => {
                    self.pos += 1;
                    TokenKind::Colon
                }
                Some(b';') => {
                    self.pos += 1;
                    TokenKind::Semi
                }
                Some(b',') => {
                    self.pos += 1;
                    TokenKind::Comma
                }
                Some(b'.') => {
                    self.pos += 1;
                    TokenKind::Dot
                }
                Some(b'?') => {
                    self.pos += 1;
                    TokenKind::Question
                }
                Some(b'+') if self.next_is(b'+') => {
                    self.pos += 2;
                    TokenKind::PlusPlus
                }
                Some(b'+') => {
                    self.pos += 1;
                    TokenKind::Plus
                }
                Some(b'-') if self.next_is(b'>') => {
                    self.pos += 2;
                    TokenKind::Arrow
                }
                Some(b'-') => {
                    self.pos += 1;
                    TokenKind::Minus
                }
                Some(b'*') => {
                    self.pos += 1;
                    TokenKind::Star
                }
                Some(b'/') => {
                    self.pos += 1;
                    TokenKind::Slash
                }
                Some(b'%') => {
                    self.pos += 1;
                    TokenKind::Percent
                }
                Some(b'=') if self.next_is(b'=') => {
                    self.pos += 2;
                    TokenKind::EqEq
                }
                Some(b'=') if self.next_is(b'>') => {
                    self.pos += 2;
                    TokenKind::FatArrow
                }
                Some(b'=') => {
                    self.pos += 1;
                    TokenKind::Eq
                }
                Some(b'!') if self.next_is(b'=') => {
                    self.pos += 2;
                    TokenKind::BangEq
                }
                Some(b'!') => {
                    self.pos += 1;
                    TokenKind::Bang
                }
                Some(b'<') if self.next_is(b'=') => {
                    self.pos += 2;
                    TokenKind::LtEq
                }
                Some(b'<') => {
                    self.pos += 1;
                    TokenKind::Lt
                }
                Some(b'>') if self.next_is(b'=') => {
                    self.pos += 2;
                    TokenKind::GtEq
                }
                Some(b'>') => {
                    self.pos += 1;
                    TokenKind::Gt
                }
                Some(b'&') if self.next_is(b'&') => {
                    self.pos += 2;
                    TokenKind::AndAnd
                }
                Some(b'|') if self.next_is(b'|') => {
                    self.pos += 2;
                    TokenKind::OrOr
                }
                Some(b'|') => {
                    self.pos += 1;
                    TokenKind::Pipe
                }
                Some(b'"') => self.lex_string()?,
                Some(ch) if ch.is_ascii_digit() => self.lex_number()?,
                Some(ch) if is_ident_start(ch) || is_upper_start(ch) => self.lex_ident(),
                Some(ch) => {
                    return Err(Diagnostic::new(
                        "E_PARSE_001",
                        format!("unexpected character `{}`", ch as char),
                        Span::new(self.file_id, start, start + 1),
                    ));
                }
            };
            let end = self.pos;
            tokens.push(Token {
                kind: kind.clone(),
                span: Span::new(self.file_id, start, end),
            });
            if matches!(kind, TokenKind::Eof) {
                break;
            }
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn next_is(&self, expected: u8) -> bool {
        self.bytes.get(self.pos + 1).copied() == Some(expected)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.pos += 1;
            }
            if self.peek() == Some(b'#') {
                while let Some(ch) = self.peek() {
                    self.pos += 1;
                    if ch == b'\n' {
                        break;
                    }
                }
                continue;
            }
            break;
        }
    }

    fn lex_number(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
            self.pos += 1;
        }
        let is_float = if self.peek() == Some(b'.')
            && matches!(self.bytes.get(self.pos + 1), Some(ch) if ch.is_ascii_digit())
        {
            self.pos += 1;
            while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
                self.pos += 1;
            }
            true
        } else {
            false
        };
        let text = &self.input[start..self.pos];
        if is_float {
            text.parse::<f64>().map(TokenKind::Float).map_err(|_| {
                Diagnostic::new(
                    "E_PARSE_002",
                    "invalid float literal",
                    Span::new(self.file_id, start, self.pos),
                )
            })
        } else {
            text.parse::<i64>().map(TokenKind::Int).map_err(|_| {
                Diagnostic::new(
                    "E_PARSE_003",
                    "invalid integer literal",
                    Span::new(self.file_id, start, self.pos),
                )
            })
        }
    }

    fn lex_ident(&mut self) -> TokenKind {
        let start = self.pos;
        self.pos += 1;
        while matches!(self.peek(), Some(ch) if is_ident_rest(ch)) {
            self.pos += 1;
        }
        TokenKind::Ident(self.input[start..self.pos].to_string())
    }

    fn lex_string(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        self.pos += 1;
        let mut text = String::new();
        let mut parts = Vec::new();
        while let Some(ch) = self.peek() {
            match ch {
                b'"' => {
                    self.pos += 1;
                    if !text.is_empty() {
                        parts.push(StringPart::Text(std::mem::take(&mut text)));
                    }
                    return Ok(TokenKind::String(parts));
                }
                b'\\' => {
                    self.pos += 1;
                    let Some(escaped) = self.peek() else {
                        return Err(Diagnostic::new(
                            "E_PARSE_004",
                            "unterminated string escape",
                            Span::new(self.file_id, start, self.pos),
                        ));
                    };
                    self.pos += 1;
                    let decoded = match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'{' => '{',
                        b'}' => '}',
                        other => {
                            return Err(Diagnostic::new(
                                "E_PARSE_005",
                                format!("unknown string escape `\\{}`", other as char),
                                Span::new(self.file_id, self.pos - 2, self.pos),
                            ));
                        }
                    };
                    text.push(decoded);
                }
                b'{' => {
                    if !text.is_empty() {
                        parts.push(StringPart::Text(std::mem::take(&mut text)));
                    }
                    self.pos += 1;
                    let expr_start = self.pos;
                    let mut depth = 1usize;
                    let mut in_string = false;
                    let mut escaped = false;
                    while let Some(inner) = self.peek() {
                        self.pos += 1;
                        if in_string {
                            if escaped {
                                escaped = false;
                            } else if inner == b'\\' {
                                escaped = true;
                            } else if inner == b'"' {
                                in_string = false;
                            }
                            continue;
                        }
                        match inner {
                            b'"' => in_string = true,
                            b'{' => depth += 1,
                            b'}' => {
                                depth -= 1;
                                if depth == 0 {
                                    let expr_end = self.pos - 1;
                                    let expr_text =
                                        self.input[expr_start..expr_end].trim().to_string();
                                    if expr_text.is_empty() {
                                        return Err(Diagnostic::new(
                                            "E_PARSE_006",
                                            "empty interpolation",
                                            Span::new(
                                                self.file_id,
                                                expr_start.saturating_sub(1),
                                                self.pos,
                                            ),
                                        ));
                                    }
                                    parts.push(StringPart::Expr(expr_text));
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if depth != 0 {
                        return Err(Diagnostic::new(
                            "E_PARSE_007",
                            "unterminated interpolation",
                            Span::new(self.file_id, expr_start.saturating_sub(1), self.pos),
                        ));
                    }
                }
                b'\n' | b'\r' => {
                    return Err(Diagnostic::new(
                        "E_PARSE_008",
                        "unterminated string literal",
                        Span::new(self.file_id, start, self.pos),
                    ));
                }
                _ => {
                    text.push(ch as char);
                    self.pos += 1;
                }
            }
        }
        Err(Diagnostic::new(
            "E_PARSE_009",
            "unterminated string literal",
            Span::new(self.file_id, start, self.pos),
        ))
    }
}

fn is_ident_start(ch: u8) -> bool {
    ch.is_ascii_lowercase() || ch == b'_'
}

fn is_upper_start(ch: u8) -> bool {
    ch.is_ascii_uppercase()
}

fn is_ident_rest(ch: u8) -> bool {
    is_ident_start(ch) || is_upper_start(ch) || ch.is_ascii_digit() || ch == b'-'
}

#[derive(Clone, Debug)]
pub(crate) struct FileAst {
    pub(crate) imports: Vec<ImportDecl>,
    pub(crate) decls: Vec<TopDecl>,
    pub(crate) output: Expr,
}

#[derive(Clone, Debug)]
pub(crate) struct ImportDecl {
    pub(crate) path: String,
    pub(crate) names: Vec<String>,
    pub(crate) span: Span,
}

#[derive(Clone, Debug)]
pub(crate) enum TopDecl {
    Type {
        export: bool,
        name: String,
        ty: Ty,
        span: Span,
    },
    Let {
        export: bool,
        name: String,
        ann: Option<Ty>,
        value: Expr,
        span: Span,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Ty {
    pub(crate) kind: TyKind,
    pub(crate) span: Span,
}

#[derive(Clone, Debug)]
pub(crate) enum TyKind {
    Int,
    Float,
    Bool,
    String,
    Option(Box<Ty>),
    List(Box<Ty>),
    Record(Vec<FieldTy>),
    Refine {
        binder: String,
        base: Box<Ty>,
        pred: Box<Expr>,
    },
    Fun(Box<Ty>, Box<Ty>),
    Alias(String),
    LiteralUnion(Vec<String>),
    Builtin(String),
}

#[derive(Clone, Debug)]
pub(crate) struct FieldTy {
    pub(crate) name: String,
    pub(crate) ty: Ty,
    pub(crate) span: Span,
}

#[derive(Clone, Debug)]
pub(crate) struct Expr {
    pub(crate) kind: ExprKind,
    pub(crate) span: Span,
}

#[derive(Clone, Debug)]
pub(crate) enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Interp(Vec<InterpPart>),
    Var(String),
    None,
    Some(Box<Expr>),
    List(Vec<Expr>),
    Record(Vec<FieldExpr>),
    Field(Box<Expr>, String),
    If {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
    Let {
        name: String,
        ann: Option<Ty>,
        value: Box<Expr>,
        body: Box<Expr>,
    },
    Lam {
        param: String,
        param_ty: Ty,
        body: Box<Expr>,
    },
    App(Box<Expr>, Box<Expr>),
    Ascribe(Box<Expr>, Ty),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
}

#[derive(Clone, Debug)]
pub(crate) enum InterpPart {
    Text(String),
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub(crate) struct FieldExpr {
    pub(crate) name: String,
    pub(crate) value: Expr,
    pub(crate) span: Span,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum UnaryOp {
    Not,
    Neg,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

pub(crate) struct Parser {
    file_id: usize,
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub(crate) fn parse_file(file_id: usize, input: &str) -> Result<FileAst, Diagnostic> {
        let tokens = Lexer::new(file_id, input).tokenize()?;
        let mut parser = Self {
            file_id,
            tokens,
            pos: 0,
        };
        parser.file()
    }

    fn parse_embedded_expr(file_id: usize, input: &str, span: Span) -> Result<Expr, Diagnostic> {
        let tokens = Lexer::new(file_id, input).tokenize()?;
        let mut parser = Self {
            file_id,
            tokens,
            pos: 0,
        };
        let expr = parser.expr()?;
        if !parser.at_eof() {
            return Err(Diagnostic::new(
                "E_PARSE_010",
                "unexpected token after interpolation expression",
                parser.current().span,
            ));
        }
        Ok(Expr { span, ..expr })
    }

    fn file(&mut self) -> Result<FileAst, Diagnostic> {
        let mut imports = Vec::new();
        let mut decls = Vec::new();
        while self.peek_keyword("import") {
            imports.push(self.import_decl()?);
        }
        loop {
            let export = if self.peek_keyword("export") {
                self.bump();
                true
            } else {
                false
            };
            if self.peek_keyword("type") {
                decls.push(self.type_decl(export)?);
            } else if self.peek_keyword("let") && self.looks_like_top_let_decl() {
                decls.push(self.top_let_decl(export)?);
            } else if export {
                return Err(Diagnostic::new(
                    "E_PARSE_011",
                    "`export` must be followed by `type` or a top-level `let`",
                    self.current().span,
                ));
            } else {
                break;
            }
        }
        let output = self.expr()?;
        self.expect(TokenKind::Eof, "expected end of file")?;
        Ok(FileAst {
            imports,
            decls,
            output,
        })
    }

    fn looks_like_top_let_decl(&self) -> bool {
        let mut depth = 0usize;
        for token in &self.tokens[self.pos..] {
            match token.kind {
                TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => depth += 1,
                TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                    depth = depth.saturating_sub(1)
                }
                TokenKind::Semi if depth == 0 => return true,
                TokenKind::Eof => return false,
                _ => {}
            }
        }
        false
    }

    fn import_decl(&mut self) -> Result<ImportDecl, Diagnostic> {
        let start = self.expect_keyword("import")?.span;
        let path_token = self.bump().clone();
        let path = match path_token.kind {
            TokenKind::String(parts) => literal_string(parts, path_token.span)?,
            _ => {
                return Err(Diagnostic::new(
                    "E_PARSE_012",
                    "expected import path string",
                    path_token.span,
                ));
            }
        };
        self.expect(TokenKind::Colon, "expected `:` after import path")?;
        let mut names = Vec::new();
        loop {
            names.push(self.expect_ident("expected imported name")?);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let end = self
            .expect(TokenKind::Semi, "expected `;` after import")?
            .span;
        Ok(ImportDecl {
            path,
            names,
            span: start.join(end),
        })
    }

    fn type_decl(&mut self, export: bool) -> Result<TopDecl, Diagnostic> {
        let start = self.expect_keyword("type")?.span;
        let name = self.expect_type_name("expected type name")?;
        self.expect(TokenKind::Eq, "expected `=` in type declaration")?;
        let ty = self.ty()?;
        let end = self
            .expect(TokenKind::Semi, "expected `;` after type declaration")?
            .span;
        Ok(TopDecl::Type {
            export,
            name,
            ty,
            span: start.join(end),
        })
    }

    fn top_let_decl(&mut self, export: bool) -> Result<TopDecl, Diagnostic> {
        let start = self.expect_keyword("let")?.span;
        let name = self.expect_value_name("expected value name")?;
        let ann = if self.eat(TokenKind::Colon) {
            Some(self.ty()?)
        } else {
            None
        };
        self.expect(TokenKind::Eq, "expected `=` in let declaration")?;
        let value = self.expr()?;
        let end = self
            .expect(TokenKind::Semi, "expected `;` after let declaration")?
            .span;
        Ok(TopDecl::Let {
            export,
            name,
            ann,
            value,
            span: start.join(end),
        })
    }

    fn ty(&mut self) -> Result<Ty, Diagnostic> {
        self.fun_ty()
    }

    fn fun_ty(&mut self) -> Result<Ty, Diagnostic> {
        let left = self.postfix_ty()?;
        if self.eat(TokenKind::Arrow) {
            let right = self.fun_ty()?;
            let span = left.span.join(right.span);
            Ok(Ty {
                kind: TyKind::Fun(Box::new(left), Box::new(right)),
                span,
            })
        } else {
            Ok(left)
        }
    }

    fn postfix_ty(&mut self) -> Result<Ty, Diagnostic> {
        let mut ty = self.primary_ty()?;
        while self.eat(TokenKind::Question) {
            let q_span = self.previous().span;
            ty = Ty {
                span: ty.span.join(q_span),
                kind: TyKind::Option(Box::new(ty)),
            };
        }
        Ok(ty)
    }

    fn primary_ty(&mut self) -> Result<Ty, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Ident(name) if name == "Int" => Ok(Ty {
                kind: TyKind::Int,
                span: token.span,
            }),
            TokenKind::Ident(name) if name == "Float" => Ok(Ty {
                kind: TyKind::Float,
                span: token.span,
            }),
            TokenKind::Ident(name) if name == "Bool" => Ok(Ty {
                kind: TyKind::Bool,
                span: token.span,
            }),
            TokenKind::Ident(name) if name == "String" => Ok(Ty {
                kind: TyKind::String,
                span: token.span,
            }),
            TokenKind::Ident(name) if starts_upper(&name) => Ok(Ty {
                kind: TyKind::Alias(name),
                span: token.span,
            }),
            TokenKind::String(parts) => {
                let mut literals = vec![literal_string(parts, token.span)?];
                let mut span = token.span;
                while self.eat(TokenKind::Pipe) {
                    let lit_token = self.bump().clone();
                    let TokenKind::String(parts) = lit_token.kind else {
                        return Err(Diagnostic::new(
                            "E_PARSE_013",
                            "expected string literal in literal union",
                            lit_token.span,
                        ));
                    };
                    span = span.join(lit_token.span);
                    literals.push(literal_string(parts, lit_token.span)?);
                }
                Ok(Ty {
                    kind: TyKind::LiteralUnion(literals),
                    span,
                })
            }
            TokenKind::LBracket => {
                let inner = self.ty()?;
                let end = self
                    .expect(TokenKind::RBracket, "expected `]` after list type")?
                    .span;
                Ok(Ty {
                    span: token.span.join(end),
                    kind: TyKind::List(Box::new(inner)),
                })
            }
            TokenKind::LParen => {
                let ty = self.ty()?;
                self.expect(TokenKind::RParen, "expected `)` after type")?;
                Ok(ty)
            }
            TokenKind::LBrace => self.brace_ty(token.span),
            _ => Err(Diagnostic::new("E_PARSE_014", "expected type", token.span)),
        }
    }

    fn brace_ty(&mut self, start: Span) -> Result<Ty, Diagnostic> {
        if self.eat(TokenKind::RBrace) {
            return Ok(Ty {
                kind: TyKind::Record(Vec::new()),
                span: start.join(self.previous().span),
            });
        }

        let name = self.expect_ident("expected field or refinement binder")?;
        let name_span = self.previous().span;
        self.expect(
            TokenKind::Colon,
            "expected `:` in record or refinement type",
        )?;
        let first_ty = self.ty()?;

        if self.eat(TokenKind::Pipe) {
            let pred = self.expr()?;
            let end = self
                .expect(TokenKind::RBrace, "expected `}` after refinement type")?
                .span;
            return Ok(Ty {
                span: start.join(end),
                kind: TyKind::Refine {
                    binder: name,
                    base: Box::new(first_ty),
                    pred: Box::new(pred),
                },
            });
        }

        let mut fields = vec![FieldTy {
            name,
            ty: first_ty,
            span: name_span,
        }];
        while self.eat(TokenKind::Comma) {
            if self.peek(TokenKind::RBrace) {
                break;
            }
            let name = self.expect_ident("expected field name")?;
            let span = self.previous().span;
            self.expect(TokenKind::Colon, "expected `:` after field name")?;
            let ty = self.ty()?;
            fields.push(FieldTy { name, ty, span });
        }
        let end = self
            .expect(TokenKind::RBrace, "expected `}` after record type")?
            .span;
        Ok(Ty {
            kind: TyKind::Record(fields),
            span: start.join(end),
        })
    }

    fn expr(&mut self) -> Result<Expr, Diagnostic> {
        if self.peek_keyword("let") {
            return self.let_expr();
        }
        self.ascription_expr()
    }

    fn let_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.expect_keyword("let")?.span;
        let name = self.expect_value_name("expected local binding name")?;
        let ann = if self.eat(TokenKind::Colon) {
            Some(self.ty()?)
        } else {
            None
        };
        self.expect(TokenKind::Eq, "expected `=` in let expression")?;
        let value = self.expr()?;
        self.expect_keyword("in")?;
        let body = self.expr()?;
        let span = start.join(body.span);
        Ok(Expr {
            kind: ExprKind::Let {
                name,
                ann,
                value: Box::new(value),
                body: Box::new(body),
            },
            span,
        })
    }

    fn ascription_expr(&mut self) -> Result<Expr, Diagnostic> {
        let expr = self.plain_expr()?;
        if self.eat(TokenKind::Colon) {
            let ty = self.ty()?;
            let span = expr.span.join(ty.span);
            Ok(Expr {
                kind: ExprKind::Ascribe(Box::new(expr), ty),
                span,
            })
        } else {
            Ok(expr)
        }
    }

    fn plain_expr(&mut self) -> Result<Expr, Diagnostic> {
        if self.starts_lambda() {
            self.lambda_expr()
        } else if self.peek_keyword("if") {
            self.if_expr()
        } else {
            self.logic_or()
        }
    }

    fn starts_lambda(&self) -> bool {
        if !self.peek(TokenKind::LParen) {
            return false;
        }
        let mut depth = 0usize;
        for token in &self.tokens[self.pos..] {
            match token.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return self
                            .tokens
                            .get(token_index(&self.tokens, token) + 1)
                            .map(|next| matches!(next.kind, TokenKind::FatArrow))
                            .unwrap_or(false);
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
        }
        false
    }

    fn lambda_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.expect(TokenKind::LParen, "expected `(`")?.span;
        let param = self.expect_value_name("expected lambda parameter")?;
        self.expect(TokenKind::Colon, "expected `:` after lambda parameter")?;
        let param_ty = self.ty()?;
        self.expect(TokenKind::RParen, "expected `)` after lambda parameter")?;
        self.expect(TokenKind::FatArrow, "expected `=>` after lambda parameter")?;
        let body = self.expr()?;
        Ok(Expr {
            span: start.join(body.span),
            kind: ExprKind::Lam {
                param,
                param_ty,
                body: Box::new(body),
            },
        })
    }

    fn if_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.expect_keyword("if")?.span;
        let cond = self.expr()?;
        self.expect_keyword("then")?;
        let then_expr = self.expr()?;
        self.expect_keyword("else")?;
        let else_expr = self.expr()?;
        Ok(Expr {
            span: start.join(else_expr.span),
            kind: ExprKind::If {
                cond: Box::new(cond),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            },
        })
    }

    fn logic_or(&mut self) -> Result<Expr, Diagnostic> {
        self.binary_left(Self::logic_and, &[TokenKind::OrOr], &[BinaryOp::Or])
    }

    fn logic_and(&mut self) -> Result<Expr, Diagnostic> {
        self.binary_left(Self::equality, &[TokenKind::AndAnd], &[BinaryOp::And])
    }

    fn equality(&mut self) -> Result<Expr, Diagnostic> {
        self.binary_left(
            Self::relation,
            &[TokenKind::EqEq, TokenKind::BangEq],
            &[BinaryOp::Eq, BinaryOp::Ne],
        )
    }

    fn relation(&mut self) -> Result<Expr, Diagnostic> {
        self.binary_left(
            Self::additive,
            &[
                TokenKind::Lt,
                TokenKind::LtEq,
                TokenKind::Gt,
                TokenKind::GtEq,
            ],
            &[BinaryOp::Lt, BinaryOp::Le, BinaryOp::Gt, BinaryOp::Ge],
        )
    }

    fn additive(&mut self) -> Result<Expr, Diagnostic> {
        self.binary_left(
            Self::multiplicative,
            &[TokenKind::Plus, TokenKind::Minus, TokenKind::PlusPlus],
            &[BinaryOp::Add, BinaryOp::Sub, BinaryOp::Concat],
        )
    }

    fn multiplicative(&mut self) -> Result<Expr, Diagnostic> {
        self.binary_left(
            Self::unary,
            &[TokenKind::Star, TokenKind::Slash, TokenKind::Percent],
            &[BinaryOp::Mul, BinaryOp::Div, BinaryOp::Mod],
        )
    }

    fn binary_left(
        &mut self,
        next: fn(&mut Self) -> Result<Expr, Diagnostic>,
        tokens: &[TokenKind],
        ops: &[BinaryOp],
    ) -> Result<Expr, Diagnostic> {
        let mut expr = next(self)?;
        loop {
            let mut matched = None;
            for (idx, token) in tokens.iter().enumerate() {
                if self.peek(token.clone()) {
                    matched = Some((idx, self.current().span));
                    break;
                }
            }
            let Some((idx, _span)) = matched else {
                break;
            };
            self.bump();
            let rhs = next(self)?;
            let span = expr.span.join(rhs.span);
            expr = Expr {
                kind: ExprKind::Binary(ops[idx], Box::new(expr), Box::new(rhs)),
                span,
            };
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.eat(TokenKind::Bang) {
            let start = self.previous().span;
            let expr = self.unary()?;
            Ok(Expr {
                span: start.join(expr.span),
                kind: ExprKind::Unary(UnaryOp::Not, Box::new(expr)),
            })
        } else if self.eat(TokenKind::Minus) {
            let start = self.previous().span;
            let expr = self.unary()?;
            Ok(Expr {
                span: start.join(expr.span),
                kind: ExprKind::Unary(UnaryOp::Neg, Box::new(expr)),
            })
        } else {
            self.application()
        }
    }

    fn application(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.postfix()?;
        while self.starts_postfix_atom() {
            let arg = self.postfix()?;
            let span = expr.span.join(arg.span);
            expr = Expr {
                kind: ExprKind::App(Box::new(expr), Box::new(arg)),
                span,
            };
        }
        Ok(expr)
    }

    fn postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.primary()?;
        while self.eat(TokenKind::Dot) {
            let name = self.expect_ident("expected field or method name")?;
            let span = expr.span.join(self.previous().span);
            expr = Expr {
                kind: ExprKind::Field(Box::new(expr), name),
                span,
            };
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Int(value) => Ok(Expr {
                kind: ExprKind::Int(value),
                span: token.span,
            }),
            TokenKind::Float(value) => Ok(Expr {
                kind: ExprKind::Float(value),
                span: token.span,
            }),
            TokenKind::Ident(name) if name == "true" => Ok(Expr {
                kind: ExprKind::Bool(true),
                span: token.span,
            }),
            TokenKind::Ident(name) if name == "false" => Ok(Expr {
                kind: ExprKind::Bool(false),
                span: token.span,
            }),
            TokenKind::String(parts) => self.string_expr(parts, token.span),
            TokenKind::Ident(name) if name == "none" => Ok(Expr {
                kind: ExprKind::None,
                span: token.span,
            }),
            TokenKind::Ident(name) if name == "some" => {
                let value = self.expr()?;
                Ok(Expr {
                    span: token.span.join(value.span),
                    kind: ExprKind::Some(Box::new(value)),
                })
            }
            TokenKind::Ident(name) => {
                if is_reserved(&name) {
                    Err(Diagnostic::new(
                        "E_PARSE_015",
                        format!("unexpected keyword `{name}`"),
                        token.span,
                    ))
                } else {
                    Ok(Expr {
                        kind: ExprKind::Var(name),
                        span: token.span,
                    })
                }
            }
            TokenKind::LBracket => self.list_expr(token.span),
            TokenKind::LBrace => self.record_expr(token.span),
            TokenKind::LParen => {
                let expr = self.expr()?;
                self.expect(TokenKind::RParen, "expected `)` after expression")?;
                Ok(expr)
            }
            _ => Err(Diagnostic::new(
                "E_PARSE_016",
                "expected expression",
                token.span,
            )),
        }
    }

    fn string_expr(&self, parts: Vec<StringPart>, span: Span) -> Result<Expr, Diagnostic> {
        if parts.iter().all(|part| matches!(part, StringPart::Text(_))) {
            let mut value = String::new();
            for part in parts {
                if let StringPart::Text(text) = part {
                    value.push_str(&text);
                }
            }
            return Ok(Expr {
                kind: ExprKind::String(value),
                span,
            });
        }

        let mut interp = Vec::new();
        for part in parts {
            match part {
                StringPart::Text(text) => interp.push(InterpPart::Text(text)),
                StringPart::Expr(expr_text) => {
                    let expr = Parser::parse_embedded_expr(self.file_id, &expr_text, span)?;
                    interp.push(InterpPart::Expr(expr));
                }
            }
        }
        Ok(Expr {
            kind: ExprKind::Interp(interp),
            span,
        })
    }

    fn list_expr(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        let mut items = Vec::new();
        if !self.peek(TokenKind::RBracket) {
            loop {
                items.push(self.expr()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                if self.peek(TokenKind::RBracket) {
                    break;
                }
            }
        }
        let end = self
            .expect(TokenKind::RBracket, "expected `]` after list")?
            .span;
        Ok(Expr {
            kind: ExprKind::List(items),
            span: start.join(end),
        })
    }

    fn record_expr(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        let mut fields = Vec::new();
        if !self.peek(TokenKind::RBrace) {
            loop {
                let name = self.expect_ident("expected record field name")?;
                let name_span = self.previous().span;
                self.expect(TokenKind::Eq, "expected `=` after record field name")?;
                let value = self.expr()?;
                fields.push(FieldExpr {
                    name,
                    span: name_span,
                    value,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                if self.peek(TokenKind::RBrace) {
                    break;
                }
            }
        }
        let end = self
            .expect(TokenKind::RBrace, "expected `}` after record")?
            .span;
        Ok(Expr {
            kind: ExprKind::Record(fields),
            span: start.join(end),
        })
    }

    fn starts_postfix_atom(&self) -> bool {
        match &self.current().kind {
            TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::String(_)
            | TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::LBrace => true,
            TokenKind::Ident(name) => !matches!(
                name.as_str(),
                "in" | "then" | "else" | "type" | "export" | "import"
            ),
            _ => false,
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<Token, Diagnostic> {
        if self.peek_keyword(keyword) {
            Ok(self.bump().clone())
        } else {
            Err(Diagnostic::new(
                "E_PARSE_017",
                format!("expected `{keyword}`"),
                self.current().span,
            ))
        }
    }

    fn expect_ident(&mut self, message: &str) -> Result<String, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Ident(name) if !is_reserved(&name) => Ok(name),
            TokenKind::Ident(name) if starts_upper(&name) => Ok(name),
            _ => Err(Diagnostic::new("E_PARSE_018", message, token.span)),
        }
    }

    fn expect_value_name(&mut self, message: &str) -> Result<String, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Ident(name) if !is_reserved(&name) && !starts_upper(&name) => Ok(name),
            _ => Err(Diagnostic::new("E_PARSE_019", message, token.span)),
        }
    }

    fn expect_type_name(&mut self, message: &str) -> Result<String, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Ident(name) if starts_upper(&name) => Ok(name),
            _ => Err(Diagnostic::new("E_PARSE_020", message, token.span)),
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<Token, Diagnostic> {
        if self.peek(kind) {
            Ok(self.bump().clone())
        } else {
            Err(Diagnostic::new("E_PARSE_021", message, self.current().span))
        }
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.peek(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn peek(&self, kind: TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(&kind)
    }

    fn peek_keyword(&self, keyword: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Ident(name) if name == keyword)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.pos - 1]
    }

    fn bump(&mut self) -> &Token {
        let idx = self.pos;
        self.pos += 1;
        &self.tokens[idx]
    }

    fn at_eof(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }
}

fn token_index(tokens: &[Token], token: &Token) -> usize {
    tokens
        .iter()
        .position(|candidate| candidate.span == token.span)
        .unwrap_or(0)
}

fn starts_upper(name: &str) -> bool {
    name.as_bytes()
        .first()
        .map(|ch| ch.is_ascii_uppercase())
        .unwrap_or(false)
}

fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "import"
            | "export"
            | "type"
            | "let"
            | "in"
            | "if"
            | "then"
            | "else"
            | "true"
            | "false"
            | "none"
            | "some"
    )
}

fn literal_string(parts: Vec<StringPart>, span: Span) -> Result<String, Diagnostic> {
    let mut out = String::new();
    for part in parts {
        match part {
            StringPart::Text(text) => out.push_str(&text),
            StringPart::Expr(_) => {
                return Err(Diagnostic::new(
                    "E_PARSE_022",
                    "interpolation is not allowed here",
                    span,
                ));
            }
        }
    }
    Ok(out)
}

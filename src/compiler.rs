use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::diagnostic::{Diagnostic, SourceMap, Span};

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
struct FileAst {
    imports: Vec<ImportDecl>,
    decls: Vec<TopDecl>,
    output: Expr,
}

#[derive(Clone, Debug)]
struct ImportDecl {
    path: String,
    names: Vec<String>,
    span: Span,
}

#[derive(Clone, Debug)]
enum TopDecl {
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
struct Ty {
    kind: TyKind,
    span: Span,
}

#[derive(Clone, Debug)]
enum TyKind {
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
struct FieldTy {
    name: String,
    ty: Ty,
    span: Span,
}

#[derive(Clone, Debug)]
struct Expr {
    kind: ExprKind,
    span: Span,
}

#[derive(Clone, Debug)]
enum ExprKind {
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
enum InterpPart {
    Text(String),
    Expr(Expr),
}

#[derive(Clone, Debug)]
struct FieldExpr {
    name: String,
    value: Expr,
    span: Span,
}

#[derive(Clone, Copy, Debug)]
enum UnaryOp {
    Not,
    Neg,
}

#[derive(Clone, Copy, Debug)]
enum BinaryOp {
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

struct Parser {
    file_id: usize,
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn parse_file(file_id: usize, input: &str) -> Result<FileAst, Diagnostic> {
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

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    None,
    Some(Box<Value>),
    List(Vec<Value>),
    Record(Vec<(String, Value)>),
    Closure(Rc<Closure>),
    Builtin { name: String, args: Vec<Value> },
}

#[derive(Clone, Debug)]
pub struct Closure {
    param: String,
    body: Expr,
    env: RuntimeEnv,
}

type RuntimeEnv = HashMap<String, Value>;

#[derive(Clone, Debug)]
struct ValueInfo {
    ty: Ty,
    value: Value,
}

#[derive(Clone, Debug, Default)]
struct Ctx {
    types: HashMap<String, Ty>,
    values: HashMap<String, ValueInfo>,
}

#[derive(Clone, Debug, Default)]
struct Exports {
    types: HashMap<String, Ty>,
    values: HashMap<String, ValueInfo>,
}

#[derive(Clone, Debug)]
struct ModuleResult {
    exports: Exports,
    output_value: Value,
}

pub struct Compiler {
    sources: SourceMap,
    modules: HashMap<PathBuf, ModuleResult>,
    stack: Vec<PathBuf>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            sources: SourceMap::default(),
            modules: HashMap::new(),
            stack: Vec::new(),
        }
    }

    pub fn check_file(&mut self, path: &Path) -> Result<(), Diagnostic> {
        self.load_module(path)?;
        Ok(())
    }

    pub fn eval_file(&mut self, path: &Path) -> Result<Value, Diagnostic> {
        let module = self.load_module(path)?;
        reject_function_output(&module.output_value, self.empty_span())?;
        Ok(module.output_value)
    }

    pub fn render(&self, diagnostic: Diagnostic) -> String {
        self.sources.render(diagnostic)
    }

    fn empty_span(&self) -> Span {
        Span::empty(0, 0)
    }

    fn load_module(&mut self, path: &Path) -> Result<ModuleResult, Diagnostic> {
        let canonical = canonicalize_existing(path)
            .map_err(|message| Diagnostic::new("E_MODULE_001", message, self.empty_span()))?;
        if let Some(module) = self.modules.get(&canonical) {
            return Ok(module.clone());
        }
        if let Some(idx) = self.stack.iter().position(|p| p == &canonical) {
            let cycle = self.stack[idx..]
                .iter()
                .chain(std::iter::once(&canonical))
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Diagnostic::new(
                "E_MODULE_002",
                format!("cyclic import detected: {cycle}"),
                self.empty_span(),
            ));
        }

        let text = std::fs::read_to_string(&canonical).map_err(|err| {
            Diagnostic::new(
                "E_MODULE_003",
                format!("failed to read {}: {err}", canonical.display()),
                self.empty_span(),
            )
        })?;
        let file_id = self.sources.add(canonical.clone(), text.clone());
        let ast = Parser::parse_file(file_id, &text)?;

        self.stack.push(canonical.clone());
        let module = self.process_module(&canonical, ast);
        self.stack.pop();

        let module = module?;
        self.modules.insert(canonical, module.clone());
        Ok(module)
    }

    fn process_module(&mut self, path: &Path, ast: FileAst) -> Result<ModuleResult, Diagnostic> {
        let mut ctx = Ctx::default();
        let mut exports = Exports::default();

        for import in ast.imports {
            let import_path = resolve_import_path(path, &import.path)
                .map_err(|message| Diagnostic::new("E_MODULE_004", message, import.span))?;
            let imported = self.load_module(&import_path)?;
            for name in import.names {
                let mut found = false;
                if let Some(ty) = imported.exports.types.get(&name) {
                    if ctx.types.insert(name.clone(), ty.clone()).is_some() {
                        return Err(Diagnostic::new(
                            "E_NAME_001",
                            format!("duplicate imported type `{name}`"),
                            import.span,
                        ));
                    }
                    found = true;
                }
                if let Some(value) = imported.exports.values.get(&name) {
                    if ctx.values.insert(name.clone(), value.clone()).is_some() {
                        return Err(Diagnostic::new(
                            "E_NAME_002",
                            format!("duplicate imported value `{name}`"),
                            import.span,
                        ));
                    }
                    found = true;
                }
                if !found {
                    return Err(Diagnostic::new(
                        "E_MODULE_005",
                        format!("`{name}` is not exported by {}", import_path.display()),
                        import.span,
                    ));
                }
            }
        }

        for decl in ast.decls {
            match decl {
                TopDecl::Type {
                    export,
                    name,
                    ty,
                    span,
                } => {
                    if ctx.types.contains_key(&name) {
                        return Err(Diagnostic::new(
                            "E_NAME_003",
                            format!("duplicate type `{name}`"),
                            span,
                        ));
                    }
                    if ty_mentions_alias(&ty, &name) {
                        return Err(Diagnostic::new(
                            "E_TYPE_002",
                            format!("recursive type alias `{name}`"),
                            ty.span,
                        ));
                    }
                    self.check_well_formed_type(&ty, &ctx, &mut Vec::new())?;
                    ctx.types.insert(name.clone(), ty.clone());
                    if export {
                        exports.types.insert(name, ty);
                    }
                }
                TopDecl::Let {
                    export,
                    name,
                    ann,
                    value,
                    span,
                } => {
                    if ctx.values.contains_key(&name) {
                        return Err(Diagnostic::new(
                            "E_NAME_004",
                            format!("duplicate value `{name}`"),
                            span,
                        ));
                    }
                    let (ty, elaborated) = if let Some(ann) = ann {
                        self.check_well_formed_type(&ann, &ctx, &mut Vec::new())?;
                        let elaborated = self.check_expr(&value, &ann, &ctx)?;
                        (ann, elaborated)
                    } else {
                        self.synth_expr(&value, &ctx)?
                    };
                    let value = eval(&elaborated, &runtime_from_ctx(&ctx), elaborated.span)?;
                    let info = ValueInfo {
                        ty: ty.clone(),
                        value,
                    };
                    ctx.values.insert(name.clone(), info.clone());
                    if export {
                        exports.values.insert(name, info);
                    }
                }
            }
        }

        let (_output_ty, output_expr) = self.synth_expr(&ast.output, &ctx)?;
        let output_value = eval(&output_expr, &runtime_from_ctx(&ctx), ast.output.span)?;
        reject_function_output(&output_value, ast.output.span)?;
        Ok(ModuleResult {
            exports,
            output_value,
        })
    }

    fn check_well_formed_type(
        &self,
        ty: &Ty,
        ctx: &Ctx,
        stack: &mut Vec<String>,
    ) -> Result<(), Diagnostic> {
        match &ty.kind {
            TyKind::Int
            | TyKind::Float
            | TyKind::Bool
            | TyKind::String
            | TyKind::LiteralUnion(_) => Ok(()),
            TyKind::Builtin(_) => Ok(()),
            TyKind::Option(inner) | TyKind::List(inner) => {
                self.check_well_formed_type(inner, ctx, stack)
            }
            TyKind::Fun(param, result) => {
                self.check_well_formed_type(param, ctx, stack)?;
                self.check_well_formed_type(result, ctx, stack)
            }
            TyKind::Record(fields) => {
                let mut seen = HashSet::new();
                for field in fields {
                    if !seen.insert(field.name.clone()) {
                        return Err(Diagnostic::new(
                            "E_TYPE_001",
                            format!("duplicate field `{}` in record type", field.name),
                            field.span,
                        ));
                    }
                    self.check_well_formed_type(&field.ty, ctx, stack)?;
                }
                Ok(())
            }
            TyKind::Refine { binder, base, pred } => {
                self.check_well_formed_type(base, ctx, stack)?;
                let mut refine_ctx = ctx.clone();
                refine_ctx.values.insert(
                    binder.clone(),
                    ValueInfo {
                        ty: (*base.clone()),
                        value: Value::Builtin {
                            name: "__refinement_binder__".to_string(),
                            args: Vec::new(),
                        },
                    },
                );
                let (pred_ty, _) = self.synth_expr(pred, &refine_ctx)?;
                if !self.compatible(&pred_ty, &bool_ty(pred.span), ctx)? {
                    return Err(Diagnostic::new(
                        "E_REFINE_001",
                        "refinement predicate must have type Bool",
                        pred.span,
                    ));
                }
                Ok(())
            }
            TyKind::Alias(name) => {
                if stack.contains(name) {
                    return Err(Diagnostic::new(
                        "E_TYPE_002",
                        format!("recursive type alias `{name}`"),
                        ty.span,
                    ));
                }
                let Some(alias) = ctx.types.get(name) else {
                    return Err(Diagnostic::new(
                        "E_TYPE_003",
                        format!("unknown type `{name}`"),
                        ty.span,
                    ));
                };
                stack.push(name.clone());
                self.check_well_formed_type(alias, ctx, stack)?;
                stack.pop();
                Ok(())
            }
        }
    }

    fn synth_expr(&self, expr: &Expr, ctx: &Ctx) -> Result<(Ty, Expr), Diagnostic> {
        match &expr.kind {
            ExprKind::Int(_) => Ok((int_ty(expr.span), expr.clone())),
            ExprKind::Float(value) => {
                if !value.is_finite() {
                    Err(Diagnostic::new(
                        "E_TYPE_004",
                        "float literals must be finite",
                        expr.span,
                    ))
                } else {
                    Ok((float_ty(expr.span), expr.clone()))
                }
            }
            ExprKind::Bool(_) => Ok((bool_ty(expr.span), expr.clone())),
            ExprKind::String(_) => Ok((string_ty(expr.span), expr.clone())),
            ExprKind::Interp(parts) => {
                let mut checked = Vec::new();
                for part in parts {
                    match part {
                        InterpPart::Text(text) => checked.push(InterpPart::Text(text.clone())),
                        InterpPart::Expr(part_expr) => {
                            let (part_ty, elab) = self.synth_expr(part_expr, ctx)?;
                            if !is_showable(&self.expand_alias(&part_ty, ctx)?) {
                                return Err(Diagnostic::new(
                                    "E_TYPE_005",
                                    format!(
                                        "cannot interpolate value of type {}",
                                        self.ty_name(&part_ty, ctx)
                                    ),
                                    part_expr.span,
                                ));
                            }
                            checked.push(InterpPart::Expr(elab));
                        }
                    }
                }
                Ok((
                    string_ty(expr.span),
                    Expr {
                        kind: ExprKind::Interp(checked),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Var(name) => {
                if let Some(info) = ctx.values.get(name) {
                    Ok((info.ty.clone(), expr.clone()))
                } else if is_builtin_name(name) {
                    Ok((
                        Ty {
                            kind: TyKind::Builtin(name.clone()),
                            span: expr.span,
                        },
                        expr.clone(),
                    ))
                } else {
                    Err(Diagnostic::new(
                        "E_NAME_005",
                        format!("unknown identifier `{name}`"),
                        expr.span,
                    ))
                }
            }
            ExprKind::None => Err(Diagnostic::new(
                "E_TYPE_006",
                "`none` requires an expected option type",
                expr.span,
            )),
            ExprKind::Some(inner) => {
                let (inner_ty, inner_elab) = self.synth_expr(inner, ctx)?;
                Ok((
                    Ty {
                        kind: TyKind::Option(Box::new(inner_ty)),
                        span: expr.span,
                    },
                    Expr {
                        kind: ExprKind::Some(Box::new(inner_elab)),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::List(items) => {
                let Some(first) = items.first() else {
                    return Err(Diagnostic::new(
                        "E_TYPE_007",
                        "empty lists require an expected list type",
                        expr.span,
                    ));
                };
                let (item_ty, first_elab) = self.synth_expr(first, ctx)?;
                let mut elaborated = vec![first_elab];
                for item in &items[1..] {
                    elaborated.push(self.check_expr(item, &item_ty, ctx)?);
                }
                Ok((
                    Ty {
                        kind: TyKind::List(Box::new(item_ty)),
                        span: expr.span,
                    },
                    Expr {
                        kind: ExprKind::List(elaborated),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Record(fields) => {
                let mut seen = HashSet::new();
                let mut field_tys = Vec::new();
                let mut field_exprs = Vec::new();
                for field in fields {
                    if !seen.insert(field.name.clone()) {
                        return Err(Diagnostic::new(
                            "E_RECORD_001",
                            format!("duplicate field `{}`", field.name),
                            field.span,
                        ));
                    }
                    let (ty, value) = self.synth_expr(&field.value, ctx)?;
                    field_tys.push(FieldTy {
                        name: field.name.clone(),
                        ty,
                        span: field.span,
                    });
                    field_exprs.push(FieldExpr {
                        name: field.name.clone(),
                        value,
                        span: field.span,
                    });
                }
                Ok((
                    Ty {
                        kind: TyKind::Record(field_tys),
                        span: expr.span,
                    },
                    Expr {
                        kind: ExprKind::Record(field_exprs),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Field(base, name) => {
                let (base_ty, base_elab) = self.synth_expr(base, ctx)?;
                match self.expand_alias(&base_ty, ctx)?.kind {
                    TyKind::Record(fields) => {
                        let Some(field) = fields.iter().find(|field| field.name == *name) else {
                            return Err(Diagnostic::new(
                                "E_RECORD_002",
                                format!("unknown field `{name}`"),
                                expr.span,
                            ));
                        };
                        Ok((
                            field.ty.clone(),
                            Expr {
                                kind: ExprKind::Field(Box::new(base_elab), name.clone()),
                                span: expr.span,
                            },
                        ))
                    }
                    other => self.synth_method(expr.span, &base_ty, &other, base_elab, name, ctx),
                }
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let cond_elab = self.check_expr(cond, &bool_ty(cond.span), ctx)?;
                let (then_ty, then_elab) = self.synth_expr(then_expr, ctx)?;
                let else_elab = self.check_expr(else_expr, &then_ty, ctx)?;
                Ok((
                    then_ty,
                    Expr {
                        kind: ExprKind::If {
                            cond: Box::new(cond_elab),
                            then_expr: Box::new(then_elab),
                            else_expr: Box::new(else_elab),
                        },
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Let {
                name,
                ann,
                value,
                body,
            } => {
                let mut local = ctx.clone();
                let (value_ty, value_elab) = if let Some(ann) = ann {
                    self.check_well_formed_type(ann, ctx, &mut Vec::new())?;
                    let value_elab = self.check_expr(value, ann, ctx)?;
                    (ann.clone(), value_elab)
                } else {
                    self.synth_expr(value, ctx)?
                };
                let value = eval(&value_elab, &runtime_from_ctx(ctx), value.span)?;
                local.values.insert(
                    name.clone(),
                    ValueInfo {
                        ty: value_ty.clone(),
                        value,
                    },
                );
                let (body_ty, body_elab) = self.synth_expr(body, &local)?;
                Ok((
                    body_ty,
                    Expr {
                        kind: ExprKind::Let {
                            name: name.clone(),
                            ann: ann.clone(),
                            value: Box::new(value_elab),
                            body: Box::new(body_elab),
                        },
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Lam {
                param,
                param_ty,
                body,
            } => {
                self.check_well_formed_type(param_ty, ctx, &mut Vec::new())?;
                let mut local = ctx.clone();
                local.values.insert(
                    param.clone(),
                    ValueInfo {
                        ty: param_ty.clone(),
                        value: Value::Builtin {
                            name: "__lambda_param__".to_string(),
                            args: Vec::new(),
                        },
                    },
                );
                let (body_ty, body_elab) = self.synth_expr(body, &local)?;
                Ok((
                    Ty {
                        kind: TyKind::Fun(Box::new(param_ty.clone()), Box::new(body_ty)),
                        span: expr.span,
                    },
                    Expr {
                        kind: ExprKind::Lam {
                            param: param.clone(),
                            param_ty: param_ty.clone(),
                            body: Box::new(body_elab),
                        },
                        span: expr.span,
                    },
                ))
            }
            ExprKind::App(func, arg) => {
                let (func_ty, func_elab) = self.synth_expr(func, ctx)?;
                match self.expand_alias(&func_ty, ctx)?.kind {
                    TyKind::Fun(param, result) => {
                        let arg_elab = self.check_expr(arg, &param, ctx)?;
                        Ok((
                            *result,
                            Expr {
                                kind: ExprKind::App(Box::new(func_elab), Box::new(arg_elab)),
                                span: expr.span,
                            },
                        ))
                    }
                    TyKind::Builtin(name) => {
                        self.synth_builtin_app(expr.span, &name, func_elab, arg, ctx)
                    }
                    _ => Err(Diagnostic::new(
                        "E_TYPE_008",
                        format!(
                            "cannot apply non-function of type {}",
                            self.ty_name(&func_ty, ctx)
                        ),
                        func.span,
                    )),
                }
            }
            ExprKind::Ascribe(inner, ty) => {
                self.check_well_formed_type(ty, ctx, &mut Vec::new())?;
                let elab = self.check_expr(inner, ty, ctx)?;
                Ok((
                    ty.clone(),
                    Expr {
                        kind: ExprKind::Ascribe(Box::new(elab), ty.clone()),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Unary(op, inner) => match op {
                UnaryOp::Not => {
                    let inner_elab = self.check_expr(inner, &bool_ty(inner.span), ctx)?;
                    Ok((
                        bool_ty(expr.span),
                        Expr {
                            kind: ExprKind::Unary(*op, Box::new(inner_elab)),
                            span: expr.span,
                        },
                    ))
                }
                UnaryOp::Neg => {
                    let (inner_ty, inner_elab) = self.synth_expr(inner, ctx)?;
                    if matches!(
                        self.expand_alias(&inner_ty, ctx)?.kind,
                        TyKind::Int | TyKind::Float
                    ) {
                        Ok((
                            inner_ty,
                            Expr {
                                kind: ExprKind::Unary(*op, Box::new(inner_elab)),
                                span: expr.span,
                            },
                        ))
                    } else {
                        Err(Diagnostic::new(
                            "E_TYPE_009",
                            "numeric negation requires Int or Float",
                            inner.span,
                        ))
                    }
                }
            },
            ExprKind::Binary(op, left, right) => {
                self.synth_binary(expr.span, *op, left, right, ctx)
            }
        }
    }

    fn check_expr(&self, expr: &Expr, expected: &Ty, ctx: &Ctx) -> Result<Expr, Diagnostic> {
        let expanded = self.expand_alias(expected, ctx)?;
        match &expanded.kind {
            TyKind::Refine { binder, base, pred } => {
                let elab = self.check_expr(expr, base, ctx)?;
                let value = eval(&elab, &runtime_from_ctx(ctx), expr.span)?;
                self.validate_refinement(binder, pred, &value, expected, expr.span, ctx)?;
                Ok(elab)
            }
            TyKind::LiteralUnion(literals) => {
                let elab = self.check_expr(expr, &string_ty(expected.span), ctx)?;
                let value = eval(&elab, &runtime_from_ctx(ctx), expr.span)?;
                match value {
                    Value::String(value) if literals.iter().any(|lit| lit == &value) => Ok(elab),
                    Value::String(value) => Err(Diagnostic::new(
                        "E_REFINE_002",
                        format!(
                            "string literal `{value}` is not in {}",
                            self.ty_name(expected, ctx)
                        ),
                        expr.span,
                    )),
                    _ => Err(Diagnostic::new(
                        "E_REFINE_003",
                        "literal union predicate did not normalize to a string",
                        expr.span,
                    )),
                }
            }
            TyKind::Option(inner) => match &expr.kind {
                ExprKind::None => Ok(Expr {
                    kind: ExprKind::None,
                    span: expr.span,
                }),
                ExprKind::Some(value) => Ok(Expr {
                    kind: ExprKind::Some(Box::new(self.check_expr(value, inner, ctx)?)),
                    span: expr.span,
                }),
                _ => {
                    if let Ok((actual, elab)) = self.synth_expr(expr, ctx)
                        && self.compatible(&actual, expected, ctx)?
                    {
                        return Ok(elab);
                    }
                    let inner_elab = self.check_expr(expr, inner, ctx)?;
                    Ok(Expr {
                        kind: ExprKind::Some(Box::new(inner_elab)),
                        span: expr.span,
                    })
                }
            },
            TyKind::Record(expected_fields) => {
                let ExprKind::Record(actual_fields) = &expr.kind else {
                    let (actual, elab) = self.synth_expr(expr, ctx)?;
                    if self.compatible(&actual, expected, ctx)? {
                        return Ok(elab);
                    }
                    return Err(self.type_mismatch(expr.span, expected, &actual, ctx));
                };
                let mut provided: HashMap<String, &FieldExpr> = HashMap::new();
                for field in actual_fields {
                    if provided.insert(field.name.clone(), field).is_some() {
                        return Err(Diagnostic::new(
                            "E_RECORD_003",
                            format!("duplicate field `{}`", field.name),
                            field.span,
                        ));
                    }
                }
                for field in actual_fields {
                    if !expected_fields
                        .iter()
                        .any(|expected| expected.name == field.name)
                    {
                        return Err(Diagnostic::new(
                            "E_RECORD_004",
                            format!("unknown field `{}`", field.name),
                            field.span,
                        ));
                    }
                }
                let mut elaborated_fields = Vec::new();
                for expected_field in expected_fields {
                    if let Some(actual) = provided.get(&expected_field.name) {
                        elaborated_fields.push(FieldExpr {
                            name: expected_field.name.clone(),
                            value: self.check_expr(&actual.value, &expected_field.ty, ctx)?,
                            span: actual.span,
                        });
                    } else if is_option_ty(&self.expand_alias(&expected_field.ty, ctx)?) {
                        elaborated_fields.push(FieldExpr {
                            name: expected_field.name.clone(),
                            value: Expr {
                                kind: ExprKind::None,
                                span: expr.span,
                            },
                            span: expected_field.span,
                        });
                    } else {
                        return Err(Diagnostic::new(
                            "E_RECORD_005",
                            format!("missing field `{}`", expected_field.name),
                            expr.span,
                        ));
                    }
                }
                Ok(Expr {
                    kind: ExprKind::Record(elaborated_fields),
                    span: expr.span,
                })
            }
            TyKind::List(item_ty) => {
                if let ExprKind::List(items) = &expr.kind {
                    let mut elaborated = Vec::new();
                    for item in items {
                        elaborated.push(self.check_expr(item, item_ty, ctx)?);
                    }
                    Ok(Expr {
                        kind: ExprKind::List(elaborated),
                        span: expr.span,
                    })
                } else {
                    let (actual, elab) = self.synth_expr(expr, ctx)?;
                    if self.compatible(&actual, expected, ctx)? {
                        Ok(elab)
                    } else {
                        Err(self.type_mismatch(expr.span, expected, &actual, ctx))
                    }
                }
            }
            _ => {
                let (actual, elab) = self.synth_expr(expr, ctx)?;
                if self.compatible(&actual, expected, ctx)? {
                    Ok(elab)
                } else {
                    Err(self.type_mismatch(expr.span, expected, &actual, ctx))
                }
            }
        }
    }

    fn validate_refinement(
        &self,
        binder: &str,
        pred: &Expr,
        value: &Value,
        expected: &Ty,
        span: Span,
        ctx: &Ctx,
    ) -> Result<(), Diagnostic> {
        let mut env = runtime_from_ctx(ctx);
        env.insert(binder.to_string(), value.clone());
        match eval(pred, &env, pred.span)? {
            Value::Bool(true) => Ok(()),
            Value::Bool(false) => Err(Diagnostic::new("E_REFINE_004", "refinement failed", span)
                .note(format!(
                    "value `{}` does not satisfy {}",
                    value_debug(value),
                    self.ty_name(expected, ctx)
                ))),
            other => Err(Diagnostic::new(
                "E_REFINE_005",
                "refinement predicate did not normalize to Bool",
                pred.span,
            )
            .note(format!("predicate normalized to `{}`", value_debug(&other)))),
        }
    }

    fn synth_binary(
        &self,
        span: Span,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        ctx: &Ctx,
    ) -> Result<(Ty, Expr), Diagnostic> {
        match op {
            BinaryOp::And | BinaryOp::Or => {
                let left_elab = self.check_expr(left, &bool_ty(left.span), ctx)?;
                let right_elab = self.check_expr(right, &bool_ty(right.span), ctx)?;
                Ok((bool_ty(span), binary_expr(span, op, left_elab, right_elab)))
            }
            BinaryOp::Concat => {
                let left_elab = self.check_expr(left, &string_ty(left.span), ctx)?;
                let right_elab = self.check_expr(right, &string_ty(right.span), ctx)?;
                Ok((
                    string_ty(span),
                    binary_expr(span, op, left_elab, right_elab),
                ))
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                let (left_ty, left_elab) = self.synth_expr(left, ctx)?;
                let left_expanded = self.expand_alias(&left_ty, ctx)?;
                match left_expanded.kind {
                    TyKind::Int => {
                        let right_elab = self.check_expr(right, &int_ty(right.span), ctx)?;
                        Ok((int_ty(span), binary_expr(span, op, left_elab, right_elab)))
                    }
                    TyKind::Float if !matches!(op, BinaryOp::Mod) => {
                        let right_elab = self.check_expr(right, &float_ty(right.span), ctx)?;
                        Ok((float_ty(span), binary_expr(span, op, left_elab, right_elab)))
                    }
                    _ => Err(Diagnostic::new(
                        "E_TYPE_010",
                        "arithmetic operators require matching numeric operands",
                        span,
                    )),
                }
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                let (left_ty, left_elab) = self.synth_expr(left, ctx)?;
                let right_elab = self.check_expr(right, &left_ty, ctx)?;
                if !is_comparable(&self.expand_alias(&left_ty, ctx)?) {
                    return Err(Diagnostic::new(
                        "E_TYPE_011",
                        "functions and built-ins cannot be compared",
                        span,
                    ));
                }
                Ok((bool_ty(span), binary_expr(span, op, left_elab, right_elab)))
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let (left_ty, left_elab) = self.synth_expr(left, ctx)?;
                let expanded = self.expand_alias(&left_ty, ctx)?;
                match expanded.kind {
                    TyKind::Int | TyKind::Float | TyKind::String => {
                        let right_elab = self.check_expr(right, &left_ty, ctx)?;
                        Ok((bool_ty(span), binary_expr(span, op, left_elab, right_elab)))
                    }
                    _ => Err(Diagnostic::new(
                        "E_TYPE_012",
                        "ordering operators require Int, Float, or String",
                        span,
                    )),
                }
            }
        }
    }

    fn synth_method(
        &self,
        span: Span,
        base_ty: &Ty,
        expanded: &TyKind,
        base_elab: Expr,
        name: &str,
        _ctx: &Ctx,
    ) -> Result<(Ty, Expr), Diagnostic> {
        let base_expr = Box::new(base_elab);
        match (name, expanded) {
            ("isSome" | "isNone", TyKind::Option(_)) => Ok((
                bool_ty(span),
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            ("length", TyKind::List(_) | TyKind::String) => Ok((
                int_ty(span),
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            ("contains", TyKind::List(item)) => Ok((
                Ty {
                    kind: TyKind::Fun(item.clone(), Box::new(bool_ty(span))),
                    span,
                },
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            ("contains" | "startsWith" | "endsWith", TyKind::String) => Ok((
                Ty {
                    kind: TyKind::Fun(Box::new(string_ty(span)), Box::new(bool_ty(span))),
                    span,
                },
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            ("unwrapOr", TyKind::Option(item)) => Ok((
                Ty {
                    kind: TyKind::Fun(item.clone(), item.clone()),
                    span,
                },
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            _ => Err(Diagnostic::new(
                "E_TYPE_013",
                format!(
                    "type {} has no field or method `{name}`",
                    self.ty_name(base_ty, _ctx)
                ),
                span,
            )),
        }
    }

    fn synth_builtin_app(
        &self,
        span: Span,
        name: &str,
        func_elab: Expr,
        arg: &Expr,
        ctx: &Ctx,
    ) -> Result<(Ty, Expr), Diagnostic> {
        let (arg_ty, arg_elab) = self.synth_expr(arg, ctx)?;
        let arg_expanded = self.expand_alias(&arg_ty, ctx)?;
        let result_ty = match (name, &arg_expanded.kind) {
            ("show", ty) if is_showable_kind(ty) => string_ty(span),
            ("isSome" | "isNone", TyKind::Option(_)) => bool_ty(span),
            ("length", TyKind::List(_) | TyKind::String) => int_ty(span),
            ("contains", TyKind::List(item)) => Ty {
                kind: TyKind::Fun(item.clone(), Box::new(bool_ty(span))),
                span,
            },
            ("contains" | "startsWith" | "endsWith", TyKind::String) => Ty {
                kind: TyKind::Fun(Box::new(string_ty(span)), Box::new(bool_ty(span))),
                span,
            },
            ("unwrapOr", TyKind::Option(item)) => Ty {
                kind: TyKind::Fun(item.clone(), item.clone()),
                span,
            },
            _ => {
                return Err(Diagnostic::new(
                    "E_TYPE_014",
                    format!(
                        "unsupported built-in `{name}` for {}",
                        self.ty_name(&arg_ty, ctx)
                    ),
                    arg.span,
                ));
            }
        };
        Ok((
            result_ty,
            Expr {
                kind: ExprKind::App(Box::new(func_elab), Box::new(arg_elab)),
                span,
            },
        ))
    }

    fn compatible(&self, actual: &Ty, expected: &Ty, ctx: &Ctx) -> Result<bool, Diagnostic> {
        let actual = self.erase_refinements(&self.expand_alias(actual, ctx)?, ctx)?;
        let expected = self.erase_refinements(&self.expand_alias(expected, ctx)?, ctx)?;
        Ok(match (&actual.kind, &expected.kind) {
            (TyKind::Int, TyKind::Int)
            | (TyKind::Float, TyKind::Float)
            | (TyKind::Bool, TyKind::Bool)
            | (TyKind::String, TyKind::String) => true,
            (TyKind::LiteralUnion(_), TyKind::String)
            | (TyKind::String, TyKind::LiteralUnion(_)) => true,
            (TyKind::LiteralUnion(a), TyKind::LiteralUnion(b)) => a == b,
            (TyKind::Option(a), TyKind::Option(b)) | (TyKind::List(a), TyKind::List(b)) => {
                self.compatible(a, b, ctx)?
            }
            (TyKind::Record(a), TyKind::Record(b)) => {
                a.len() == b.len()
                    && a.iter().zip(b.iter()).all(|(af, bf)| {
                        af.name == bf.name && self.compatible(&af.ty, &bf.ty, ctx).unwrap_or(false)
                    })
            }
            (TyKind::Fun(ap, ar), TyKind::Fun(bp, br)) => {
                self.compatible(ap, bp, ctx)? && self.compatible(ar, br, ctx)?
            }
            _ => false,
        })
    }

    fn erase_refinements(&self, ty: &Ty, ctx: &Ctx) -> Result<Ty, Diagnostic> {
        Ok(match &ty.kind {
            TyKind::Refine { base, .. } => self.erase_refinements(base, ctx)?,
            TyKind::LiteralUnion(_) => string_ty(ty.span),
            TyKind::Option(inner) => Ty {
                kind: TyKind::Option(Box::new(self.erase_refinements(inner, ctx)?)),
                span: ty.span,
            },
            TyKind::List(inner) => Ty {
                kind: TyKind::List(Box::new(self.erase_refinements(inner, ctx)?)),
                span: ty.span,
            },
            TyKind::Record(fields) => Ty {
                kind: TyKind::Record(
                    fields
                        .iter()
                        .map(|field| {
                            Ok(FieldTy {
                                name: field.name.clone(),
                                ty: self.erase_refinements(&field.ty, ctx)?,
                                span: field.span,
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?,
                ),
                span: ty.span,
            },
            TyKind::Fun(a, b) => Ty {
                kind: TyKind::Fun(
                    Box::new(self.erase_refinements(a, ctx)?),
                    Box::new(self.erase_refinements(b, ctx)?),
                ),
                span: ty.span,
            },
            TyKind::Alias(_) => self.erase_refinements(&self.expand_alias(ty, ctx)?, ctx)?,
            _ => ty.clone(),
        })
    }

    fn expand_alias(&self, ty: &Ty, ctx: &Ctx) -> Result<Ty, Diagnostic> {
        self.expand_alias_inner(ty, ctx, &mut Vec::new())
    }

    fn expand_alias_inner(
        &self,
        ty: &Ty,
        ctx: &Ctx,
        stack: &mut Vec<String>,
    ) -> Result<Ty, Diagnostic> {
        match &ty.kind {
            TyKind::Alias(name) => {
                if stack.contains(name) {
                    return Err(Diagnostic::new(
                        "E_TYPE_015",
                        format!("recursive type alias `{name}`"),
                        ty.span,
                    ));
                }
                let Some(alias) = ctx.types.get(name) else {
                    return Err(Diagnostic::new(
                        "E_TYPE_016",
                        format!("unknown type `{name}`"),
                        ty.span,
                    ));
                };
                stack.push(name.clone());
                let expanded = self.expand_alias_inner(alias, ctx, stack)?;
                stack.pop();
                Ok(expanded)
            }
            _ => Ok(ty.clone()),
        }
    }

    fn type_mismatch(&self, span: Span, expected: &Ty, actual: &Ty, ctx: &Ctx) -> Diagnostic {
        Diagnostic::new(
            "E_TYPE_017",
            format!(
                "type mismatch: expected {}, found {}",
                self.ty_name(expected, ctx),
                self.ty_name(actual, ctx)
            ),
            span,
        )
    }

    fn ty_name(&self, ty: &Ty, ctx: &Ctx) -> String {
        match self
            .expand_alias(ty, ctx)
            .unwrap_or_else(|_| ty.clone())
            .kind
        {
            TyKind::Int => "Int".to_string(),
            TyKind::Float => "Float".to_string(),
            TyKind::Bool => "Bool".to_string(),
            TyKind::String => "String".to_string(),
            TyKind::Option(inner) => format!("{}?", self.ty_name(&inner, ctx)),
            TyKind::List(inner) => format!("[{}]", self.ty_name(&inner, ctx)),
            TyKind::Record(fields) => {
                let fields = fields
                    .iter()
                    .map(|field| format!("{} : {}", field.name, self.ty_name(&field.ty, ctx)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {fields} }}")
            }
            TyKind::Refine { binder, base, .. } => {
                format!("{{ {binder} : {} | ... }}", self.ty_name(&base, ctx))
            }
            TyKind::Fun(param, result) => {
                format!(
                    "{} -> {}",
                    self.ty_name(&param, ctx),
                    self.ty_name(&result, ctx)
                )
            }
            TyKind::Alias(name) => name,
            TyKind::LiteralUnion(values) => values
                .iter()
                .map(|value| format!("{value:?}"))
                .collect::<Vec<_>>()
                .join(" | "),
            TyKind::Builtin(name) => format!("<builtin {name}>"),
        }
    }
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path)
        .map_err(|err| format!("failed to resolve {}: {err}", path.display()))
}

fn resolve_import_path(from: &Path, import: &str) -> Result<PathBuf, String> {
    let base = from.parent().unwrap_or_else(|| Path::new("."));
    let path = base.join(import);
    canonicalize_existing(&path)
}

fn runtime_from_ctx(ctx: &Ctx) -> RuntimeEnv {
    ctx.values
        .iter()
        .map(|(name, info)| (name.clone(), info.value.clone()))
        .collect()
}

fn bool_ty(span: Span) -> Ty {
    Ty {
        kind: TyKind::Bool,
        span,
    }
}

fn int_ty(span: Span) -> Ty {
    Ty {
        kind: TyKind::Int,
        span,
    }
}

fn float_ty(span: Span) -> Ty {
    Ty {
        kind: TyKind::Float,
        span,
    }
}

fn string_ty(span: Span) -> Ty {
    Ty {
        kind: TyKind::String,
        span,
    }
}

fn is_option_ty(ty: &Ty) -> bool {
    matches!(ty.kind, TyKind::Option(_))
}

fn ty_mentions_alias(ty: &Ty, name: &str) -> bool {
    match &ty.kind {
        TyKind::Alias(alias) => alias == name,
        TyKind::Option(inner) | TyKind::List(inner) => ty_mentions_alias(inner, name),
        TyKind::Record(fields) => fields
            .iter()
            .any(|field| ty_mentions_alias(&field.ty, name)),
        TyKind::Refine { base, .. } => ty_mentions_alias(base, name),
        TyKind::Fun(param, result) => {
            ty_mentions_alias(param, name) || ty_mentions_alias(result, name)
        }
        TyKind::Int
        | TyKind::Float
        | TyKind::Bool
        | TyKind::String
        | TyKind::LiteralUnion(_)
        | TyKind::Builtin(_) => false,
    }
}

fn binary_expr(span: Span, op: BinaryOp, left: Expr, right: Expr) -> Expr {
    Expr {
        kind: ExprKind::Binary(op, Box::new(left), Box::new(right)),
        span,
    }
}

fn is_builtin_name(name: &str) -> bool {
    matches!(
        name,
        "show"
            | "isSome"
            | "isNone"
            | "length"
            | "contains"
            | "startsWith"
            | "endsWith"
            | "unwrapOr"
    )
}

fn is_showable(ty: &Ty) -> bool {
    is_showable_kind(&ty.kind)
}

fn is_showable_kind(ty: &TyKind) -> bool {
    matches!(
        ty,
        TyKind::Int | TyKind::Float | TyKind::Bool | TyKind::String | TyKind::LiteralUnion(_)
    )
}

fn is_comparable(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Fun(_, _) | TyKind::Builtin(_) => false,
        TyKind::Option(inner) | TyKind::List(inner) => is_comparable(inner),
        TyKind::Record(fields) => fields.iter().all(|field| is_comparable(&field.ty)),
        TyKind::Refine { base, .. } => is_comparable(base),
        _ => true,
    }
}

fn eval(expr: &Expr, env: &RuntimeEnv, span: Span) -> Result<Value, Diagnostic> {
    match &expr.kind {
        ExprKind::Int(value) => Ok(Value::Int(*value)),
        ExprKind::Float(value) => Ok(Value::Float(*value)),
        ExprKind::Bool(value) => Ok(Value::Bool(*value)),
        ExprKind::String(value) => Ok(Value::String(value.clone())),
        ExprKind::Interp(parts) => {
            let mut out = String::new();
            for part in parts {
                match part {
                    InterpPart::Text(text) => out.push_str(text),
                    InterpPart::Expr(expr) => {
                        let value = eval(expr, env, expr.span)?;
                        out.push_str(&show_value(&value, expr.span)?);
                    }
                }
            }
            Ok(Value::String(out))
        }
        ExprKind::Var(name) => env
            .get(name)
            .cloned()
            .or_else(|| {
                is_builtin_name(name).then(|| Value::Builtin {
                    name: name.clone(),
                    args: Vec::new(),
                })
            })
            .ok_or_else(|| {
                Diagnostic::new(
                    "E_RUNTIME_001",
                    format!("unknown runtime identifier `{name}`"),
                    expr.span,
                )
            }),
        ExprKind::None => Ok(Value::None),
        ExprKind::Some(value) => Ok(Value::Some(Box::new(eval(value, env, value.span)?))),
        ExprKind::List(items) => items
            .iter()
            .map(|item| eval(item, env, item.span))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        ExprKind::Record(fields) => fields
            .iter()
            .map(|field| {
                Ok((
                    field.name.clone(),
                    eval(&field.value, env, field.value.span)?,
                ))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()
            .map(Value::Record),
        ExprKind::Field(base, name) => {
            let base_value = eval(base, env, base.span)?;
            match base_value {
                Value::Record(fields) => fields
                    .into_iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, value)| value)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "E_RUNTIME_002",
                            format!("unknown field `{name}`"),
                            expr.span,
                        )
                    }),
                value => eval_method(name, value, expr.span),
            }
        }
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => match eval(cond, env, cond.span)? {
            Value::Bool(true) => eval(then_expr, env, then_expr.span),
            Value::Bool(false) => eval(else_expr, env, else_expr.span),
            other => Err(Diagnostic::new(
                "E_RUNTIME_003",
                format!("if condition evaluated to `{}`", value_debug(&other)),
                cond.span,
            )),
        },
        ExprKind::Let {
            name, value, body, ..
        } => {
            let value = eval(value, env, value.span)?;
            let mut local = env.clone();
            local.insert(name.clone(), value);
            eval(body, &local, body.span)
        }
        ExprKind::Lam { param, body, .. } => Ok(Value::Closure(Rc::new(Closure {
            param: param.clone(),
            body: (*body.clone()),
            env: env.clone(),
        }))),
        ExprKind::App(func, arg) => {
            let func_value = eval(func, env, func.span)?;
            let arg_value = eval(arg, env, arg.span)?;
            apply_value(func_value, arg_value, expr.span)
        }
        ExprKind::Ascribe(inner, _) => eval(inner, env, inner.span),
        ExprKind::Unary(op, inner) => {
            let value = eval(inner, env, inner.span)?;
            match (op, value) {
                (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
                (UnaryOp::Neg, Value::Int(value)) => Ok(Value::Int(-value)),
                (UnaryOp::Neg, Value::Float(value)) => Ok(Value::Float(-value)),
                (_, other) => Err(Diagnostic::new(
                    "E_RUNTIME_004",
                    format!("invalid unary operand `{}`", value_debug(&other)),
                    expr.span,
                )),
            }
        }
        ExprKind::Binary(op, left, right) => {
            let left = eval(left, env, left.span)?;
            if matches!(op, BinaryOp::And) {
                return match left {
                    Value::Bool(false) => Ok(Value::Bool(false)),
                    Value::Bool(true) => eval(right, env, right.span),
                    other => Err(Diagnostic::new(
                        "E_RUNTIME_005",
                        format!("invalid boolean operand `{}`", value_debug(&other)),
                        expr.span,
                    )),
                };
            }
            if matches!(op, BinaryOp::Or) {
                return match left {
                    Value::Bool(true) => Ok(Value::Bool(true)),
                    Value::Bool(false) => eval(right, env, right.span),
                    other => Err(Diagnostic::new(
                        "E_RUNTIME_006",
                        format!("invalid boolean operand `{}`", value_debug(&other)),
                        expr.span,
                    )),
                };
            }
            let right = eval(right, env, right.span)?;
            eval_binary(*op, left, right, expr.span)
        }
    }
    .map_err(|err| {
        if err.span.end == 0 {
            Diagnostic { span, ..err }
        } else {
            err
        }
    })
}

fn eval_method(name: &str, receiver: Value, span: Span) -> Result<Value, Diagnostic> {
    match name {
        "isSome" => Ok(Value::Bool(matches!(receiver, Value::Some(_)))),
        "isNone" => Ok(Value::Bool(matches!(receiver, Value::None))),
        "length" => match receiver {
            Value::String(value) => Ok(Value::Int(value.chars().count() as i64)),
            Value::List(items) => Ok(Value::Int(items.len() as i64)),
            other => Err(Diagnostic::new(
                "E_RUNTIME_007",
                format!("length is not supported on `{}`", value_debug(&other)),
                span,
            )),
        },
        "contains" | "startsWith" | "endsWith" | "unwrapOr" => Ok(Value::Builtin {
            name: name.to_string(),
            args: vec![receiver],
        }),
        _ => Err(Diagnostic::new(
            "E_RUNTIME_008",
            format!("unsupported method `{name}`"),
            span,
        )),
    }
}

fn apply_value(func: Value, arg: Value, span: Span) -> Result<Value, Diagnostic> {
    match func {
        Value::Closure(closure) => {
            let mut env = closure.env.clone();
            env.insert(closure.param.clone(), arg);
            eval(&closure.body, &env, closure.body.span)
        }
        Value::Builtin { name, mut args } => {
            args.push(arg);
            apply_builtin(name, args, span)
        }
        other => Err(Diagnostic::new(
            "E_RUNTIME_009",
            format!("cannot apply `{}`", value_debug(&other)),
            span,
        )),
    }
}

fn apply_builtin(name: String, args: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    let arity = match name.as_str() {
        "show" | "isSome" | "isNone" | "length" => 1,
        "contains" | "startsWith" | "endsWith" | "unwrapOr" => 2,
        _ => {
            return Err(Diagnostic::new(
                "E_RUNTIME_010",
                format!("unknown built-in `{name}`"),
                span,
            ));
        }
    };
    if args.len() < arity {
        return Ok(Value::Builtin { name, args });
    }
    match name.as_str() {
        "show" => Ok(Value::String(show_value(&args[0], span)?)),
        "isSome" => Ok(Value::Bool(matches!(args[0], Value::Some(_)))),
        "isNone" => Ok(Value::Bool(matches!(args[0], Value::None))),
        "length" => match &args[0] {
            Value::String(value) => Ok(Value::Int(value.chars().count() as i64)),
            Value::List(items) => Ok(Value::Int(items.len() as i64)),
            other => Err(Diagnostic::new(
                "E_RUNTIME_011",
                format!("length is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "contains" => match (&args[0], &args[1]) {
            (Value::String(haystack), Value::String(needle)) => {
                Ok(Value::Bool(haystack.contains(needle)))
            }
            (Value::List(items), needle) => Ok(Value::Bool(
                items.iter().any(|item| values_equal(item, needle)),
            )),
            (other, _) => Err(Diagnostic::new(
                "E_RUNTIME_012",
                format!("contains is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "startsWith" => match (&args[0], &args[1]) {
            (Value::String(value), Value::String(prefix)) => {
                Ok(Value::Bool(value.starts_with(prefix)))
            }
            (other, _) => Err(Diagnostic::new(
                "E_RUNTIME_013",
                format!("startsWith is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "endsWith" => match (&args[0], &args[1]) {
            (Value::String(value), Value::String(suffix)) => {
                Ok(Value::Bool(value.ends_with(suffix)))
            }
            (other, _) => Err(Diagnostic::new(
                "E_RUNTIME_014",
                format!("endsWith is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "unwrapOr" => match &args[0] {
            Value::Some(value) => Ok((**value).clone()),
            Value::None => Ok(args[1].clone()),
            other => Err(Diagnostic::new(
                "E_RUNTIME_015",
                format!("unwrapOr is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        _ => unreachable!(),
    }
}

fn eval_binary(op: BinaryOp, left: Value, right: Value, span: Span) -> Result<Value, Diagnostic> {
    match op {
        BinaryOp::Add => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (a, b) => runtime_type_error("addition", &a, &b, span),
        },
        BinaryOp::Sub => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (a, b) => runtime_type_error("subtraction", &a, &b, span),
        },
        BinaryOp::Mul => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (a, b) => runtime_type_error("multiplication", &a, &b, span),
        },
        BinaryOp::Div => match (left, right) {
            (Value::Int(_), Value::Int(0)) => {
                Err(Diagnostic::new("E_RUNTIME_016", "division by zero", span))
            }
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
            (Value::Float(_), Value::Float(0.0)) => {
                Err(Diagnostic::new("E_RUNTIME_017", "division by zero", span))
            }
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (a, b) => runtime_type_error("division", &a, &b, span),
        },
        BinaryOp::Mod => match (left, right) {
            (Value::Int(_), Value::Int(0)) => {
                Err(Diagnostic::new("E_RUNTIME_018", "modulo by zero", span))
            }
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
            (a, b) => runtime_type_error("modulo", &a, &b, span),
        },
        BinaryOp::Concat => match (left, right) {
            (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{a}{b}"))),
            (a, b) => runtime_type_error("string concatenation", &a, &b, span),
        },
        BinaryOp::Eq => Ok(Value::Bool(values_equal(&left, &right))),
        BinaryOp::Ne => Ok(Value::Bool(!values_equal(&left, &right))),
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            order_values(op, left, right, span)
        }
        BinaryOp::And | BinaryOp::Or => unreachable!("short-circuited in eval"),
    }
}

fn runtime_type_error(
    op: &str,
    left: &Value,
    right: &Value,
    span: Span,
) -> Result<Value, Diagnostic> {
    Err(Diagnostic::new(
        "E_RUNTIME_019",
        format!(
            "{op} is not supported for `{}` and `{}`",
            value_debug(left),
            value_debug(right)
        ),
        span,
    ))
}

fn order_values(op: BinaryOp, left: Value, right: Value, span: Span) -> Result<Value, Diagnostic> {
    let ordering = match (left, right) {
        (Value::Int(a), Value::Int(b)) => a.partial_cmp(&b),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(&b),
        (Value::String(a), Value::String(b)) => a.partial_cmp(&b),
        (a, b) => {
            return runtime_type_error("ordering", &a, &b, span);
        }
    }
    .ok_or_else(|| Diagnostic::new("E_RUNTIME_020", "values are not orderable", span))?;
    Ok(Value::Bool(match op {
        BinaryOp::Lt => ordering.is_lt(),
        BinaryOp::Le => ordering.is_le(),
        BinaryOp::Gt => ordering.is_gt(),
        BinaryOp::Ge => ordering.is_ge(),
        _ => unreachable!(),
    }))
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::None, Value::None) => true,
        (Value::Some(a), Value::Some(b)) => values_equal(a, b),
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| values_equal(a, b))
        }
        (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ak, av), (bk, bv))| ak == bk && values_equal(av, bv))
        }
        _ => false,
    }
}

fn show_value(value: &Value, span: Span) -> Result<String, Diagnostic> {
    match value {
        Value::Int(value) => Ok(value.to_string()),
        Value::Float(value) if value.is_finite() => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::String(value) => Ok(value.clone()),
        _ => Err(Diagnostic::new(
            "E_RUNTIME_021",
            format!("cannot show `{}`", value_debug(value)),
            span,
        )),
    }
}

fn reject_function_output(value: &Value, span: Span) -> Result<(), Diagnostic> {
    match value {
        Value::Closure(_) | Value::Builtin { .. } => Err(Diagnostic::new(
            "E_OUTPUT_001",
            "function escaped into output",
            span,
        )),
        Value::Some(value) => reject_function_output(value, span),
        Value::List(values) => {
            for value in values {
                reject_function_output(value, span)?;
            }
            Ok(())
        }
        Value::Record(fields) => {
            for (_, value) in fields {
                reject_function_output(value, span)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn value_debug(value: &Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::None => "none".to_string(),
        Value::Some(value) => format!("some {}", value_debug(value)),
        Value::List(items) => format!(
            "[{}]",
            items.iter().map(value_debug).collect::<Vec<_>>().join(", ")
        ),
        Value::Record(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(name, value)| format!("{name} = {}", value_debug(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Closure(_) => "<function>".to_string(),
        Value::Builtin { name, .. } => format!("<builtin {name}>"),
    }
}

pub fn emit_json(value: &Value, pretty: bool) -> Result<String, Diagnostic> {
    let mut out = String::new();
    write_json(value, pretty, 0, &mut out, Span::empty(0, 0))?;
    Ok(out)
}

fn write_json(
    value: &Value,
    pretty: bool,
    indent: usize,
    out: &mut String,
    span: Span,
) -> Result<(), Diagnostic> {
    match value {
        Value::Int(value) => write!(out, "{value}").unwrap(),
        Value::Float(value) if value.is_finite() => write!(out, "{value}").unwrap(),
        Value::Float(_) => {
            return Err(Diagnostic::new(
                "E_OUTPUT_002",
                "cannot emit non-finite float as JSON",
                span,
            ));
        }
        Value::Bool(value) => write!(out, "{value}").unwrap(),
        Value::String(value) => write_json_string(value, out),
        Value::None => out.push_str("null"),
        Value::Some(value) => write_json(value, pretty, indent, out, span)?,
        Value::List(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                if pretty {
                    out.push('\n');
                    out.push_str(&" ".repeat(indent + 2));
                }
                write_json(item, pretty, indent + 2, out, span)?;
            }
            if pretty && !items.is_empty() {
                out.push('\n');
                out.push_str(&" ".repeat(indent));
            }
            out.push(']');
        }
        Value::Record(fields) => {
            out.push('{');
            for (idx, (name, value)) in fields.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                if pretty {
                    out.push('\n');
                    out.push_str(&" ".repeat(indent + 2));
                }
                write_json_string(name, out);
                out.push(':');
                if pretty {
                    out.push(' ');
                }
                write_json(value, pretty, indent + 2, out, span)?;
            }
            if pretty && !fields.is_empty() {
                out.push('\n');
                out.push_str(&" ".repeat(indent));
            }
            out.push('}');
        }
        Value::Closure(_) | Value::Builtin { .. } => {
            return Err(Diagnostic::new(
                "E_OUTPUT_003",
                "function escaped into output",
                span,
            ));
        }
    }
    Ok(())
}

fn write_json_string(value: &str, out: &mut String) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

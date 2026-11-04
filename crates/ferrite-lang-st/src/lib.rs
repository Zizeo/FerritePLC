use ferrite_core::{
    ExecutablePou, IecValue, LanguageKind, PouId, RuntimeError, ScanContext, TagTable,
    parse_iec_time_literal,
};
use ferrite_ir::{IrBinaryOp, IrError, IrExpr, IrStmt, IrUnaryOp};
use logos::Logos;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum StError {
    #[error("Lexer error in ST POU '{pou}' at byte {span_start}..{span_end}: unexpected '{slice}'")]
    LexError {
        pou: String,
        span_start: usize,
        span_end: usize,
        slice: String,
    },

    #[error("Syntax error in ST POU '{pou}': {message}")]
    SyntaxError { pou: String, message: String },

    #[error("Validation error in ST POU '{pou}': {source}")]
    Ir {
        pou: String,
        #[source]
        source: IrError,
    },
}

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"\(\*[^*]*\*+([^*)][^*]*\*+)*\)")]
pub enum Token {
    #[regex(r"(?i)PROGRAM")]
    Program,
    #[regex(r"(?i)END_PROGRAM")]
    EndProgram,
    #[regex(r"(?i)IF")]
    If,
    #[regex(r"(?i)THEN")]
    Then,
    #[regex(r"(?i)ELSIF")]
    Elsif,
    #[regex(r"(?i)ELSE")]
    Else,
    #[regex(r"(?i)END_IF")]
    EndIf,
    #[regex(r"(?i)CASE")]
    Case,
    #[regex(r"(?i)OF")]
    Of,
    #[regex(r"(?i)END_CASE")]
    EndCase,
    #[regex(r"(?i)FOR")]
    For,
    #[regex(r"(?i)TO")]
    To,
    #[regex(r"(?i)BY")]
    By,
    #[regex(r"(?i)DO")]
    Do,
    #[regex(r"(?i)END_FOR")]
    EndFor,
    #[regex(r"(?i)WHILE")]
    While,
    #[regex(r"(?i)END_WHILE")]
    EndWhile,
    #[regex(r"(?i)REPEAT")]
    Repeat,
    #[regex(r"(?i)UNTIL")]
    Until,
    #[regex(r"(?i)END_REPEAT")]
    EndRepeat,
    #[regex(r"(?i)EXIT")]
    Exit,
    #[regex(r"(?i)CONTINUE")]
    Continue,
    #[regex(r"(?i)RETURN")]
    Return,
    #[regex(r"(?i)AND")]
    And,
    #[regex(r"(?i)OR")]
    Or,
    #[regex(r"(?i)XOR")]
    Xor,
    #[regex(r"(?i)NOT")]
    Not,
    #[regex(r"(?i)MOD")]
    Mod,
    #[regex(r"(?i)TRUE")]
    True,
    #[regex(r"(?i)FALSE")]
    False,

    #[regex(r"(?i)(TIME|T)#[0-9]+(\.[0-9]+)?(ms|s|m)", |lex| lex.slice().to_string())]
    TimeLiteral(String),

    #[regex(r"%[IQMigm][XBWDxbwd][0-9]+(\.[0-7])?", |lex| lex.slice().to_ascii_uppercase())]
    DirectAddress(String),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().to_string())]
    RealLiteral(String),

    #[regex(r"[0-9]+", |lex| lex.slice().to_string())]
    IntLiteral(String),

    #[regex(r"'[^']*'", |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
    StringLiteral(String),

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z0-9_]+)*", |lex| lex.slice().to_string())]
    Ident(String),

    #[token(":=")]
    Assign,
    #[token("=>")]
    OutputAssign,
    #[token("<>")]
    Ne,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("=")]
    Eq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token(";")]
    Semi,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
}

/// Converts a 0-based byte offset in `src` into 1-based `(line, column)` coordinates.
pub fn byte_offset_to_line_col(src: &str, byte_offset: usize) -> (usize, usize) {
    let clamped = byte_offset.min(src.len());
    let prefix = &src[..clamped];
    let mut line = 1usize;
    let mut col = 1usize;
    for ch in prefix.chars() {
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub fn tokenize_with_spans(
    src: &str,
    pou_name: &str,
) -> Result<(Vec<Token>, Vec<std::ops::Range<usize>>), StError> {
    let mut lexer = Token::lexer(src);
    let mut tokens = Vec::new();
    let mut spans = Vec::new();
    while let Some(res) = lexer.next() {
        match res {
            Ok(tok) => {
                tokens.push(tok);
                spans.push(lexer.span());
            }
            Err(()) => {
                let span = lexer.span();
                return Err(StError::LexError {
                    pou: pou_name.to_string(),
                    span_start: span.start,
                    span_end: span.end,
                    slice: lexer.slice().to_string(),
                });
            }
        }
    }
    Ok((tokens, spans))
}

pub fn tokenize(src: &str, pou_name: &str) -> Result<Vec<Token>, StError> {
    let (tokens, _) = tokenize_with_spans(src, pou_name)?;
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    spans: &'a [std::ops::Range<usize>],
    src: &'a str,
    pos: usize,
    pou_name: &'a str,
}

impl<'a> Parser<'a> {
    fn new(
        tokens: &'a [Token],
        spans: &'a [std::ops::Range<usize>],
        src: &'a str,
        pou_name: &'a str,
    ) -> Self {
        Self {
            tokens,
            spans,
            src,
            pos: 0,
            pou_name,
        }
    }

    fn syntax_error(&self, token_idx: usize, detail: impl Into<String>) -> StError {
        let offset = self
            .spans
            .get(token_idx)
            .map(|r| r.start)
            .unwrap_or(self.src.len());
        let (line, col) = byte_offset_to_line_col(self.src, offset);
        StError::SyntaxError {
            pou: self.pou_name.to_string(),
            message: format!("line {line}, col {col}: {}", detail.into()),
        }
    }

    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&'a Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), StError> {
        let err_idx = self.pos;
        match self.next() {
            Some(tok) if tok == expected => Ok(()),
            Some(other) => Err(self.syntax_error(
                err_idx,
                format!("expected {expected:?}, found {other:?}"),
            )),
            None => Err(self.syntax_error(
                err_idx,
                format!("expected {expected:?}, found end of input"),
            )),
        }
    }

    fn expect_ident(&mut self) -> Result<String, StError> {
        let err_idx = self.pos;
        match self.next() {
            Some(Token::Ident(s)) | Some(Token::DirectAddress(s)) => Ok(s.clone()),
            Some(other) => {
                Err(self.syntax_error(err_idx, format!("expected identifier, found {other:?}")))
            }
            None => Err(self.syntax_error(err_idx, "expected identifier, found end of input")),
        }
    }

    fn parse_program(&mut self) -> Result<Vec<IrStmt>, StError> {
        if self.peek() == Some(&Token::Program) {
            self.next();
            let _prog_name = self.expect_ident()?;
            if self.peek() == Some(&Token::Semi) {
                self.next();
            }
        }
        let stmts = self.parse_stmt_list(&[Token::EndProgram])?;
        if self.peek() == Some(&Token::EndProgram) {
            self.next();
            if self.peek() == Some(&Token::Semi) {
                self.next();
            }
        }
        if let Some(rem) = self.peek() {
            return Err(self.syntax_error(
                self.pos,
                format!("unexpected token {rem:?} after statements"),
            ));
        }
        Ok(stmts)
    }

    fn parse_stmt_list(&mut self, terminators: &[Token]) -> Result<Vec<IrStmt>, StError> {
        let mut stmts = Vec::new();
        while let Some(tok) = self.peek() {
            if terminators.contains(tok) {
                break;
            }
            if *tok == Token::Semi {
                self.next();
                continue;
            }
            stmts.push(self.parse_stmt()?);
            if self.peek() == Some(&Token::Semi) {
                self.next();
            }
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<IrStmt, StError> {
        let err_idx = self.pos;
        match self.peek() {
            Some(Token::If) => self.parse_if(),
            Some(Token::Case) => self.parse_case(),
            Some(Token::For) => self.parse_for(),
            Some(Token::While) => self.parse_while(),
            Some(Token::Repeat) => self.parse_repeat(),
            Some(Token::Exit) => {
                self.next();
                Ok(IrStmt::Exit)
            }
            Some(Token::Continue) => {
                self.next();
                Ok(IrStmt::Continue)
            }
            Some(Token::Return) => {
                self.next();
                Ok(IrStmt::Return)
            }
            Some(Token::Ident(_)) | Some(Token::DirectAddress(_)) => {
                let name = self.expect_ident()?;
                if self.peek() == Some(&Token::LParen) {
                    self.parse_fb_call(name)
                } else {
                    self.expect(&Token::Assign)?;
                    let expr = self.parse_expr()?;
                    Ok(IrStmt::Assign { target: name, expr })
                }
            }
            Some(other) => Err(self.syntax_error(
                err_idx,
                format!("unexpected token {other:?} at start of statement"),
            )),
            None => Err(self.syntax_error(err_idx, "unexpected end of input in statement")),
        }
    }

    fn parse_if(&mut self) -> Result<IrStmt, StError> {
        self.expect(&Token::If)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::Then)?;
        let body = self.parse_stmt_list(&[Token::Elsif, Token::Else, Token::EndIf])?;
        let mut branches = vec![(cond, body)];
        while self.peek() == Some(&Token::Elsif) {
            self.next();
            let c = self.parse_expr()?;
            self.expect(&Token::Then)?;
            let b = self.parse_stmt_list(&[Token::Elsif, Token::Else, Token::EndIf])?;
            branches.push((c, b));
        }
        let else_body = if self.peek() == Some(&Token::Else) {
            self.next();
            self.parse_stmt_list(&[Token::EndIf])?
        } else {
            Vec::new()
        };
        self.expect(&Token::EndIf)?;
        Ok(IrStmt::If {
            branches,
            else_body,
        })
    }

    fn parse_case(&mut self) -> Result<IrStmt, StError> {
        self.expect(&Token::Case)?;
        let selector = self.parse_expr()?;
        self.expect(&Token::Of)?;
        let mut arms = Vec::new();
        let mut else_body = Vec::new();

        while let Some(tok) = self.peek() {
            if *tok == Token::EndCase {
                break;
            }
            if *tok == Token::Else {
                self.next();
                if self.peek() == Some(&Token::Colon) {
                    self.next();
                }
                else_body = self.parse_stmt_list(&[Token::EndCase])?;
                break;
            }
            // Parse one or more integer labels separated by commas followed by `:`
            let mut labels = Vec::new();
            loop {
                let sign = if self.peek() == Some(&Token::Minus) {
                    self.next();
                    -1
                } else {
                    1
                };
                match self.next() {
                    Some(Token::IntLiteral(s)) => {
                        let v = s.parse::<i32>().map_err(|_| StError::SyntaxError {
                            pou: self.pou_name.to_string(),
                            message: format!("invalid CASE integer label '{s}'"),
                        })?;
                        labels.push(sign * v);
                    }
                    other => {
                        return Err(StError::SyntaxError {
                            pou: self.pou_name.to_string(),
                            message: format!("expected CASE integer label, found {other:?}"),
                        });
                    }
                }
                if self.peek() == Some(&Token::Comma) {
                    self.next();
                } else {
                    break;
                }
            }
            self.expect(&Token::Colon)?;
            let mut arm_stmts = Vec::new();
            while let Some(t) = self.peek() {
                if *t == Token::EndCase || *t == Token::Else {
                    break;
                }
                // Check if next tokens start a new case label (`IntLiteral` followed by `,` or `:`)
                if matches!(t, Token::IntLiteral(_) | Token::Minus) {
                    break;
                }
                if *t == Token::Semi {
                    self.next();
                    continue;
                }
                arm_stmts.push(self.parse_stmt()?);
                if self.peek() == Some(&Token::Semi) {
                    self.next();
                }
            }
            arms.push((labels, arm_stmts));
        }

        self.expect(&Token::EndCase)?;
        Ok(IrStmt::Case {
            selector,
            arms,
            else_body,
        })
    }

    fn parse_for(&mut self) -> Result<IrStmt, StError> {
        self.expect(&Token::For)?;
        let var = self.expect_ident()?;
        self.expect(&Token::Assign)?;
        let start = self.parse_expr()?;
        self.expect(&Token::To)?;
        let end = self.parse_expr()?;
        let step = if self.peek() == Some(&Token::By) {
            self.next();
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(&Token::Do)?;
        let body = self.parse_stmt_list(&[Token::EndFor])?;
        self.expect(&Token::EndFor)?;
        Ok(IrStmt::For {
            var,
            start,
            end,
            step,
            body,
        })
    }

    fn parse_while(&mut self) -> Result<IrStmt, StError> {
        self.expect(&Token::While)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::Do)?;
        let body = self.parse_stmt_list(&[Token::EndWhile])?;
        self.expect(&Token::EndWhile)?;
        Ok(IrStmt::While { cond, body })
    }

    fn parse_repeat(&mut self) -> Result<IrStmt, StError> {
        self.expect(&Token::Repeat)?;
        let body = self.parse_stmt_list(&[Token::Until])?;
        self.expect(&Token::Until)?;
        let until_cond = self.parse_expr()?;
        if self.peek() == Some(&Token::Semi) {
            self.next();
        }
        self.expect(&Token::EndRepeat)?;
        Ok(IrStmt::Repeat { body, until_cond })
    }

    fn parse_fb_call(&mut self, instance: String) -> Result<IrStmt, StError> {
        self.expect(&Token::LParen)?;
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        while let Some(tok) = self.peek() {
            if *tok == Token::RParen {
                break;
            }
            let pin = self.expect_ident()?;
            match self.next() {
                Some(Token::Assign) => {
                    let expr = self.parse_expr()?;
                    inputs.push((pin, expr));
                }
                Some(Token::OutputAssign) => {
                    let target = self.expect_ident()?;
                    outputs.push((pin, target));
                }
                other => {
                    return Err(StError::SyntaxError {
                        pou: self.pou_name.to_string(),
                        message: format!("expected ':=' or '=>' in FB call, found {other:?}"),
                    });
                }
            }
            if self.peek() == Some(&Token::Comma) {
                self.next();
            } else {
                break;
            }
        }
        self.expect(&Token::RParen)?;
        Ok(IrStmt::FbCall {
            instance,
            inputs,
            outputs,
        })
    }

    fn parse_expr(&mut self) -> Result<IrExpr, StError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<IrExpr, StError> {
        let mut left = self.parse_xor()?;
        while self.peek() == Some(&Token::Or) {
            self.next();
            let right = self.parse_xor()?;
            left = IrExpr::Binary {
                op: IrBinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_xor(&mut self) -> Result<IrExpr, StError> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::Xor) {
            self.next();
            let right = self.parse_and()?;
            left = IrExpr::Binary {
                op: IrBinaryOp::Xor,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<IrExpr, StError> {
        let mut left = self.parse_comparison()?;
        while self.peek() == Some(&Token::And) {
            self.next();
            let right = self.parse_comparison()?;
            left = IrExpr::Binary {
                op: IrBinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<IrExpr, StError> {
        let mut left = self.parse_additive()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Eq => IrBinaryOp::Eq,
                Token::Ne => IrBinaryOp::Ne,
                Token::Lt => IrBinaryOp::Lt,
                Token::Le => IrBinaryOp::Le,
                Token::Gt => IrBinaryOp::Gt,
                Token::Ge => IrBinaryOp::Ge,
                _ => break,
            };
            self.next();
            let right = self.parse_additive()?;
            left = IrExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<IrExpr, StError> {
        let mut left = self.parse_multiplicative()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Plus => IrBinaryOp::Add,
                Token::Minus => IrBinaryOp::Sub,
                _ => break,
            };
            self.next();
            let right = self.parse_multiplicative()?;
            left = IrExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<IrExpr, StError> {
        let mut left = self.parse_unary()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Star => IrBinaryOp::Mul,
                Token::Slash => IrBinaryOp::Div,
                Token::Mod => IrBinaryOp::Mod,
                _ => break,
            };
            self.next();
            let right = self.parse_unary()?;
            left = IrExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<IrExpr, StError> {
        match self.peek() {
            Some(Token::Not) => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(IrExpr::Unary {
                    op: IrUnaryOp::Not,
                    expr: Box::new(expr),
                })
            }
            Some(Token::Minus) => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(IrExpr::Unary {
                    op: IrUnaryOp::Neg,
                    expr: Box::new(expr),
                })
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<IrExpr, StError> {
        match self.next() {
            Some(Token::True) => Ok(IrExpr::Literal(IecValue::Bool(true))),
            Some(Token::False) => Ok(IrExpr::Literal(IecValue::Bool(false))),
            Some(Token::TimeLiteral(s)) => {
                let ms = parse_iec_time_literal(s).ok_or_else(|| StError::SyntaxError {
                    pou: self.pou_name.to_string(),
                    message: format!("invalid time literal '{s}'"),
                })?;
                Ok(IrExpr::Literal(IecValue::Time(ms)))
            }
            Some(Token::RealLiteral(s)) => {
                let r = s.parse::<f32>().map_err(|_| StError::SyntaxError {
                    pou: self.pou_name.to_string(),
                    message: format!("invalid REAL literal '{s}'"),
                })?;
                Ok(IrExpr::Literal(IecValue::Real(r)))
            }
            Some(Token::IntLiteral(s)) => {
                if let Ok(i) = s.parse::<i16>() {
                    Ok(IrExpr::Literal(IecValue::Int(i)))
                } else if let Ok(d) = s.parse::<i32>() {
                    Ok(IrExpr::Literal(IecValue::Dint(d)))
                } else {
                    Err(StError::SyntaxError {
                        pou: self.pou_name.to_string(),
                        message: format!("invalid integer literal '{s}'"),
                    })
                }
            }
            Some(Token::StringLiteral(s)) => Ok(IrExpr::Literal(IecValue::String(s.clone()))),
            Some(Token::Ident(s)) => {
                if self.peek() == Some(&Token::LParen) {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        args.push(self.parse_expr()?);
                        while self.peek() == Some(&Token::Comma) {
                            self.next();
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.expect(&Token::RParen)?;
                    Ok(IrExpr::Call {
                        function_name: s.clone(),
                        args,
                    })
                } else {
                    Ok(IrExpr::Symbol(s.clone()))
                }
            }
            Some(Token::DirectAddress(s)) => Ok(IrExpr::Symbol(s.clone())),
            Some(Token::LParen) => {
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            other => Err(StError::SyntaxError {
                pou: self.pou_name.to_string(),
                message: format!("expected expression, found {other:?}"),
            }),
        }
    }
}

/// Parses a standalone ST expression (used in ST and Grafcet transition receptivities).
pub fn parse_st_expression(src: &str, pou_name: &str) -> Result<IrExpr, StError> {
    let (tokens, spans) = tokenize_with_spans(src, pou_name)?;
    let mut parser = Parser::new(&tokens, &spans, src, pou_name);
    let expr = parser.parse_expr()?;
    if let Some(rem) = parser.peek() {
        return Err(parser.syntax_error(
            parser.pos,
            format!("unexpected token {rem:?} in expression"),
        ));
    }
    Ok(expr)
}

/// Parses a list of ST statements (used in ST POUs and Grafcet step actions).
pub fn parse_st_statements(src: &str, pou_name: &str) -> Result<Vec<IrStmt>, StError> {
    let (tokens, spans) = tokenize_with_spans(src, pou_name)?;
    let mut parser = Parser::new(&tokens, &spans, src, pou_name);
    parser.parse_program()
}

#[derive(Debug, Clone, PartialEq)]
pub struct StProgram {
    pub name: String,
    pub source: String,
    pub statements: Vec<IrStmt>,
}

impl StProgram {
    pub fn parse(name: impl Into<String>, source: impl Into<String>) -> Result<Self, StError> {
        let name = name.into();
        let source = source.into();
        let statements = parse_st_statements(&source, &name)?;
        Ok(Self {
            name,
            source,
            statements,
        })
    }

    pub fn validate(&self, tags: &TagTable) -> Result<(), StError> {
        for stmt in &self.statements {
            stmt.validate(tags, &self.name, false)
                .map_err(|source| StError::Ir {
                    pou: self.name.clone(),
                    source,
                })?;
        }
        Ok(())
    }

    pub fn compile(&self, tags: &TagTable) -> Result<CompiledStPou, StError> {
        self.validate(tags)?;
        Ok(CompiledStPou {
            id: PouId::new(self.name.clone()),
            statements: self.statements.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct CompiledStPou {
    id: PouId,
    statements: Vec<IrStmt>,
}

impl ExecutablePou for CompiledStPou {
    fn id(&self) -> PouId {
        self.id.clone()
    }

    fn kind(&self) -> LanguageKind {
        LanguageKind::St
    }

    fn execute(&mut self, ctx: &mut ScanContext<'_>) -> Result<(), RuntimeError> {
        let pou_name = self.id.as_str();
        let _ = IrStmt::execute_block(&self.statements, ctx, pou_name)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_core::{IecType, ProcessImage, StandardFbKind, TagEntry};

    #[test]
    fn test_st_control_flow_and_cross_language_mutation() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();

        tags.register_sfc_step("1")?;
        tags.register_tag(TagEntry::new(
            "Tank_Level",
            IecType::Int,
            Some("%MW10".parse()?),
            IecValue::Int(0),
            "Tank level",
        ))?;
        tags.register_tag(TagEntry::new(
            "Pump_Speed",
            IecType::Real,
            None,
            IecValue::Real(0.0),
            "Calculated pump speed",
        ))?;
        tags.register_tag(TagEntry::new(
            "Batch_Mode",
            IecType::Int,
            None,
            IecValue::Int(1),
            "Mode selector",
        ))?;
        tags.register_tag(TagEntry::new(
            "Dose_Done",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Dosing timer output",
        ))?;
        tags.create_fb("Dose_Timer", StandardFbKind::Ton);

        let st_src = r#"
        PROGRAM math_pid
            (* Check if Grafcet Step 1 is active *)
            IF X1.state AND Tank_Level < 100 THEN
                Tank_Level := Tank_Level + 25;
                CASE Batch_Mode OF
                    1: Pump_Speed := 75.5;
                    2: Pump_Speed := 100.0;
                    ELSE: Pump_Speed := 50.0;
                END_CASE;
            ELSE
                Pump_Speed := 0.0;
            END_IF;

            Dose_Timer(IN := X1.state, PT := T#100ms, Q => Dose_Done);
        END_PROGRAM
        "#;

        let prog = StProgram::parse("math_pid", st_src)?;
        let mut compiled = prog.compile(&tags)?;

        let mut ctx = ScanContext::new(&mut img, &mut tags, 50, 1);
        // Simulate Grafcet activating Step 1
        ctx.with_sfc_engine(|sfc_ctx| sfc_ctx.write_symbol("X1.state", IecValue::Bool(true)))?;

        compiled.execute(&mut ctx)?;
        assert_eq!(ctx.read_symbol("Tank_Level")?, IecValue::Int(25));
        assert_eq!(ctx.read_symbol("Pump_Speed")?, IecValue::Real(75.5));
        assert!(!ctx.read_bool("Dose_Done")?);

        // Second 50ms scan -> Dose_Timer reaches 100ms!
        compiled.execute(&mut ctx)?;
        assert_eq!(ctx.read_symbol("Tank_Level")?, IecValue::Int(50));
        assert!(ctx.read_bool("Dose_Done")?);

        Ok(())
    }

    #[test]
    fn test_st_iec_math_functions() -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::default();
        tags.register_tag(TagEntry::new(
            "Val_In",
            IecType::Real,
            None,
            IecValue::Real(16.0),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Val_Out",
            IecType::Real,
            None,
            IecValue::Real(0.0),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Val_Clamped",
            IecType::Real,
            None,
            IecValue::Real(0.0),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Val_Max",
            IecType::Real,
            None,
            IecValue::Real(0.0),
            "",
        ))?;

        let st_src = r#"
        PROGRAM math_test
            Val_Out := SQRT(Val_In) + ABS(-5.0);
            Val_Clamped := LIMIT(0.0, Val_Out, 7.0);
            Val_Max := MAX(Val_Out, 100.0);
        END_PROGRAM
        "#;

        let prog = StProgram::parse("math_test", st_src)?;
        let mut compiled = prog.compile(&tags)?;
        let mut ctx = ScanContext::new(&mut img, &mut tags, 10, 1);
        compiled.execute(&mut ctx)?;

        // SQRT(16.0) + ABS(-5.0) = 4.0 + 5.0 = 9.0
        assert_eq!(ctx.read_symbol("Val_Out")?, IecValue::Real(9.0));
        // LIMIT(0.0, 9.0, 7.0) = 7.0
        assert_eq!(ctx.read_symbol("Val_Clamped")?, IecValue::Real(7.0));
        // MAX(9.0, 100.0) = 100.0
        assert_eq!(ctx.read_symbol("Val_Max")?, IecValue::Real(100.0));
        Ok(())
    }

    #[test]
    fn test_st_repeat_exit_continue_return_and_extended_stdlib() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::default();
        tags.register_tag(TagEntry::new(
            "Counter",
            IecType::Int,
            None,
            IecValue::Int(0),
            "",
        ))?;
        tags.register_tag(TagEntry::new("i", IecType::Int, None, IecValue::Int(0), ""))?;
        tags.register_tag(TagEntry::new(
            "Sum_Odd",
            IecType::Int,
            None,
            IecValue::Int(0),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Selected",
            IecType::Real,
            None,
            IecValue::Real(0.0),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Shifted",
            IecType::Int,
            None,
            IecValue::Int(0),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Msg_Out",
            IecType::String,
            None,
            IecValue::String(String::new()),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "After_Return",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;

        let st_src = r#"
        PROGRAM ext_test
            REPEAT
                Counter := Counter + 1;
                IF Counter = 5 THEN
                    EXIT;
                END_IF;
            UNTIL Counter >= 10
            END_REPEAT;

            FOR i := 1 TO 6 DO
                IF (i MOD 2) = 0 THEN
                    CONTINUE;
                END_IF;
                Sum_Odd := Sum_Odd + i;
            END_FOR;

            Selected := SEL(TRUE, 10.5, MUX(1, 20.0, 42.5, 99.0));
            Shifted := SHL(3, 2) + REAL_TO_INT(FLOOR(3.9));
            Msg_Out := CONCAT(LEFT('FERRITE_PLC', 7), RIGHT('0123_OK', 3));

            RETURN;
            After_Return := TRUE;
        END_PROGRAM
        "#;

        let prog = StProgram::parse("ext_test", st_src)?;
        let mut compiled = prog.compile(&tags)?;
        let mut ctx = ScanContext::new(&mut img, &mut tags, 10, 1);
        compiled.execute(&mut ctx)?;

        assert_eq!(ctx.read_symbol("Counter")?, IecValue::Int(5));
        assert_eq!(ctx.read_symbol("Sum_Odd")?, IecValue::Int(9)); // 1 + 3 + 5
        assert_eq!(ctx.read_symbol("Selected")?, IecValue::Real(42.5));
        assert_eq!(ctx.read_symbol("Shifted")?, IecValue::Int(15)); // (3 << 2) + 3 = 15
        assert_eq!(
            ctx.read_symbol("Msg_Out")?,
            IecValue::String("FERRITE_OK".to_string())
        );
        assert!(!ctx.read_bool("After_Return")?);

        // Verify line:col reporting on syntax errors
        let bad_src = "PROGRAM bad\n    Counter := 1;\n    IF Counter\nEND_PROGRAM";
        let err = StProgram::parse("bad", bad_src).err();
        assert!(err.is_some());
        if let Some(StError::SyntaxError { message, .. }) = err {
            assert!(message.contains("line 4"));
        }

        Ok(())
    }
}

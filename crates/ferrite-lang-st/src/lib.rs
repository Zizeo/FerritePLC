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

pub fn tokenize(src: &str, pou_name: &str) -> Result<Vec<Token>, StError> {
    let mut lexer = Token::lexer(src);
    let mut tokens = Vec::new();
    while let Some(res) = lexer.next() {
        match res {
            Ok(tok) => tokens.push(tok),
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
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    pou_name: &'a str,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], pou_name: &'a str) -> Self {
        Self {
            tokens,
            pos: 0,
            pou_name,
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
        match self.next() {
            Some(tok) if tok == expected => Ok(()),
            Some(other) => Err(StError::SyntaxError {
                pou: self.pou_name.to_string(),
                message: format!("expected {expected:?}, found {other:?}"),
            }),
            None => Err(StError::SyntaxError {
                pou: self.pou_name.to_string(),
                message: format!("expected {expected:?}, found end of input"),
            }),
        }
    }

    fn expect_ident(&mut self) -> Result<String, StError> {
        match self.next() {
            Some(Token::Ident(s)) | Some(Token::DirectAddress(s)) => Ok(s.clone()),
            Some(other) => Err(StError::SyntaxError {
                pou: self.pou_name.to_string(),
                message: format!("expected identifier, found {other:?}"),
            }),
            None => Err(StError::SyntaxError {
                pou: self.pou_name.to_string(),
                message: "expected identifier, found end of input".to_string(),
            }),
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
            return Err(StError::SyntaxError {
                pou: self.pou_name.to_string(),
                message: format!("unexpected token {rem:?} after statements"),
            });
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
        match self.peek() {
            Some(Token::If) => self.parse_if(),
            Some(Token::Case) => self.parse_case(),
            Some(Token::For) => self.parse_for(),
            Some(Token::While) => self.parse_while(),
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
            Some(other) => Err(StError::SyntaxError {
                pou: self.pou_name.to_string(),
                message: format!("unexpected token {other:?} at start of statement"),
            }),
            None => Err(StError::SyntaxError {
                pou: self.pou_name.to_string(),
                message: "unexpected end of input in statement".to_string(),
            }),
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
            Some(Token::Ident(s)) | Some(Token::DirectAddress(s)) => {
                Ok(IrExpr::Symbol(s.clone()))
            }
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
    let tokens = tokenize(src, pou_name)?;
    let mut parser = Parser::new(&tokens, pou_name);
    let expr = parser.parse_expr()?;
    if let Some(rem) = parser.peek() {
        return Err(StError::SyntaxError {
            pou: pou_name.to_string(),
            message: format!("unexpected token {rem:?} in expression"),
        });
    }
    Ok(expr)
}

/// Parses a list of ST statements (used in ST POUs and Grafcet step actions).
pub fn parse_st_statements(src: &str, pou_name: &str) -> Result<Vec<IrStmt>, StError> {
    let tokens = tokenize(src, pou_name)?;
    let mut parser = Parser::new(&tokens, pou_name);
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
        for stmt in &self.statements {
            stmt.execute(ctx, pou_name)?;
        }
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
}

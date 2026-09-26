use super::ast::{BinOp, Expr, ExprKind, Program, Stmt, StmtKind};
use super::header::Header;
use super::token::{Pos, Spanned, Token};

const MAX_DEPTH: usize = 256;

pub fn parse(header: Header, tokens: Vec<Spanned>) -> Result<Program, String> {
    let mut parser = Parser {
        tokens,
        index: 0,
        depth: 0,
    };
    let body = parser.statements(Token::Eof)?;
    Ok(Program { header, body })
}

struct Parser {
    tokens: Vec<Spanned>,
    index: usize,
    depth: usize,
}

impl Parser {
    fn statements(&mut self, end: Token) -> Result<Vec<Stmt>, String> {
        let mut body = Vec::new();
        loop {
            self.skip_newlines();
            if *self.peek() == end || *self.peek() == Token::Eof {
                return Ok(body);
            }
            body.push(self.statement()?);
        }
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        let pos = self.pos();
        let kind = match self.peek() {
            Token::Input => self.input()?,
            Token::Let => self.let_statement()?,
            Token::Func => self.func()?,
            Token::Return => self.return_statement()?,
            Token::Name(_) => self.assign()?,
            _ => return Err(self.error("expected a statement")),
        };
        self.end_of_statement()?;
        Ok(Stmt { kind, pos })
    }

    fn input(&mut self) -> Result<StmtKind, String> {
        self.bump();
        let name = self.name()?;
        self.expect(Token::Colon, "`:` after the input name")?;
        let min = self.expr()?;
        self.expect(Token::DotDot, "`..` between the minimum and maximum")?;
        let max = self.expr()?;
        self.expect(Token::Equals, "`=` and a default value")?;
        let default = self.expr()?;
        Ok(StmtKind::Input {
            name,
            min,
            max,
            default,
        })
    }

    fn let_statement(&mut self) -> Result<StmtKind, String> {
        self.bump();
        let name = self.name()?;
        self.expect(Token::Equals, "`=` after the name")?;
        let value = self.expr()?;
        Ok(StmtKind::Let { name, value })
    }

    fn assign(&mut self) -> Result<StmtKind, String> {
        let name = self.name()?;
        self.expect(Token::Equals, "`=` after the name")?;
        let value = self.expr()?;
        Ok(StmtKind::Assign { name, value })
    }

    fn func(&mut self) -> Result<StmtKind, String> {
        self.enter()?;
        self.bump();
        let name = self.name()?;
        self.expect(Token::LParen, "`(` after the function name")?;
        let params = self.list(Self::name)?;
        self.expect(Token::LBrace, "`{` to start the function body")?;
        let body = self.statements(Token::RBrace)?;
        self.expect(Token::RBrace, "`}` to end the function body")?;
        self.leave();
        Ok(StmtKind::Func { name, params, body })
    }

    fn return_statement(&mut self) -> Result<StmtKind, String> {
        self.bump();
        Ok(StmtKind::Return(self.expr()?))
    }

    fn end_of_statement(&mut self) -> Result<(), String> {
        match self.peek() {
            Token::Newline => {
                self.bump();
                Ok(())
            }
            Token::Eof | Token::RBrace => Ok(()),
            _ => Err(self.error("expected the end of the line")),
        }
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.enter()?;
        let expr = self.binary(0)?;
        self.leave();
        Ok(expr)
    }

    fn binary(&mut self, min_strength: u8) -> Result<Expr, String> {
        let mut left = self.unary()?;

        while let Some((op, strength)) = binary_op(self.peek()) {
            if strength < min_strength {
                break;
            }
            self.bump();
            self.skip_newlines();
            let right = self.binary(strength + 1)?;
            let pos = left.pos;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                pos,
            };
        }

        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if *self.peek() != Token::Minus {
            return self.postfix();
        }

        let pos = self.pos();
        self.bump();
        self.enter()?;
        let inner = self.unary()?;
        self.leave();
        Ok(Expr {
            kind: ExprKind::Negate(Box::new(inner)),
            pos,
        })
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;

        while *self.peek() == Token::Dot {
            self.bump();
            let field = self.name()?;
            let pos = expr.pos;
            expr = Expr {
                kind: ExprKind::Field {
                    target: Box::new(expr),
                    field,
                },
                pos,
            };
        }

        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let pos = self.pos();
        let kind = match self.peek().clone() {
            Token::Num(value) => {
                self.bump();
                ExprKind::Num(value)
            }
            Token::Rgba(rgb) => {
                self.bump();
                ExprKind::Rgba(rgb)
            }
            Token::Name(name) => {
                self.bump();
                if *self.peek() == Token::LParen {
                    self.bump();
                    let args = self.list(Self::expr)?;
                    ExprKind::Call { name, args }
                } else {
                    ExprKind::Name(name)
                }
            }
            Token::LParen => {
                self.bump();
                self.skip_newlines();
                let inner = self.expr()?;
                self.skip_newlines();
                self.expect(Token::RParen, "`)`")?;
                return Ok(inner);
            }
            _ => return Err(self.error("expected a value")),
        };
        Ok(Expr { kind, pos })
    }

    fn list<T>(&mut self, item: impl Fn(&mut Self) -> Result<T, String>) -> Result<Vec<T>, String> {
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if *self.peek() == Token::RParen {
                break;
            }
            items.push(item(self)?);
            self.skip_newlines();
            if *self.peek() != Token::Comma {
                break;
            }
            self.bump();
        }
        self.expect(Token::RParen, "`,` or `)`")?;
        Ok(items)
    }

    fn name(&mut self) -> Result<String, String> {
        let Token::Name(name) = self.peek() else {
            return Err(self.error("expected a name"));
        };
        let name = name.clone();
        self.bump();
        Ok(name)
    }

    fn expect(&mut self, token: Token, what: &str) -> Result<(), String> {
        if *self.peek() != token {
            return Err(self.error(&format!("expected {what}")));
        }
        self.bump();
        Ok(())
    }

    fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error("too deeply nested"));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn skip_newlines(&mut self) {
        while *self.peek() == Token::Newline {
            self.bump();
        }
    }

    fn error(&self, message: &str) -> String {
        let pos = self.pos();
        format!("{message} at line {}, column {}", pos.line, pos.col)
    }

    fn bump(&mut self) {
        if self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.index].token
    }

    fn pos(&self) -> Pos {
        self.tokens[self.index].pos
    }
}

fn binary_op(token: &Token) -> Option<(BinOp, u8)> {
    match token {
        Token::Plus => Some((BinOp::Add, 1)),
        Token::Minus => Some((BinOp::Sub, 1)),
        Token::Star => Some((BinOp::Mul, 2)),
        Token::Slash => Some((BinOp::Div, 2)),
        _ => None,
    }
}

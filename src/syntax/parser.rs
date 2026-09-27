use super::ast::{BinOp, Expr, ExprKind, Program, Stmt, StmtKind};
use super::header::Header;
use super::token::{Pos, Spanned, Token};

const MAX_DEPTH: usize = 256;

pub fn parse(header: Header, tokens: Vec<Spanned>) -> Result<Program, String> {
    let body = Parser::new(tokens, false).statements(Token::Eof, Vec::new())?;
    Ok(Program { header, body })
}

pub fn parse_prelude(tokens: Vec<Spanned>) -> Result<Vec<Stmt>, String> {
    Parser::new(tokens, true).statements(Token::Eof, Vec::new())
}

struct Parser {
    tokens: Vec<Spanned>,
    index: usize,
    depth: usize,
    capitals: bool,
}

impl Parser {
    fn new(tokens: Vec<Spanned>, capitals: bool) -> Parser {
        Parser {
            tokens,
            index: 0,
            depth: 0,
            capitals,
        }
    }

    fn statements(&mut self, end: Token, mut defined: Vec<String>) -> Result<Vec<Stmt>, String> {
        let mut body = Vec::new();
        loop {
            self.skip_newlines();
            if *self.peek() == end || *self.peek() == Token::Eof {
                return Ok(body);
            }
            let stmt = self.statement()?;
            if let StmtKind::Input { name, .. }
            | StmtKind::Let { name, .. }
            | StmtKind::Func { name, .. } = &stmt.kind
            {
                if defined.contains(name) {
                    return Err(error_at(&format!("`{name}` is already defined"), stmt.pos));
                }
                defined.push(name.clone());
            }
            body.push(stmt);
        }
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        let pos = self.pos();
        let kind = match self.peek() {
            Token::Input => self.input()?,
            Token::Let => self.let_statement()?,
            Token::Func => self.func()?,
            Token::Return => self.return_statement()?,
            Token::Draw => self.draw()?,
            _ => return Err(self.error("expected a statement")),
        };
        self.end_of_statement()?;
        Ok(Stmt { kind, pos })
    }

    fn input(&mut self) -> Result<StmtKind, String> {
        self.bump();
        let name = self.new_name()?;
        let range = if *self.peek() == Token::Colon {
            self.bump();
            let min = self.expr()?;
            self.expect(Token::DotDot, "`..` between the minimum and maximum")?;
            let max = self.expr()?;
            Some((min, max))
        } else {
            None
        };
        self.expect(Token::Equals, "`=` and a default value")?;
        let default = self.expr()?;
        Ok(StmtKind::Input {
            name,
            range,
            default,
        })
    }

    fn let_statement(&mut self) -> Result<StmtKind, String> {
        self.bump();
        let name = self.new_name()?;
        self.expect(Token::Equals, "`=` after the name")?;
        let value = self.expr()?;
        Ok(StmtKind::Let { name, value })
    }

    fn func(&mut self) -> Result<StmtKind, String> {
        self.enter()?;
        self.bump();
        let name = self.new_name()?;
        self.expect(Token::LParen, "`(` after the function name")?;
        let params = self.list(Self::new_name)?;
        for (i, param) in params.iter().enumerate() {
            if params[..i].contains(param) {
                return Err(self.error(&format!("`{param}` is already a parameter")));
            }
        }
        self.expect(Token::LBrace, "`{` to start the function body")?;
        let body = self.statements(Token::RBrace, params.clone())?;
        self.expect(Token::RBrace, "`}` to end the function body")?;
        self.leave();
        Ok(StmtKind::Func { name, params, body })
    }

    fn return_statement(&mut self) -> Result<StmtKind, String> {
        self.bump();
        Ok(StmtKind::Return(self.expr()?))
    }

    fn draw(&mut self) -> Result<StmtKind, String> {
        self.bump();
        Ok(StmtKind::Draw(self.expr()?))
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
        let expr = if matches!(self.peek(), Token::Name(_)) && *self.peek_second() == Token::Arrow {
            self.lambda()?
        } else {
            self.binary(0)?
        };
        self.leave();
        Ok(expr)
    }

    fn lambda(&mut self) -> Result<Expr, String> {
        let pos = self.pos();
        let param = self.new_name()?;
        self.bump();
        self.skip_newlines();
        let body = self.expr()?;
        Ok(Expr {
            kind: ExprKind::Lambda {
                param,
                body: Box::new(body),
            },
            pos,
        })
    }

    fn binary(&mut self, min_strength: u8) -> Result<Expr, String> {
        let mut left = self.pipe()?;

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

    fn pipe(&mut self) -> Result<Expr, String> {
        let mut expr = self.unary()?;

        while self.pipe_follows() {
            self.skip_newlines();
            self.bump();
            self.skip_newlines();
            let pos = expr.pos;
            let name = self.name()?;
            let mut args = vec![expr];
            if *self.peek() == Token::LParen {
                self.bump();
                args.extend(self.list(Self::expr)?);
            }
            expr = Expr {
                kind: ExprKind::Call { name, args },
                pos,
            };
        }

        Ok(expr)
    }

    fn pipe_follows(&self) -> bool {
        self.tokens[self.index..]
            .iter()
            .map(|spanned| &spanned.token)
            .find(|token| **token != Token::Newline)
            == Some(&Token::Pipe)
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

    fn new_name(&mut self) -> Result<String, String> {
        let pos = self.pos();
        let name = self.name()?;
        let capital = name.chars().any(|c| c.is_ascii_uppercase())
            && !name.chars().any(|c| c.is_ascii_lowercase());
        if capital && !self.capitals {
            let message =
                format!("`{name}` is all capitals, which only the engine and prelude use");
            return Err(error_at(&message, pos));
        }
        Ok(name)
    }

    fn error(&self, message: &str) -> String {
        error_at(message, self.pos())
    }

    fn bump(&mut self) {
        if self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.index].token
    }

    fn peek_second(&self) -> &Token {
        let next = (self.index + 1).min(self.tokens.len() - 1);
        &self.tokens[next].token
    }

    fn pos(&self) -> Pos {
        self.tokens[self.index].pos
    }
}

fn error_at(message: &str, pos: Pos) -> String {
    format!("{message} at line {}, column {}", pos.line, pos.col)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::header::parse_header;
    use crate::syntax::lexer::lex;

    fn load(body: &str) -> Result<Program, String> {
        let src = format!("~fold v1 256x256\n{body}");
        let (header, rest) = parse_header(&src)?;
        parse(header, lex(rest, 2)?)
    }

    fn first_draw(body: &str) -> ExprKind {
        let program = load(body).unwrap();
        match program.body.into_iter().last().unwrap().kind {
            StmtKind::Draw(expr) => expr.kind,
            other => panic!("expected draw, found {other:?}"),
        }
    }

    #[test]
    fn cookbook_recipes_parse() {
        let book = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/COOKBOOK.md"));
        let recipes: Vec<&str> = book.split("```").skip(1).step_by(2).collect();
        assert_eq!(recipes.len(), book.matches("\n### ").count());
        for recipe in recipes {
            if let Err(e) = load(recipe.trim_start()) {
                panic!("{e}\n{recipe}");
            }
        }
    }

    #[test]
    fn spec_example_parses() {
        let spec = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/SPEC.md"));
        let example = spec.split("```").nth(1).unwrap();
        let (header, rest) = parse_header(example.trim_start()).unwrap();
        parse(header, lex(rest, 2).unwrap()).unwrap();
    }

    #[test]
    fn pipe_becomes_call_with_subject_first() {
        let ExprKind::Call { name, args } = first_draw("draw sun |> fill(#ffaa00)") else {
            panic!("expected a call");
        };
        assert_eq!(name, "fill");
        assert_eq!(args[0].kind, ExprKind::Name("sun".into()));
        assert_eq!(args[1].kind, ExprKind::Rgba(0xffaa00ff));
    }

    #[test]
    fn pipe_binds_tighter_than_multiply() {
        let ExprKind::Binary { op, left, .. } = first_draw("draw sun |> fill(#ffaa00) * 0.5")
        else {
            panic!("expected a binary");
        };
        assert_eq!(op, BinOp::Mul);
        assert!(matches!(left.kind, ExprKind::Call { .. }));
    }

    #[test]
    fn pipe_step_without_brackets() {
        let ExprKind::Call { name, args } = first_draw("draw petal |> mirror") else {
            panic!("expected a call");
        };
        assert_eq!(name, "mirror");
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn pipe_continues_on_next_line() {
        let ExprKind::Call { name, .. } =
            first_draw("draw sun\n    |> at(1, 2)\n    |> fill(#ffaa00)")
        else {
            panic!("expected a call");
        };
        assert_eq!(name, "fill");
    }

    #[test]
    fn inline_function() {
        let ExprKind::Call { args, .. } = first_draw("draw shape(pt => length(pt) - 60)") else {
            panic!("expected a call");
        };
        assert!(matches!(&args[0].kind, ExprKind::Lambda { param, .. } if param == "pt"));
    }

    #[test]
    fn colour_input_has_no_range() {
        let program = load("input accent = #3366ff").unwrap();
        assert!(matches!(
            program.body[0].kind,
            StmtKind::Input { range: None, .. }
        ));
    }

    #[test]
    fn eight_digit_colour_keeps_alpha() {
        let ExprKind::Call { args, .. } = first_draw("draw sun |> fill(#00000066)") else {
            panic!("expected a call");
        };
        assert_eq!(args[1].kind, ExprKind::Rgba(0x00000066));
    }

    #[test]
    fn reserved_word_is_an_error() {
        let error = load("let if = 1").unwrap_err();
        assert!(error.contains("`if` is reserved"), "{error}");
    }
}

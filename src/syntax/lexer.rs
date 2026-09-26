use super::token::{Pos, Spanned, Token};
use std::iter::Peekable;
use std::str::Chars;

pub fn lex(src: &str, first_line: usize) -> Result<Vec<Spanned>, String> {
    let mut lexer = Lexer {
        chars: src.chars().peekable(),
        line: first_line,
        col: 1,
    };
    let mut tokens = Vec::new();

    loop {
        lexer.skip_blanks();
        let pos = Pos {
            line: lexer.line,
            col: lexer.col,
        };

        let Some(c) = lexer.bump() else {
            tokens.push(Spanned {
                token: Token::Eof,
                pos,
            });
            return Ok(tokens);
        };

        let token = lexer
            .token(c)
            .map_err(|e| format!("{e} at line {}, column {}", pos.line, pos.col))?;
        tokens.push(Spanned { token, pos });
    }
}

struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    line: usize,
    col: usize,
}

impl Lexer<'_> {
    fn token(&mut self, c: char) -> Result<Token, String> {
        let token = match c {
            '\n' => Token::Newline,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '=' => Token::Equals,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            ',' => Token::Comma,
            ':' => Token::Colon,
            '.' if self.peek() == Some('.') => {
                self.bump();
                Token::DotDot
            }
            '.' => Token::Dot,
            '#' => return self.colour(),
            '0'..='9' => self.number(c),
            'a'..='z' | 'A'..='Z' | '_' => self.word(c),
            _ => return Err(format!("unexpected `{c}`")),
        };
        Ok(token)
    }

    fn number(&mut self, first: char) -> Token {
        let mut text = format!("{first}{}", self.take_while(|c| c.is_ascii_digit()));

        if self.peek() == Some('.') && self.peek_second().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
            text += ".";
            text += &self.take_while(|c| c.is_ascii_digit());
        }

        Token::Number(text.parse().expect("digits with at most one '.'"))
    }

    fn word(&mut self, first: char) -> Token {
        let word = format!(
            "{first}{}",
            self.take_while(|c| c.is_ascii_alphanumeric() || c == '_')
        );

        match word.as_str() {
            "let" => Token::Let,
            "input" => Token::Input,
            "func" => Token::Func,
            "return" => Token::Return,
            _ => Token::Name(word),
        }
    }

    fn colour(&mut self) -> Result<Token, String> {
        let hex = self.take_while(|c| c.is_ascii_alphanumeric());

        match u32::from_str_radix(&hex, 16) {
            Ok(rgb) if hex.len() == 6 => Ok(Token::Colour(rgb)),
            _ => Err(format!(
                "bad colour `#{hex}`, expected 6 hex digits like `#ffaa00`"
            )),
        }
    }

    fn skip_blanks(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | '\r' => {
                    self.bump();
                }
                '/' if self.peek_second() == Some('/') => {
                    self.take_while(|c| c != '\n');
                }
                _ => return,
            }
        }
    }

    fn take_while(&mut self, keep: impl Fn(char) -> bool) -> String {
        let mut text = String::new();
        while let Some(c) = self.peek().filter(|&c| keep(c)) {
            self.bump();
            text.push(c);
        }
        text
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn peek_second(&self) -> Option<char> {
        self.chars.clone().nth(1)
    }
}

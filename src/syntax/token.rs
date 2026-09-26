#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Let,
    Input,
    Func,
    Return,

    Name(String),
    Number(f32),
    Colour(u32),

    Plus,
    Minus,
    Star,
    Slash,
    Equals,

    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    DotDot,

    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub line: usize,
    pub col: usize,
}

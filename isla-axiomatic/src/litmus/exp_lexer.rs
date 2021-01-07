
use std::fmt;

use isla_lib::lexer::*;
use crate::litmus::exp::ExpParseError;

pub struct ExpLexer<'input> {
    lexer: Lexer<'input>,
}

impl<'input> ExpLexer<'input> {
    pub fn new(input: &'input str) -> Self {
        ExpLexer { lexer: Lexer::new(input) }
    }
}

#[derive(Clone, Debug)]
pub enum Tok<'input> {
    Nat(&'input str),
    Id(&'input str),
    Hex(&'input str),
    Bin(&'input str),
    Implies,
    Not,
    And,
    Or,
    Lparen,
    Rparen,
    Colon,
    Eq,
    Star,
    Comma,
    True,
    False
}

impl<'input> fmt::Display for Tok<'input> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct Keyword {
    word: &'static str,
    token: Tok<'static>,
    len: usize,
}

impl Keyword {
    pub fn new(kw: &'static str, tok: Tok<'static>) -> Self {
        Keyword { word: kw, token: tok, len: kw.len() }
    }
}

lazy_static! {
    static ref KEYWORDS: Vec<Keyword> = {
        use Tok::*;
        let mut table = Vec::new();
        table.push(Keyword::new("->", Implies));
        table.push(Keyword::new("~", Not));
        table.push(Keyword::new("&", And));
        table.push(Keyword::new("|", Or));
        table.push(Keyword::new("(", Lparen));
        table.push(Keyword::new(")", Rparen));
        table.push(Keyword::new(":", Colon));
        table.push(Keyword::new("=", Eq));
        table.push(Keyword::new("*", Star));
        table.push(Keyword::new(",", Comma));
        table.push(Keyword::new("true", True));
        table.push(Keyword::new("false", False));
        table
    };
}

pub type Span<'input> = Result<(usize, Tok<'input>, usize), ExpParseError>;

impl<'input> Iterator for ExpLexer<'input> {
    type Item = Span<'input>;

    fn next(&mut self) -> Option<Self::Item> {
        use Tok::*;
        self.lexer.consume_whitespace()?;
        let start_pos = self.lexer.pos;

        for k in KEYWORDS.iter() {
            if self.lexer.buf.starts_with(k.word) {
                self.lexer.pos += k.len;
                self.lexer.buf = &self.lexer.buf[k.len..];
                return Some(Ok((start_pos, k.token.clone(), self.lexer.pos)));
            }
        }

        match self.lexer.consume_regex(&ID_REGEX) {
            None => (),
            Some((from, id, to)) => return Some(Ok((from, Id(id), to))),
        }
 
        match self.lexer.consume_regex(&HEX_REGEX) {
            None => (),
            Some((from, bits, to)) => return Some(Ok((from, Hex(&bits), to))),
        }

        match self.lexer.consume_regex(&BIN_REGEX) {
            None => (),
            Some((from, bits, to)) => return Some(Ok((from, Bin(&bits), to))),
        }

        match self.lexer.consume_regex(&NAT_REGEX) {
            None => (),
            Some((from, n, to)) => return Some(Ok((from, Nat(n), to))),
        }

        Some(Err(ExpParseError::Lex { pos: self.lexer.pos }))
    }
}

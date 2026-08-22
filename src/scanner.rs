//! This file scans the grammar.

use std::{iter::Peekable, str::CharIndices};

pub enum TokenKind {
    At,
    Colon,
    VerticalBar,
    OpenParen,
    CloseParen,
    Star,
    Plus,
    Question,
    Equal,
    Arrow,

    Identifier(String),
    String(String),
    Eof,

    Error,
}

pub struct Scanner<'a> {
    source: &'a str,
    chars: Peekable<CharIndices<'a>>,
    level: u32,
}

impl<'a> Scanner<'a> {
    fn new(source: &'a str) -> Self {
        Scanner {
            source,
            chars: source.char_indices().peekable(),
            level: 0,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        self.chars.next()
    }

    fn position(&mut self) -> usize {
        self.chars.peek().map_or(self.source.len(), |&(pos, _)| pos)
    }

    fn skip_whitespace(&mut self) {
        while let Some(' ' | '\t' | '\r' | '\n') = self.peek() {
            self.advance();
        }
    }

    fn scan_identifier(&mut self, start: usize) -> String {
        while let Some('a'..='z' | 'A'..='Z' | '0'..='9' | '_') = self.peek() {
            self.advance();
        }
        self.source[start..self.position()].to_string()
    }

    fn scan_string(&mut self, start: usize) -> Option<String> {
        while let Some((_, character)) = self.advance() {
            if character == '"' {
                return Some(self.source[start..self.position()].to_string())
            }
        }
        None
    }

    fn next_token(&mut self) -> TokenKind {
        self.skip_whitespace();

        let (start, character) = match self.advance() {
            Some(result) => result,
            None => return TokenKind::Eof,
        };

        match character {
            '@' => TokenKind::At,
            ':' => TokenKind::Colon,
            '|' => TokenKind::VerticalBar,
            '(' => {
                self.level += 1;
                TokenKind::OpenParen
            },
            ')' => {
                if self.level == 0 {
                    TokenKind::Error
                }
                else {
                    self.level -= 1;
                    TokenKind::CloseParen
                }
            },
            '*' => TokenKind::Star,
            '+' => TokenKind::Plus,
            '?' => TokenKind::Question,
            '=' => {
                if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::Arrow
                }
                else{
                    TokenKind::Equal
                }
            },
            'a'..='z' | 'A'..='Z' | '_' => TokenKind::Identifier(self.scan_identifier(start)),
            '"' => match self.scan_string(start) {
                Some(content) => TokenKind::String(content),
                None => TokenKind::Error,
            },
            _ => TokenKind::Error
        }
    }
}


impl<'a> Iterator for Scanner<'a> {
    type Item = TokenKind;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_token() {
            TokenKind::Eof => None,
            token => Some(token), 
        }
    }
}

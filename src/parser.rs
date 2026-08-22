use std::iter::Peekable;

use crate::ast::{
    SymbolId, RuleId, ProductionId, ArenaRange, AstArena,
    TerminalDef, NonterminalDef, Production, RuleKind
};
use crate::scanner::{Scanner, TokenKind};

pub enum GrammarErrorKind {
    InvalidTerminalDef,
    InvalidNonterminalDef,
    Eof
}

pub struct GrammarError<'a> {
    pub kind: GrammarErrorKind,
    pub token: Option<TokenKind<'a>>,
}

pub struct GrammarParser<'a> {
    arena: AstArena<'a>,
    scanner: Peekable<Scanner<'a>>,
}

impl<'a> GrammarParser<'a> {
    fn new(scanner: Scanner<'a>) -> Self {
        Self {
            arena: AstArena::new(),
            scanner: scanner.peekable()
        }
    }

    fn parse_toplevel_item(&mut self) -> Result<SymbolId, GrammarError<'a>> {
        match self.scanner.next() {
            Some(token @ TokenKind::At) => self.parse_nonterminal_def(token),
            Some(token) => self.parse_terminal_def(token).or_else(
                |err| self.parse_nonterminal_def(err.token.unwrap())),
            None => Err(GrammarError { kind: GrammarErrorKind::Eof, token: None }),
        }
    }

    fn parse_terminal_def(&mut self, token: TokenKind<'a>) -> Result<SymbolId, GrammarError<'a>> {
        match token {
            TokenKind::Identifier(name) => match self.scanner.peek() {
                Some(TokenKind::Equal) => {
                    self.scanner.next();
                    match self.scanner.next() {
                        Some(TokenKind::String(content)) => Ok(self.arena.add_terminal(name, content)),
                        next => Err(
                            GrammarError { kind: GrammarErrorKind::InvalidTerminalDef, token: next }
                        ),
                    }
                }
                _ => Err(GrammarError { kind: GrammarErrorKind::InvalidTerminalDef, token: Some(token) }),
            },
            _ => panic!("Terminal declaration should be identifier")
        }
    }

    fn parse_nonterminal_def(&mut self, token: TokenKind<'a>) -> Result<SymbolId, GrammarError<'a>> {
        let (is_entrypoint, next_token) = match token {
            TokenKind::At => (true, self.scanner.next()),
            _ => (false, Some(token)),
        };

        let Some(TokenKind::Identifier(name)) = next_token else {
            return Err(GrammarError { kind: GrammarErrorKind::InvalidNonterminalDef, token: next_token })
        };

        match self.scanner.next() {
            Some(TokenKind::Colon) => {}
            next => return Err(GrammarError { kind: GrammarErrorKind::InvalidNonterminalDef, token: next })
        }

        let start = self.arena.production_bound();
        while let Some(TokenKind::VerticalBar) = self.scanner.peek() {
            self.scanner.next();
            self.parse_rule_group();
        }
        let end = self.arena.production_bound();
        Ok(self.arena.add_nonterminal(name, is_entrypoint, ArenaRange { start, end }))
    }

    fn parse_rule_group(&mut self) -> RuleId {
        todo!()
    }

    fn parse_rule_group_item(&mut self) -> RuleId {
        todo!()
    }

    fn parse_rule_suffix(&mut self) -> RuleId {
        todo!()
    }

    fn parse_rule_atom(&mut self) -> RuleId {
        todo!()
    }
}

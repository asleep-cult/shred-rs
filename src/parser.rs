use std::iter::Peekable;

use crate::ast::{
    SymbolId, RuleId, ProductionId, ArenaRange, AstArena,
    TerminalDef, NonterminalDef, Production, RuleKind
};
use crate::scanner::{Scanner, TokenKind};

pub enum GrammarErrorKind {
    InvalidTerminalDef,
    InvalidNonterminalDef,
    InvalidRuleItem,
    MaybeNonterminal,
    Eof,
}

pub struct GrammarError<'a> {
    pub kind: GrammarErrorKind,
    pub token: Option<TokenKind<'a>>,
}

pub struct GrammarParser<'a> {
    pub arena: AstArena<'a>,
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
            Some(token) => match self.parse_terminal_def(token) {
                Err(GrammarError { kind: GrammarErrorKind::MaybeNonterminal, token: Some(token) }) => 
                    self.parse_nonterminal_def(token),
                result => result,
            },
            None => Err(GrammarError { kind: GrammarErrorKind::Eof, token: None }),
        }
    }

    fn parse_terminal_def(&mut self, token: TokenKind<'a>) -> Result<SymbolId, GrammarError<'a>> {
        let TokenKind::Identifier(name) = token else {
            return Err(GrammarError { kind: GrammarErrorKind::InvalidTerminalDef, token: Some(token) });
        };

        match self.scanner.peek() {
            Some(TokenKind::Equal) => {
                self.scanner.next();
                match self.scanner.next() {
                    Some(TokenKind::String(content)) => Ok(self.arena.add_terminal(name, content)),
                    next => Err(GrammarError { kind: GrammarErrorKind::InvalidTerminalDef, token: next }),
                }
            }
            _ => Err(GrammarError { kind: GrammarErrorKind::MaybeNonterminal, token: Some(token) }),
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
            self.parse_production()?;
        }
        let end = self.arena.production_bound();
        Ok(self.arena.add_nonterminal(name, is_entrypoint, ArenaRange { start, end }))
    }

    fn parse_production(&mut self) -> Result<ProductionId, GrammarError<'a>> {
        let rule = self.parse_rule_group()?;
        let action = match self.scanner.peek() {
            Some(TokenKind::Identifier(_)) => {
                let Some(TokenKind::Identifier(action)) = self.scanner.next() else {
                    unreachable!()
                };
                // I did this instead of using the match arm because the action is &&str
                // from peek. The compiler implores me to use .map(|v| &**v) which seems
                // cursed to an innocent Python dev like myself.
                Some(action)
            },
            _ => None,
        };

        Ok(self.arena.add_production(rule, action))
    }

    fn parse_rule_group(&mut self) -> Result<RuleId, GrammarError<'a>> {
        let start = self.arena.rule_bound();
        while let Some(TokenKind::OpenParen | TokenKind::Identifier(_) | TokenKind::String(_)) = self.scanner.peek() {
            self.parse_rule_group_item()?;
        }
        let end = self.arena.rule_bound();

        let rule = if start + 1 == end {
            RuleId(start as u32)
        }
        else {
            self.arena.add_rule(RuleKind::Group { items: ArenaRange { start, end } })
        };
        Ok(rule)
    }

    fn parse_rule_group_item(&mut self) -> Result<RuleId, GrammarError<'a>> {
        let item = match self.scanner.next() {
            Some(TokenKind::OpenParen) => {
                let group = self.parse_rule_group()?;

                match self.scanner.next() {
                    Some(TokenKind::CloseParen) => group,
                    next => return Err(GrammarError { kind: GrammarErrorKind::InvalidRuleItem, token: next }),
                }
            },
            Some(token @ (TokenKind::Identifier(_) | TokenKind::String(_))) => self.parse_rule_atom(token),
            token => return Err(GrammarError { kind: GrammarErrorKind::InvalidRuleItem, token: token })
        };

        Ok(self.parse_rule_suffix(item))
    }

    fn parse_rule_suffix(&mut self, item: RuleId) -> RuleId {
        match self.scanner.peek() {
            Some(TokenKind::Star) => {
                self.scanner.next();
                self.arena.add_rule(RuleKind::Star(item))
            },
            Some(TokenKind::Plus) => {
                self.scanner.next();
                self.arena.add_rule(RuleKind::Plus(item))
            },
            Some(TokenKind::Question) => {
                self.scanner.next();
                self.arena.add_rule(RuleKind::Optional(item))
            },
            _ => item,
        }
    }

    fn parse_rule_atom(&mut self, token: TokenKind<'a>) -> RuleId {
        match token {
            TokenKind::Identifier(content) => self.arena.add_rule(RuleKind::Name(content)),
            TokenKind::String(content) => self.arena.add_rule(RuleKind::String(content)),
            _ => panic!("parse_rule_atom only takes identifier or string"),
        }
    }
}

use std::iter::Peekable;

use crate::ast::{RuleId, AstArena, RuleKind};
use crate::scanner::{Scanner, TokenKind};

#[derive(Debug)]
pub enum GrammarErrorKind {
    InvalidTerminalDef,
    InvalidNonterminalDef,
    InvalidRuleItem,
    MaybeNonterminal,
    Eof,
}

#[derive(Debug)]
pub struct GrammarError {
    pub kind: GrammarErrorKind,
    pub token: Option<TokenKind>,
}

pub struct GrammarParser<'a> {
    pub arena: AstArena,
    scanner: Peekable<Scanner<'a>>,
}

impl<'a> GrammarParser<'a> {
    pub fn new(scanner: Scanner<'a>) -> Self {
        Self {
            arena: AstArena::new(),
            scanner: scanner.peekable()
        }
    }

    pub fn parse_ast(mut self) -> Result<AstArena, GrammarError> {
        while let Some(_) = self.scanner.peek() {
            self.parse_toplevel_item()?;
        }
        Ok(self.arena)
    }

    pub fn skip_newline(&mut self) {
        if let Some(TokenKind::Newline) = self.scanner.peek() {
            self.scanner.next();
        }
    }

    fn parse_toplevel_item(&mut self) -> Result<(), GrammarError> {
        self.skip_newline();

        match self.scanner.next() {
            Some(token @ TokenKind::At) => self.parse_nonterminal_def(token),
            Some(token) => match self.parse_terminal_def(token) {
                Err(GrammarError { kind: GrammarErrorKind::MaybeNonterminal, token: Some(token) }) => 
                    self.parse_nonterminal_def(token),
                result => result,
            }
            None => Err(GrammarError { kind: GrammarErrorKind::Eof, token: None }),
        }
    }

    fn parse_terminal_def(&mut self, token: TokenKind) -> Result<(), GrammarError> {
        match self.scanner.peek() {
            Some(TokenKind::Equal) => {
                let TokenKind::Identifier(name) = token else {
                    return Err(GrammarError { kind: GrammarErrorKind::InvalidTerminalDef, token: Some(token) });
                };

                self.scanner.next();
                match self.scanner.next() {
                    Some(TokenKind::String(content)) => {
                        self.arena.add_terminal(name, Some(content));
                        Ok(())
                    }
                    next => Err(GrammarError { kind: GrammarErrorKind::InvalidTerminalDef, token: next }),
                }
            }
            Some(TokenKind::Colon) => Err(GrammarError { kind: GrammarErrorKind::MaybeNonterminal, token: Some(token) }),
            _ => match token {
                TokenKind::Identifier(name) => {
                    self.arena.add_terminal(name, None);
                    Ok(())
                }
                _ => Err(GrammarError { kind: GrammarErrorKind::InvalidTerminalDef, token: Some(token) })
            }
        }
    }

    fn parse_nonterminal_def(&mut self, token: TokenKind) -> Result<(), GrammarError> {
        let (entrypoint, next_token) = match token {
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

        self.skip_newline();
        let start = self.arena.production_bound();
        self.scanner.next_if(|tok| matches!(tok, TokenKind::VerticalBar));
        self.parse_production()?;

        self.skip_newline();
        while let Some(TokenKind::VerticalBar) = self.scanner.peek() {
            self.scanner.next();
            self.parse_production()?;
            self.skip_newline();
        }
        let end = self.arena.production_bound();
        self.arena.add_nonterminal(name, entrypoint, end - start);
        Ok(())
    }

    fn parse_production(&mut self) -> Result<(), GrammarError> {
        let rule = self.parse_rule_group()?;
        let action = match self.scanner.peek() {
            Some(TokenKind::Arrow) => {
                self.scanner.next();
                self.skip_newline();
                match self.scanner.next() {
                    Some(TokenKind::Identifier(action)) => Some(action),
                    token => {
                        return Err(GrammarError { kind: GrammarErrorKind::InvalidNonterminalDef, token });
                    }
                }
            },
            _ => None,
        };

        self.arena.add_production(rule, action);
        Ok(())
    }

    fn parse_rule_group(&mut self) -> Result<RuleId, GrammarError> {
        let mut items = Vec::new();
        while let Some(TokenKind::OpenParen | TokenKind::Identifier(_) | TokenKind::String(_)) = self.scanner.peek() {
            items.push(self.parse_rule_group_item()?);
        }

        let rule = if items.len() == 0 {
            items[0]
        }
        else {
            self.arena.add_rule(RuleKind::Group { items })
        };

        if let Some(TokenKind::VerticalBar) = self.scanner.peek() {
            self.scanner.next();
            let right = self.parse_rule_group()?;
            Ok(self.arena.add_rule(RuleKind::Alternative { left: rule, right }))
        }
        else {
            Ok(rule)
        }
    }

    fn parse_rule_group_item(&mut self) -> Result<RuleId, GrammarError> {
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
            }
            Some(TokenKind::Plus) => {
                self.scanner.next();
                self.arena.add_rule(RuleKind::Plus(item))
            }
            Some(TokenKind::Question) => {
                self.scanner.next();
                self.arena.add_rule(RuleKind::Optional(item))
            }
            _ => item,
        }
    }

    fn parse_rule_atom(&mut self, token: TokenKind) -> RuleId {
        match token {
            TokenKind::Identifier(content) => self.arena.add_rule(RuleKind::Name(content)),
            TokenKind::String(content) => self.arena.add_rule(RuleKind::String(content)),
            _ => panic!("parse_rule_atom only takes identifier or string"),
        }
    }
}

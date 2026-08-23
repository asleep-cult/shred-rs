//! This file defines the AST for the grammar.

#[derive(Clone, Copy)]
pub struct TerminalId(pub u16);

#[derive(Clone, Copy)]
pub struct NonterminalId(pub u16);

#[derive(Clone, Copy)]
pub struct RuleId(pub u16);

#[derive(Clone, Copy)]
pub struct ProductionId(pub u16);

pub struct ArenaRange {
    pub start: usize,
    pub end: usize,
}

pub struct AstArena {
    pub(crate) terminals: Vec<TerminalDef>,
    pub(crate) nonterminals: Vec<NonterminalDef>,
    pub(crate) productions: Vec<Production>,
    pub(crate) rules: Vec<RuleKind>,
}

impl AstArena {
    pub fn new() -> Self {
        Self {
            terminals: Vec::new(),
            nonterminals: Vec::new(),
            productions: Vec::new(),
            rules: Vec::new(),
        }
    }

    pub fn add_terminal(&mut self, name: String, value: Option<String>) -> TerminalId {
        let id = TerminalId(self.terminals.len() as u16);
        self.terminals.push(TerminalDef { name, value });
        id
    }

    pub fn add_nonterminal(&mut self, name: String, entrypoint: bool, productions: usize) -> NonterminalId {
        let id = NonterminalId(self.terminals.len() as u16);
        self.nonterminals.push(NonterminalDef { name, entrypoint, productions });
        id
    }

    pub fn add_production(&mut self, rule: RuleId, action: Option<String>) -> ProductionId {
        let id = ProductionId(self.productions.len() as u16);
        self.productions.push(Production { rule, action });
        id
    }

    pub fn add_rule(&mut self, rule: RuleKind) -> RuleId {
        let id = RuleId(self.rules.len() as u16);
        self.rules.push(rule);
        id
    }

    pub fn production_bound(&self) -> usize {
        self.productions.len()
    }

    pub fn rule_bound(&self) -> usize {
        self.rules.len()
    }
}

pub struct TerminalDef {
    pub name: String,
    pub value: Option<String>,
}

pub struct NonterminalDef {
    pub name: String,
    pub entrypoint: bool,
    pub productions: usize,
}

pub struct Production {
    pub rule: RuleId,
    pub action: Option<String>,
}

pub enum RuleKind {
    Star(RuleId),
    Plus(RuleId),
    Optional(RuleId),
    Alternative { left: RuleId, right: RuleId },
    Group { items: ArenaRange },
    String(String),
    Name(String),
}

//! This file defines the AST for the grammar.

#[derive(Debug, Clone, Copy)]
pub struct RuleId(pub u16);

#[derive(Debug)]
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

    pub fn add_terminal(&mut self, name: String, value: Option<String>) {
        self.terminals.push(TerminalDef { name, value });
    }

    pub fn add_nonterminal(&mut self, name: String, entrypoint: bool, productions: usize) {
        self.nonterminals.push(NonterminalDef { name, entrypoint, productions });
    }

    pub fn add_production(&mut self, rule: RuleId, action: Option<String>) {
        self.productions.push(Production { rule, action });
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

#[derive(Debug)]
pub struct TerminalDef {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug)]
pub struct NonterminalDef {
    pub name: String,
    pub entrypoint: bool,
    pub productions: usize,
}

#[derive(Debug)]
pub struct Production {
    pub rule: RuleId,
    pub action: Option<String>,
}

#[derive(Debug)]
pub enum RuleKind {
    Star(RuleId),
    Plus(RuleId),
    Optional(RuleId),
    Alternative { left: RuleId, right: RuleId },
    Group { items: Vec<RuleId> },
    String(String),
    Name(String),
}

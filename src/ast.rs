//! This file defines the AST for the grammar.

pub struct SymbolId(pub u32);
pub struct RuleId(pub u32);
pub struct ProductionId(pub u32);

pub struct ArenaRange {
    pub start: u32,
    pub end: u32,
}

pub struct AstArena<'a> {
    terminals: Vec<TerminalDef<'a>>,
    nonterminals: Vec<NonterminalDef<'a>>,
    productions: Vec<Production<'a>>,
    rules: Vec<RuleKind<'a>>,
}

impl<'a> AstArena<'a> {
    pub fn new() -> Self {
        Self {
            terminals: Vec::new(),
            nonterminals: Vec::new(),
            productions: Vec::new(),
            rules: Vec::new(),
        }
    }

    pub fn add_terminal(&mut self, name: &'a str, value: &'a str) -> SymbolId {
        let id = SymbolId(self.terminals.len() as u32);
        self.terminals.push(TerminalDef { name, value });
        id
    }

    pub fn add_nonterminal(&mut self, name: &'a str, is_entrypoint: bool, productions: ArenaRange) -> SymbolId {
        let id = SymbolId(self.terminals.len() as u32);
        self.nonterminals.push(NonterminalDef { name, is_entrypoint, productions });
        id
    }

    pub fn add_production(&mut self, rule: RuleId, action: Option<&'a str>) -> ProductionId {
        let id = ProductionId(self.productions.len() as u32);
        self.productions.push(Production { rule, action });
        id
    }

    pub fn add_rule(&mut self, rule: RuleKind<'a>) -> RuleId {
        let id = RuleId(self.rules.len() as u32);
        self.rules.push(rule);
        id
    }

    pub fn terminal(&self, id: SymbolId) -> &TerminalDef {
        &self.terminals[id.0 as usize]
    }

    pub fn nonterminal(&self, id: SymbolId) -> &NonterminalDef {
        &self.nonterminals[id.0 as usize]
    }

    pub fn production(&self, id: ProductionId) -> &Production {
        &self.productions[id.0 as usize]
    }

    pub fn rule(&self, id: RuleId) -> &RuleKind {
        &self.rules[id.0 as usize]
    }

    pub fn production_range(&self, range: ArenaRange) -> &'a [Production] {
        &self.productions[range.start as usize..range.end as usize]
    }

    pub fn rule_range(&self, range: ArenaRange) -> &'a [RuleKind] {
        &self.rules[range.start as usize..range.end as usize]
    }

    pub fn production_bound(&self) -> u32 {
        self.productions.len() as u32
    }

    pub fn rule_bound(&self) -> u32 {
        self.rules.len() as u32
    }
}

pub struct TerminalDef<'a> {
    name: &'a str,
    value: &'a str,
}

pub struct NonterminalDef<'a> {
    name: &'a str,
    is_entrypoint: bool,
    productions: ArenaRange,
}

pub struct Production<'a> {
    rule: RuleId,
    action: Option<&'a str>,
}

pub enum RuleKind<'a> {
    Star(RuleId),
    Plus(RuleId),
    Optional(RuleId),
    Alternative { left: RuleId, right: RuleId },
    Group { items: ArenaRange },
    String(&'a str),
    Name(&'a str),
}

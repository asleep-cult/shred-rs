use std::collections::HashMap;

use crate::ast::{ProductionId, SymbolId};


enum SymbolKind {
    Nonterminal { name: String, entrypoint: bool, productions: Vec<Production> },
    Terminal { name: String, value: Option<String> },
}

pub struct Symbol {
    id: SymbolId,
    kind: SymbolKind,
}

struct Production {
    lhs_id: ProductionId,
    rhs: Vec<SymbolId>,
    action: Option<String>,
}

pub struct InternedSymbols {
    terminals: Vec<Symbol>,
    nonterminals: Vec<Symbol>,
    terminal_map: HashMap<String, SymbolId>,
    nonterminal_map: HashMap<String, SymbolId>,
}

impl InternedSymbols {
    pub fn add_terminal(&mut self, id: SymbolId, name: String, value: Option<String>) {
        self.terminals.push(Symbol {
            id, kind: SymbolKind::Terminal { name: name.clone(), value },
        });
        self.terminal_map.insert(name, id);
    }

    pub fn add_nonterminal(&mut self, id: SymbolId, name: String, entrypoint: bool) {
        self.nonterminals.push(Symbol { 
            id, kind: SymbolKind::Nonterminal { name: name.clone(), entrypoint, productions: Vec::new() }
        });
        self.nonterminal_map.insert(name, id);
    }
}

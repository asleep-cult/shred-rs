use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct SymbolId(pub u16);

pub const EOF_ID: SymbolId = SymbolId(0);

#[derive(Clone, Copy)]
pub struct ProductionId(pub u16);


pub enum SymbolKind {
    Nonterminal { entrypoint: bool, productions: Vec<ProductionId> },
    Terminal { value: Option<String> },
}

pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
}

pub struct NonterminalProduction {
    pub id: ProductionId,
    pub lhs_id: SymbolId,
    pub rhs: Vec<SymbolId>,
    pub action: Option<String>,
}

pub struct InternedSymbols {
    pub(crate) terminals: Vec<Symbol>,
    pub(crate) nonterminals: Vec<Symbol>,
    pub(crate) productions: Vec<NonterminalProduction>,
    terminal_map: HashMap<String, SymbolId>,
    nonterminal_map: HashMap<String, SymbolId>,
    prod_id_count: u16,
}

impl InternedSymbols {
    pub fn search_terminal(&self, name: &str) -> Option<&Symbol> {
        self.terminal_map.get(name).map(|&id| &self.nonterminals[self.terminal_index(id)])
    }

    pub fn search_nonterminal(&self, name: &str) -> Option<&Symbol> {
        self.nonterminal_map.get(name).map(|&id| &self.nonterminals[self.nonterminal_index(id)])
    }

    pub fn terminal(&self, id: SymbolId) -> &Symbol {
        &self.terminals[self.terminal_index(id)]
    }

    pub fn nonterminal(&self, id: SymbolId) -> &Symbol {
        &self.nonterminals[self.nonterminal_index(id)]
    }

    pub fn production(&self, id: ProductionId) -> &NonterminalProduction {
        &self.productions[id.0 as usize]
    }

    pub fn add_terminal(&mut self, id: SymbolId, name: String, value: Option<String>) { 
        let key = value.clone().unwrap_or_else(|| name.clone());

        self.terminals.push(Symbol {
            id, name: name, kind: SymbolKind::Terminal { value },
        });
        self.terminal_map.insert(key, id);
    }

    pub fn add_nonterminal(&mut self, id: SymbolId, name: String, entrypoint: bool) {
        self.nonterminals.push(Symbol { 
            id, name: name.clone(), kind: SymbolKind::Nonterminal { entrypoint, productions: Vec::new() }
        });
        self.nonterminal_map.insert(name, id);
    }

    pub fn terminal_index(&self, id: SymbolId) -> usize {
        id.0 as usize
    }

    pub fn nonterminal_index(&self, id: SymbolId) -> usize {
        id.0 as usize - self.terminals.len()
    }

    pub fn next_sym_id(&self) -> SymbolId {
        // SymbolId(0) reserved for EOF
        SymbolId((self.terminals.len() + self.nonterminals.len() + 1) as u16)
    }

    pub fn next_prod_id(&mut self) -> ProductionId {
        let id = ProductionId(self.prod_id_count);
        self.prod_id_count += 1;
        id
    }

    pub fn add_production(&mut self, production: NonterminalProduction) {
        let index = self.nonterminal_index(production.lhs_id);
        let Symbol {
            kind: SymbolKind::Nonterminal { productions, .. }, ..
        } = &mut self.nonterminals[index] else { unreachable!() };

        productions.push(production.id);
        self.productions.push(production);
    }
}

use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct SymbolId(pub u32);

#[derive(Clone, Copy)]
pub struct ProductionId(pub u32);


pub enum SymbolKind {
    Nonterminal { entrypoint: bool, productions: Vec<ProductionId> },
    Terminal { value: Option<String> },
}

pub struct Symbol {
    pub id: SymbolId,
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
    prod_id_count: u32,
    terminal_map: HashMap<String, SymbolId>,
    nonterminal_map: HashMap<String, SymbolId>,
}

impl InternedSymbols {
    pub fn search_terminal(&self, name: &str) -> Option<&Symbol> {
        self.terminal_map.get(name).map(|&id| &self.nonterminals[self.nonterminal_index(id)])
    }

    pub fn search_nonterminal(&self, name: &str) -> Option<&Symbol> {
        self.nonterminal_map.get(name).map(|&id| &self.nonterminals[self.nonterminal_index(id)])
    }

    pub fn add_terminal(&mut self, id: SymbolId, name: String, value: Option<String>) { 
        let key = value.as_ref().cloned().unwrap_or(name);
        self.terminals.push(Symbol {
            id, kind: SymbolKind::Terminal { value },
        });
        self.terminal_map.insert(key, id);
    }

    pub fn add_nonterminal(&mut self, id: SymbolId, name: String, entrypoint: bool) {
        self.nonterminals.push(Symbol { 
            id, kind: SymbolKind::Nonterminal { entrypoint, productions: Vec::new() }
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
        SymbolId((self.terminals.len() + self.nonterminals.len()) as u32)
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

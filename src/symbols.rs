use core::fmt;
use std::collections::HashMap;
use std::fmt::Write;

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct SymbolId(pub u16);

pub const EOF_ID: SymbolId = SymbolId(0);

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct ProductionId(pub u16);


#[derive(Debug)]
pub enum SymbolKind {
    Nonterminal { entrypoint: bool, productions: Vec<ProductionId> },
    Terminal { value: Option<String> },
}

#[derive(Debug)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
}

impl Symbol {
    pub fn written_as(&self) -> String {
        match self {
            Symbol { kind: SymbolKind::Terminal { value: Some(value) }, .. } => format!("\"{}\"", value),
            Symbol { name, .. } => name.clone(),
        }
    }

    pub fn format_string(&self, interned: &InternedSymbols, buffer: &mut String) -> Result<(), fmt::Error> {
        match self {
            Symbol { kind: SymbolKind::Nonterminal { productions, .. }, .. } => {
                write!(buffer, "<nonterminal-def: {}>:\n", self.name)?;
                
                for &production_id in productions {
                    write!(buffer, "|")?;
                    
                    let production = interned.production(production_id);
                    for &symbol_id in &production.rhs {
                        let symbol = interned.symbol(symbol_id);
                        write!(buffer, " {}", symbol.written_as())?;
                    }
                    write!(buffer, "\n")?;
                }
            }
            Symbol { kind: SymbolKind::Terminal { .. }, ..} => {
                write!(buffer, "{}", self.written_as())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct NonterminalProduction {
    pub id: ProductionId,
    pub lhs_id: SymbolId,
    pub rhs: Vec<SymbolId>,
    pub action: Option<String>,
}

#[derive(Debug)]
pub struct InternedSymbols {
    pub(crate) terminals: Vec<Symbol>,
    pub(crate) nonterminals: Vec<Symbol>,
    pub(crate) productions: Vec<Option<NonterminalProduction>>,
    pub terminal_map: HashMap<String, SymbolId>,
    nonterminal_map: HashMap<String, SymbolId>,
}

impl InternedSymbols {
    pub fn new() -> Self {
        let mut interned_symbols = InternedSymbols {
            terminals: Vec::new(),
            nonterminals: Vec::new(),
            productions: Vec::new(),
            terminal_map: HashMap::new(),
            nonterminal_map: HashMap::new(),
        };
        interned_symbols.add_terminal(EOF_ID, String::from("EOF"), None);
        interned_symbols
    }

    pub fn iter_productions(&self) -> impl Iterator<Item = &NonterminalProduction> {
        self.productions.iter().map(|prod| prod.as_ref().unwrap())
    }

    pub fn search_terminal(&self, name: &str) -> Option<&Symbol> {
        self.terminal_map.get(name).map(|&id| &self.terminals[self.terminal_index(id)])
    }

    pub fn search_nonterminal(&self, name: &str) -> Option<&Symbol> {
        self.nonterminal_map.get(name).map(|&id| &self.nonterminals[self.nonterminal_index(id)])
    }

    pub fn symbol(&self, id: SymbolId) -> &Symbol {
        if (id.0 as usize) < self.terminals.len() {
            self.terminal(id)
        }
        else {
            self.nonterminal(id)
        }
    }

    pub fn terminal(&self, id: SymbolId) -> &Symbol {
        &self.terminals[self.terminal_index(id)]
    }

    pub fn nonterminal(&self, id: SymbolId) -> &Symbol {
        &self.nonterminals[self.nonterminal_index(id)]
    }

    pub fn production(&self, id: ProductionId) -> &NonterminalProduction {
        self.productions[id.0 as usize].as_ref().unwrap()
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
        SymbolId((self.terminals.len() + self.nonterminals.len()) as u16)
    }

    pub fn next_prod_id(&mut self) -> ProductionId {
        let id = ProductionId(self.productions.len() as u16);
        self.productions.push(None);
        id
    }

    pub fn add_production(&mut self, production: NonterminalProduction) {
        let index = self.nonterminal_index(production.lhs_id);
        let Symbol {
            kind: SymbolKind::Nonterminal { productions, .. }, ..
        } = &mut self.nonterminals[index] else { unreachable!() };

        let id = production.id;
        productions.push(id);
        self.productions[id.0 as usize] = Some(production);
    }
}

use core::fmt;
use std::collections::HashMap;
use std::fmt::Write;

#[derive(Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct SymbolId(pub u16);

pub const EOF_ID: SymbolId = SymbolId(0);

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct ProductionId(pub u16);

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct ActionId(pub u16);

pub const DEFAULT_ACTION: ActionId = ActionId(0);
pub const SEQUENCE_ACTION: ActionId = ActionId(1);
pub const PREPEND_ACTION: ActionId = ActionId(2);
pub const FLATTEN_ACTION: ActionId = ActionId(3);
pub const OPTION_ACTION: ActionId = ActionId(4);

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
    pub fn is_terminal(&self) -> bool {
        matches!(self, Symbol { kind: SymbolKind::Terminal { .. }, .. })
    }

    pub fn is_nonterminal(&self) -> bool {
        matches!(self, Symbol { kind: SymbolKind::Nonterminal { .. }, .. })
    }

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
    pub action: ActionId,
}

#[derive(Debug)]
pub struct InternedSymbols {
    pub terminals: Vec<Symbol>,
    pub nonterminals: Vec<Symbol>,
    pub productions: Vec<Option<NonterminalProduction>>,
    actions: Vec<String>,
    terminal_map: HashMap<String, SymbolId>,
    nonterminal_map: HashMap<String, SymbolId>,
}

impl InternedSymbols {
    pub fn new() -> Self {
        let mut interned_symbols = InternedSymbols {
            terminals: Vec::new(),
            nonterminals: Vec::new(),
            productions: Vec::new(),
            actions: Vec::new(),
            terminal_map: HashMap::new(),
            nonterminal_map: HashMap::new(),
        };
        interned_symbols.add_terminal(EOF_ID, String::from("EOF"), None);
        interned_symbols.add_intrincic_action(String::from("@default"), DEFAULT_ACTION);
        interned_symbols.add_intrincic_action(String::from("@sequence"), SEQUENCE_ACTION);
        interned_symbols.add_intrincic_action(String::from("@prepend"), PREPEND_ACTION);
        interned_symbols.add_intrincic_action(String::from("@flatten"), FLATTEN_ACTION);
        interned_symbols.add_intrincic_action(String::from("@option"), OPTION_ACTION);
        interned_symbols
    }

    fn add_intrincic_action(&mut self, action: String, id: ActionId) {
        let cloned = action.clone();
        self.add_action(action);
        debug_assert_eq!(self.search_action(&cloned), Some(id));
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

    pub fn search_action(&self, action: &str) -> Option<ActionId> {
        self.actions.iter()
            .position(|act| act == action)
            .map(|pos| ActionId(pos as u16))
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

    pub fn add_action(&mut self, action: String) -> ActionId {
        self.search_action(&action)
            .unwrap_or_else(|| {
                let index = self.actions.len();
                self.actions.push(action);
                ActionId(index as u16)
            })
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

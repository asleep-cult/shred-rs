use std::io;

use crate::symbols::{InternedSymbols, ProductionId, Symbol, SymbolId, EOF_ID};

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct StateId(pub u16);

const UNSET_GOTO: u16 = 0;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum ActionKind {
    Reject,
    Shift(StateId),
    Reduce(ProductionId),
    Accept(ProductionId),
}

impl ActionKind {
    pub fn value(&self) -> u16 {
        match *self {
            ActionKind::Reject => 0,
            ActionKind::Shift(..) => 1,
            ActionKind::Reduce(..) => 2,
            ActionKind::Accept(..) => 3,
        }
    }
}

impl From<ActionKind> for u16 {
    fn from(kind: ActionKind) -> u16 {
        let action = kind.value();
        let number = match kind {
            ActionKind::Reject => 0,
            ActionKind::Shift(StateId(id)) => id,
            ActionKind::Reduce(ProductionId(id)) | ActionKind::Accept(ProductionId(id)) => id,
        };
        (action << 14) | (number & 0x3FFF)
    }
}

impl From<u16> for ActionKind {
    fn from(value: u16) -> ActionKind {
        let number = value & 0x3FFF;
        match (value & 0xC000) >> 14 {
            0 => {
                assert_eq!(number, 0);
                ActionKind::Reject
            },
            1 => ActionKind::Shift(StateId(number)),
            2 => ActionKind::Reduce(ProductionId(number)),
            3 => ActionKind::Accept(ProductionId(number)),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub enum ActionConflictKind {
    Impossible,  // Indicates an internal error with the parser generator
    Unrecoverable,  // Indacates a grammar is not LR(1)
    Recovered,  // A shift/reduce conflict that defaulted to the shift
}

#[derive(Debug)]
pub struct ActionConflict {
    pub kind: ActionConflictKind,
    pub state_id: StateId,
    pub symbol_id: SymbolId,
    pub existing_entry: ActionKind,
    pub new_entry: ActionKind,
}

#[derive(Debug)]
pub struct GotoConflict {
    // This is an impossible conflict that can arise under the same 
    // circumstances of ActionConflictKind::Impossible
    pub state_id: StateId,
    pub symbol_id: SymbolId,
    pub existing_entry: StateId,
    pub new_entry: StateId,
}

pub struct ParseTable<'a> {
    pub interned_symbols: &'a InternedSymbols,
    number_of_states: usize,
    actions: Box<[u16]>,
    gotos: Box<[u16]>,
}

impl<'a> ParseTable<'a> {
    pub fn new(interned_symbols: &'a InternedSymbols, number_of_states: usize) -> Self {
        ParseTable {
            interned_symbols,
            number_of_states,
            actions: vec![0; number_of_states * interned_symbols.terminals.len()].into_boxed_slice(),
            gotos: vec![0; number_of_states * interned_symbols.nonterminals.len()].into_boxed_slice(),
        }
    }

    pub fn action_index(&self, state_id: StateId, symbol_id: SymbolId) -> usize {
        (state_id.0 as usize * self.interned_symbols.terminals.len()) + self.interned_symbols.terminal_index(symbol_id)
    }

    pub fn goto_index(&self, state_id: StateId, symbol_id: SymbolId) -> usize {
        (state_id.0 as usize * self.interned_symbols.nonterminals.len()) + self.interned_symbols.nonterminal_index(symbol_id)
    }

    pub fn all_actions(&self, state_id: StateId) -> impl Iterator<Item = ActionKind> + use<'_> {
        let terminals = self.interned_symbols.terminals.len();
        let state_id = state_id.0 as usize;
        self.actions[state_id * terminals..(state_id + 1) * terminals].iter().copied().map(Into::into)
    }

    pub fn all_gotos(&self, state_id: StateId) -> impl Iterator<Item = StateId> + use<'_> {
        let nonterminals = self.interned_symbols.nonterminals.len();
        let state_id = state_id.0 as usize;
        self.gotos[state_id * nonterminals..(state_id + 1) * nonterminals]
            .iter()
            .copied()
            .filter(|&n| n != UNSET_GOTO)
            .map(|n| StateId(n - 1))
    }

    pub fn action(&self, state_id: StateId, symbol_id: SymbolId) -> ActionKind {
        self.actions[self.action_index(state_id, symbol_id)].into()
    }

    pub fn goto(&self, state_id: StateId, symbol_id: SymbolId) -> Option<StateId> {
        match self.gotos[self.goto_index(state_id, symbol_id)] {
            UNSET_GOTO => None,
            value => Some(StateId(value - 1)),
        }
    }

    pub fn add_accept(&mut self, state_id: StateId, production_id: ProductionId) -> Result<(), ActionConflict> {
        let new_entry = ActionKind::Accept(production_id);
        match self.action(state_id, EOF_ID) {
            ActionKind::Reject => {
                self.actions[self.action_index(state_id, EOF_ID)] = new_entry.into();
                Ok(())
            },
            existing_entry => {
                let conflict = ActionConflict {
                    kind: ActionConflictKind::Unrecoverable,
                    state_id,
                    symbol_id: EOF_ID,
                    existing_entry,
                    new_entry
                };
                Err(conflict)
            }
        }
    }

    pub fn add_shift(
        &mut self,
        state_id: StateId,
        symbol_id: SymbolId,
        next_state_id: StateId,
    ) -> Result<(), ActionConflict> {
        let new_entry = ActionKind::Shift(next_state_id);
        match self.action(state_id, symbol_id) {
            ActionKind::Reject => {
                self.actions[self.action_index(state_id, symbol_id)] = new_entry.into();
                Ok(())
            }
            existing_entry => {
                let kind = match existing_entry {
                    ActionKind::Reject => unreachable!(),
                    ActionKind::Shift(_) => ActionConflictKind::Impossible,
                    ActionKind::Reduce(_) => {
                        self.actions[self.action_index(state_id, symbol_id)] = new_entry.into();
                        ActionConflictKind::Recovered
                    }
                    ActionKind::Accept(_) => ActionConflictKind::Unrecoverable,
                };
                Err(ActionConflict { kind, state_id, symbol_id, existing_entry, new_entry })
            } 
        }
    }

    pub fn add_reduce(
        &mut self,
        state_id: StateId,
        symbol_id: SymbolId,
        production_id: ProductionId,
    ) -> Result<(), ActionConflict> {
        let new_entry = ActionKind::Reduce(production_id);
        match self.action(state_id, symbol_id) {
            ActionKind::Reject => {
                self.actions[self.action_index(state_id, symbol_id)] = new_entry.into();
                Ok(())
            }
            existing_entry => {
                let kind = match existing_entry {
                    ActionKind::Reject => unreachable!(),
                    ActionKind::Shift(_) => ActionConflictKind::Recovered,
                    ActionKind::Reduce(_) | ActionKind::Accept(_) => ActionConflictKind::Unrecoverable,
                };
                Err(ActionConflict { kind, state_id, symbol_id, existing_entry, new_entry })
            }
        }
    }

    pub fn add_goto(
        &mut self,
        state_id: StateId,
        symbol_id: SymbolId,
        next_state_id: StateId,
    ) -> Result<(), GotoConflict> {
        match self.goto(state_id, symbol_id) {
            Some(existing_entry) =>
                Err(GotoConflict { state_id, symbol_id, existing_entry, new_entry: next_state_id }),
            None => {
                self.gotos[self.goto_index(state_id, symbol_id)] = next_state_id.0 + 1;
                Ok(())
            }
        }
    }

    pub fn action_map(&self, state_id: StateId) -> impl Iterator<Item = (&Symbol, ActionKind)> {
        self.all_actions(state_id)
            .enumerate()
            .filter(|(_, act)| !matches!(act, ActionKind::Reject))
            .map(move |(idx, act)| (&self.interned_symbols.terminals[idx], act))
    }

    pub fn goto_map(&self, state_id: StateId) -> impl Iterator<Item = (&Symbol, StateId)> {
        self.all_gotos(state_id)
            .enumerate()
            .map(move |(idx, state_id)| (&self.interned_symbols.nonterminals[idx], state_id))
    }

    pub fn dump_table<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
        for idx in 0..self.number_of_states {
            let state_id = StateId(idx as u16);
            write!(writer, "<state #{}>", idx)?;

            let actions: Vec<(&Symbol, ActionKind)> = self.action_map(state_id).collect();
            write!(writer, "\n[ Actions: {} ]", actions.len())?;

            for (symbol, action) in actions {
                match action {
                    ActionKind::Shift(StateId(state_id)) => {
                        write!(writer, "\n    (for symbol {}) SHIFT -> state #{}", symbol.name, state_id)?;
                    }
                    ActionKind::Reduce(production_id) => {
                        let production = self.interned_symbols.production(production_id);
                        let lhs = self.interned_symbols.nonterminal(production.lhs_id);
                        write!(writer, "\n    (for symbol {}) REDUCE [production: {}]", symbol.name, lhs.name)?;
                    }
                    ActionKind::Accept(production_id) => {
                        let production = self.interned_symbols.production(production_id);
                        let lhs = self.interned_symbols.nonterminal(production.lhs_id);
                        write!(writer, "\n    (for symbol {}) ACCEPT [production: {}]", symbol.name, lhs.name)?;
                    }
                    ActionKind::Reject => { unreachable!() }
                }
            }

            let gotos: Vec<(&Symbol, StateId)> = self.goto_map(state_id).collect();
            write!(writer, "\n[ Gotos: {} ]", gotos.len())?;

            for (symbol, next_state_id) in gotos {
                write!(writer, "\n    (for symbol {}) -> state #{}", symbol.name, next_state_id.0)?;
            }

            write!(writer, "\n")?;
        }
        Ok(())
    }
}

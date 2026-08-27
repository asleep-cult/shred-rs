use crate::lr::CanonicalCollection;
use crate::symbols::{InternedSymbols, Symbol, SymbolId, SymbolKind};
use crate::table::{ParseTable, StateId};

pub struct TableGenerator<'a>{
    interned_symbols: &'a InternedSymbols,
    canonical_collection: CanonicalCollection,
}

impl<'a> TableGenerator<'a> {
    pub fn new(interned_symbols: &'a InternedSymbols, canonical_collection: CanonicalCollection) -> Self {
        TableGenerator { interned_symbols, canonical_collection }
    }

    pub fn generate_table(&mut self) -> ParseTable<'a> {
        let mut table = ParseTable::new(
            &self.interned_symbols,
            self.canonical_collection.context.kernels.len()
        );

        for ((from_state, symbol_id), to_state) in &self.canonical_collection.transitions {
            match self.interned_symbols.symbol(*symbol_id) {
                Symbol { kind: SymbolKind::Terminal { .. }, .. } => {
                    table.add_shift(*from_state, *symbol_id, *to_state);
                }
                Symbol { kind: SymbolKind::Nonterminal { .. }, .. } => {
                    table.add_goto(*from_state, *symbol_id, *to_state);
                }
            }
        }

        let state_ids: Vec<StateId> = (0..self.canonical_collection.context.kernels.len()).into_iter()
            .map(|n| StateId(n as u16))
            .collect();

        for state_id in state_ids {
            let items = &self.canonical_collection.context.kernels[state_id.0 as usize];
            for item in items {
                let (production_id, position) = self.canonical_collection.context.item_core(item.index);
                let production = self.interned_symbols.production(production_id);

                if production.rhs.len() <= position as usize {
                    let lhs = self.interned_symbols.nonterminal(production.lhs_id);
                    if let Symbol { kind: SymbolKind::Nonterminal { entrypoint: true, .. }, .. } = lhs {
                        table.add_accept(state_id, production_id);
                    }
                    else {
                        let mut had_any_lookahead = false;
                        for lookahead_number in &item.lookahead {
                            had_any_lookahead = true;
                            let lookahead_symbol = SymbolId(lookahead_number as u16);
                            table.add_reduce(state_id, lookahead_symbol, production_id);
                        }

                        if !had_any_lookahead {
                            panic!(
                                "Found no lookahead for necessary reduction of {} in state # {}",
                                lhs.name,
                                state_id.0,
                            )
                        }
                    }
                }
            }
        }

        for (state_id, item) in &self.canonical_collection.epsilon_transitions {
            let (production_id, position) = self.canonical_collection.context.item_core(item.index);

            let production = self.interned_symbols.production(production_id);
            assert!(position == 0 && production.rhs.len() == 0);

            let mut had_any_lookahead = false;
            for lookahead_number in &item.lookahead {
                had_any_lookahead = true;
                let lookahead_symbol = SymbolId(lookahead_number as u16);
                table.add_reduce(*state_id, lookahead_symbol, production_id);
            }

            if !had_any_lookahead {
                let lhs = self.interned_symbols.symbol(production.lhs_id);
                panic!(
                    "Found no lookahead for necessary reduction of {} in state # {}",
                    lhs.name,
                    state_id.0,
                )
            }
        }

        table
    }
}

use crate::scanner::Scanner;
use crate::parser::GrammarParser;
use crate::lowering::LoweringContext;
use crate::computation::ComputationEngine;

use shred_core::diagnostics::DiagnosticInfo;
use shred_core::lr::CanonicalCollection;
use shred_core::symbols::{InternedSymbols, Symbol, SymbolId, SymbolKind};
use shred_core::table::{ParseTable, StateId};

pub struct TableGenerator {
    interned_symbols: InternedSymbols,
    canonical_collection: CanonicalCollection,
}

impl TableGenerator {
    pub fn new(interned_symbols: InternedSymbols, canonical_collection: CanonicalCollection) -> Self {
        TableGenerator { interned_symbols, canonical_collection }
    }

    pub fn generate_from_grammar(source: &str) -> ParseTable {
        let scanner = Scanner::new(source);
        let parser = GrammarParser::new(scanner);
        let arena = parser.parse_ast().unwrap();

        let ctx = LoweringContext::new(arena);
        let interned_symbols = ctx.lower_symbols().unwrap();

        let mut diag_info = DiagnosticInfo::new();

        let ctx = ComputationEngine::new(&interned_symbols);
        let collection = ctx.compute_canonical_collection(&mut diag_info);

        let generator = Self::new(interned_symbols, collection);
        generator.generate_table(&mut diag_info)
    }

    pub fn generate_table(self, diag_info: &mut DiagnosticInfo) -> ParseTable {
        let mut table = ParseTable::new(
            self.interned_symbols,
            self.canonical_collection.context.kernels.len()
        );

        for ((from_state, symbol_id), to_state) in &self.canonical_collection.transitions {
            match table.interned_symbols.symbol(*symbol_id) {
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
                let production = table.interned_symbols.production(production_id);

                if production.rhs.len() <= position as usize {
                    let lhs_id = production.lhs_id;
                    if let Symbol { kind: SymbolKind::Nonterminal { entrypoint: true, .. }, .. } =
                        table.interned_symbols.nonterminal(lhs_id)
                    {
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
                                table.interned_symbols.nonterminal(lhs_id).name,
                                state_id.0,
                            )
                        }
                    }
                }
            }
        }

        for (state_id, item) in &self.canonical_collection.epsilon_transitions {
            let (production_id, position) = self.canonical_collection.context.item_core(item.index);

            let production = table.interned_symbols.production(production_id);
            let lhs_id = production.lhs_id;
            assert!(position == 0 && production.rhs.len() == 0);

            let mut had_any_lookahead = false;
            for lookahead_number in &item.lookahead {
                had_any_lookahead = true;
                let lookahead_symbol = SymbolId(lookahead_number as u16);
                table.add_reduce(*state_id, lookahead_symbol, production_id);
            }

            if !had_any_lookahead {
                let lhs = table.interned_symbols.symbol(lhs_id);
                panic!(
                    "Found no lookahead for necessary reduction of {} in state # {}",
                    lhs.name,
                    state_id.0,
                )
            }
        }

        diag_info.dump_table(&table);
        table
    }
}

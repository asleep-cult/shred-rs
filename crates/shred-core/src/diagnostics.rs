use std::io;

use crate::symbols::{InternedSymbols, Symbol};
use crate::lr::{LRContext, Lr1Item};
use crate::table::{ActionKind, ParseTable, StateId};
use crate::symbols::SymbolId;


#[derive(Default)]
pub struct DiagnosticInfo {
    closure_writer: Option<Box<dyn io::Write>>,
    table_writer: Option<Box<dyn io::Write>>,
}

impl DiagnosticInfo {
    pub fn new() -> Self {
        DiagnosticInfo::default()
    }

    pub fn set_closure_writer(&mut self, writer: Box<dyn io::Write>) {
        self.closure_writer = Some(writer);
    }

    pub fn set_table_writer(&mut self, writer: Box<dyn io::Write>) {
        self.table_writer = Some(writer);
    }

    fn write_closure(
        &mut self,
        interned_symbols: &InternedSymbols,
        context: &LRContext,
        state_id: StateId,
        items: &[Lr1Item],
    ) -> io::Result<()> {
        let Some(ref mut writer) = self.closure_writer else { return Ok(()) };
        write!(writer, "<parser closure dump #{}>\n", state_id.0)?;

        for (i, item) in items.iter().enumerate() {
            let (production_id, position) = context.item_core(item.index);
            let production = interned_symbols.production(production_id);
            let lhs = interned_symbols.nonterminal(production.lhs_id);

            let names = item.lookahead.into_iter()
                .map(|sym| interned_symbols.symbol(SymbolId(sym as u16)).name.clone())
                .collect::<Vec<String>>()
                .join(", ");

            write!(writer, "    ({}., pos={}, lookahead={{{}}}): {} ->", i, position, names, lhs.name)?;

            for (j, &symbol_id) in production.rhs.iter().enumerate() {
                if j == position as usize {
                    write!(writer, " *")?;
                }

                write!(writer, " {}", interned_symbols.symbol(symbol_id).written_as())?;
            }

            if production.rhs.len() <= position as usize {
                write!(writer, " *")?;
            }
            write!(writer, "\n")?;
        }
        Ok(())
    }

    pub fn dump_closure(
        &mut self,
        interned_symbols: &InternedSymbols,
        context: &LRContext,
        state_id: StateId,
        items: &[Lr1Item],
    ) {
        let result = self.write_closure(interned_symbols, context, state_id, items);
        if let Err(err) = result {
            println!("An error occured while trying to write a closure to the writer: {}", err.to_string());
        }
    }

    fn write_table(&mut self, table: &ParseTable) -> io::Result<()> {
        let Some(ref mut writer) = self.table_writer else { return Ok(()) };
        for idx in 0..table.number_of_states {
            let state_id = StateId(idx as u16);
            write!(writer, "<state #{}>", idx)?;

            let actions: Vec<(&Symbol, ActionKind)> = table.action_map(state_id).collect();
            write!(writer, "\n[ Actions: {} ]", actions.len())?;

            for (symbol, action) in actions {
                match action {
                    ActionKind::Shift(StateId(state_id)) => {
                        write!(writer, "\n    (for symbol {}) SHIFT -> state #{}", symbol.name, state_id)?;
                    }
                    ActionKind::Reduce(production_id) => {
                        let production = table.interned_symbols.production(production_id);
                        let lhs = table.interned_symbols.nonterminal(production.lhs_id);
                        write!(writer, "\n    (for symbol {}) REDUCE [production: {}]", symbol.name, lhs.name)?;
                    }
                    ActionKind::Accept(production_id) => {
                        let production = table.interned_symbols.production(production_id);
                        let lhs = table.interned_symbols.nonterminal(production.lhs_id);
                        write!(writer, "\n    (for symbol {}) ACCEPT [production: {}]", symbol.name, lhs.name)?;
                    }
                    ActionKind::Reject => { unreachable!() }
                }
            }

            let gotos: Vec<(&Symbol, StateId)> = table.goto_map(state_id).collect();
            write!(writer, "\n[ Gotos: {} ]", gotos.len())?;

            for (symbol, next_state_id) in gotos {
                write!(writer, "\n    (for symbol {}) -> state #{}", symbol.name, next_state_id.0)?;
            }

            write!(writer, "\n")?;
        }
        Ok(())
    }

    pub fn dump_table(&mut self, table: &ParseTable) {
        let result = self.write_table(table);
        if let Err(err) = result {
            println!("An error occured while trying to write a table to the writer: {}", err.to_string());
        }
    }
}

use crate::ast::{
    SymbolId, RuleId, ProductionId, ArenaRange, AstArena,
    TerminalDef, NonterminalDef, Production, RuleKind
};
use crate::symbols::{Symbol, InternedSymbols};

struct LoweringContext {
    ast_arena: AstArena,
    interned_symbols: InternedSymbols,
}

impl LoweringContext {
    pub fn initialize_terminals(&mut self) {
        for (symbol_id, terminal) in self.ast_arena.iter_terminals() {
            // TODO: Figure out if its ok to clone these values "so" many times
            self.interned_symbols.add_terminal(symbol_id, terminal.name.clone(), terminal.value.clone())
        }
    }

    pub fn initialize_nonterminals(&mut self) {
        for (symbol_id, nonterminal) in self.ast_arena.iter_nonterminals() {
            self.interned_symbols.add_nonterminal(symbol_id, nonterminal.name.clone(), nonterminal.entrypoint);
        }
    }
}

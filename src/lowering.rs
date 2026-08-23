use crate::ast::{RuleId, AstArena, RuleKind};
use crate::symbols::{InternedSymbols, NonterminalProduction, SymbolId, Symbol, SymbolKind};

enum LoweringErrorKind {
    UnknownName(String),
    UnknownTerminal(String),
}

struct LoweringContext {
    interned_symbols: InternedSymbols,
    implicit_nonterminal_count: u32,
}

impl LoweringContext {
    pub fn lower_symbols(&mut self, arena: AstArena) -> Result<(), LoweringErrorKind> {
        for terminal in arena.terminals.into_iter() {
            self.interned_symbols.add_terminal(
                self.interned_symbols.next_sym_id(),
                terminal.name,
                terminal.value,
            );
        }

        let mut prod_ranges: Vec<(SymbolId, usize)> = Vec::with_capacity(arena.nonterminals.len());
        for nonterminal in arena.nonterminals.into_iter() {
            let id = self.interned_symbols.next_sym_id();
            self.interned_symbols.add_nonterminal(
                id,
                nonterminal.name,
                nonterminal.entrypoint,
            );
            prod_ranges.push((id, nonterminal.productions))
        }

        let mut prod_iterator = arena.productions.into_iter();
        for (symbol_id, size) in prod_ranges.into_iter() {
            for production in prod_iterator.by_ref().take(size) {
                let mut sym_production = NonterminalProduction {
                    id: self.interned_symbols.next_prod_id(),
                    lhs_id: symbol_id,
                    rhs: Vec::new(),
                    action: production.action,
                };
                self.initialize_production(&arena.rules, production.rule, &mut sym_production)?;
                self.interned_symbols.add_production(sym_production);
            }
        }
        Ok(())
    }

    fn add_implicit_nonterminal(&mut self, prefix: &str) -> SymbolId {
        let name = format!("{}_{}", prefix, self.implicit_nonterminal_count);
        self.implicit_nonterminal_count += 1;
        let id = self.interned_symbols.next_sym_id();
        self.interned_symbols.add_nonterminal(id, name, false);
        id
    }

    pub fn initialize_production(
        &mut self,
        rules: &Vec<RuleKind>,
        rule: RuleId,
        production: &mut NonterminalProduction,
    ) -> Result<(), LoweringErrorKind> {
        match &rules[rule.0 as usize] {
            RuleKind::Star(inner) => {
                self.add_star_rule(rules, "star", *inner, production)?;
            }
            RuleKind::Plus(inner) => {
                // plus_x = nonterminal
                let id = self.add_implicit_nonterminal("plus");
                production.rhs.push(id);

                // | rule plus_x*
                let mut plus_production = NonterminalProduction {
                    id: self.interned_symbols.next_prod_id(),
                    lhs_id: id,
                    rhs: Vec::new(),
                    action: Some(String::from("@prepend")),
                };
                self.initialize_production(rules, *inner, &mut plus_production)?;
                self.add_star_rule(rules, "star_of_plus", *inner, &mut plus_production)?;

                self.interned_symbols.add_production(plus_production);
            }
            RuleKind::Optional(inner) => {
                // optional_x = nonterminal
                let id = self.add_implicit_nonterminal("optional");
                production.rhs.push(id);

                // | epsilon
                let epsilon_production = NonterminalProduction {
                    id: self.interned_symbols.next_prod_id(),
                    lhs_id: id,
                    rhs: Vec::new(),
                    action: Some(String::from("@optional")),
                };
                self.interned_symbols.add_production(epsilon_production);

                // | rule
                let mut optional_production = NonterminalProduction {
                    id: self.interned_symbols.next_prod_id(),
                    lhs_id: id,
                    rhs: Vec::new(),
                    action: Some(String::from("@optional")),
                };
                self.initialize_production(rules, *inner, &mut optional_production)?;
                self.interned_symbols.add_production(optional_production);
            }
            RuleKind::Alternative { left, right } => {
                // alternative_x = nonterminal
                let id = self.add_implicit_nonterminal("alternative");
                production.rhs.push(id);

                // | left
                let mut left_production = NonterminalProduction {
                    id: self.interned_symbols.next_prod_id(),
                    lhs_id: id,
                    rhs: Vec::new(),
                    action: None,
                };
                self.initialize_production(rules, *left, &mut left_production)?;
                self.interned_symbols.add_production(left_production);

                // | right
                let mut right_production = NonterminalProduction {
                    id: self.interned_symbols.next_prod_id(),
                    lhs_id: id,
                    rhs: Vec::new(),
                    action: None,
                };
                self.initialize_production(rules, *right, &mut right_production)?;
                self.interned_symbols.add_production(right_production);
            }
            RuleKind::Group { items } => {
                for i in items.start..items.end {
                    self.initialize_production(rules, RuleId(i as u16), production)?;
                } 
            }
            RuleKind::String(content) => {
                match self.interned_symbols.search_terminal(content) {
                    Some(Symbol { id, kind: SymbolKind::Terminal { .. }, .. }) => {
                        production.rhs.push(*id);
                    },
                    _ => return Err(LoweringErrorKind::UnknownTerminal(content.clone())),
                }
            }
            RuleKind::Name(content) => {
                let result = self.interned_symbols.search_nonterminal(content).or_else(
                    || self.interned_symbols.search_terminal(content)
                );
                match result {
                    Some(Symbol { id, .. }) => {
                        production.rhs.push(*id);
                    },
                    _ => return Err(LoweringErrorKind::UnknownName(content.clone()))
                }
            }
        }
        Ok(())
    }

    fn add_star_rule(
        &mut self,
        rules: &Vec<RuleKind>,
        prefix: &str,
        rule: RuleId,
        production: &mut NonterminalProduction,
    ) -> Result<(), LoweringErrorKind> {
        // star_x = nonterminal
        let id = self.add_implicit_nonterminal(&prefix);
        production.rhs.push(id);

        // | epsilon
        let epsilon_production = NonterminalProduction {
            id: self.interned_symbols.next_prod_id(),
            lhs_id: id,
            rhs: Vec::new(),
            action: Some(String::from("@sequence")),
        };
        self.interned_symbols.add_production(epsilon_production);

        // | nonterminal expr
        let mut star_production = NonterminalProduction {
            id: self.interned_symbols.next_prod_id(),
            lhs_id: id,
            rhs: Vec::new(),
            action: Some(String::from("@sequence")),
        };
        self.initialize_production(rules, rule, &mut star_production)?;
        self.interned_symbols.add_production(star_production);
        Ok(())
    }
}

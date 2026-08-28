use std::collections::{HashMap};
use std::io;

use crate::bitset::Bitset;
use crate::lr::{CanonicalCollection, LOOKAHEAD_SET_SIZE, LRContext, LookaheadSet, Lr1Item};
use crate::symbols::{EOF_ID, InternedSymbols, Symbol, SymbolId, SymbolKind};
use crate::table::StateId;

pub struct ComputationEngine<'a> {
    interned_symbols: &'a InternedSymbols,
    context: LRContext,
    epsilon_nonterminals: Bitset<Vec<u64>>,
    first_sets: Vec<LookaheadSet>,

    pending_gotos: HashMap<SymbolId, Vec<Lr1Item>>,
    pending_epsilons: Vec<Lr1Item>,

    closure_lookaheads: Vec<LookaheadSet>,
    closure_bitset: Bitset<Vec<u64>>,
    closure_worklist: Vec<usize>,
}

impl<'a> ComputationEngine<'a> {
    pub fn new(interned_symbols: &'a InternedSymbols) -> Self {
        let context = LRContext::new(interned_symbols);

        let items = context.item_offsets[context.item_offsets.len() - 1];
        ComputationEngine {
            closure_bitset: <Bitset<Vec<u64>>>::new(items),
            context: context,
            interned_symbols,
            epsilon_nonterminals: <Bitset<Vec<u64>>>::new(interned_symbols.nonterminals.len()),
            first_sets: vec![LookaheadSet::new(); interned_symbols.nonterminals.len()],
            pending_gotos: HashMap::new(),
            pending_epsilons: Vec::new(),
            closure_lookaheads: vec![LookaheadSet::new(); items],
            closure_worklist: Vec::new(),
        }
    }

    pub fn dump_items<T: io::Write>(&self, writer: &mut T, state_id: StateId, items: &[Lr1Item]) -> io::Result<()> {
        write!(writer, "<parser items dump #{}>\n", state_id.0)?;

        for (i, item) in items.iter().enumerate() {
            let (production_id, position) = self.context.item_core(item.index);
            let production = self.interned_symbols.production(production_id);
            let lhs = self.interned_symbols.nonterminal(production.lhs_id);

            let names = item.lookahead.into_iter()
                .map(|sym| self.interned_symbols.symbol(SymbolId(sym as u16)).name.clone())
                .collect::<Vec<String>>()
                .join(", ");

            write!(writer, "    ({}., pos={}, lookahead={{{}}}): {} ->", i, position, names, lhs.name)?;

            for (j, &symbol_id) in production.rhs.iter().enumerate() {
                if j == position as usize {
                    write!(writer, " *")?;
                }

                write!(writer, " {}", self.interned_symbols.symbol(symbol_id).written_as())?;
            }

            if production.rhs.len() <= position as usize {
                write!(writer, " *")?;
            }
            write!(writer, "\n")?;
        }
        Ok(())
    }

    fn compute_epsilon_nonterminals(&mut self) {
        for production in self.interned_symbols.iter_productions() {
            if production.rhs.len() == 0 {
                let index = self.interned_symbols.nonterminal_index(production.lhs_id);
                self.epsilon_nonterminals.add(index);
            }
        }

        let mut changed = true;
        while changed {
            changed = false;

            for production in self.interned_symbols.iter_productions() {
                if !self.has_epsilon_nonterminal(production.lhs_id)
                    && production.rhs.iter().all(|&sym| self.has_epsilon_nonterminal(sym))
                {
                    let index = self.interned_symbols.nonterminal_index(production.lhs_id);
                    self.epsilon_nonterminals.add(index);
                    changed = true;
                }
            }
        }
    }

    pub fn has_epsilon_nonterminal(&self, symbol_id: SymbolId) -> bool {
        self.interned_symbols.symbol(symbol_id).is_nonterminal()
        && self.epsilon_nonterminals.has(self.interned_symbols.nonterminal_index(symbol_id))
    }

    pub fn compute_first_sets(&mut self) {
        for production in self.interned_symbols.iter_productions() {
            for &symbol_id in &production.rhs {
                if self.interned_symbols.symbol(symbol_id).is_terminal() {
                    self.first_sets[self.interned_symbols.nonterminal_index(production.lhs_id)].add(symbol_id.0 as usize);
                }

                if !self.has_epsilon_nonterminal(symbol_id) {
                    break;
                }
            }
        }

        let mut changed = true;
        while changed {
            changed = false;

            for production in self.interned_symbols.iter_productions() {
                for &symbol_id in &production.rhs {
                    if self.interned_symbols.symbol(symbol_id).is_nonterminal() {
                        let lhs_index = self.interned_symbols.nonterminal_index(production.lhs_id);
                        let rhs_index = self.interned_symbols.nonterminal_index(symbol_id);

                        if lhs_index != rhs_index {
                            let rhs_lookahead = self.first_sets[rhs_index].clone();
                            if self.first_sets[lhs_index].inplace_union(&rhs_lookahead) {
                                changed = true;
                            }
                        }
                    }
                    if !self.has_epsilon_nonterminal(symbol_id) {
                        break;
                    }
                }
            }
        }
    }

    fn first_set<T: Iterator<Item = &'a SymbolId>>(&self, symbols: T) -> LookaheadSet {
        let mut result = Bitset([0; LOOKAHEAD_SET_SIZE]);
        for &symbol_id in symbols {
            if self.interned_symbols.symbol(symbol_id).is_nonterminal() {
                result.inplace_union(&self.first_sets[self.interned_symbols.nonterminal_index(symbol_id)]);
            }
            else {
                result.add(symbol_id.0 as usize);
            }
            if !self.has_epsilon_nonterminal(symbol_id) {
                break;
            }
        }
        result
    }

    pub fn compute_closure(&mut self, items: &[Lr1Item]) -> Vec<Lr1Item> {
        self.closure_worklist.clear();
        self.closure_bitset.clear();
        for lookahead in &mut self.closure_lookaheads {
            lookahead.clear();
        }

        for item in items {
            self.closure_lookaheads.insert(item.index, item.lookahead.clone());
            self.closure_worklist.push(item.index);
            self.closure_bitset.add(item.index);
        }

        let mut worklist_index = 0;
        while worklist_index < self.closure_worklist.len() {
            let item_index = self.closure_worklist[worklist_index];
            worklist_index += 1;

            let (production_id, position) = self.context.item_core(item_index);
            
            let production = self.interned_symbols.production(production_id);
            if production.rhs.len() <= position as usize {
                continue;
            }
            
            let current_symbol = self.interned_symbols.symbol(production.rhs[position as usize]);
            let Symbol { kind: SymbolKind::Nonterminal { productions, .. }, .. } = current_symbol else {
                continue;
            };

            let lookahead = &self.closure_lookaheads[item_index];
            let trailing_symbols = &production.rhs[(position as usize) + 1..];
            let mut next_lookahead = self.first_set(trailing_symbols.iter());

            if trailing_symbols.iter().all(|&sym| self.has_epsilon_nonterminal(sym)) {
                next_lookahead.inplace_union(lookahead);
            }

            for &inner_production_id in productions {
                let inner_item_index = self.context.item_index(inner_production_id, 0);
                let was_added = self.closure_bitset.add(inner_item_index);
                if was_added {
                    self.closure_worklist.push(inner_item_index);
                }

                let existing_lookahead = &mut self.closure_lookaheads[inner_item_index];
                if existing_lookahead.inplace_union(&next_lookahead) {
                    if !was_added {
                        self.closure_worklist.push(inner_item_index);
                    }
                }
            }
        }

        let mut closure_items = Vec::with_capacity(self.closure_worklist.len());
        for index in &self.closure_bitset {
            closure_items.push(Lr1Item { index, lookahead: self.closure_lookaheads[index] })
        }

        closure_items.sort_by_key(|item| self.context.item_core(item.index));
        closure_items
    }

    pub fn compute_goto(&mut self, items: &[Lr1Item]) {
        for item in items {
            let (production_id, position) = self.context.item_core(item.index);
            let production = self.interned_symbols.production(production_id);

            if production.rhs.len() > position as usize {
                let item = Lr1Item {
                    index: self.context.item_index(production_id, position + 1),
                    lookahead: item.lookahead.clone(),
                };

                let symbol_id = production.rhs[position as usize];
                self.pending_gotos.entry(symbol_id)
                    .or_default()
                    .push(item);
            }
            else if production.rhs.len() == 0 {
                self.pending_epsilons.push(item.clone());
            }
        }
    }

    pub fn compute_canonical_collection(mut self) -> CanonicalCollection {
        self.compute_epsilon_nonterminals();
        self.compute_first_sets();

        let production_ids = self.interned_symbols.nonterminals.iter()
            .map(|sym| {
                if let Symbol { kind: SymbolKind::Nonterminal { entrypoint: true, productions }, ..} = sym {
                    assert_eq!(productions.len(), 1);
                    Some(productions[0])
                }
                else {
                    None
                }
            })
            .flatten();

        let mut entrypoint_states = HashMap::new();
        let mut worklist = Vec::new();

        for production_id in production_ids {
            let item_index = self.context.item_index(production_id, 0);
            let mut lookahead = Bitset([0; LOOKAHEAD_SET_SIZE]);
            lookahead.add(EOF_ID.0 as usize);

            let entry_item = Lr1Item { index: item_index, lookahead };

            let items = vec![entry_item];
            let (state_id, _) = self.context.canonicalize_state(items.clone());
            entrypoint_states.insert(production_id, state_id);
            worklist.push((state_id, items));
        }
        assert!(entrypoint_states.len() > 0);

        let mut transitions = HashMap::new();
        let mut epsilon_transitions = Vec::new();

        //let mut fp = fs::File::create("./output.txt").unwrap();
        while let Some((state_id, items)) = worklist.pop() {
            //self.dump_items(&mut fp, state_id, &items);
            let closure = self.compute_closure(&items);
            self.compute_goto(&closure);
            //self.dump_items(&mut fp, state_id, &closure);

            for (symbol_id, inner_items) in self.pending_gotos.drain() {
                let (next_state_id, was_added) = self.context.canonicalize_state(inner_items.clone());
                transitions.insert((state_id, symbol_id), next_state_id);

                if was_added {
                    worklist.push((next_state_id, inner_items));
                }
            }

            for inner_item in self.pending_epsilons.drain(0..) {
                epsilon_transitions.push((state_id, inner_item));
            }
        }

        CanonicalCollection { context: self.context, transitions, epsilon_transitions }
    }
} 

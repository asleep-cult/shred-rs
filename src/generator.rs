use std::collections::{HashMap, HashSet};

use crate::symbols::{EOF_ID, InternedSymbols, ProductionId, Symbol, SymbolId, SymbolKind};
use crate::table::ParseTable;

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct LookaheadSet(pub Box<[u64]>);

impl LookaheadSet {
    fn new(size: usize) -> Self {
        let amount = if size % 64 != 0 { (size / 64) + 1 } else { size / 64 };
        LookaheadSet(vec![0; amount as usize].into_boxed_slice())
    }

    fn add(&mut self, symbol_id: SymbolId) {
        let index = symbol_id.0 as usize / 64;
        let remainder = symbol_id.0 as usize % 64;
        self.0[index] |= 1 << remainder;
    }

    fn inplace_union(&mut self, other: &LookaheadSet) -> bool {
        let mut changed = false;
        assert_eq!(self.0.len(), other.0.len());
        for (set, &other_set) in self.0.iter_mut().zip(other.0.iter()) {
            if *set & other_set != other_set {
                changed = true;
                *set |= other_set;
            }
        }
        changed
    }

    fn is_superset(&self, other: &LookaheadSet) -> bool {
        assert_eq!(self.0.len(), other.0.len());
        self.0.iter().zip(other.0.iter())
            .all(|(&set, &other_set)| set & other_set == other_set)
    }
}

pub struct LookaheadIterator {
    iterator: std::vec::IntoIter<u64>,
    current: u64,
    idx: u16,
}

impl Iterator for LookaheadIterator {
    type Item = SymbolId;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current == 0 {
            self.current = self.iterator.next()?;
            self.idx += 1;
        }

        let lowest_bit = self.current.trailing_zeros();
        self.current &= self.current - 1;
        Some(SymbolId((self.idx - 1) * 64 + lowest_bit as u16))
    }
}

impl IntoIterator for LookaheadSet {
    type Item = SymbolId;
    type IntoIter = LookaheadIterator;

    fn into_iter(self) -> Self::IntoIter {
        LookaheadIterator { iterator: self.0.into_iter(), current: 0, idx: 0 }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct ParserItem(pub ProductionId, pub usize);

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct InternedParserItem(pub u32);

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct CanonicalCollection(pub Box<[InternedParserItem]>, pub Box<[LookaheadSet]>);

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct InternedCanonicalCollection(pub u16);

pub struct ParserState {
    pub items: Vec<InternedParserItem>,
    pub lookahead: HashMap<InternedParserItem, LookaheadSet>,
}

pub struct GeneratorContext<'a> {
    interned_symbols: &'a InternedSymbols,
    parse_table: ParseTable<'a>,
    epsilon_nonterminals: HashSet<SymbolId>,
    first_sets: Vec<LookaheadSet>,
    zero_items: Vec<InternedParserItem>,
    interned_items: Vec<ParserItem>,
    items_lookup: HashMap<ParserItem, InternedParserItem>,
    interned_canonical_collections: Vec<CanonicalCollection>,
    canonical_collections_lookup: HashMap<CanonicalCollection, InternedCanonicalCollection>,
    precomputed_gotos: HashMap<(InternedCanonicalCollection, SymbolId), InternedCanonicalCollection>,
    transitions: HashMap<(InternedCanonicalCollection, SymbolId), InternedCanonicalCollection>,
}

impl<'a> GeneratorContext<'a> {
    fn compute_epsilon_nonterminals(&mut self) {
        for nonterminal in &self.interned_symbols.nonterminals {
            let Symbol {
                kind: SymbolKind::Nonterminal { productions, .. }, ..
            } = nonterminal else { unreachable!() };

            let has_epsilon = productions.iter().any(
                |&id| self.interned_symbols.production(id).rhs.len() == 0
            );
            if has_epsilon {
                self.epsilon_nonterminals.insert(nonterminal.id);
            }
        }

        let mut changed = true;
        while changed {
            changed = false;

            for nonterminal in &self.interned_symbols.nonterminals {
                if self.epsilon_nonterminals.contains(&nonterminal.id) {
                    continue;
                }

                let Symbol {
                    kind: SymbolKind::Nonterminal { productions, .. }, ..
                } = nonterminal else { unreachable!() };

                let has_epsilon = productions.iter().any(
                    |&id| {
                        let production = self.interned_symbols.production(id);
                        production.rhs.iter().all(|sym| self.epsilon_nonterminals.get(sym).is_some())
                });
                if has_epsilon {
                    changed = true;
                    self.epsilon_nonterminals.insert(nonterminal.id);
                }
            }
        }
    }

    pub fn compute_first_sets(&mut self) {
        for nonterminal in &self.interned_symbols.nonterminals {
            let Symbol {
                kind: SymbolKind::Nonterminal { productions, .. }, ..
            } = nonterminal else { unreachable!() };

            for &production_id in productions.iter() {
                let production = self.interned_symbols.production(production_id);

                for &symbol_id in production.rhs.iter() {
                    match self.interned_symbols.symbol(symbol_id) {
                        Symbol { kind: SymbolKind::Terminal { .. }, .. } => {
                            self.first_sets[self.interned_symbols.nonterminal_index(nonterminal.id)].add(symbol_id);
                        }
                        _ => {}
                    }

                    if !self.epsilon_nonterminals.contains(&symbol_id) {
                        break;
                    }
                }
            } 
        }

        let mut changed = true;
        while changed {
            changed = false;

            for nonterminal in &self.interned_symbols.nonterminals {
                let Symbol {
                    kind: SymbolKind::Nonterminal { productions, .. }, ..
                } = nonterminal else { unreachable!() };

                let index = self.interned_symbols.nonterminal_index(nonterminal.id);
                for &production_id in productions {
                    let production = self.interned_symbols.production(production_id);

                    for &symbol_id in &production.rhs {
                        match self.interned_symbols.symbol(symbol_id) {
                            Symbol { kind: SymbolKind::Nonterminal { .. }, .. } => {
                                let next_index = self.interned_symbols.nonterminal_index(symbol_id);

                                // The borrow checker strikes again. This cannot be idiomatic, then again what do I know
                                let (this_lookahead, that_lookahead) = if index < next_index {
                                    let (left, right) = self.first_sets.split_at_mut(next_index);
                                    (&mut left[index], &right[0])
                                }
                                else {
                                    let (left, right) = self.first_sets.split_at_mut(index);
                                    (&mut right[0], &left[next_index])
                                };
                                changed = changed || this_lookahead.inplace_union(that_lookahead);
                            }
                            _ => {}
                        }

                        if !self.epsilon_nonterminals.contains(&symbol_id) {
                            break;
                        }
                    }
                } 
            }
        }
    }

    pub fn compute_zero_items(&mut self) {
        for production in &self.interned_symbols.productions {
            let interned_item = self.interned_item(ParserItem(production.id, 0));
            self.zero_items.push(interned_item);
        }
    }

    pub fn zero_items(&self, symbol: &Symbol) -> impl Iterator<Item = InternedParserItem> {
        let Symbol {
            kind: SymbolKind::Nonterminal { productions, .. }, ..
        } = symbol else { unreachable!() };

        productions.iter().map(|id| self.zero_items[id.0 as usize])
    }

    fn first_set<T: Iterator<Item = SymbolId>>(&self, symbols: T) -> LookaheadSet {
        let mut result = LookaheadSet::new(self.interned_symbols.nonterminals.len());
        for symbol_id in symbols {
            match self.interned_symbols.symbol(symbol_id) {
                Symbol { kind: SymbolKind::Terminal { .. }, .. } => result.add(symbol_id),
                _ => {
                    result.inplace_union(&self.first_sets[self.interned_symbols.nonterminal_index(symbol_id)]);
                },
            }
        }
        result
    }

    pub fn interned_item(&mut self, item: ParserItem) -> InternedParserItem {
        match self.items_lookup.get(&item) {
            Some(interned_item) => *interned_item,
            _ => {
                let interned_item = InternedParserItem(self.interned_items.len() as u32);
                self.items_lookup.insert(item, interned_item);
                interned_item
            }
        }
    }

    pub fn canonicalize_state(&mut self, mut state: ParserState) -> InternedCanonicalCollection {
        state.items.sort();
        let lookahead = state.items.iter()
            .map(|item| state.lookahead.remove_entry(item).expect("Item has no lookahead").1)
            .collect::<Vec<LookaheadSet>>()
            .into_boxed_slice();

        let collection = CanonicalCollection(state.items.into_boxed_slice(), lookahead);
        match self.canonical_collections_lookup.get(&collection) {
            Some(interned) => *interned,
            _ => {
                let interned_collection = InternedCanonicalCollection(self.interned_canonical_collections.len() as u16);
                self.canonical_collections_lookup.insert(collection, interned_collection);
                interned_collection
            }
        }
    }

    pub fn compute_closure(&mut self, mut state: ParserState) -> InternedCanonicalCollection {
        let mut changed = true;
        while changed {
            changed = false;
            let mut buffer = Vec::new();

            for interned_item in state.items.iter() {
                let ParserItem(production_id, position) = self.interned_items[interned_item.0 as usize];
                let production = self.interned_symbols.production(production_id);
                if position >= production.rhs.len() {
                    continue;
                }

                let current_symbol = self.interned_symbols.symbol(production.rhs[position]);
                if !matches!(current_symbol, Symbol { kind: SymbolKind::Nonterminal { .. }, .. }) {
                    continue;
                }

                let lookahead = &state.lookahead[interned_item];
                let trailing_symbols = &production.rhs[position + 1..];
                let mut next_lookahead = self.first_set(trailing_symbols.to_owned().into_iter());

                if trailing_symbols.iter().all(|id| self.epsilon_nonterminals.contains(id)) {
                    next_lookahead.inplace_union(lookahead);
                }

                for next_interned_item in self.zero_items(current_symbol) {
                    if !state.items.contains(&next_interned_item) {
                        changed = true;
                        buffer.push(next_interned_item);
                        state.lookahead.insert(next_interned_item, next_lookahead.clone());
                    }

                    else if !state.lookahead[&next_interned_item].is_superset(&next_lookahead) {
                        changed = true;
                        match state.lookahead.get_mut(&next_interned_item) {
                            Some(lookahead) => {
                                lookahead.inplace_union(&next_lookahead);
                            },
                            _ => panic!("Item has no lookahead"),
                        }
                    }
                }
            }

            state.items.append(&mut buffer);
        }

        self.canonicalize_state(state)
    }

    pub fn compute_goto(
        &mut self,
        interned_collection: InternedCanonicalCollection,
        symbol: &Symbol,
    ) -> InternedCanonicalCollection {
        let mut state = ParserState { items: Vec::new(), lookahead: HashMap::new() };

        // Another awkward encounter with the borrow checker...
        // I would assume this is optimized away or very cheap, who knows
        let interned_canonical_collections = std::mem::take(&mut self.interned_canonical_collections);
        let CanonicalCollection(interned_items, lookahead) = &interned_canonical_collections[interned_collection.0 as usize];

        for (interned_item, lookahead) in interned_items.iter().zip(lookahead.iter()) {
            let ParserItem(production_id, position) = self.interned_items[interned_item.0 as usize];
            let production = self.interned_symbols.production(production_id);

            if production.rhs.len() > position && production.rhs[position] == symbol.id {
                let inner_interned_item = self.interned_item(ParserItem(production_id, position + 1));
                state.items.push(inner_interned_item);
                state.lookahead.insert(inner_interned_item, lookahead.clone());
            }
        }

        self.interned_canonical_collections = interned_canonical_collections;
        let next_interned_collection = self.canonicalize_state(state);
        self.precomputed_gotos.insert((next_interned_collection, symbol.id), next_interned_collection);
        next_interned_collection
    }

    pub fn compute_canonical_collection(&mut self, production_id: ProductionId) {
        let interned_item = self.zero_items[production_id.0 as usize];

        let mut lookahead = LookaheadSet::new(self.interned_symbols.terminals.len());
        lookahead.add(EOF_ID);
        let entry_state = ParserState {
            items: vec![interned_item],
            lookahead: HashMap::from([(interned_item, lookahead)])
        };
        self.compute_closure(entry_state);

        let mut changed = true;
        while changed {
            changed = false;
            // Further attempts at appeasing the borrow checker
            let mut buffer = Vec::new();
            let mut transitions: HashMap<(InternedCanonicalCollection, SymbolId), InternedCanonicalCollection> = HashMap::new();

            for (collection, &interned_collection) in &self.canonical_collections_lookup {
                for &interned_item in &collection.0 {
                    let ParserItem(production_id, position) = self.interned_items[interned_item.0 as usize];
                    let production = self.interned_symbols.production(production_id);
                    if production.rhs.len() <= position {
                        continue;
                    }

                    let current_symbol = production.rhs[position];
                    match self.precomputed_gotos.get(&(interned_collection, current_symbol)) {
                        Some(next_interned) => {
                            transitions.insert((interned_collection, current_symbol), *next_interned);
                        },
                        None => {
                            changed = true;
                            buffer.push((interned_collection, self.interned_symbols.symbol(current_symbol)));
                        }
                    };
                }
            }

            for (interned_collection, symbol) in buffer {
                let next_interned = self.compute_goto(interned_collection, symbol);
                self.precomputed_gotos.insert((interned_collection, symbol.id), next_interned);
            }

            self.transitions.extend(transitions);
        }
    }
} 

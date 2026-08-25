use std::collections::{HashMap, HashSet};

use crate::symbols::{EOF_ID, InternedSymbols, ProductionId, Symbol, SymbolId, SymbolKind};
use crate::table::{ParseTable, StateId};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
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

pub struct LookaheadIterator<'a> {
    iterator: std::slice::Iter<'a, u64>,
    current: u64,
    idx: u16,
}

impl<'a> Iterator for LookaheadIterator<'a> {
    type Item = SymbolId;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current == 0 {
            self.current = *self.iterator.next()?;
            self.idx += 1;
        }

        let lowest_bit = self.current.trailing_zeros();
        self.current &= self.current - 1;
        Some(SymbolId((self.idx - 1) * 64 + lowest_bit as u16))
    }
}

impl<'a> IntoIterator for &'a LookaheadSet {
    type Item = SymbolId;
    type IntoIter = LookaheadIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        LookaheadIterator { iterator: self.0.iter(), current: 0, idx: 0 }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct ParserItem(pub ProductionId, pub usize);

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct InternedParserItem(pub u32);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct CanonicalCollection(pub Box<[InternedParserItem]>, pub Box<[LookaheadSet]>);

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct InternedCanonicalCollection(pub u16);

impl From<InternedCanonicalCollection> for StateId {
    fn from(value: InternedCanonicalCollection) -> Self {
        StateId(value.0)
    }
}

pub struct ParserState {
    pub items: Vec<InternedParserItem>,
    pub lookahead: HashMap<InternedParserItem, LookaheadSet>,
}

pub struct GeneratorContext<'a> {
    interned_symbols: &'a InternedSymbols,
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
    pub fn new(interned_symbols: &'a InternedSymbols) -> Self {
        let first_sets: Vec<LookaheadSet> = (0..interned_symbols.nonterminals.len())
            .map(|_| LookaheadSet::new(interned_symbols.terminals.len()))
            .collect();

        GeneratorContext {
            interned_symbols,
            epsilon_nonterminals: HashSet::new(),
            first_sets: first_sets,
            zero_items: Vec::with_capacity(interned_symbols.productions.len()),
            interned_items: Vec::new(),
            items_lookup: HashMap::new(),
            interned_canonical_collections: Vec::new(),
            canonical_collections_lookup: HashMap::new(),
            precomputed_gotos: HashMap::new(),
            transitions: HashMap::new(),
        }
    }

    fn compute_epsilon_nonterminals(&mut self) {
        for nonterminal in &self.interned_symbols.nonterminals {
            let Symbol {
                kind: SymbolKind::Nonterminal { productions, .. }, ..
            } = nonterminal else { unreachable!() };

            let has_epsilon = productions.iter().any(|&id| self.interned_symbols.production(id).rhs.len() == 0);
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
                        production.rhs.iter().all(|sym| self.epsilon_nonterminals.contains(sym))
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

            for &production_id in productions {
                let production = self.interned_symbols.production(production_id);

                for &symbol_id in &production.rhs {
                    if matches!(self.interned_symbols.symbol(symbol_id), Symbol { kind: SymbolKind::Terminal { .. }, .. }) {
                        self.first_sets[self.interned_symbols.nonterminal_index(nonterminal.id)].add(symbol_id);
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
                        if matches!(self.interned_symbols.symbol(symbol_id), Symbol { kind: SymbolKind::Nonterminal { .. }, .. }) {
                            let next_index = self.interned_symbols.nonterminal_index(symbol_id);

                            if index != next_index {
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

    pub fn zero_items(&self, symbol: &Symbol) -> impl Iterator<Item = InternedParserItem> + use<'_> {
        let Symbol {
            kind: SymbolKind::Nonterminal { productions, .. }, ..
        } = symbol else { unreachable!() };

        productions.clone().into_iter().map(move |id| self.zero_items[id.0 as usize])
    }

    fn first_set<T: Iterator<Item = SymbolId>>(&self, symbols: T) -> LookaheadSet {
        let mut result = LookaheadSet::new(self.interned_symbols.nonterminals.len());
        for symbol_id in symbols {
            match self.interned_symbols.symbol(symbol_id) {
                Symbol { kind: SymbolKind::Terminal { .. }, .. } => {
                    result.add(symbol_id);
                }
                Symbol { kind: SymbolKind::Nonterminal { .. }, .. } => {
                    result.inplace_union(&self.first_sets[self.interned_symbols.nonterminal_index(symbol_id)]);
                }
            }
            if !self.epsilon_nonterminals.contains(&symbol_id) {
                break;
            }
        }
        result
    }

    pub fn interned_item(&mut self, item: ParserItem) -> InternedParserItem {
        match self.items_lookup.get(&item) {
            Some(interned_item) => *interned_item,
            _ => {
                let interned_item = InternedParserItem(self.interned_items.len() as u32);
                self.interned_items.push(item);
                self.items_lookup.insert(item, interned_item);
                interned_item
            }
        }
    }

    pub fn canonicalize_state(&mut self, mut state: ParserState) -> InternedCanonicalCollection {
        state.items.sort();

        let mut state_lookahead = state.lookahead;
        let lookahead = state.items.iter()
            .map(|item| state_lookahead.remove_entry(item).expect("Item has no lookahead").1)
            .collect::<Vec<LookaheadSet>>()
            .into_boxed_slice();

        let collection = CanonicalCollection(state.items.into_boxed_slice(), lookahead);
        match self.canonical_collections_lookup.get(&collection) {
            Some(interned) => *interned,
            _ => {
                let interned_collection = InternedCanonicalCollection(self.interned_canonical_collections.len() as u16);
                self.canonical_collections_lookup.insert(collection.clone(), interned_collection); // hold refs to collection in hashmap?
                self.interned_canonical_collections.push(collection);
                interned_collection
            }
        }
    }

    pub fn compute_closure(&mut self, mut state: ParserState) -> InternedCanonicalCollection {
        let mut changed = true;
        while changed {
            changed = false;
            let mut buffer = Vec::new();

            for interned_item in &state.items {
                let ParserItem(production_id, position) = self.interned_items[interned_item.0 as usize];
                let production = self.interned_symbols.production(production_id);
                if production.rhs.len() <= position {
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
        symbol_id: SymbolId,
    ) -> InternedCanonicalCollection {
        let mut state = ParserState { items: Vec::new(), lookahead: HashMap::new() };

        // Another awkward encounter with the borrow checker...
        // I would assume this is optimized away or very cheap, who knows
        let interned_canonical_collections = std::mem::take(&mut self.interned_canonical_collections);
        let CanonicalCollection(interned_items, lookahead) = &interned_canonical_collections[interned_collection.0 as usize];

        for (interned_item, lookahead) in interned_items.iter().zip(lookahead.iter()) {
            let ParserItem(production_id, position) = self.interned_items[interned_item.0 as usize];
            let production = self.interned_symbols.production(production_id);

            if production.rhs.len() > position && production.rhs[position] == symbol_id {
                let inner_interned_item = self.interned_item(ParserItem(production_id, position + 1));
                state.items.push(inner_interned_item);
                state.lookahead.insert(inner_interned_item, lookahead.clone());
            }
        }

        self.interned_canonical_collections = interned_canonical_collections;
        let next_interned_collection = self.compute_closure(state);
        self.precomputed_gotos.insert((interned_collection, symbol_id), next_interned_collection);
        next_interned_collection
    }

    pub fn compute_canonical_collection<T: Iterator<Item = ProductionId>>(&mut self, production_ids: T) {
        let mut entrypoint_states = HashMap::new();
        for production_id in production_ids {
            let interned_item = self.zero_items[production_id.0 as usize];
            let mut lookahead = LookaheadSet::new(self.interned_symbols.terminals.len());
            lookahead.add(EOF_ID);

            let entry_state = ParserState {
                items: vec![interned_item],
                lookahead: HashMap::from([(interned_item, lookahead)])
            };
            let interned_collection = self.compute_closure(entry_state);
            entrypoint_states.insert(production_id, interned_collection);
        }
        assert!(entrypoint_states.len() > 0);

        let mut transitions = std::mem::take(&mut self.transitions);
        let mut changed = true;
        while changed {
            changed = false;
            // Further attempts at appeasing the borrow checker
            let mut buffer = Vec::new();

            for (collection, &interned_collection) in &self.canonical_collections_lookup {
                for &interned_item in &collection.0 {
                    let ParserItem(production_id, position) = self.interned_items[interned_item.0 as usize];
                    let production = self.interned_symbols.production(production_id);
                    if production.rhs.len() <= position {
                        continue;
                    }

                    let current_symbol = production.rhs[position];
                    match self.precomputed_gotos.get(&(interned_collection, current_symbol)) {
                        Some(&next_interned) => {
                            match transitions.get(&(interned_collection, current_symbol)) {
                                Some(&existing_entry) => {
                                    if existing_entry != next_interned {
                                        panic!("Trantisions differ");
                                    }
                                }
                                _ => {
                                    changed = true;
                                    transitions.insert((interned_collection, current_symbol), next_interned);
                                }
                            }
                        },
                        None => {
                            changed = true;
                            buffer.push((interned_collection, current_symbol));
                        }
                    };
                }
            }

            for (interned_collection, symbol_id) in buffer {
                let next_interned_collection = self.compute_goto(interned_collection, symbol_id);
                transitions.insert((interned_collection, symbol_id), next_interned_collection);
            }
        }
    
        self.transitions = transitions;
    }

    pub fn compute_table(&mut self) -> ParseTable<'a> {
        self.compute_epsilon_nonterminals();
        self.compute_first_sets();
        self.compute_zero_items();

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

        self.compute_canonical_collection(production_ids);
        let mut table = ParseTable::new(&self.interned_symbols, self.interned_canonical_collections.len());

        for (collection, &interned_collection) in &self.canonical_collections_lookup {
            let mut nonterminals = HashSet::new();

            let CanonicalCollection(interned_items, lookaheads) = collection;
            for (interned_item, lookahead) in interned_items.iter().zip(lookaheads.iter()) {
                let ParserItem(production_id, position) = self.interned_items[interned_item.0 as usize];
                let production = self.interned_symbols.production(production_id);

                if production.rhs.len() <= position {
                    let lhs = self.interned_symbols.nonterminal(production.lhs_id);
                    if let Symbol { kind: SymbolKind::Nonterminal { entrypoint: true, .. }, .. } = lhs {
                        table.add_accept(interned_collection.into(), production_id).unwrap();
                    }
                    else {
                        for lookahead_symbol in lookahead {
                            table.add_reduce(interned_collection.into(), lookahead_symbol, production_id).unwrap();
                        }
                    }
                }
                else {
                    let symbol_id = production.rhs[position];
                    if nonterminals.contains(&symbol_id) {
                        continue;
                    }

                    let Some(&next_interned_collection) = self.transitions.get(&(interned_collection, symbol_id)) else {
                        continue
                    };

                    match self.interned_symbols.symbol(symbol_id) {
                        Symbol { kind: SymbolKind::Terminal { .. }, .. } => {
                            table.add_shift(interned_collection.into(), symbol_id, next_interned_collection.into()).unwrap();
                        }
                        Symbol { kind: SymbolKind::Nonterminal { .. }, .. } => {
                            nonterminals.insert(symbol_id);
                            table.add_goto(interned_collection.into(), symbol_id, next_interned_collection.into()).unwrap();
                        }
                    }

                }
            }
        }

        table
    }
} 

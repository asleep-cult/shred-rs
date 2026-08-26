use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hasher, Hash};
use std::io;

use crate::symbols::{EOF_ID, InternedSymbols, ProductionId, Symbol, SymbolId, SymbolKind};
use crate::table::{ParseTable, StateId};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct LookaheadSet(pub Box<[u64]>);

impl LookaheadSet {
    pub fn size(amount: usize) -> usize {
        if amount % 64 != 0 { (amount / 64) + 1 } else { amount / 64 }
    }

    fn new(amount: usize) -> Self {
        LookaheadSet(vec![0; Self::size(amount)].into_boxed_slice())
    }

    fn add(&mut self, symbol_id: SymbolId) {
        let index = symbol_id.0 as usize / 64;
        let remainder = symbol_id.0 as usize % 64;
        self.0[index] |= 1 << remainder;
    }

    fn inplace_union(&mut self, other: &LookaheadSet) -> bool {
        let mut changed = false;
        assert_eq!(self.0.len(), other.0.len());

        for idx in 0..self.0.len() {
            let combined = self.0[idx] | other.0[idx];
            if self.0[idx] != combined {
                changed = true;
                self.0[idx] = combined;
            }
        }
        changed
    }
}

pub struct LookaheadIterator<T> {
    iterator: T,
    current: u64,
    idx: u16,
}

impl<'a, T: Iterator<Item = &'a u64>> Iterator for LookaheadIterator<T> {
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
    type IntoIter = LookaheadIterator<std::slice::Iter<'a, u64>>;

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

impl CanonicalCollection {
    pub fn zip(&self) -> impl Iterator<Item = (InternedParserItem, &LookaheadSet)> {
        self.0.iter().map(|&item| item).zip(self.1.iter())
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct InternedCanonicalCollection(pub u16);

impl From<InternedCanonicalCollection> for StateId {
    fn from(value: InternedCanonicalCollection) -> Self {
        StateId(value.0)
    }
}

#[derive(Default)]
pub struct GeneratorInterner {
    pub items: Vec<ParserItem>,
    pub item_lookup: HashMap<u64, Vec<usize>>,
    pub collections: Vec<CanonicalCollection>,
    pub collection_lookup: HashMap<u64, Vec<usize>>,
}

impl GeneratorInterner {
    pub fn new() -> Self {
        GeneratorInterner {
            items: Vec::new(),
            item_lookup: HashMap::new(),
            collections: Vec::new(),
            collection_lookup: HashMap::new(),
        }
    }

    pub fn intern_item(&mut self, production_id: ProductionId, position: usize) -> InternedParserItem {
        let item = ParserItem(production_id, position);

        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        let hash = hasher.finish();

        if let Some(bucket) = self.item_lookup.get(&hash) {
            for &idx in bucket {
                if self.items[idx] == item {
                    return InternedParserItem(idx as u32);
                }
            }
        }

        let length = self.items.len();
        self.items.push(item);

        self.item_lookup.entry(hash).or_default().push(length);
        InternedParserItem(length as u32)
    }

    pub fn intern_collection(&mut self, collection: CanonicalCollection) -> InternedCanonicalCollection {
        let mut hasher = DefaultHasher::new();
        collection.hash(&mut hasher);
        let hash = hasher.finish();

        if let Some(bucket) = self.collection_lookup.get(&hash) {
            for &idx in bucket {
                if self.collections[idx] == collection {
                    return InternedCanonicalCollection(idx as u16);
                }
            }
        }

        let length = self.collections.len();
        self.collections.push(collection);

        self.collection_lookup.entry(hash).or_default().push(length);
        InternedCanonicalCollection(length as u16)
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
    zero_items: Vec<Vec<InternedParserItem>>,
    interner: GeneratorInterner,
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
            zero_items: Vec::with_capacity(interned_symbols.nonterminals.len()),
            interner: GeneratorInterner::new(),
            precomputed_gotos: HashMap::new(),
            transitions: HashMap::new(),
        }
    }

    pub fn dump_collection<T: io::Write>(&self, writer: &mut T, interned_collection: InternedCanonicalCollection) -> io::Result<()> {
        write!(writer, "<parser state dump {}>\n", interned_collection.0)?;

        for (i, (interned_item, lookahead)) in self.interner.collections[interned_collection.0 as usize].zip().enumerate() {
            let ParserItem(production_id, position) = self.interner.items[interned_item.0 as usize];
            let production = self.interned_symbols.production(production_id);
            let lhs = self.interned_symbols.nonterminal(production.lhs_id);

            let names = lookahead.into_iter()
                .map(|sym| self.interned_symbols.symbol(sym).name.clone())
                .collect::<Vec<String>>()
                .join(", ");

            write!(writer, "    ({}., pos={}, lookahead={{{}}}): {} ->", i, position, names, lhs.name)?;

            for (j, &symbol_id) in production.rhs.iter().enumerate() {
                if j == position {
                    write!(writer, " *")?;
                }

                match self.interned_symbols.symbol(symbol_id) {
                    Symbol { kind: SymbolKind::Terminal { value: Some(value) }, .. } => {
                        write!(writer, " {}", value)?;
                    }
                    Symbol { name, .. } => {
                        write!(writer, " {}", name)?;
                    }
                }
            }

            if production.rhs.len() <= position {
                write!(writer, " *")?;
            }
            write!(writer, "\n")?;
        }
        Ok(())
    }

    fn compute_epsilon_nonterminals(&mut self) {
        for production in self.interned_symbols.productions.values() {
            if production.rhs.len() == 0 {
                self.epsilon_nonterminals.insert(production.lhs_id);
            }
        }

        let mut changed = true;
        while changed {
            changed = false;

            for production in self.interned_symbols.productions.values() {
                if !self.epsilon_nonterminals.contains(&production.lhs_id) && production.rhs.iter().all(
                    |sym| self.epsilon_nonterminals.contains(sym)
                ) {
                    self.epsilon_nonterminals.insert(production.lhs_id);
                    changed = true;
                }
            }
        }
    }

    pub fn compute_first_sets(&mut self) {
        for production in self.interned_symbols.productions.values() {
            for &symbol_id in &production.rhs {
                if matches!(self.interned_symbols.symbol(symbol_id), Symbol { kind: SymbolKind::Terminal { .. }, .. }) {
                    self.first_sets[self.interned_symbols.nonterminal_index(production.lhs_id)].add(symbol_id);
                }

                if !self.epsilon_nonterminals.contains(&symbol_id) {
                    break;
                }
            }
        }

        let mut changed = true;
        while changed {
            changed = false;

            for production in self.interned_symbols.productions.values() {
                for &symbol_id in &production.rhs {
                    if matches!(self.interned_symbols.symbol(symbol_id), Symbol { kind: SymbolKind::Nonterminal { .. }, .. }) {
                        let lhs_index = self.interned_symbols.nonterminal_index(production.lhs_id);
                        let rhs_index = self.interned_symbols.nonterminal_index(symbol_id);

                        if lhs_index != rhs_index {
                            let rhs_lookahead = self.first_sets[rhs_index].clone();
                            if self.first_sets[lhs_index].inplace_union(&rhs_lookahead) {
                                changed = true;
                            }
                        }
                    }
                    if !self.epsilon_nonterminals.contains(&symbol_id) {
                        break;
                    }
                }
            }
        }
    }

    pub fn compute_zero_items(&mut self) {
        for nonterminal in &self.interned_symbols.nonterminals {
            let Symbol {
                kind: SymbolKind::Nonterminal { productions, .. }, ..
            } = nonterminal else { unreachable!() };

            let mut items = Vec::with_capacity(productions.len());
            for &production_id in productions {
                items.push(self.interner.intern_item(production_id, 0));
            }

            self.zero_items.push(items);
        }
    }

    fn first_set<T: Iterator<Item = &'a SymbolId>>(&self, symbols: T) -> LookaheadSet {
        let mut result = LookaheadSet::new(self.interned_symbols.terminals.len());
        for &symbol_id in symbols {
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

    pub fn canonicalize_state(&mut self, mut state: ParserState) -> InternedCanonicalCollection {
        state.items.sort();
    
        let lookahead = state.items.iter()
            .map(|item| {
                state.lookahead.remove(item).expect("Item has no lookahead")
            })
            .collect::<Vec<LookaheadSet>>();

        let collection = CanonicalCollection(state.items.into_boxed_slice(), lookahead.into_boxed_slice());
        self.interner.intern_collection(collection)
    }

    pub fn compute_closure(&mut self, mut state: ParserState) -> InternedCanonicalCollection {
        let mut worklist = Vec::from(state.items.clone());

        while let Some(interned_item) = worklist.pop() {
            let ParserItem(production_id, position) = self.interner.items[interned_item.0 as usize];
            let production = self.interned_symbols.production(production_id);
            if production.rhs.len() <= position {
                continue;
            }

            let current_symbol = self.interned_symbols.symbol(production.rhs[position]);
            if !matches!(current_symbol, Symbol { kind: SymbolKind::Nonterminal { .. }, .. }) {
                continue;
            }

            let lookahead = &state.lookahead[&interned_item];
            let trailing_symbols = &production.rhs[position + 1..];
            let mut next_lookahead = self.first_set(trailing_symbols.iter());

            if trailing_symbols.iter().all(|id| self.epsilon_nonterminals.contains(id)) {
                next_lookahead.inplace_union(lookahead);
            }

            for &next_interned_item in &self.zero_items[self.interned_symbols.nonterminal_index(current_symbol.id)] {
                match state.lookahead.entry(next_interned_item) {
                    Entry::Vacant(entry) => {
                        entry.insert(next_lookahead.clone());
                        state.items.push(next_interned_item);
                        worklist.push(next_interned_item);
                    }
                    Entry::Occupied(mut entry) => {
                        if entry.get_mut().inplace_union(&next_lookahead) {
                            worklist.push(next_interned_item);
                        }
                    }
                }
            }
        }

        self.canonicalize_state(state)
    }

    pub fn compute_goto(
        &mut self,
        interned_collection: InternedCanonicalCollection,
        symbol_id: SymbolId,
    ) -> InternedCanonicalCollection {
        let mut state = ParserState { items: Vec::new(), lookahead: HashMap::new() };

        let collections = std::mem::take(&mut self.interner.collections);
        let collection  = &collections[interned_collection.0 as usize];
    
        for (item, lookahead) in collection.zip() {
            let ParserItem(production_id, position) = self.interner.items[item.0 as usize];
            let production = self.interned_symbols.production(production_id);

            if production.rhs.len() > position && production.rhs[position] == symbol_id {
                let interned_item = self.interner.intern_item(production_id, position + 1);
                state.items.push(interned_item);
                state.lookahead.insert(interned_item, lookahead.clone());
            }
        }

        self.interner.collections = collections;
        let next_interned_collection = self.compute_closure(state);
        self.precomputed_gotos.insert((interned_collection, symbol_id), next_interned_collection);
        next_interned_collection
    }

    pub fn compute_canonical_collection<T: Iterator<Item = ProductionId>>(&mut self, production_ids: T) {
        let mut entrypoint_states = HashMap::new();
        for production_id in production_ids {
            let interned_item = self.interner.intern_item(production_id, 0);
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
        let mut worklist: Vec<InternedCanonicalCollection> = (0..self.interner.collections.len())
            .map(|n| InternedCanonicalCollection(n as u16))
            .collect();

        while let Some(interned_collection) = worklist.pop() {
            let mut buffer = Vec::new();
            let mut pending_transitions = Vec::new();

            let collection = &self.interner.collections[interned_collection.0 as usize];
            for &interned_item in &collection.0 {
                let ParserItem(production_id, position) = self.interner.items[interned_item.0 as usize];
                let production = self.interned_symbols.production(production_id);
                if production.rhs.len() <= position {
                    continue;
                }

                let current_symbol = production.rhs[position];
                match self.precomputed_gotos.entry((interned_collection, current_symbol)) {
                    Entry::Vacant(_) => {
                        buffer.push(current_symbol);
                    }
                    Entry::Occupied(entry) => {
                        pending_transitions.push((current_symbol, *entry.get()));
                    }
                }
            }

            for symbol_id in buffer {
                let next_interned = self.compute_goto(interned_collection, symbol_id);
                worklist.push(next_interned);
                pending_transitions.push((symbol_id, next_interned));
            }

            for (symbol_id, next_interned) in pending_transitions {
                match transitions.entry((interned_collection, symbol_id)) {
                    Entry::Vacant(entry) => {
                        entry.insert(next_interned);
                    }
                    Entry::Occupied(entry) => {
                        if *entry.get() != next_interned {
                            panic!("Entries differ");
                        }
                    }
                }
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
        let mut table = ParseTable::new(&self.interned_symbols, self.interner.collections.len());

        let interned_collections = (0..self.interner.collections.len())
            .map(|n| InternedCanonicalCollection(n as u16));

        for interned_collection in interned_collections {
            let mut nonterminals = HashSet::new();

            let collection = &self.interner.collections[interned_collection.0 as usize];
            for (interned_item, lookahead) in collection.zip() {
                let ParserItem(production_id, position) = self.interner.items[interned_item.0 as usize];
                let production = self.interned_symbols.production(production_id);

                if production.rhs.len() <= position {
                    let lhs = self.interned_symbols.nonterminal(production.lhs_id);
                    if let Symbol { kind: SymbolKind::Nonterminal { entrypoint: true, .. }, .. } = lhs {
                        table.add_accept(interned_collection.into(), production_id);
                    }
                    else {
                        for lookahead_symbol in lookahead {
                            table.add_reduce(interned_collection.into(), lookahead_symbol, production_id);
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
                            table.add_shift(interned_collection.into(), symbol_id, next_interned_collection.into());
                        }
                        Symbol { kind: SymbolKind::Nonterminal { .. }, .. } => {
                            nonterminals.insert(symbol_id);
                            table.add_goto(interned_collection.into(), symbol_id, next_interned_collection.into());
                        }
                    }

                }
            }
        }

        table
    }
} 

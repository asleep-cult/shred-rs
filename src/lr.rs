use std::collections::HashMap;
use rustc_hash::FxHasher;
use std::hash::Hasher;

use crate::symbols::{InternedSymbols, ProductionId, SymbolId};
use crate::bitset::{Bitset, BitsetData};
use crate::table::StateId;

pub const LOOKAHEAD_SET_SIZE: usize = 2;
pub type LookaheadSet = Bitset<[u64; LOOKAHEAD_SET_SIZE]>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Lr1Item {
    pub index: usize,
    pub lookahead: LookaheadSet,
}

pub struct CanonicalCollection {
    pub context: LRContext,
    pub transitions: HashMap<(StateId, SymbolId), StateId>,
    pub epsilon_transitions: Vec<(StateId, Lr1Item)>,
}

pub struct LRContext {
    pub item_offsets: Vec<usize>,
    pub kernels: Vec<Vec<Lr1Item>>,
    pub kernel_ids: HashMap<u64, StateId>,
}

impl LRContext {
    pub fn new(interned_symbols: &InternedSymbols) -> Self {
        let mut item_offsets = Vec::with_capacity(interned_symbols.productions.len());

        let mut offset = 0;
        for production in interned_symbols.iter_productions() {
            item_offsets.push(offset);
            offset += production.rhs.len() + 1;
        }

        Self {
            item_offsets,
            kernels: Vec::new(),
            kernel_ids: HashMap::new(),
        }
    }

    pub fn item_index(&self, production_id: ProductionId, position: u8) -> usize {
        self.item_offsets[production_id.0 as usize] + position as usize
    }

    pub fn item_core(&self, index: usize) -> (ProductionId, u8) {
        let (production_id, position) = match self.item_offsets.binary_search(&index) {
            Ok(production_id) => (production_id, 0),
            Err(res) => {
                let production_id = res - 1;
                let position = index - self.item_offsets[production_id];
                (production_id, position)
            }
        };

        (ProductionId(production_id as u16), position as u8)
    }

    fn get_hash(&self, items: &Vec<Lr1Item>) -> u64 {
        let mut hasher = FxHasher::default();

        for item in items {
            hasher.write_usize(item.index);

            for word in item.lookahead.0.iter_words() {
                hasher.write_u64(word);
            }
        }

        hasher.finish()
    }

    pub fn canonicalize_state(&mut self, mut items: Vec<Lr1Item>) -> (StateId, bool) {
        items.sort_by_key(|item| self.item_core(item.index));

        let hash = self.get_hash(&items);
        if let Some(&existing_entry) = self.kernel_ids.get(&hash) {
            if self.kernels[existing_entry.0 as usize] == items {
                return (existing_entry, false);
            }
        }

        let id = StateId(self.kernels.len() as u16);
        self.kernel_ids.insert(hash, id);
        self.kernels.push(items);
        (id, true)
    }
}

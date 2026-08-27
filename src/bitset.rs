use std::{iter::Copied};

pub trait BitsetData {
    type Iter<'a>: Iterator<Item = u64> where Self: 'a;

    fn word_len(&self) -> usize;
    fn iter_words(&self) -> Self::Iter<'_>;
    fn get_word(&self, index: usize) -> u64;
    fn set_word(&mut self, index: usize, word: u64);
    fn clear_words(&mut self);
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct Bitset<T>(pub T);

impl BitsetData for Vec<u64> {
    type Iter<'a> = Copied<std::slice::Iter<'a, u64>>;

    fn word_len(&self) -> usize {
        self.len()
    }

    fn iter_words(&self) -> Self::Iter<'_> {
        self.iter().copied()
    }

    fn get_word(&self, index: usize) -> u64 {
        self[index]
    }

    fn set_word(&mut self, index: usize, word: u64) {
        self[index] = word
    }

    fn clear_words(&mut self) {
        self.fill(0);
    }
}

impl<const N: usize> BitsetData for [u64; N] {
    type Iter<'a> = Copied<std::slice::Iter<'a, u64>>;

    fn word_len(&self) -> usize {
        self.len()
    }

    fn iter_words(&self) -> Self::Iter<'_> {
        self.iter().copied()
    }

    fn get_word(&self, index: usize) -> u64 {
        self[index]
    }

    fn set_word(&mut self, index: usize, word: u64) {
        self[index] = word
    }

    fn clear_words(&mut self) {
        self.fill(0);
    }
}

impl Bitset<Vec<u64>> {
    pub fn new(amount: usize) -> Self {
        Bitset(vec![0; Self::required_size(amount)])
    }
}

impl<T: BitsetData> Bitset<T> {
    pub fn required_size(amount: usize) -> usize {
        amount.div_ceil(64)
    }

    pub fn add(&mut self, index: usize) -> bool {
        // Return false if it was already there
        let word_index = index as usize / 64;
        let remainder = index as usize % 64;
        debug_assert!(word_index < self.0.word_len());

        let word = self.0.get_word(word_index);
        let result = word | 1 << remainder;
        self.0.set_word(word_index, result);

        word != result
    }

    pub fn has(&self, index: usize) -> bool {
        let word_index = index as usize / 64;
        let remainder = index as usize % 64;

        let word = self.0.get_word(word_index);
        (word & 1 << remainder) != 0
    }

    pub fn inplace_union<U: BitsetData>(&mut self, other: &Bitset<U>) -> bool {
        let mut changed = false;
        debug_assert_eq!(self.0.word_len(), other.0.word_len());

        for idx in 0..self.0.word_len() {
            let existing = self.0.get_word(idx);
            let combined = existing | other.0.get_word(idx);
            if existing != combined {
                changed = true;
                self.0.set_word(idx, combined);
            }
        }
        changed
    }

    pub fn is_superset<U: BitsetData>(&self, other: &Bitset<U>) -> bool {
        self.0.iter_words().zip(other.0.iter_words())
            .all(|(a, b)| (a & b) == b)
    }

    pub fn clear(&mut self) {
        self.0.clear_words();
    }
}

pub struct BitsetIterator<T> {
    iterator: T,
    word: u64,
    idx: usize,
}

impl<T: Iterator<Item = u64>> Iterator for BitsetIterator<T> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while self.word == 0 {
            self.word = self.iterator.next()?;
            self.idx += 1;
        }

        let lowest_bit = self.word.trailing_zeros() as usize;
        self.word &= self.word - 1;
        Some((self.idx - 1) * 64 + lowest_bit)
    }
}

impl<'a, T: BitsetData> IntoIterator for &'a Bitset<T> {
    type Item = usize;
    type IntoIter = BitsetIterator<T::Iter<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        BitsetIterator { iterator: self.0.iter_words(), word: 0, idx: 0 }
    }
}

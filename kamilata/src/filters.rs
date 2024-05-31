#[derive(Debug, Clone)]
pub struct Filter<const N: usize>(Box<[u8; N]>);

impl<const N: usize> Filter<N> {
    /// Creates an empty filter.
    pub fn new() -> Self {
        Filter(Box::new([0; N]))
    }

    /// Clears the filter.
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// Gets a bit in the filter.
    pub fn get_bit(&self, idx: usize) -> bool {
        if idx >= self.bit_len() {
            return false;
        }
        let bit_idx = idx.rem_euclid(8);
        let byte_idx = idx.div_euclid(8);
        let bit = unsafe {
            (self.0.get_unchecked(byte_idx) >> bit_idx) & 1
        };
        bit != 0
    }

    /// Gets a word in the filter.
    pub fn get_word<S: crate::store::Store<N>>(&self, word: &str) -> bool {
        S::hash_word(word).into_iter().all(|hash| self.get_bit(hash))
    }

    /// Sets a bit in the filter.
    pub fn set_bit(&mut self, idx: usize, value: bool) {
        if idx >= self.bit_len() {
            return;
        }
        let bit_idx = idx.rem_euclid(8);
        let byte_idx = idx.div_euclid(8);
        let bit = value as u8;
        let keeping_mask = !(1 << bit_idx);
        unsafe {
            *self.0.get_unchecked_mut(byte_idx) = (self.0.get_unchecked(byte_idx) & keeping_mask) + (bit << bit_idx);
        }
    }

    /// Adds a word in the filter.
    pub fn add_word<S: crate::store::Store<N>>(&mut self, word: &str) {
        S::hash_word(word).into_iter().for_each(|hash| self.set_bit(hash, true));
    }

    /// Returns the number of bits set to 1 in the filter.
    pub fn count_set_bits(&self) -> usize {
        self.0.iter().map(|byte| byte.count_ones() as usize).sum()
    }

    /// Returns the proportion of bits that are set to 1.
    pub fn load(&self) -> f64 {
        self.count_set_bits() as f64 / self.bit_len() as f64
    }

    /// Returns the number of bytes in the filter.
    pub const fn len(&self) -> usize {
        N
    }

    /// Returns the number of bits in the filter.
    pub const fn bit_len(&self) -> usize {
        N*8
    }

    /// Returns true if the filter is empty.
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
}

impl<const N: usize> Default for Filter<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> std::ops::BitOr for Filter<N> {
    type Output = Self;

    fn bitor(mut self, other: Self) -> Self::Output {
        for byte_idx in 0..N {
            unsafe {
                *self.0.get_unchecked_mut(byte_idx) |= *other.0.get_unchecked(byte_idx);
            }
        }
        self
    }
}

impl<const N: usize> Filter<N> {
    pub fn bitor_assign_ref(&mut self, other: &Self) {
        for byte_idx in 0..N {
            unsafe {
                *self.0.get_unchecked_mut(byte_idx) |= *other.0.get_unchecked(byte_idx);
            }
        }
    }
}

impl<const N: usize> std::ops::BitOrAssign for Filter<N> {
    fn bitor_assign(&mut self, other: Self) {
        self.bitor_assign_ref(&other);
    }
}

impl<const N: usize> From<&[u8]> for Filter<N> {
    fn from(bytes: &[u8]) -> Self {
        let mut filter = Filter::new();
        for byte_idx in 0..N {
            // TODO check that bytes.len() == N
            unsafe {
                *filter.0.get_unchecked_mut(byte_idx) = *bytes.get_unchecked(byte_idx);
            }
        }
        filter
    }
}

impl<const N: usize> From<&Filter<N>> for Vec<u8> {
    fn from(filter: &Filter<N>) -> Self {
        filter.0.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_ops() {
        let mut filter = Filter::<4>::new();
        assert!(filter.is_empty());
        filter.set_bit(8, true);
        assert_eq!(filter.count_set_bits(), 1);
        filter.set_bit(8, false);
        filter.set_bit(9, true);
        assert_eq!(filter.count_set_bits(), 1);
        filter.set_bit(10, true);
        assert_eq!(filter.count_set_bits(), 2);
    }

    #[test]
    fn or_ops() {
        let mut filter1 = Filter::<4>::new();
        filter1.set_bit(8, true);
        let mut filter2 = Filter::<4>::new();
        filter2.set_bit(9, true);
        let filter3 = filter1 | filter2;
        assert_eq!(filter3.count_set_bits(), 2);

        let mut filter1 = Filter::<4>::new();
        filter1.set_bit(8, true);
        let mut filter2 = Filter::<4>::new();
        filter2.set_bit(8, true);
        filter2.set_bit(9, true);
        let filter3 = filter1 | filter2;
        assert_eq!(filter3.count_set_bits(), 2);
    }
}

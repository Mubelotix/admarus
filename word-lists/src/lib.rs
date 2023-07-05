mod word_lists;
pub use word_lists::*;

pub trait HackTraitSortedContains<T> {
    /// Like `contains()` but optimized for sorted arrays.
    fn sorted_contains(&self, item: T) -> bool;
}

impl<T: Ord> HackTraitSortedContains<T> for Vec<T> {
    fn sorted_contains(&self, item: T) -> bool {
        self.binary_search(&item).is_ok()
    }
}

impl<T: Ord> HackTraitSortedContains<T> for [T] {
    fn sorted_contains(&self, item: T) -> bool {
        self.binary_search(&item).is_ok()
    }
}

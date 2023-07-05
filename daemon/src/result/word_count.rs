use crate::prelude::*;

/// Used to count words but counts different types of words separately.
/// The sum of all fields is the total number of words.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WordCount {
    /// Counters for: h1, h2, h3, h4, h5, h6, strong, em, regular, small, s
    data: [usize; 11]
}

impl WordCount {
    pub const fn h1(&self) -> usize {
        self.data[0]
    }

    pub const fn h2(&self) -> usize {
        self.data[1]
    }

    pub const fn h3(&self) -> usize {
        self.data[2]
    }

    pub const fn h4(&self) -> usize {
        self.data[3]
    }

    pub const fn h5(&self) -> usize {
        self.data[4]
    }

    pub const fn h6(&self) -> usize {
        self.data[5]
    }

    pub const fn strong(&self) -> usize {
        self.data[6]
    }

    pub const fn em(&self) -> usize {
        self.data[7]
    }

    pub const fn regular(&self) -> usize {
        self.data[8]
    }

    pub const fn small(&self) -> usize {
        self.data[9]
    }

    pub const fn s(&self) -> usize {
        self.data[10]
    }

    pub fn sum(&self) -> usize {
        self.data.iter().sum()
    }
    
    pub fn weighted_sum(&self) -> f64 {
        self.h1() as f64 * 10.0
            + self.h2() as f64 * 9.0
            + self.h3() as f64 * 8.0
            + self.h4() as f64 * 7.0
            + self.h5() as f64 * 6.0
            + self.h6() as f64 * 5.5
            + self.strong() as f64 * 4.0
            + self.em() as f64 * 1.1
            + self.regular() as f64 * 1.0
            + self.small() as f64 * 0.3
            + self.s() as f64 * 0.1
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add(&mut self, h1: bool, h2: bool, h3: bool, h4: bool, h5: bool, h6: bool, strong: bool, em: bool, small: bool, s: bool) {
        if h1 { self.data[0] += 1; return }
        if h2 { self.data[1] += 1; return }
        if h3 { self.data[2] += 1; return }
        if h4 { self.data[3] += 1; return }
        if h5 { self.data[4] += 1; return }
        if h6 { self.data[5] += 1; return }
        if strong { self.data[6] += 1; return }
        if em { self.data[7] += 1; return }
        if small { self.data[9] += 1; return }
        if s { self.data[10] += 1; return }
        self.data[8] += 1;
    }
}

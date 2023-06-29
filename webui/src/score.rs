use crate::prelude::*;

#[derive(Clone, Copy)]
pub struct Score {
    val: f64,
}

impl From<f64> for Score {
    fn from(val: f64) -> Self {
        Self { val }
    }
}

impl PartialEq for Score {
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}

impl Eq for Score {}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.val.partial_cmp(&other.val)
    }
}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl std::fmt::Display for Score {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.val)
    }
}

impl std::fmt::Debug for Score {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.val)
    }
}

pub struct Scores {
    pub tf_score: Score,
    pub length_score: Score,
    pub popularity_score: Score,
}

impl Scores {
    pub fn general_score(&self) -> Score {
        Score::from(self.tf_score.val * 0.25 + self.length_score.val * 0.25 + self.popularity_score.val * 0.5)
    }
}

impl PartialEq for Scores {
    fn eq(&self, other: &Self) -> bool {
        self.tf_score == other.tf_score && self.length_score == other.length_score
    }
}

impl Eq for Scores {}

impl PartialOrd for Scores {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.general_score().partial_cmp(&other.general_score())
    }
}

impl Ord for Scores {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

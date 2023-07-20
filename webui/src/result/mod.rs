#![allow(clippy::module_inception)]
#![allow(dead_code)]

mod result;
mod scores;
mod word_count;
mod ranked;
mod format;
mod verification;

pub use {result::*, scores::*, word_count::*, ranked::*, format::*, verification::*};

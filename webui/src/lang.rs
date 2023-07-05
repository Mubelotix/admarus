use word_lists::*;

pub enum Lang {
    English,
}

impl Lang {
    pub fn common_words(&self) -> &[&str] {
        match self {
            Lang::English => WORDS_EN,
        }
    }
}

use word_lists::*;

pub enum Lang {
    English,
}

impl Lang {
    fn word_list(&self) -> &[&str] {
        match self {
            Lang::English => WORDS_EN,
        }
    }
}

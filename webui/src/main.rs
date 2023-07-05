mod app;
mod prelude;
mod util;
mod result;
mod api;
mod score;
mod lucky;
mod lang;

#[path = "search/search.rs"]
mod search;
#[path = "settings/settings.rs"]
mod settings;
#[path = "results/results.rs"]
mod results;
#[path = "components/search_bar/search_bar.rs"]
mod search_bar;

mod query;

use crate::prelude::*;

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into());
    };
}

fn main() {
    yew::Renderer::<App>::new().render();
}

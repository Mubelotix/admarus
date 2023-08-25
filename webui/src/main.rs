mod app;
mod prelude;
mod util;
mod result;
mod api_bodies;
mod api;
mod lucky;
mod lang;

#[path = "pages/search/search.rs"]
mod search;
#[path = "pages/settings/settings.rs"]
mod settings;
#[path = "pages/results/results.rs"]
mod results;
#[path = "components/search_bar/search_bar.rs"]
mod search_bar;
#[path = "components/connection_status/connection_status.rs"]
mod connection_status;
#[path = "components/result/result.rs"]
mod result_comp;
#[path = "components/image_grid/image_grid.rs"]
mod image_grid;

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

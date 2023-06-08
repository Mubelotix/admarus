
pub enum Document {
    Html(HtmlDocument),
}

pub struct HtmlDocument {
    data: String,
}

impl HtmlDocument {
    pub fn new(data: String) -> HtmlDocument {
        HtmlDocument {
            data,
        }
    }
}



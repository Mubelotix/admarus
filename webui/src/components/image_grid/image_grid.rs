use crate::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct ImageGridProps {
    pub images: Rc<Vec<String>>,
}

pub struct ImageGrid {
    elements: HashMap<String, Html>,
    rows: Vec<Vec<String>>,
    sizes: HashMap<String, (u32, u32)>,
}

impl Component for ImageGrid {
    type Message = ();
    type Properties = ImageGridProps;

    fn create(ctx: &Context<Self>) -> Self {
        let mut elements = HashMap::new();
        let mut rows = vec![Vec::new()];
        let mut sizes = HashMap::new();

        for image in ctx.props().images.iter() {
            elements.insert(image.to_owned(), html! { <div class="image-grid-container"><img src={image.to_owned()} /></div> });
            rows[0].push(image.to_owned());
        }

        ImageGrid { elements, rows, sizes }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let mut rows = Vec::new();

        for row in &self.rows {
            rows.push(html! {
                <div class="image-grid-row">
                    {row.iter().filter_map(|cid| self.elements.get(cid)).cloned().collect::<Html>()}
                </div>
            });
        }

        rows.into_iter().collect()
    }
}

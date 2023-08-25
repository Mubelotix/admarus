use crate::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct ImageGridProps {
    pub images: Rc<Vec<String>>,
}

pub struct ImageGrid {
    elements: HashMap<String, Html>,
    rows: Vec<Vec<String>>,
    sizes: HashMap<String, (u32, u32)>,
    row_width: f32,
    row_height: f32,
}



impl ImageGrid {
    fn row_loads(&self) -> Vec<f32> {
        self.rows.iter().map(|cids| {
            let mut width = 0.0;

            for cid in cids {
                if let Some((w, h)) = self.sizes.get(cid) {
                    width += *w as f32 * (*h as f32 / self.row_height);
                }
                width += 5.0;
            }
            if width > 0.0 {
                width -= 5.0;
            }

            width / self.row_width
        }).collect()
    }

    fn display_row_loads(&self) {
        let loads = self.row_loads();
        log!("{:?}", loads);
    }
}

impl Component for ImageGrid {
    type Message = ();
    type Properties = ImageGridProps;

    fn create(ctx: &Context<Self>) -> Self {
        let mut elements = HashMap::new();
        let mut rows = vec![Vec::new(); ctx.props().images.len() / 5];
        let mut sizes = HashMap::new();

        for (i, image) in ctx.props().images.iter().enumerate() {
            elements.insert(image.to_owned(), html! { <div class="image-grid-container"><img src={image.to_owned()} /></div> });
            rows[i / 5].push(image.to_owned());
        }

        ImageGrid {
            elements,
            rows,
            sizes,
            row_width: 424.0,
            row_height: 171.0
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let mut rows = Vec::new();
        self.display_row_loads();

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

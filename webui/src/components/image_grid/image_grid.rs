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
                    width += *w as f32 * (self.row_height / *h as f32);
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

pub enum ImageGridMessage {
    ImageLoaded(String, u32, u32),
}

impl Component for ImageGrid {
    type Message = ImageGridMessage;
    type Properties = ImageGridProps;

    fn create(ctx: &Context<Self>) -> Self {
        let mut elements = HashMap::new();
        let mut rows = vec![Vec::new(); ctx.props().images.len() / 5];
        let mut sizes = HashMap::new();

        for (i, image) in ctx.props().images.iter().enumerate() {
            let id = image.to_owned();
            elements.insert(image.to_owned(), html! {
                <div class="image-grid-container">
                    <img
                        src={image.to_owned()}
                        onload={ctx.link().callback(move |e: web_sys::Event| {
                            let img = e.target().unwrap().dyn_into::<web_sys::HtmlImageElement>().unwrap();
                            ImageGridMessage::ImageLoaded(id.clone(), img.natural_width(), img.natural_height())
                        })} />
                </div>
            });
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

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ImageGridMessage::ImageLoaded(id, width, height) => {
                self.sizes.insert(id, (width, height));
                true
            }
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

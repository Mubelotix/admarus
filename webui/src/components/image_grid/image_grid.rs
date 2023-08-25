use crate::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct ImageGridProps {
    pub images: Rc<Vec<String>>,
}

pub struct ImageGrid {
    elements: HashMap<String, Html>,
    loading: HashSet<String>,
    rows: Vec<Vec<String>>,
    sizes: HashMap<String, (u32, u32)>,
    row_width: f32,
    row_height: f32,
}

impl ImageGrid {
    fn potential_load(&self, i: usize, img_width: u32, img_height: u32) -> f32 {
        let mut width = 0.0;

        for cid in &self.rows[i] {
            if let Some((w, h)) = self.sizes.get(cid) {
                width += *w as f32 * (self.row_height / *h as f32);
            }
            width += 5.0;
        }
        width += img_width as f32 * (self.row_height / img_height as f32);

        width / self.row_width
    }

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
        let mut loading = HashSet::new();

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
            loading.insert(image.to_owned());
        }

        ImageGrid {
            elements,
            loading,
            rows: Vec::new(),
            sizes: HashMap::new(),
            row_width: 424.0,
            row_height: 171.0
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ImageGridMessage::ImageLoaded(id, width, height) => {
                if !self.loading.remove(&id) {
                    return false;
                }
                self.sizes.insert(id.clone(), (width, height));
                for i in 0..self.rows.len() {
                    let load = self.potential_load(i, width, height);
                    if load <= 1.1 {
                        self.rows[i].push(id);
                        return true;
                    }
                }
                self.rows.push(vec![id]);
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
        rows.push(html! {
            <div class="image-grid-loading">
                {self.loading.iter().filter_map(|cid| self.elements.get(cid)).cloned().collect::<Html>()}
            </div>
        });

        rows.into_iter().collect()
    }
}

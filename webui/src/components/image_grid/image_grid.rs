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
}

pub enum ImageGridMessage {
    StartLoading(String),
    ImageLoaded(String, u32, u32),
}

impl Component for ImageGrid {
    type Message = ImageGridMessage;
    type Properties = ImageGridProps;

    fn create(ctx: &Context<Self>) -> Self {
        let images = Rc::clone(&ctx.props().images);
        let link = ctx.link().clone();
        spawn_local(async move {
            for id in images.iter() {
                link.send_message(ImageGridMessage::StartLoading(id.to_owned()));
                sleep(Duration::from_millis(100)).await;
            }
        });

        ImageGrid {
            elements: HashMap::new(),
            loading: HashSet::new(),
            rows: Vec::new(),
            sizes: HashMap::new(),
            row_width: 424.0,
            row_height: 171.0
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ImageGridMessage::StartLoading(id) => {
                let id2 = id.clone();
                self.elements.insert(id.to_owned(), html! {
                    <div class="image-grid-container">
                        <img
                            src={id.to_owned()}
                            onload={ctx.link().callback(move |e: web_sys::Event| {
                                let img = e.target().unwrap().dyn_into::<web_sys::HtmlImageElement>().unwrap();
                                ImageGridMessage::ImageLoaded(id2.clone(), img.natural_width(), img.natural_height())
                            })} />
                    </div>
                });
                self.loading.insert(id);
                true
            }
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

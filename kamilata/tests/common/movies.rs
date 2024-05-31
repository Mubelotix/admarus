use super::*;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Movie {
    pub id: usize,
    pub title: String,
    pub overview: String,
    pub genres: Vec<String>,
    pub poster: String,
    pub release_date: i64,
}

impl Movie {
    fn full_text(&self) -> String {
        let mut full_text = String::new();
        full_text.push_str(&self.title);
        full_text.push(' ');
        full_text.push_str(&self.overview);
        full_text.push(' ');
        for genre in &self.genres {
            full_text.push_str(genre);
            full_text.push(' ');
        }
        full_text = full_text.to_lowercase();
        full_text
    }

    pub fn words(&self) -> Vec<String> {
        self.full_text().split(|c: char| c.is_whitespace() || c.is_ascii_punctuation()).filter(|w| w.len() >= 3).map(|w| w.to_string()).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MovieQuery {
    words: Vec<String>,
}

impl<const N: usize> SearchQuery<N> for MovieQuery {
    type ParsingError = serde_json::Error;

    fn match_score(&self, filter: &Filter<N>) -> u32 {
        let mut matches = 0;
        for word in &self.words {
            if filter.get_word::<MovieIndex<N>>(word) {
                matches += 1;
            }
        }
        matches
    }

    fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::ParsingError> {
        serde_json::from_slice(bytes)
    }
}

impl From<Vec<String>> for MovieQuery {
    fn from(words: Vec<String>) -> Self {
        MovieQuery { words }
    }
}

impl From<&[&str]> for MovieQuery {
    fn from(words: &[&str]) -> Self {
        MovieQuery { words: words.iter().map(|w| w.to_string()).collect() }
    }
}

#[derive(Default)]
pub struct MovieIndexInner<const N: usize> {
    movies: Vec<Movie>,
    filter: Filter<N>,
}

#[derive(Default)]
pub struct MovieIndex<const N: usize> {
    inner: Arc<RwLock<MovieIndexInner<N>>>,
}

#[async_trait]
impl<const N: usize> Store<N> for MovieIndex<N> {
    type Result = Movie;
    type Query = MovieQuery;

    fn hash_word(word: &str) -> Vec<usize> {
        let mut result = 1usize;
        const RANDOM_SEED: [usize; 16] = [542587211452, 5242354514, 245421154, 4534542154, 542866467, 545245414, 7867569786914, 88797854597, 24542187316, 645785447, 434963879, 4234274, 55418648642, 69454242114688, 74539841, 454214578213];
        for c in word.bytes() {
            for i in 0..8 {
                result = result.overflowing_mul(c as usize + RANDOM_SEED[i*2]).0;
                result = result.overflowing_add(c as usize + RANDOM_SEED[i*2+1]).0;
            }
        }
        vec![result % (N * 8)]
    }

    async fn get_filter(&self) -> Filter<N> {
        self.inner.read().await.filter.clone()
    }

    fn search(&self, query: Arc<Self::Query>) -> ResultStreamBuilderFut<Movie> {
        let inner2 = Arc::clone(&self.inner);
        Box::pin(async move {
            // We are in the future in charge of creating a stream

            let inner = inner2.read().await;
            let matching_movies = match query.match_score(&inner.filter) {
                0 => Vec::new(),
                _ => inner.movies.iter().filter(move |movie| {
                    let mut matches = 0;
                    for query_word in &query.words {
                        if movie.words().contains(query_word) {
                            matches += 1;
                        }
                    }
                    matches >= 1 // 1 is an arbitrary value
                }).cloned().collect::<Vec<_>>(),
            };

            let mut futures = Vec::new();
            for movie in matching_movies {
                futures.push(async move {
                    // We are in a future that will be polled when next() is called on the stream
                    // Here, we can do some heavy work for the current result
                    movie.clone()
                });
            }

            let mut stream = FuturesUnordered::new();
            stream.extend(futures);
            let stream: ResultStream<Movie> = Box::pin(stream);
            stream
        })
    }
}

impl<const N: usize> MovieIndex<N> {
    pub async fn insert_document(&self, doc: Movie) {
        let mut inner = self.inner.write().await;
        doc.words().iter().for_each(|w| inner.filter.add_word::<Self>(w));
        inner.movies.push(doc);
    }

    pub async fn insert_documents(&self, docs: &[Movie]) {
        let mut inner = self.inner.write().await;
        for doc in docs {
            doc.words().iter().for_each(|w| inner.filter.add_word::<Self>(w));
            inner.movies.push(doc.to_owned());
        }
    }
}

impl SearchResult for Movie {
    type Cid = usize;
    type ParsingError = serde_json::Error;

    fn cid(&self) -> Self::Cid {
        self.id
    }

    fn into_bytes(self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::ParsingError> {
        serde_json::from_slice(bytes)
    }
}

pub fn get_movies() -> Vec<Movie> {
    let data = match std::fs::read_to_string("movies.json") {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::process::Command::new("sh")
                .arg("-c")
                .arg("wget https://www.meilisearch.com/movies.json")
                .output()
                .expect("failed to download movies.json");
            std::fs::read_to_string("movies.json").unwrap()
        },
        e => e.unwrap(),
    };

    serde_json::from_str::<Vec<Movie>>(&data).unwrap()
}

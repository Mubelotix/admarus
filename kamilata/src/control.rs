use crate::prelude::*;

/// Primitive type representing a search priority, used to determine which results to load first.
#[derive(Debug, Clone)]
pub enum FixedSearchPriority {
    /// Get results fast without taking relevance into account
    Speed,
    /// Focus on finding the best results first
    Relevance
}

/// Advanced search priority that can change over time based on variable parameters
#[derive(Debug, Clone)]
pub enum SearchPriority {
    /// Simple search priority that cannot change automatically
    Fixed(FixedSearchPriority),
    /// Variable search priority that can change over time
    /// 
    /// Note: If you need more control, remember you can programmatically change the priority using [OngoingSearchController::set_priority].
    Variable {
        /// What priority to use first
        first: Box<SearchPriority>,
        /// Until how many documents have been found
        until_documents: usize,
        /// What priority to use after
        then: Box<SearchPriority>,
    }
}

impl SearchPriority {
    /// Builds a priority fixed to [FixedSearchPriority::Speed]
    pub fn speed() -> SearchPriority {
        SearchPriority::Fixed(FixedSearchPriority::Speed)
    }

    /// Builds a priority fixed to [FixedSearchPriority::Relevance]
    pub fn relevance() -> SearchPriority {
        SearchPriority::Fixed(FixedSearchPriority::Relevance)
    }

    /// Determines the current [FixedSearchPriority] based on parameters
    pub fn get_priority(&self, documents_found: usize) -> FixedSearchPriority {
        match self {
            SearchPriority::Fixed(p) => p.clone(),
            SearchPriority::Variable { first, until_documents, then } => {
                if documents_found < *until_documents {
                    first.get_priority(documents_found)
                } else {
                    then.get_priority(documents_found)
                }
            }
        }
    }
}

/// Configuration for a search
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Priority of the search
    pub priority: SearchPriority,
    /// Maximum number of concurrent requests to send to peers
    pub req_limit: usize,
    /// Number of milliseconds to wait for a response before considering the peer unresponsive
    pub timeout_ms: usize,
}

impl SearchConfig {
    pub fn new(priority: SearchPriority, req_limit: usize, timeout_ms: usize) -> SearchConfig {
        SearchConfig {
            priority,
            req_limit,
            timeout_ms,
        }
    }

    pub fn with_priority(self, priority: SearchPriority) -> SearchConfig {
        SearchConfig {
            priority,
            ..self
        }
    }

    pub fn with_req_limit(self, req_limit: usize) -> SearchConfig {
        // TODO: req_limit should be at least 1
        SearchConfig {
            req_limit,
            ..self
        }
    }

    pub fn with_timeout_ms(self, timeout_ms: usize) -> SearchConfig {
        SearchConfig {
            timeout_ms,
            ..self
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            priority: SearchPriority::Variable {
                first: Box::new(SearchPriority::speed()),
                until_documents: 25,
                then: Box::new(SearchPriority::relevance()),
            },
            req_limit: 10,
            timeout_ms: 50000,
        }
    }
}

pub(crate) struct OngoingSearchState<const N: usize, S: Store<N>> {
    query: Arc<S::Query>,
    config: SearchConfig,
    queried_peers: usize,
    final_peers: usize,
    ongoing_queries: usize,
}

impl<const N: usize, S: Store<N>> OngoingSearchState<N, S> {
    pub(crate) fn new(query: S::Query, config: SearchConfig) -> OngoingSearchState<N, S> {
        OngoingSearchState {
            query: Arc::new(query),
            config,
            queried_peers: 0,
            final_peers: 0,
            ongoing_queries: 0,
        }
    }

    pub(crate) fn into_pair(self) -> (OngoingSearchController<N, S>, OngoingSearchFollower<N, S>) {
        let (sender, receiver) = channel(100);
        let inner = Arc::new(RwLock::new(self));

        (
            OngoingSearchController {
                receiver,
                inner: inner.clone(),
            },
            OngoingSearchFollower {
                sender,
                inner,
            },
        )
    }
}

/// A search controller is an handle to an ongoing search, started with [KamilataBehaviour::search].
/// It allows getting results asynchronously, and to control the search.
pub struct OngoingSearchController<const N: usize, S: Store<N>> {
    receiver: Receiver<(S::Result, PeerId)>,
    inner: Arc<RwLock<OngoingSearchState<N, S>>>,
}

pub struct OngoingSearchFollower<const N: usize, S: Store<N>> {
    sender: Sender<(S::Result, PeerId)>,
    inner: Arc<RwLock<OngoingSearchState<N, S>>>,
}

impl<const N: usize, S: Store<N>> OngoingSearchController<N, S> {
    /// Waits for the next search result.
    pub async fn recv(&mut self) -> Option<(S::Result, PeerId)> {
        self.receiver.recv().await
    }

    /// Returns the next search result if available.
    pub fn try_recv(&mut self) -> Result<(S::Result, PeerId), tokio::sync::mpsc::error::TryRecvError> {
        self.receiver.try_recv()
    }

    /// Returns a copy of the ongoing queries.
    pub async fn query(&self) -> Arc<S::Query> {
        Arc::clone(&self.inner.read().await.query)
    }

    /// Returns the current search config.
    pub async fn config(&self) -> SearchConfig {
        self.inner.read().await.config.clone()
    }

    /// Sets the search config.
    pub async fn set_config(&self, config: SearchConfig) {
        self.inner.write().await.config = config;
    }

    /// Returns the current search priority.
    pub async fn priority(&self) -> SearchPriority {
        self.inner.read().await.config.priority.clone()
    }

    /// Sets the search priority.
    pub async fn set_priority(&self, priority: SearchPriority) {
        self.inner.write().await.config.priority = priority;
    }

    /// Returns the concurrent request limit.
    pub async fn req_limit(&self) -> usize {
        self.inner.read().await.config.req_limit
    }

    /// Returns the timeout in milliseconds.
    pub async fn timeout_ms(&self) -> usize {
        self.inner.read().await.config.timeout_ms
    }

    /// Returns the number of peers that have been queried.
    pub async fn queried_peers(&self) -> usize {
        self.inner.read().await.queried_peers
    }

    /// Returns the number of peers that have been queried and returned local results.
    pub async fn final_peers(&self) -> usize {
        self.inner.read().await.final_peers
    }

    /// Returns the number of ongoing queries.
    pub async fn ongoing_queries(&self) -> usize {
        self.inner.read().await.ongoing_queries
    }

    /// Stops the search and returns all search results that have not been consumed yet.
    pub async fn finish(mut self) -> SearchResults<S::Result> {
        let mut search_results = Vec::new();
        while let Ok(search_result) = self.try_recv() {
            search_results.push(search_result);
        }

        let inner = self.inner.read().await;

        SearchResults {
            hits: search_results,
            queried_peers: inner.queried_peers,
            final_peers: inner.final_peers,
        }
    }
}

impl<const N: usize, S: Store<N>> OngoingSearchFollower<N, S> {
    /// Sends a search result to the controler.
    pub async fn send(&self, search_result: (S::Result, PeerId)) -> Result<(), tokio::sync::mpsc::error::SendError<(S::Result, PeerId)>> {
        self.sender.send(search_result).await
    }

    /// Detects if search is closed
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    /// Returns a copy of the ongoing queries.
    pub async fn query(&self) -> Arc<S::Query> {
        Arc::clone(&self.inner.read().await.query)
    }

    /// Returns the current search config.
    pub async fn config(&self) -> SearchConfig {
        self.inner.read().await.config.clone()
    }

    /// Sets query/peer counts.
    pub async fn set_query_counts(&self, queried_peers: usize, final_peers: usize, ongoing_queries: usize) {
        let mut inner = self.inner.write().await;
        inner.queried_peers = queried_peers;
        inner.final_peers = final_peers;
        inner.ongoing_queries = ongoing_queries;
    }
}

impl<const N: usize, S: Store<N>> Clone for OngoingSearchFollower<N, S> {
    fn clone(&self) -> Self {
        OngoingSearchFollower {
            sender: self.sender.clone(),
            inner: Arc::clone(&self.inner),
        }
    }
}

/// A struct containing search results and some useful information about how the search went.
#[derive(Debug)]
pub struct SearchResults<T: SearchResult> {
    /// Contains search results in the order they were received.
    /// Results that have already been [received](OngoingSearchControler::recv) are not included.
    pub hits: Vec<(T, PeerId)>,
    /// Numbers of peers that have been queried
    pub queried_peers: usize,
    /// Numbers of peers that have been able to provide us with hits
    pub final_peers: usize,
}

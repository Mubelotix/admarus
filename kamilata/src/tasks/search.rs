//! This module contains the algorithm that is used for discovering results on the network.

use super::*;
use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;

// Constants for each [FixedSearchPriority]. They are used as const generics.
const ANY: usize = 0;
const SPEED: usize = 1;
const RELEVANCE: usize = 2;

/// Information about a provider, generic over the priority of the search.
/// The priority will determine the implementation of [Ord] on this struct.
#[derive(Debug, PartialEq, Eq)]
struct ProviderInfo<const PRIORITY: usize> {
    peer_id: PeerId,
    match_scores: Vec<u32>,
    addresses: Vec<Multiaddr>,
}

/// A trait allowing APIs over any [ProviderInfo], regardless of the priority.
trait AnyProviderInfo: Sized {
    fn into_parts(self) -> (PeerId, Vec<u32>, Vec<Multiaddr>);
    fn into_whatever(self) -> ProviderInfo<ANY> {
        let (peer_id, queries, addresses) = self.into_parts();
        ProviderInfo { peer_id, match_scores: queries, addresses }
    }
    fn into_speed(self) -> ProviderInfo<SPEED> {
        let (peer_id, queries, addresses) = self.into_parts();
        ProviderInfo { peer_id, match_scores: queries, addresses }
    }
    fn into_relevance(self) -> ProviderInfo<RELEVANCE> {
        let (peer_id, queries, addresses) = self.into_parts();
        ProviderInfo { peer_id, match_scores: queries, addresses }
    }
}

impl AnyProviderInfo for ProviderInfo<ANY> {
    fn into_parts(self) -> (PeerId, Vec<u32>, Vec<Multiaddr>) {
        (self.peer_id, self.match_scores, self.addresses)
    }
}
impl AnyProviderInfo for ProviderInfo<SPEED> {
    fn into_parts(self) -> (PeerId, Vec<u32>, Vec<Multiaddr>) {
        (self.peer_id, self.match_scores, self.addresses)
    }
}
impl AnyProviderInfo for ProviderInfo<RELEVANCE> {
    fn into_parts(self) -> (PeerId, Vec<u32>, Vec<Multiaddr>) {
        (self.peer_id, self.match_scores, self.addresses)
    }
}
impl AnyProviderInfo for (PeerId, Vec<u32>, Vec<Multiaddr>) {
    fn into_parts(self) -> (PeerId, Vec<u32>, Vec<Multiaddr>) {
        self
    }
}

/// A [BinaryHeap] that can change its way of ordering its elements.
enum ProviderBinaryHeap {
    Speed(BinaryHeap<ProviderInfo<SPEED>>),
    Relevance(BinaryHeap<ProviderInfo<RELEVANCE>>),
}

impl ProviderBinaryHeap {
    fn push(&mut self, provider: impl AnyProviderInfo) {
        match self {
            ProviderBinaryHeap::Speed(heap) => heap.push(provider.into_speed()),
            ProviderBinaryHeap::Relevance(heap) => heap.push(provider.into_relevance()),
        }
    }

    fn pop(&mut self) -> Option<ProviderInfo<ANY>> {
        match self {
            ProviderBinaryHeap::Speed(heap) => heap.pop().map(|provider| provider.into_whatever()),
            ProviderBinaryHeap::Relevance(heap) => heap.pop().map(|provider| provider.into_whatever()),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            ProviderBinaryHeap::Speed(heap) => heap.is_empty(),
            ProviderBinaryHeap::Relevance(heap) => heap.is_empty(),
        }
    }

    fn update_priority(&mut self, priority: SearchPriority, documents_found: usize) {
        match priority.get_priority(documents_found) {
            FixedSearchPriority::Speed => match self {
                ProviderBinaryHeap::Speed(_) => (),
                ProviderBinaryHeap::Relevance(heap) => {
                    let mut new_heap = BinaryHeap::new();
                    for provider in heap.drain() {
                        new_heap.push(provider.into_speed());
                    }
                    *self = ProviderBinaryHeap::Speed(new_heap);
                }
            },
            FixedSearchPriority::Relevance => match self {
                ProviderBinaryHeap::Speed(heap) => {
                    let mut new_heap = BinaryHeap::new();
                    for provider in heap.drain() {
                        new_heap.push(provider.into_relevance());
                    }
                    *self = ProviderBinaryHeap::Relevance(new_heap);
                }
                ProviderBinaryHeap::Relevance(_) => (),
            },
        }
    }
}

impl<const PRIORITY: usize> ProviderInfo<PRIORITY> {
    fn nearest(&self) -> Option<(usize, u32)> {
        self.match_scores.iter().enumerate().find(|(_, score)| **score > 0).map(|(dist, score)| (dist, *score))
    }

    fn best(&self) -> Option<(usize, u32)> {
        self.match_scores.iter().enumerate().max_by_key(|(_, score)| **score).map(|(dist, score)| (dist, *score))
    }
}

impl std::cmp::Ord for ProviderInfo<SPEED> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.nearest(), other.nearest()) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some((dist1, score1)), Some((dist2, score2))) => match dist1.cmp(&dist2) {
                Ordering::Equal => score2.cmp(&score1).reverse(),
                Ordering::Less => Ordering::Greater,
                Ordering::Greater => Ordering::Less,
            },
        }
    }
}

impl std::cmp::PartialOrd for ProviderInfo<SPEED> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl std::cmp::Ord for ProviderInfo<RELEVANCE> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.best(), other.best()) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some((dist1, score1)), Some((dist2, score2))) => match score2.cmp(&score1) {
                Ordering::Equal => dist1.cmp(&dist2).reverse(),
                Ordering::Less => Ordering::Greater,
                Ordering::Greater => Ordering::Less,
            },
        }
    }
}

impl std::cmp::PartialOrd for ProviderInfo<RELEVANCE> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

async fn search_one<const N: usize, S: Store<N>>(
    query: Arc<S::Query>,
    behaviour_controller: BehaviourController<N, S>,
    search_follower: OngoingSearchFollower<N, S>,
    addresses: Vec<Multiaddr>,
    our_peer_id: PeerId,
    remote_peer_id: PeerId,
) -> Option<(PeerId, Vec<ProviderInfo<ANY>>)> {
    debug!("{our_peer_id} Querying {remote_peer_id} for results");

    // Dial the peer, orders the handle to request it, and wait for the response
    let (over_notifier, over_receiver) = oneshot_channel();
    let (routes_sender, mut routes_receiver) = channel(100);
    behaviour_controller.dial_peer_and_message(remote_peer_id, addresses, BehaviorToHandlerEvent::SearchRequest { query, routes_sender, result_sender: search_follower, over_notifier }).await;
    
    let Some(routes) = routes_receiver.recv().await else {return None};
    let routes = routes.into_iter().map(|distant_match|
        ProviderInfo {
            peer_id: distant_match.peer_id.into(),
            match_scores: distant_match.match_scores,
            addresses: distant_match.addresses.into_iter().filter_map(|a| a.parse().ok()).collect(),
        }
    ).collect::<Vec<_>>();

    let _ = over_receiver.await;

    Some((remote_peer_id, routes))
}
 
pub(crate) async fn search<const N: usize, S: Store<N>>(
    search_follower: OngoingSearchFollower<N, S>,
    behaviour_controller: BehaviourController<N, S>,
    db: Arc<Db<N, S>>,
    our_peer_id: PeerId,
) -> TaskOutput {
    info!("{our_peer_id} Starting search task");
    let query = search_follower.query().await;

    // Query ourselves
    let db2 = Arc::clone(&db);
    let query2 = Arc::clone(&query);
    let search_follower2 = search_follower.clone();
    spawn(async move {
        let fut = db2.store().search(query2);
        let mut stream = fut.await;
        while let Some(result) = stream.next().await {
            let _ = search_follower2.send((result, our_peer_id)).await;
        }
    });
    let routes = db.search_routes(&query).await;
    let mut config;
    let mut providers = ProviderBinaryHeap::Speed(BinaryHeap::new());
    let mut already_queried = HashSet::new();
    for (peer_id, queries) in routes {
        providers.push((peer_id, queries, Vec::new()));
    }

    // Keep querying new peers for new results
    let mut ongoing_requests = Vec::new();
    loop {
        search_follower.set_query_counts(already_queried.len(), 0, ongoing_requests.len()).await; // TODO: value instead of 0
        config = search_follower.config().await;
        providers.update_priority(config.priority, 0); // TODO: value instead of 0

        // TODO: update query if needed

        // Start new requests until limit is reached
        while ongoing_requests.len() < config.req_limit {
            let Some(provider) = providers.pop() else {break};
            already_queried.insert(provider.peer_id);
            let search = search_one::<N,S>(Arc::clone(&query), behaviour_controller.clone(), search_follower.clone(), provider.addresses, our_peer_id, provider.peer_id);
            ongoing_requests.push(Box::pin(timeout(Duration::from_millis(config.timeout_ms as u64), search)));
        }

        // Ends the loop when no more requests can be made
        if providers.is_empty() && ongoing_requests.is_empty() {
            break;
        }

        // Wait for one of the ongoing requests to finish
        let (r, _, remaining_requests) = futures::future::select_all(ongoing_requests).await;
        ongoing_requests = remaining_requests;
        let (_peer_id, routes) = match r {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(_) => {
                warn!("{our_peer_id} Search request timed out");
                continue
            },
        };
        if search_follower.is_closed() {
            warn!("{our_peer_id} Search interrupted due to results being dropped");
            return TaskOutput::None;
        }
        for route in routes {
            if !already_queried.contains(&route.peer_id) && !route.addresses.is_empty() {
                providers.push(route);
            }
        }
    }

    info!("{our_peer_id} Search task finished");
    
    TaskOutput::None
}

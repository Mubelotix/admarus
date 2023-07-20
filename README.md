<h1 align="center">Admarus</h1>

<p align="center">
    <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License: MIT"/></a>
    <img alt="Lines of code badge" src="https://img.shields.io/badge/total%20lines-8157-blue">
    <a href="https://census.admarus.net/"><img alt="Documents in corpus badge" src="https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fcensus.admarus.net%2Fapi%2Fv0%2Fstats&query=%24.stats_1h.documents&suffix=%20documents&label=corpus&color=purple"></a>
    <a href="https://census.admarus.net/"><img alt="Peers in network badge" src="https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fcensus.admarus.net%2Fapi%2Fv0%2Fstats&query=%24.stats_1h.peers&suffix=%20peers&label=network&color=purple"></a>
    <img alt="GitHub last commit" src="https://img.shields.io/github/last-commit/Mubelotix/admarus-daemon?color=%23347d39" alt="last commit badge"/>
    <a href="https://github.com/Mubelotix/admarus/issues?q=is%3Aissue+is%3Aclosed"><img alt="GitHub closed issues" src="https://img.shields.io/github/issues-closed-raw/Mubelotix/admarus-daemon?color=%23347d39" alt="closed issues badge"/></a>
</p>

<p align="center">Peer-to-Peer Search Engine for IPFS</p>

Admarus unlocks the full potential of IPFS by making it searchable. It is an open, decentralized network of peers indexing their IPFS documents. Admarus relies on no central authority, and is censorship-resistant by design.

<p align="center">
    <a href="https://www.youtube.com/watch?v=AKGpNKwBrOY"><img src="https://admarus.net/demo.gif#2" alt="Demo GIF of searching on Admarus."/></a>
</p>

🔥 [**Try the gateway-based demo!**](https://admarus.net/) 🔥

<!-- todo: talk about how it incentivize IPFS -->

## Features

- **No storage use**: Admarus indexes data that's already stored on your Kubo node
- **Search operators**: The `AND`, `OR`, and `NOT` are supported
- **Trustless**: Results are verified by the client and malicious peers are banned when detected
- **Language detection**: Language is detected even when document metadata isn't accurate
- [**Scalable**](#scalability): The network speed can only increase as more peers join
- **Censorship-resistant**: Censors would need full control of more than 95% of the network to affect information reachability
- **Open**: Anyone can join the network, nodes don't discriminate on obscure criteria (hi emails)
- **Decentralized**: There is no central authority at all, and multiple peer discovery mechanisms are available
- **Blockchain-free**: No blockchain is used, and no token is required to use the network. Just peer-to-peer magic

## Scalability

There is no use in having a search engine if it breaks under load.
While previous attempts at building a peer-to-peer, powerful search engine have all failed, **Admarus was designed with scalability in mind**.
Actually, Admarus gets faster and more reliable as the network grows to thousands of peers.
This is all thanks to the [Kamilata protocol](https://github.com/mubelotix/kamilata) and its routing algorithm for queries.
It allows Admarus to download results at constant speed, regardless of the size of the network.
Of course, the client cannot afford to download and rank millions of results, but we actually don't need a [recall](https://en.wikipedia.org/wiki/Precision_and_recall) of 100%.
That's because some documents are more popular than others, and these are the ones we want to find.
(By default, 50% of the score of a document is based on its popularity.)
The more popular a document is, the faster we will find it, hence we don't need 100% recall to find the top `n` results.
I have only been able to run simulations with up to `30000` peers, so the behavior of the network with millions of peers is still unknown.
However, we can confidently say that Admarus can scale to at least a `200k` peers and billions of documents.
Even if problems were to arise, these would be limited to queries made of a single common word, as for each additional term, you divide the pool of peers to query by a huge factor.

The current slowness of the network is due to the fact that there are not enough peers on it.
They are doing their best at generating results for queries. Each result is generated from the document.
The thing is, each document must be read from the Kubo store on the disk, which only yields 11 documents per second on my machines.
As we parallelize the process by querying multiple peers concurrently, search will get significantly faster.

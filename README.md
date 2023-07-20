<h1 align="center">Admarus</h1>

<p align="center">
    <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License: MIT"/></a>
    <img alt="Lines of code badge" src="https://img.shields.io/badge/total%20lines-8157-blue">
    <a href="https://census.admarus.net/"><img alt="Documents in corpus badge" src="https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fcensus.admarus.net%2Fapi%2Fv0%2Fstats&query=%24.stats_24h.documents&suffix=%20documents&label=corpus&color=purple"></a>
    <a href="https://census.admarus.net/"><img alt="Peers in network badge" src="https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fcensus.admarus.net%2Fapi%2Fv0%2Fstats&query=%24.stats_24h.peers&suffix=%20peers&label=network&color=purple"></a>
    <img alt="GitHub last commit" src="https://img.shields.io/github/last-commit/Mubelotix/admarus-daemon?color=%23347d39" alt="last commit badge"/>
    <a href="https://github.com/Mubelotix/admarus/issues?q=is%3Aissue+is%3Aclosed"><img alt="GitHub closed issues" src="https://img.shields.io/github/issues-closed-raw/Mubelotix/admarus-daemon?color=%23347d39" alt="closed issues badge"/></a>
</p>

<p align="center">Peer-to-Peer Search Engine for IPFS</p>

Admarus unlocks the full potential of IPFS by making it searchable. It is an open, decentralized network of peers indexing their IPFS documents. Admarus relies on no central authority, and is censorship-resistant by design.

<p align="center">
    <a href="https://www.youtube.com/watch?v=AKGpNKwBrOY"><img src="https://admarus.net/demo.gif#2" alt="Demo GIF of searching on Admarus."/></a>
</p>

🔥 [**Try the gateway-based demo!**](https://admarus.net/) 🔥

## Abstract

Admarus is a peer-to-peer search engine for IPFS. It is based on the [Kamilata](https://github.com/mubelotix/kamilata) protocol.

This repository contains a lightweight daemon for Admarus. The daemon works in tandem with the [Kubo](https://github.com/ipfs/kubo) IPFS daemon. Files you pin with Kubo will be indexed by Admarus, and made available to the network. No additional storage is required by the Admarus daemon. 

This daemon provides an API that can be used by other applications as a gateway to the Admarus network. An official Admarus web interface is in development.

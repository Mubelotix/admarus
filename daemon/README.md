<h1 align="center">Admarus</h1>

<p align="center">
    <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License: MIT"></a>
    <img alt="Lines of code" src="https://img.shields.io/tokei/lines/github/Mubelotix/admarus-daemon">
    <img alt="GitHub last commit" src="https://img.shields.io/github/last-commit/Mubelotix/admarus-daemon?color=%23347d39" alt="last commit badge">
    <img alt="GitHub closed issues" src="https://img.shields.io/github/issues-closed-raw/Mubelotix/admarus-daemon?color=%23347d39" alt="closed issues badge">
</p>

<p align="center">A Peer-to-Peer Search Engine for IPFS</p>

## Abstract

Admarus is a peer-to-peer search engine for IPFS. It is based on the [Kamilata](https://github.com/mubelotix/kamilata) protocol.

This repository contains a lightweight daemon for Admarus. The daemon works in tandem with the [Kubo](https://github.com/ipfs/kubo) IPFS daemon. Files you pin with Kubo will be indexed by Admarus, and made available to the network. No additional storage is required by the Admarus daemon. 

This daemon provides an API that can be used by other applications as a gateway to the Admarus network. An official Admarus web interface is in development.

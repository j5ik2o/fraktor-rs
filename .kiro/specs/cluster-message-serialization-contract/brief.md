# Brief: cluster-message-serialization-contract

## Problem

cluster message serializer contract は active hard follow-up として残っており、gossip envelope、pubsub protocol、actor-core serialization、std/wire の境界をまたぐ。これを最後に整理しないと、各 protocol が個別 codec を持ちすぎる。

## Current State

actor-core には serialization boundary があり、cluster std adaptor には membership wire delta がある。cluster gap analysis では cluster message serializer contract が std/wire + actor-core serialization の follow-up として残っている。

## Desired Outcome

cluster message の serializer contract が、actor-core serialization と cluster std/wire の間で明確になる。gossip と pubsub の protocol message が同じ contract で扱えるようになり、完全な protobuf binary compatibility は非目標として切り分けられる。

## Approach

gossip / pubsub protocol の message set が定まった後に、cluster message serializer registry、type id / manifest、versioning、unknown payload error、std wire bridge を定義する。actor-core serialization を再設計せず、cluster が必要とする接続点を明確にする。

## Scope

- **In**: cluster message serializer contract、std/wire bridge、actor-core serialization 接続点、versioning / unknown payload handling。
- **Out**: protobuf serializer の完全バイナリ互換、Pekko wire protocol 完全互換、actor-core serialization 全体の再設計。

## Boundary Candidates

- actor-core serialization: generic serialization port
- cluster core protocol messages: gossip / pubsub message contracts
- cluster std/wire: codec implementation and transport framing

## Out of Boundary

- Gossip merge semantics
- PubSub mediator semantics
- Remote transport lifecycle

## Upstream / Downstream

- **Upstream**: `cluster-gossip-heartbeat-protocol`, `cluster-pubsub-mediator-protocol`
- **Downstream**: future interoperability and wire compatibility follow-ups

## Existing Spec Touchpoints

- **Extends**: actor-core serialization boundary, cluster std wire implementation
- **Adjacent**: `openspec/specs/cluster-adaptor-std-remote-delivery`

## Constraints

完全な Pekko / protobuf binary compatibility は scope 外。Rust runtime contract と versioned serializer 接続点を優先する。

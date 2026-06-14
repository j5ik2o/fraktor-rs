# Brief: cluster-membership-reachability-model

## Problem

active medium 項目の membership 系 follow-up は、node identity、data center、member status、reachability、indirect connection handling が混在している。これらを先に整理しないと、gossip、heartbeat、downing、pubsub の前提が安定しない。

## Current State

`cluster-core-kernel/src/membership` には membership table、node record/status、vector clock、gossip state、coordinator がある。gap analysis では `UniqueAddress` semantics、data center membership、`WeaklyUp` compatibility、`Reachability` matrix、indirect connection handling が未実装または未整理として残っている。

## Desired Outcome

membership core が、Pekko comparison に耐える node identity と reachability model を持つ。後続の gossip/heartbeat、downing、pubsub が同じ membership snapshot と reachability evidence を参照できる。

## Approach

UniqueAddress 相当の identity semantics、data center 属性、WeaklyUp 互換 status、Reachability matrix、indirect connection evidence を `membership` の core contract として定義する。std transport や heartbeat 実装はこの spec では所有せず、入力として扱える型と state transition を優先する。

## Scope

- **In**: `UniqueAddress` semantics、data center membership、`WeaklyUp` compatibility、`Reachability` matrix、indirect connection handling。
- **Out**: full Gossip merge/tombstone、CrossDc heartbeat、SplitBrainResolver、DistributedPubSub protocol。

## Boundary Candidates

- core/membership: node identity、status、reachability evidence、snapshot
- core/downing_provider: reachability input の消費側 boundary
- core/pub_sub: reachability-aware routing の将来利用点

## Out of Boundary

- TCP heartbeat scheduling
- lease-based majority
- topic registry gossip

## Upstream / Downstream

- **Upstream**: `cluster-active-compatibility-baseline`
- **Downstream**: `cluster-gossip-heartbeat-protocol`, `cluster-downing-sbr-decision-model`, `cluster-pubsub-mediator-protocol`

## Existing Spec Touchpoints

- **Extends**: existing membership implementation under `cluster-core-kernel`
- **Adjacent**: `openspec/specs/cluster-grain-runtime-operational-contract`, `openspec/specs/cluster-provider-boundary`

## Constraints

core は `no_std` を維持する。data center や WeaklyUp は Pekko 語彙を参照しても、fraktor-rs の Grain runtime roadmap を Pekko Cluster public API parity に変更しない。

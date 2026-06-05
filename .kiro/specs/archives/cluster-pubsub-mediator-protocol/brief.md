# Brief: cluster-pubsub-mediator-protocol

## Problem

Distributed PubSub の active follow-up は mediator protocol、settings、path semantics、topic registry gossip / delta collection にまたがる。現在の pub_sub 実装と membership gossip の境界を整理しないと、delivery policy と topology dissemination が混ざる。

## Current State

`cluster-core-kernel/src/pub_sub` には cluster pub-sub API、delivery endpoint、batching producer、delivery policy がある。`cluster-adaptor-std/src/pub_sub` には std delivery actor がある。gap analysis では `DistributedPubSubMediator` protocol、`DistributedPubSubSettings`、`Send` / `SendToAll` path semantics、topic registry gossip / delta collection が active follow-up として残っている。

## Desired Outcome

DistributedPubSubMediator 相当の protocol と settings が core/pub_sub に定義され、path semantics と topic registry dissemination が membership/gossip と接続できる。std adaptor は delivery execution を担当し、protocol semantics を所有しない。

## Approach

core/pub_sub に mediator command/event、settings、path addressing、topic registry delta contract を置く。topic registry gossip は `cluster-gossip-heartbeat-protocol` の gossip substrate に依存させ、pubsub 固有の delta collection と delivery semantics をこの spec に閉じる。

## Scope

- **In**: `DistributedPubSubMediator` protocol、`DistributedPubSubSettings`、`Send` / `SendToAll` path semantics、topic registry gossip / delta collection。
- **Out**: Gossip substrate 本体、cluster message serializer framework、Distributed Data / CRDT。

## Boundary Candidates

- core/pub_sub: mediator protocol、settings、topic registry delta
- core/membership: member/topology input
- std/pub_sub: delivery actor / transport bridge

## Out of Boundary

- CRDT-based Distributed Data
- Cluster Sharding delivery controller
- Full Pekko pubsub public API parity

## Upstream / Downstream

- **Upstream**: `cluster-membership-reachability-model`, `cluster-gossip-heartbeat-protocol`
- **Downstream**: `cluster-message-serialization-contract`, future pubsub showcases

## Existing Spec Touchpoints

- **Extends**: existing `cluster-core-kernel/src/pub_sub` implementation
- **Adjacent**: `openspec/specs/cluster-core-module-organization`

## Constraints

pubsub は membership/gossip を利用するが、membership merge semantics を所有しない。Distributed Data / CRDT は deferred scope のため含めない。

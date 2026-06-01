# Brief: cluster-gossip-heartbeat-protocol

## Problem

gossip と heartbeat の active follow-up は medium と hard にまたがり、wire envelope、membership merge、seen digest、tombstone、cross-DC heartbeat が分散している。membership model が定まった後に、protocol contract と std transport integration を分けて定義する必要がある。

## Current State

`cluster-core-kernel/src/membership` には gossip state / coordinator / transport port があり、`cluster-adaptor-std/src/membership` には Tokio gossip transport と wire delta が存在する。gap analysis では `GossipEnvelope`、dedicated cluster heartbeat protocol、full `Gossip` merge / tombstone / seen digest、`CrossDcClusterHeartbeat` が active follow-up として残っている。

## Desired Outcome

membership gossip が envelope、merge、tombstone、seen digest、heartbeat evidence を明確な contract として持つ。std adaptor は wire / transport を実装し、core の gossip semantics を所有しない。

## Approach

core/membership に gossip protocol state と heartbeat evidence の contract を置き、std/wire に envelope encode/decode と transport lifecycle を置く。medium 項目の `GossipEnvelope` と dedicated heartbeat を先に通し、hard 項目の full merge/tombstone/seen digest と CrossDc heartbeat を同じ spec の段階的 task として扱う。

## Scope

- **In**: `GossipEnvelope`、dedicated cluster heartbeat protocol、full `Gossip` merge / tombstone / seen digest、`CrossDcClusterHeartbeat`。
- **Out**: DowningStrategy decision、lease majority、pubsub topic registry gossip、serializer framework 全体。

## Boundary Candidates

- core/membership: gossip state、merge rule、seen digest、heartbeat evidence
- std/wire: envelope serialization と transport framing
- std/membership: Tokio transport driver

## Out of Boundary

- SBR decision
- generic discovery backend
- actor message serializer registry

## Upstream / Downstream

- **Upstream**: `cluster-membership-reachability-model`
- **Downstream**: `cluster-pubsub-mediator-protocol`, `cluster-message-serialization-contract`

## Existing Spec Touchpoints

- **Extends**: existing membership gossip implementation
- **Adjacent**: `openspec/specs/cluster-provider-boundary`, `openspec/specs/cluster-adaptor-std-remote-delivery`

## Constraints

core は `no_std` を維持する。std adaptor は Tokio transport と wire codec を持てるが、membership merge semantics は core 側に残す。

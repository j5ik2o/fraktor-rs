# Brief: actor-blocking-bounded-mailbox-compat

## Problem

Pekko の bounded mailbox family は `pushTimeOut` を持つが、fraktor-rs の bounded queue は reject / evict / dead letter observation に寄せている。async-first actor adapters 方針では低優先度だが、Pekko parity としては明確な medium gap であり、互換 option として設計境界を決める必要がある。

## Current State

bounded / priority / stable-priority / deque / control-aware queue family は存在する。満杯時は overflow strategy に従い、損失は dead letters で観測される。enqueue 側が空きを待つ contract、timeout まで待って失敗する contract、std adaptor での blocking / async wait integration はない。

## Desired Outcome

`pushTimeOut` 付き bounded mailbox の互換 contract が core と std adaptor の境界で定義される。core は policy / result / capability を所有し、std adaptor は必要な wait mechanism を提供する。default は既存 async-first overflow behavior を維持する。

## Approach

blocking wait を core に直接持ち込まず、core には timeout-capable enqueue contract と observability を置く。std adaptor で timeout wait を実現する場合の executor / blocker / scheduler の責務を明確にし、Embassy には強制しない。

## Scope

- **In**: pushTimeOut semantics の contract、timeout-capable bounded mailbox policy、std adaptor compatibility option、dead letter / rejection observability、tests
- **Out**: 既存 bounded mailbox default behavior の変更、Embassy adaptor への blocking wait 強制、Pekko HOCON config の完全移植

## Boundary Candidates

- core mailbox policy と std wait implementation
- overflow strategy と timeout wait strategy
- control-aware bounded semantics と normal bounded semantics

## Out of Boundary

- queue type / lookupByQueueType resolution
- BalancingDispatcher compatibility contract
- mailbox run loop の全面 rewrite

## Upstream / Downstream

- **Upstream**: actor-mailbox-resolution-contract
- **Downstream**: std adaptor mailbox compatibility showcase、将来の config-driven mailbox compatibility option

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: actor-mailbox-resolution-contract、actor-adaptor-std dispatch implementation

## Constraints

async-first 方針を壊さず、互換 option と default behavior を分ける。`actor-core-kernel` は std blocking primitive に依存しない。timeout behavior は deterministic test 可能な clock / scheduler abstraction に接続する。

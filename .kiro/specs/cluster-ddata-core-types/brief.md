# Brief: cluster-ddata-core-types

## Problem

Distributed Data / CRDT（カテゴリ9）は fraktor-rs で全面未実装（0/27 概念）であり、Phase 3 の Replicator runtime を実装する前提となる「データ型の語彙」が存在しない。CRDT のデータ型と merge 法則は Replicator がなくても純粋に定義・検証できるため、ここを先に契約化することで Phase 3 の runtime spec をデータ型設計から切り離せる。gap analysis カテゴリ9 の easy / trivial 項目（9 概念）と、その前提となる medium 項目（`ReplicatedData` 基底 SPI）を 1 spec に束ねる。

## Current State

- 対応モジュールなし。`cluster-core-kernel` に `ddata` モジュールを新設する。
- membership 用の `VectorClock` は存在するが、これは membership version 管理用であり、CRDT の pruning / dot 管理用 `VersionVector`（Phase 2 / medium）とは別物。本 spec では混同を避けるため CRDT 用 version vector は扱わない。

## Desired Outcome

- CRDT の基底 SPI（Pekko `ReplicatedData` / `DeltaReplicatedData` / `ReplicatedDelta` / `RemovedNodePruning` 相当）が trait として定義され、merge の結合則・可換則・冪等性が property test で検証される。
- 基本 CRDT 型: `Flag`（enable-only）、`GCounter`（grow-only counter）、`PNCounter`（increment/decrement counter）、`PNCounterMap` が実装される。
- key 階層（Pekko `Key[T]` 相当）と `SelfUniqueAddress`（CRDT 更新時の自ノード識別 newtype）が定義される。
- read/write consistency level（`ReadLocal` 〜 `ReadAll` / `WriteLocal` 〜 `WriteAll`、Majority / MajorityPlus 含む）と補助 protocol 型（`GetReplicaCount` / `ReplicaCount` / `FlushChanges` 相当）が Replicator protocol の語彙として定義される。

## Approach

Pekko の CRDT 設計（merge ベース + delta 対応）を Rust の所有権モデルに合わせて再設計する。merge は `self` を消費して新しい値を返す（または `&mut self`）形を design 段階で決定し、CQS 原則と immutability-policy に整合させる。counter の node 単位集計は `SelfUniqueAddress` を明示引数で受け取り、暗黙のグローバル状態を持たない。consistency level / 補助型は Replicator 不在でも意味が定義できる pure な語彙型として先行定義する。

## Scope

- **In**:
  - `ddata` モジュール新設（`cluster-core-kernel/src/ddata/`）
  - `ReplicatedData` 系基底 SPI（merge / delta / pruning の trait 契約）
  - `Key` 階層、`SelfUniqueAddress`
  - `Flag` / `GCounter` / `PNCounter` / `PNCounterMap`
  - read/write consistency levels、`GetReplicaCount` / `ReplicaCount` / `FlushChanges` 語彙型
  - merge 法則の property test（結合・可換・冪等）
- **Out**:
  - `Replicator` runtime / gossip 接続 / `ReplicatorSettings`（Phase 3 / hard）
  - `ORSet` / `ORMap` / `ORMultiMap` / `LWWRegister` / `LWWMap` / `VersionVector`（Phase 2 / medium — dot / tombstone / clock semantics が重く別 spec とする）
  - `DurableStore` SPI / std adapter（Phase 2 / medium）
  - typed `DistributedData` extension / `ReplicatorMessageAdapter`（Phase 2 / medium）

## Boundary Candidates

- 基底 SPI（trait）と具象 CRDT 型の分離
- データ型（pure、本 spec）と replication runtime（Phase 3）の分離
- counter 系（本 spec）と observed-remove 系 / LWW 系（別 spec）の分離

## Out of Boundary

- CRDT の wire serialization（cluster-message-serialization-contract のパターンに後続 spec で接続）
- pub_sub registry gossip の ddata 化（構造ギャップ分析の将来観点）

## Upstream / Downstream

- **Upstream**: なし（独立した新規モジュール）。membership `VectorClock` とは意図的に独立
- **Downstream**: Phase 2 の OR/LWW 系 CRDT・`VersionVector`・`DurableStore`、Phase 3 の `Replicator` runtime、typed ddata extension

## Existing Spec Touchpoints

- **Extends**: なし（新規境界）
- **Adjacent**: cluster-membership-reachability-model（`VectorClock` と命名・責務を混同しないこと）、cluster-message-serialization-contract（将来の payload kind 追加先）

## Constraints

- `cluster-core-kernel` の `no_std` + alloc で完結させる。
- merge の API 形状（消費 vs `&mut self`）は CQS 原則（.agents/rules/rust/cqs-principle.md)と immutability-policy に従い design で確定し、内部可変性は使わない。
- 1 公開型 1 ファイル、sibling `_test.rs`、ambiguous-suffix 等の構造 lint に従う。
- 命名は Pekko の CRDT ドメイン用語（`GCounter`、`PNCounter`、`Flag` 等）をそのまま採用する（reference-implementation 命名優先）。

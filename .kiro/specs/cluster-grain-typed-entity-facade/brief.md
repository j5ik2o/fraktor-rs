# Brief: cluster-grain-typed-entity-facade

## Problem

cluster の typed 層（`cluster-core-typed`）には Cluster facade しかなく、Grain（virtual actor）API は untyped のままである。Pekko の `EntityTypeKey[M]` / `EntityRef[M]` に相当する、メッセージ型でパラメータ化された型安全な entity 参照がないため、typed actor で grain を使うコードは型消去された `GrainKey` / `GrainRef` を直接扱い、誤った message 型の送信をコンパイル時に防げない。gap analysis カテゴリ8 の easy 項目。

## Current State

- `GrainKey` / `GrainRef`（`cluster-core-kernel/src/grain/`）は実装済み。`GrainRef::request` / `request_future` が Pekko `EntityRef.ask` 相当の機能を提供する。
- `cluster-core-typed` は `Cluster` / `ClusterCommand` / `ClusterStateSubscription` 等の membership facade のみで、grain への typed wrapper は存在しない。
- actor-core の typed 層（`TypedActorRef<M>` 等）が typed wrapper の先行パターンとして存在する。

## Desired Outcome

- メッセージ型 `M` でパラメータ化された typed grain key / typed grain ref（Pekko `EntityTypeKey[M]` / `EntityRef[M]` 相当）が `cluster-core-typed` に定義され、tell / ask（request / request_future）が型安全に呼べる。
- typed ActorSystem から grain facade を取得する setup 統合点（Pekko `ClusterShardingSetup` 相当の最小面）が定義される。
- untyped `GrainKey` / `GrainRef` との相互変換が明示的な API として提供される。

## Approach

actor-core の typed 層が untyped kernel を包む方式（薄い facade、ロジックは kernel 側）を踏襲し、`GrainKey` / `GrainRef` の上に型パラメータ付き wrapper を載せる。kernel 側の挙動は変更しない。typed wrapper は `PhantomData<M>` ベースの zero-cost 抽象とし、codec（`GrainCodec` / `SerializationGrainCodec`）との整合は型レベルで担保する。

## Scope

- **In**:
  - typed grain key（`EntityTypeKey[M]` 相当）
  - typed grain ref（`EntityRef[M]` 相当、tell / request / request_future）
  - typed ActorSystem からの取得経路（setup 統合の最小面）
  - untyped との相互変換 API
- **Out**:
  - `Entity[M, E]` / `EntityContext` 相当の typed behavior factory（gap analysis Phase 2 / medium）
  - sharding runtime（placement / activation）の変更
  - `ShardingEnvelope` / extractor SPI（cluster-sharding-extractor-contract が所有）

## Boundary Candidates

- typed wrapper（core-typed）と kernel grain API（core-kernel）の境界 — kernel は無変更が理想
- setup 統合点と `ClusterExtension` の境界

## Out of Boundary

- grain の lifecycle（activation / passivation）変更
- message serialization の変更

## Upstream / Downstream

- **Upstream**: 既存 grain API（`GrainKey` / `GrainRef` / `GrainCodec`）、actor-core typed 層の wrapper パターン
- **Downstream**: cluster-sharding-extractor-contract（envelope が typed key を参照する）、Phase 2 の `Entity[M, E]` typed behavior factory

## Existing Spec Touchpoints

- **Extends**: なし（新規境界）
- **Adjacent**: cluster-core-typed の既存 `Cluster` facade（同居するが責務は別）、grain runtime 系の OpenSpec changes

## Constraints

- `cluster-core-typed` の `no_std` 境界を維持。
- typed 層は薄い wrapper に留め、重いロジックを持たない（typed/untyped 分離の構造ルール）。
- 命名は Pekko ドメイン用語（`EntityTypeKey` / `EntityRef`）と既存 Grain 語彙（`GrainKey` / `GrainRef`）の対応を design で明示し、CONTEXT.md の語彙と衝突しないよう先に用語を確定する。

# Brief: cluster-sharding-extractor-contract

## Problem

メッセージから entity id / shard（partition）を導出する SPI が存在せず、利用者がメッセージルーティング規則を差し替える手段がない。Pekko の `ShardingEnvelope` / `ShardingMessageExtractor` に相当する契約がないため、Kafka 互換の Murmur2 partitioning のような標準実装も置き場がない。gap analysis カテゴリ8 の easy 項目（extractor 実装群）と、その前提となる medium 項目（envelope / extractor SPI）を 1 spec に束ねる。

## Current State

- `SerializedMessage` / `GrainCodec` / `SerializationGrainCodec`（message の符号化）と `RendezvousHasher`（placement のハッシュ）は実装済みだが、「メッセージ → entity id / partition の抽出」を差し替え可能にする SPI 契約がない。
- `GrainRpcRouter` がメッセージ配送を担うが、抽出規則は固定的。

## Desired Outcome

- envelope 契約（Pekko `ShardingEnvelope[M]` 相当: entity id + payload）と extractor SPI（Pekko `ShardingMessageExtractor[E, M]` 相当: envelope から entity id / shard id / unwrapped message を導出する trait）が core/grain に定義される。
- 標準実装群: HashCode ベース（envelope あり / なし）、Murmur2（Kafka 互換 partitioning）が提供される。
- 既存の grain 配送経路（`GrainRpcRouter` / placement 解決）が extractor SPI を経由できる接続点が定義される。

## Approach

SPI（trait）を先に定義し、既存の固定的な抽出ロジックをデフォルト実装として SPI 配下に移す。標準実装群は pure な計算（no_std で完結）として sibling テストで検証する。Murmur2 は Kafka の partitioning と互換になるよう参照ベクタでテストする。

## Scope

- **In**:
  - envelope 型と extractor SPI（trait 契約）
  - HashCode / HashCodeNoEnvelope / Murmur2 の標準実装
  - 既存配送経路への接続点（デフォルト extractor の差し替え可能化）
- **Out**:
  - shard allocation / rebalance strategy（gap analysis Phase 3 / hard — 抽出と配置決定は別責務）
  - `ClusterShardingSettings` 包括契約（Phase 2 / medium）
  - wire serialization の変更（cluster-message-serialization-contract の領域）

## Boundary Candidates

- extractor SPI（pure 計算）と配送経路接続（grain runtime 統合）の分離
- envelope 型と typed facade（cluster-grain-typed-entity-facade）の境界 — envelope が typed key を参照する場合は facade 完了後に統合

## Out of Boundary

- placement（どのノードに置くか）の決定 — extractor は「どの entity / partition か」だけを決める
- activation / passivation

## Upstream / Downstream

- **Upstream**: cluster-grain-typed-entity-facade（typed envelope の型参照）、既存 `GrainCodec` / `SerializedMessage` / `RendezvousHasher`
- **Downstream**: Phase 3 の shard allocation / rebalance（extractor の shard id を入力に使う）、Phase 2 の `ClusterShardingSettings`（extractor 選択を設定に載せる）

## Existing Spec Touchpoints

- **Extends**: なし（新規境界）
- **Adjacent**: cluster-message-serialization-contract（`SerializedMessage` を読むが変更しない）、grain runtime 系 OpenSpec changes

## Constraints

- `cluster-core-kernel` の `no_std` 境界を維持。標準実装は alloc のみに依存して実装する。
- Murmur2 実装は Kafka のリファレンス出力との互換を参照ベクタテストで証明する。
- 1 公開型 1 ファイル / sibling `_test.rs` などの構造 lint に従う。

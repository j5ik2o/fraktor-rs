# Brief: cluster-sharding-health-and-join-compat

## Problem

grain / placement（sharding 相当）の運用面に 2 つの欠けがある。(1) ノードの grain runtime が「メッセージを受けられる状態か」を外部（ロードバランサ、orchestrator、readiness probe）へ伝える health check 契約がない。(2) join 時の設定互換チェック（`JoinCompatibilityComposition`）が pubsub / downing / SBR / failure-detector の key しか合成せず、grain / placement 設定の不一致（Pekko `JoinConfigCompatCheckSharding` 相当）を join 前に検出できない。gap analysis カテゴリ8 / カテゴリ10 の easy 項目。

## Current State

- `JoinCompatibilityComposition` / `ClusterCompatibilityKeyCatalog`（core/topology, extension）は実装済みで、key 追加のパターンが確立している（failure detector が先行例）。
- grain 側には `KindRegistry` / `VirtualActorRegistry` / `PlacementCoordinatorCore` / `PartitionIdentityLookupConfig` 等の状態と設定が存在するが、readiness の集約 view も join compat key もない。
- `cluster-adaptor-std` には health check エンドポイント相当の仕組みがない。

## Desired Outcome

- grain runtime の readiness 判定契約（登録済み kind の有無、placement coordinator の状態、membership の自ノード状態から readiness を導出する pure な判定）が core に定義され、std 側でそれを外部公開する adapter（関数 / ハンドラ形態）が提供される（Pekko `ClusterShardingHealthCheck` 相当）。
- grain / placement の join-relevant 設定（partition 数、identity lookup 設定など、ノード間で一致すべき値）が join compatibility key として `ClusterCompatibilityKeyCatalog` に追加され、不一致時に mismatch reason を生成する（Pekko `JoinConfigCompatCheckSharding` 相当）。

## Approach

readiness 判定は core の pure な判定型として定義し（`&self` query）、std adapter はそれを呼ぶ薄い橋にする。join compat key は failure detector で確立した「single key + detail に差分 field」パターンを踏襲する。

## Scope

- **In**:
  - readiness 判定契約（core）と std 公開 adapter
  - grain / placement 設定の join compatibility key と mismatch reason 生成
- **Out**:
  - 包括的な `ClusterShardingSettings` 契約（gap analysis Phase 2 / medium — key の対象は現行設定に限る）
  - HTTP サーバ等の具体的な probe endpoint 実装（adapter は判定関数の公開まで。endpoint 配線は利用者側）
  - metrics（`cluster-metrics` は別スコープ）

## Boundary Candidates

- readiness 判定（pure、core）と外部公開（std adapter）の分離
- join compat key の対象設定の選定（ノード間一致が必須の値だけを key にする）

## Out of Boundary

- placement / activation の挙動変更
- liveness（プロセス生存）判定 — readiness（トラフィック受け入れ可否）のみを扱う

## Upstream / Downstream

- **Upstream**: cluster-active-compatibility-baseline（完了済み、join compatibility 基盤）、grain runtime の既存状態（`KindRegistry` / `PlacementCoordinatorCore`）
- **Downstream**: Phase 2 の `ClusterShardingSettings` 包括契約（key の対象を拡張する）、運用系 showcase

## Existing Spec Touchpoints

- **Extends**: cluster-active-compatibility-baseline の compatibility key catalog を拡張する（spec 自体は新規）
- **Adjacent**: configure-cluster-failure-detector（join compat key パターンの先行例）、cluster-discovery-provider-interop（provider lifecycle と readiness の関係に注意）

## Constraints

- readiness 判定は `cluster-core-kernel`（`no_std`）で完結させ、I/O を持ち込まない。
- join compat key は「ノード間で一致しないと join を拒否すべき値」だけに限定し、ローカルチューニング値（タイムアウト等）を key にしない。

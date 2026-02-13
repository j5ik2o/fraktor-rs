# ギャップ分析: tokio-gossip-transport-gossiper

## 前提
- 要件は生成済みだが未承認のため、分析結果は要件調整の材料として扱う。

## 1. 現状調査（既存アセット）

### 主要コンポーネント
- Gossip/メンバーシップ基盤: `modules/cluster/src/core/gossip_engine.rs`, `membership_coordinator.rs`, `membership_table.rs`
- GossipTransport 抽象: `modules/cluster/src/core/gossip_transport.rs`, `gossip_outbound.rs`
- std ドライバ: `modules/cluster/src/std/membership_coordinator_driver.rs`
- Gossiper 抽象/共有ラッパ: `modules/cluster/src/core/gossiper.rs`, `gossiper_shared.rs`, `noop_gossiper.rs`
- EventStream 連携: `modules/cluster/src/core/cluster_extension.rs`（TopologyUpdated の適用）
- サンプル: `modules/cluster/examples/membership_gossip_topology_std/main.rs`（in-memory transport）

### 観測されたパターン/制約
- `core` は no_std 前提、std 実装は `modules/cluster/src/std` に集約。
- `GossipTransport` は `send`/`poll_deltas` の同期 API で、受信はポーリング型。
- `Gossiper` は `start/stop` のみで、`ClusterCore::start_*` から起動される。
- 1ファイル1型・tests.rs 配置・mod.rs 禁止の lint 制約がある。

### 既存の統合ポイント
- `MembershipCoordinatorDriverGeneric` が `GossipTransport` と `EventStream` を接続し、TopologyUpdated を発行。
- `ClusterExtension` が EventStream の `ClusterEvent::TopologyUpdated` を `ClusterCore` に適用。

## 2. 要件対応マップ（Requirement-to-Asset Map）

| 要件 | 既存アセット | 充足状況 | ギャップ種別 | ギャップ/備考 |
|---|---|---|---|---|
| 要件1: Tokio GossipTransport の送受信 | `GossipTransport` trait, `MembershipCoordinatorDriverGeneric`, std サンプルの `DemoTransport` | **Partial** | Missing | Tokio でのネットワーク transport 実装が存在しない。ワイヤ形式/アドレス解決が未定義。 |
| 要件2: Tokio Gossiper のライフサイクル | `Gossiper` trait, `NoopGossiper` | **Missing** | Missing | Tokio で driver を回す実装が無い。start/stop の具体挙動が未定義。 |
| 要件3: MembershipCoordinator 連携 | `MembershipCoordinatorDriverGeneric` + EventStream publish | **Partial** | Constraint | driver はあるが、Tokio Gossiper からの起動/周期駆動が未実装。 |
| 要件4: Tokio gossip サンプル | `membership_gossip_topology_std` など std サンプル | **Partial** | Missing | in-memory bus のみ。Tokio transport を使う実行例がない。 |
| 要件5: ビルド境界と検証 | std/core 分離、`modules/cluster/src/std.rs` | **Partial** | Constraint | std 内に新規モジュール追加が必要。no_std への影響を避ける設計が必要。 |

## 3. ギャップと制約

### 明確な不足（Missing）
- Tokio ネットワーク上で動作する `GossipTransport` 実装。
- Tokio タスク/タイマーを用いた `Gossiper` 実装（周期処理・停止制御）。
- Tokio transport を使う `examples` の実働サンプル。
- 送受信の検証用テスト（std 側の tests.rs）。

### 仕様上の空白（Unknown/Decision Needed）
- Gossip のワイヤ形式（`MembershipDelta` のシリアライズ方式）。
- Gossip の通信方式（TCP/UDP、単方向/双方向、再送/冪等性）。
- `GossipOutbound.target` を authority と見なす前提の明文化。
- `TimerInstant` への時刻換算（Tick 周期の基準値）。

### 既存制約
- core に `#[cfg(feature = "std")]` を置けないため、Tokio 実装は std に限定する必要がある。
- `GossipTransport` の API は同期/ポーリング型であり、非同期 API への変更は影響が大きい。

## 4. 実装アプローチの選択肢

### Option A: 既存ドライバ拡張（std に transport + gossiper を追加）
**概要**: `MembershipCoordinatorDriverGeneric` を回す Tokio 用 `Gossiper` と、Tokio `GossipTransport` を追加  
**利点**: 既存の driver と EventStream 経路を活用できる  
**欠点**: transport のワイヤ設計が未決定、start/stop の設計が必要

### Option B: 新規コンポーネント分離
**概要**: Tokio transport と gossiper を独立モジュールとして追加し、examples はその上に構築  
**利点**: 責務分離が明確、テスト容易  
**欠点**: 新規 API/設定が増える

### Option C: ハイブリッド（最小 transport + 段階的に拡張）
**概要**: まずは最小通信（単一ノード/簡易プロトコル）で動作する transport を作り、拡張  
**利点**: 実装リスクを抑えつつ前進できる  
**欠点**: 仕様変更の可能性が残る

## 5. 複雑度/リスク評価
- **Effort**: M（3–7日）  
  - 新規 std 実装＋サンプル＋テスト作成が必要
- **Risk**: Medium  
  - ワイヤ形式と通信方式の決定により設計が左右される

## 6. Research Needed（設計フェーズ持ち越し）
- protoactor-go の gossip wire 形式の調査（互換要否の判断材料）
- TCP/UDP の選定と、再送・信頼性の扱い
- `MembershipDelta` のシリアライズ戦略（独自/serde/prost 等）

## 7. 設計フェーズへの提案
- **推奨検討**: Option A から着手し、transport 仕様が確定しない場合は Option C で段階導入
- **要決定事項**:
  - wire 形式と transport プロトコル
  - `Gossiper` の周期処理と停止シーケンス
  - examples/テストの最小成功基準

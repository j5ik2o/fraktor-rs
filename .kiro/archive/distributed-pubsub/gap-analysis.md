# ギャップ分析: distributed-pubsub

## 前提
- 要件は生成済みだが未承認のため、本分析は要件調整の材料として扱う。

## 1. 現状調査（既存アセット）

### 主要コンポーネント
- PubSub 抽象: `modules/cluster/src/core/cluster_pub_sub.rs`（start/stop のみ）
- PubSub 実装: `modules/cluster/src/core/cluster_pub_sub_impl.rs`（EventStream 連動 + PubSubBroker）
- インメモリ broker: `modules/cluster/src/core/pub_sub_broker.rs`
- イベント/メトリクス: `pub_sub_event.rs`, `pub_sub_metrics.rs`, `pub_sub_topic_metrics.rs`
- 配送ポリシー/パーティション挙動: `delivery_policy.rs`, `partition_behavior.rs`
- 既定: `NoopClusterPubSub` がデフォルト
- 統合口: `ClusterExtensionInstaller` で pubsub factory 注入、`ClusterCore::start_*` で起動
- TopicActorKind: `kind_registry::TOPIC_ACTOR_KIND`（実体 Actor は未定義）

### 観測されたパターン/制約
- `core` は no_std 前提。std 依存は `modules/cluster/src/std/*` に隔離する必要がある。
- `ClusterPubSub` は start/stop のライフサイクルのみで、公開API（subscribe/publish/unsubscribe）が不足。
- `PubSubBroker` は **配送実体を持たない**（購読集合管理＋イベント/メトリクスのみ）。
- 1ファイル1型、tests.rs などの lint 制約に従う必要がある。

### 既存の統合ポイント
- `EventStream` への `cluster-pubsub` 拡張イベント発火（`ClusterPubSubImpl`）。
- `ClusterEvent::StartupFailed` 経由の起動失敗通知。
- `ClusterEvent::TopologyUpdated` などトポロジ変化イベントは存在するが pubsub 連動は未実装。

## 2. 要件対応マップ（Requirement-to-Asset Map）

| 要件 | 既存アセット | 充足状況 | ギャップ/備考 |
|---|---|---|---|
| 要件1: トピック購読と送達 | `PubSubBroker`/`ClusterPubSubImpl` | **Partial** | subscribe/publish は存在するが、unsubscribe・配送実体・分散送達が未実装 |
| 要件2: 配送フローとバッチ配送 | なし | **Missing** | DeliveryActor/Batch/時間条件が未実装 |
| 要件3: Producer/Publisher API | なし | **Missing** | 公開APIが存在せず、受理/拒否の明確なレスポンス仕様が不足 |
| 要件4: 分散ルーティングとトポロジ追従 | `ClusterEvent::TopologyUpdated` | **Partial** | トポロジイベントはあるが PubSub へ未接続 |
| 要件5: 観測性と運用 | `PubSubEvent`/`PubSubMetrics` | **Partial** | Publish/Subscribe イベントはあるが「送達成功/失敗」イベントが不足 |
| 要件6: no_std/std 互換性 | `PubSubBroker`/`ClusterPubSubImpl` | **Partial** | core 側の基盤はあるが分散配送・APIが未整備 |

## 3. ギャップと制約

### 明確な不足（Missing）
- DeliveryActor / TopicActor / 配送ワークフロー（分散配送の実体）。
- BatchingProducer / Publisher API（公開インターフェイス、キュー制御、タイムアウト）。
- unsubscribe、購読対象の種類（Pid/ClusterIdentity）と識別子設計。
- 送達成功/失敗イベント・再送/失敗扱いの方針。

### 仕様上の空白（Unknown/Decision Needed）
- 購読者の識別方式（PID/ClusterIdentity/文字列IDのどれを正とするか）。
- バッチの境界条件（サイズ/時間/失敗時の再試行方針）。
- トポロジ追従の責務分離（PubSub が直接判断するか Provider/Topology 経由にするか）。
- 観測イベントの粒度（配信結果、遅延、キュー長など）。

### 既存制約
- no_std で動作するコアAPIを先に設計し、std 側で I/O と実行基盤を補完する必要がある。
- 既存命名ルール（曖昧サフィックス禁止、1ファイル1型）に従う必要がある。

## 4. 実装アプローチの選択肢

### Option A: 既存コンポーネント拡張
**概要**: `PubSubBroker`/`ClusterPubSubImpl` に API と配送機構を追加  
**利点**: 既存基盤を再利用できる  
**欠点**: ClusterPubSubImpl の責務が肥大化しやすい

### Option B: 新規コンポーネント追加
**概要**: TopicActor/DeliveryActor/Publisher/Producer を新設し、Broker は購読・メトリクスのみ担当  
**利点**: 責務分離が明確、テスト容易  
**欠点**: 新規インターフェイス設計が必要、ファイル数増

### Option C: ハイブリッド
**概要**: Broker + EventStream を維持しつつ、分散配送と API を別レイヤで構築  
**利点**: 既存アセットを活かしつつ分散責務を分離  
**欠点**: 統合点が増え、設計調整が必要

## 5. 複雑度/リスク評価
- **Effort**: L（1–2週間）  
  - 分散配送/バッチ/観測の複合実装が必要
- **Risk**: Medium〜High  
  - 失敗時の配送保証とトポロジ追従の設計が難所

## 6. Research Needed（設計フェーズ持ち越し）
- protoactor-go の PubSub 実装（`pubsub_delivery.go`, `pubsub_batch.go`, `pubsub_publisher.go`, `pubsub_producer_*`）の対応方針
- バッチ配送のシリアライズ方式と返信設計
- 送達失敗の分類（Timeout/DeadLetter 等）とイベントへの反映方法

## 7. 設計フェーズへの提案
- **推奨検討**: Option C（Broker/イベント基盤は維持し、配送とAPIを別層に分離）
- **要決定事項**:
  - 購読識別子の型とAPI表現
  - バッチ境界条件と失敗時ポリシー
  - トポロジ追従の責務分担（PubSub vs Provider/Topology）

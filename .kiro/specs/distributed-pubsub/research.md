# 調査ログ: distributed-pubsub

## Summary
- 既存の `PubSubBroker`/`ClusterPubSubImpl` を維持しつつ、トピックアクター + ノード配送アクターを追加する構成が最小の分散拡張となる。
- 配送は「ノード単位で 1 回送信して各ノード内で配布する」モデルが適合し、トポロジは EventStream 経由で追従する。
- Producer/Publisher はバッチサイズと待機時間の両条件でフラッシュできる設計が必要で、キュー上限やタイムアウトも明示する。

## Discovery Scope
- WebSearch: Pekko/Akka 分散 PubSub の配送モデル、Proto.Actor PubSub のアーキテクチャと Producer 設計。
- Codebase: `modules/cluster/src/core` の `ClusterPubSub*` / `PubSubBroker` / `PubSubEvent` / `ClusterEvent`。
- References: `references/protoactor-go/cluster/pubsub_*` の実装詳細。

## Research Log

### 1. Pekko/Akka 分散 PubSub の配送モデル
**Findings**
- 分散 PubSub はゴシップで購読状態を共有し、最終的整合性の中でノード単位のルーティングを行う。
- Publish はクラスタ内の各ノードへ 1 回送信し、ノード内の購読者へはローカルで配布される。

**Sources**
- https://pekko.apache.org/api/pekko/snapshot/org/apache/pekko/cluster/pubsub/DistributedPubSubMediator.html
- https://doc.akka.io/japi/akka/2.2.4/akka/cluster/pubsub/DistributedPubSubMediator.Publish.html

**Implications**
- `PubSubTopicActor` が購読者をノード単位に束ね、`PubSubDeliveryActor` を各ノードに配置する構成が最小コスト。
- トポロジ更新は「最終的整合性」前提で設計し、ローカル購読者への配送を常に優先する。

### 2. Proto.Actor PubSub の構成
**Findings**
- Topic アクターは購読者を保持し、ノード単位にグルーピングして `DeliverBatchRequest` を送る。
- Member 配送アクターが購読者ごとに配信し、失敗は `NotifyAboutFailingSubscribers` で Topic に返す。

**Sources**
- https://proto.actor/docs/cluster/pubsub/
- `references/protoactor-go/cluster/pubsub_topic.go`, `references/protoactor-go/cluster/pubsub_delivery.go`

**Implications**
- 失敗した購読者を Topic アクターで除去する設計が必要。
- 配送失敗は EventStream に出力し、購読状態の更新と観測イベントを連動させる。

### 3. Publisher/Producer のバッチング
**Findings**
- Publisher は `initialize`/`publish`/`publish_batch` を持ち、Producer はバッチサイズ・待機時間・キュー上限・タイムアウトを持つ。
- バッチ配送は配信タイムアウトやエラーハンドラを設け、受理/拒否を返す設計が前提。

**Sources**
- https://proto.actor/docs/cluster/pubsub/
- `references/protoactor-go/cluster/pubsub_publisher.go`, `references/protoactor-go/cluster/pubsub_producer.go`

**Implications**
- `BatchingProducerConfig` にサイズ/待機時間/キュー上限/タイムアウトを入れる。
- 公開 API は受理/拒否と理由を返す `PublishAck` を必須化する。

### 4. 既存コードの観測性
**Findings**
- 既存 `PubSubBroker` は Topic/Subscription と Partition メトリクスを EventStream に流すが、配送成功/失敗イベントが不足。

**Sources**
- `modules/cluster/src/core/pub_sub_event.rs`

**Implications**
- `PubSubEvent` を拡張し、Publish 受理/配送成功/配送失敗/購読変更を明示的に発火する。

## Architecture Pattern Evaluation
- **Option A (既存拡張のみ)**: 実装最小だが責務が肥大化し、配送/バッチ/トポロジが集中する。
- **Option B (新規コンポーネント分離)**: 責務分離は明確だが統合点が増える。
- **Option C (Broker + EventStream 維持 + 配送層分離)**: 既存資産を活かしつつ配送層を独立できる。

**Decision**: Option C を採用。Broker と EventStream を維持し、Topic/Delivery/Producer/Publisher を分離する。

## Design Decisions
- `ClusterPubSub` は分散 PubSub の制御面を一元化し、API は `PubSubApiGeneric` から提供する。
- Topic アクター + ノード配送アクターの 2 層で配送する（ノード単位で 1 回送信）。
- バッチングは size / wait の両条件でフラッシュし、キュー上限とタイムアウトを明示する。
- EventStream で Publish 受理/配送成功/配送失敗/購読変更/メトリクスを統一的に観測する。
- core は no_std 準拠、std 実装は実行基盤と I/O を補完するだけに留める。

## Risks and Mitigations
- **トポロジの最終的整合性**: 更新遅延で一時的に配送漏れが起こりうる → ローカル配送を最優先し、再参加時に再評価。
- **バッチキュー肥大化**: メモリ消費リスク → キュー上限 + 拒否理由返却で制御。
- **配送失敗の分類**: 失敗理由が曖昧 → DeliveryStatus を enum 化し EventStream に記録。

## Open Questions
- 購読者識別子の公開 API で `ActorRef`/`ClusterIdentity` のどちらを必須にするか。
- 配送保証（AtMostOnce/AtLeastOnce）の既定値と Topic 単位設定の粒度。

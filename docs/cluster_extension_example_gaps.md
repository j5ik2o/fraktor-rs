# cluster_extension_* サンプルの未完了点メモ

現状の `modules/cluster/examples/cluster_extension_no_std` / `cluster_extension_tokio` は「拡張の起動とクラスタイベント流し」を最小限に体験するための擬似例です。ProtoActor-Go 同等のクラスタ挙動には、以下の不足実装・改善が残っています。

## 1. トポロジ伝搬の自動化（cluster モジュール内で対応）
- **不足**: サンプル側で `on_topology` を手動呼び出ししている。Gossip/MemberList からのコールバック連結が未接続。
- **割当**:
  - `core/gossiper.rs` + `core/gossip_engine/`…トポロジ差分を検知し `ClusterTopology` を生成
  - `core/cluster_core.rs`…受信口（`on_topology`）は既存、呼び出し元を Gossiper からに変更
  - `core/cluster_extension.rs`…Gossiper からのイベントを受け、`ClusterCore::on_topology` を呼ぶブリッジ
  - `core/cluster_provider.rs`…必要ならトポロジ通知用コールバックを追加して Gossiper に橋渡し

## 2. 実トランスポートとの統合（cluster + remote）
- **不足**: TokioTcpTransport と結線した join/leave の実動サンプルがない。
- **割当**:
  - `modules/cluster/examples/cluster_extension_tokio` …削除または統合サンプルに置換（手動 on_topology 呼びを廃止）
  - `modules/remote/src/std/transport/tokio_tcp/*` と `cluster` の Gossiper/Provider を接続し、実際の join/leave を流す
  - `core/cluster_provider.rs` の実装（新規 Provider）で remoting を起動し、Gossip へ authority/seed 情報を渡す

## 3. ClusterProvider 連携（cluster）
- **不足**: Provider からの起動・停止・トポロジ変化通知を Extension に橋渡しするコールバックが未実装（NoopProvider 依存）。
- **割当**:
  - `core/cluster_provider.rs` にトポロジ通知フック（例: `fn subscribe_topology(&self, callback: ArcShared<dyn TopologyHandler>)`）を追加
  - `core/cluster_extension.rs` でコールバック登録し、受け取ったトポロジを `ClusterCore::on_topology` へ転送

## 4. Gossip / PubSub の実装ダミー化（cluster）
- **不足**: サンプルではロギングのみのダミー実装。最新トポロジ配信や TopicKind 連動を実際には行っていない。
- **割当**:
  - `core/gossip_engine/` 配下で最低限のトポロジ配布を実装し、`Gossiper` トrait 実装を実用化
  - `core/cluster_pub_sub.rs` と `core/pub_sub_broker.rs` を繋ぎ、TopicKind 前提の購読受付を有効化

## 5. メトリクス・イベント検証（cluster）
- **不足**: metrics 有効時のメンバー数・仮想アクター数の整合性を自動テストでカバーしていない。
- **割当**:
  - `core/cluster_core/tests.rs` に統合テストを追加し、Gossiper 経由の topology 更新 → metrics 更新 → EventStream 出力を検証
  - 必要なら `core/cluster_metrics.rs` / `core/cluster_metrics_snapshot.rs` を拡張してテストフックを用意

## 6. ドキュメント反映（docs）
- **不足**: `example.md` は計画段階のまま。実装された経路と実行手順（Tokio 版 / no_std 版）を反映していない。
- **割当**:
  - `docs/guides/` か `.kiro/specs/protoactor-go-cluster-impl/example.md` を更新し、実際の統合サンプル手順を記載
  - 手動 `on_topology` サンプルは撤去し、完全経路のサンプルに差し替える旨を明記

## 7. example の書き換え方針（最終形）
- **削除するもの**
  - 手動 `on_topology` 呼び出し（Gossip/MemberList 経由の自動通知に置換）
  - ログ専用ダミー実装（`LoggingGossiper`/`LoggingPubSub`/`LoggingIdentityLookup`/`EmptyBlockListProvider` 等）
  - 手動 BlockList 注入・擬似トポロジ入力
- **残すもの（本質）**
  - ClusterExtensionId の生成と登録
  - Kind 登録（member/client）と `start_member` / `start_client`
  - EventStream 購読（`EventStreamEvent::Extension { name: "cluster", ... }` をログする最小コード）
  - シンプルなメッセージ往復デモ（Grain 等への tell/ask）
  - 終了処理（`shutdown(true)` と ActorSystem の terminate）
- **個別方針**
  - `cluster_extension_no_std`: Manual TickDriver は残しつつ、上記以外を削除して最小化。
  - `cluster_extension_tokio`: TokioTcpTransport と接続した統合サンプルに差し替え、手動 on_topology は廃止。
  - `example.md`: 上記最終形の実行手順・前提 features・期待ログを追記し、手動ステップは除去。

## 2. 実トランスポートとの統合
- **不足**: TokioTcpTransport と結線した join/leave の実動サンプルがない。
- **必要作業**: `modules/remote` の Tokio TCP を使い、2 ノードで Rendezvous/IdentityLookup/PID キャッシュ無効化まで通る統合例を用意する。

## 3. ClusterProvider 連携
- **不足**: Provider からの起動・停止・トポロジ変化通知を Extension に橋渡しするコールバックが未実装（NoopProvider 依存）。
- **必要作業**: Provider インターフェイスにトポロジ通知口を追加し、実装側で `ClusterTopology` を生成して拡張に渡す。

## 4. Gossip / PubSub の実装ダミー化
- **不足**: サンプルではロギングのみのダミー実装。最新トポロジ配信や TopicKind 連動を実際には行っていない。
- **必要作業**: core の GossipEngine / PubSubBroker と結線するか、最小の in-process 実装を用意してイベント配布を確認できるようにする。

## 5. メトリクス・イベント検証
- **不足**: metrics 有効時のメンバー数・仮想アクター数の整合性を自動テストでカバーしていない。
- **必要作業**: トポロジ更新・シャットダウン時のメトリクス更新と EventStream 出力を結合テストで検証する。

## 6. ドキュメント反映
- **不足**: `example.md` は計画段階のまま。実装された経路と実行手順（Tokio 版 / no_std 版）を反映していない。
- **必要作業**: 上記が実装でき次第、`example.md` を更新し、実行手順・前提 feature・期待ログを追記する。

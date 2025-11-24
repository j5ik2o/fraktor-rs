# ギャップ分析: protoactor-go-cluster-extension-samples

## 1. 現状把握
- **サンプル (no_std)**: `modules/cluster/examples/cluster_extension_no_std/main.rs` は `ManualTestDriver` 前提で、`ClusterExtension::on_topology` を手動呼び出し。`DemoGossiper/PubSub/IdentityLookup/Provider` はスタート・ストップのみのダミーでトポロジ通知なし。BlockList は固定値を返すのみ。
- **サンプル (Tokio)**: `modules/cluster/examples/cluster_extension_tokio/main.rs` は `NoopClusterProvider` と Logging 系ダミーを使用し、`ClusterTopology` を手動で2回投入して join/leave を模擬。Remoting は `RemotingExtensionInstaller` + `TokioActorRefProviderInstaller` を起動するが、Gossip/Provider と接続せず TokioTcpTransport との実 join/leave は未実装。
- **ClusterCore/Extension**: `cluster_core.rs` は pubsub→gossip→provider の起動/停止と `on_topology` によるメトリクス更新・EventStream 発火を提供するが、Gossiper/Provider から自動で `on_topology` を呼ぶ経路が存在しない。`ClusterExtensionGeneric` も手動 `on_topology` を公開するのみ。
- **インターフェイス制約**: `Gossiper` は start/stop のみ、トポロジ通知コールバックなし。`ClusterProvider` も start_member/client/shutdown のみでトポロジ・イベントを拡張へ伝搬できない。`NoopClusterProvider` 以外の実装が存在しない。
- **Gossip/PubSub 実装**: `gossip_engine` と `pub_sub_broker` にロジックはあるが、Gossiper / ClusterPubSub trait と接続する実装が無く、サンプルは Logging/No-op に留まる。
- **観測性/ドキュメント**: EventStream への cluster 拡張イベントは core で出るが、サンプルはダミー依存のため期待ログと実挙動が乖離。`example.md` などサンプル手順は未更新（手動 on_topology 手順が前提のまま）。

## 2. 要件に対するギャップ
- 要件1 自動トポロジ伝搬: `Gossiper`/`ClusterProvider` にトポロジ通知経路がなく、`cluster_extension_*` サンプルは手動 `on_topology` に依存（Missing）。
- 要件2 Tokio TCP 実動デモ: 現行 Tokio サンプルは `NoopClusterProvider` + LoggingGossiper で、TokioTcpTransport と gossip/provider を結線せず join/leave を実ネットワークで観測できない（Missing）。
- 要件3 Provider 連携: Provider からの起動/停止/トポロジ通知を受け取るコールバックが trait/extension に存在せず、ブリッジコードも無い（Missing）。
- 要件4 Gossip/PubSub 連動: trait 実装がダミーのみで `gossip_engine` / `pub_sub_broker` に繋がっていない。TopicKind 登録と購読・配信の経路がサンプルで検証不可（Missing）。
- 要件5 観測性・ドキュメント一貫性: metrics 更新は `on_topology` 内にあるが、Gossiper 経由の実トポロジで検証する統合テスト/サンプルが無い。`example.md` は手動ステップ依存で最新構成と不整合（Missing）。

## 3. 実装アプローチ案
### Option A: 既存拡張に組み込み
- `Gossiper` にトポロジ配布/購読コールバックを追加し、`core/gossip_engine` を使う実装を std/no_std 双方に用意。`ClusterExtension` が購読して `ClusterCore::on_topology` を呼ぶ。
- `ClusterProvider` にトポロジ通知登録 API（例: subscribe_topology）を追加し、Tokio 向け Provider を新設して Remoting/TokioTcpTransport 起動・seed/authority 注入・join/leave 受信を実装。
- `cluster_extension_tokio` を新 Provider/Gossiper/PubSub に差し替え、手動 `on_topology` を削除。ログと EventStream を実観測できるようにする。
- Pros: 既存構造を保ちつつ欠落部分を補完。Cons: Trait 変更が広く影響、後方非互換を許容する前提。

### Option B: 新規統合サンプル＋アダプタ層
- サンプル専用に「MiniClusterHarness」を追加し、TokioTcpTransport・GossipEngine・PubSubBroker をまとめて起動するアダプタを実装。ClusterCore とはイベントチャネルで接続し、サンプルはハーネス API のみを呼ぶ。
- Pros: コアへの変更を小さくしつつサンプル体験を完成できる。Cons: 二重の経路ができ、本体との乖離や重複実装リスク。

### Option C: 段階的ハイブリッド
- フェーズ1: Option A の trait 拡張とブリッジ実装で自動トポロジ伝搬を最小実装（単一プロセス in-process gossip/pubsub）。
- フェーズ2: TokioTcpTransport を組み込んだ Provider/Gossiper 実装と 2 ノード相当の e2e サンプルへ拡張。
- Pros: リスクを段階化しやすい。Cons: 中間フェーズでサンプルのネットワーク完全性が限定される。

## 4. 努力度・リスク
- **労力: L (1–2 週間)** — trait 拡張・Gossip/Provider/PubSub 実装追加・Tokio サンプル刷新・統合テスト整備・ドキュメント更新まで跨る。
- **リスク: 中〜高** — Remoting/TokioTcpTransport との結線、trait 非互換変更、no_std/std 両対応が必要。並行処理・イベント配線の不整合で回帰する可能性がある。

## 5. Research Needed
- GossipEngine を ClusterCore へ通知する最小 API 形状（pull/push、イベントチャネルの有無）。
- TokioTcpTransport と membership/gossip の接続点（authority 生成、seed ブートストラップ、Ack/Reconciling の扱い）。
- PubSubBroker を TopicKind とどう同期させるか（自動登録と再同期のタイミング）。
- 既存テストインフラで複数ノード相当の統合テストをどう立てるか（tokio::test / manual driver / feature gates）。

## 推奨
Option C を推奨。まず trait 拡張と in-process gossip/pubsub ブリッジで自動トポロジ伝搬と metrics/イベントの整合を確保し、その後 TokioTcpTransport を用いた 2 ノード統合サンプルとドキュメント更新を進める。 ***!

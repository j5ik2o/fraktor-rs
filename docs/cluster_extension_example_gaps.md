# cluster_extension_* サンプルの未完了点メモ

現状の `modules/cluster/examples/cluster_extension_no_std` / `cluster_extension_tokio` は「拡張の起動とクラスタイベント流し」を最小限に体験するための擬似例です。ProtoActor-Go 同等のクラスタ挙動には、以下の不足実装・改善が残っています。

## 1. トポロジ伝搬の自動化
- **不足**: サンプル側で `on_topology` を手動呼び出ししている。Gossip/MemberList からのコールバック連結が未接続。
- **必要作業**: `Gossiper`/`MemberList` から `ClusterCore::on_topology` への通知経路を実装し、ClusterProvider 側が membership 変更をプッシュできるフックを追加する。

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

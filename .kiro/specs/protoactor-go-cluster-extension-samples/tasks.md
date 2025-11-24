# 実装計画

- [ ] 1. EventStream ベースのトポロジ通知経路を整備する
- [x] 1.1 ClusterEvent に TopologyUpdated を追加し、重複ハッシュ抑止を実装する
  - ClusterEvent enum に TopologyUpdated { topology, joined, left, blocked } を追加し、既存拡張イベントと整合させる
  - ClusterCore で直近ハッシュを保持し、同一ハッシュは publish しないロジックを組み込む
  - _Requirements: 1.2,1.3,1.4,5.1,5.3_
- [x] 1.2 ClusterExtension が EventStream を購読して ClusterCore::on_topology を呼ぶようにする
  - TopologyUpdated を購読する subscriber を追加し、ハッシュ抑止後のトポロジのみ適用する
  - 適用時に metrics と blocked 情報を更新し EventStream へ反映する
  - _Requirements: 1.1,1.3,1.4,5.1,5.3_

- [ ] 2. サンプル用 Provider/Gossiper/PubSub を EventStream 方式に差し替える
- [x] 2.1 SampleTcpProvider を EventStream publish 方式で実装する
  - Remoting/TokioTcpTransport の membership イベントを ClusterTopology へ写像し、TopologyUpdated を publish する
  - join/leave で joined/left を構成し、BlockListProvider の値を blocked に含める
  - _Requirements: 2.1,2.2,2.3,2.4,3.1,3.2,4.4,5.3_
- [x] 2.2 InprocSampleProvider/Gossiper/PubSub を静的トポロジ publish に対応させる
  - 静的 ClusterTopology を EventStream に publish し、自動適用を確認できるようにする
  - GossipEngine は Phase1 では未使用とし、in-process サンプルは静的 publish のみで動作させる
  - _Requirements: 1.1,1.2,1.4,4.1,4.4,5.1_
- [ ] 2.3 PubSubImpl を EventStream 経由に統一し TopicKind 前提で起動する
  - TopicActorKind 登録を前提に publish/subscribe フローを整備し、起動失敗時は EventStream にエラーを発火する
  - _Requirements: 4.1,4.2,4.3,5.3_

- [ ] 3. Phase1 実行経路（静的トポロジ＋EventStream）を完成させる
- [ ] 3.1 cluster_extension_no_std を静的トポロジ publish 版に更新する
  - ダミー依存を差し替え、手動 `on_topology` 呼びを削除し EventStream publish に置換する
  - Manual TickDriver で少数ステップ回し、TopologyUpdated と metrics の出力を確認できるようにする
  - _Requirements: 1.1,1.2,1.4,5.3_
- [ ] 3.2 cluster_extension_tokio を静的トポロジ publish で起動する手順を整備する
  - SampleTcpProvider を静的モードで起動し、2 ノード相当の TopologyUpdated を publish して EventStream ログを確認する
  - README/サンプルコメントに差し替え手順（provider 差し替え可能）を追記する
  - _Requirements: 2.1,2.2,5.4_
- [ ] 3.3 Phase1 統合テストを追加する
  - 静的 TopologyUpdated を publish → ClusterCore が metrics/イベントを更新し、PID キャッシュ無効化・blocked 反映を確認する
  - _Requirements: 1.1,1.2,1.4,3.3,5.1,5.3_

- [ ] 4. Phase2: GossipEngine + TokioTcpTransport を結線する
- [ ] 4.1 SampleTcpProvider で seed/authority を GossipEngine に渡す経路を実装する
  - Remoting 初期化後に GossipEngine へ seed/authority を登録し、起動/停止イベントを EventStream に発火する
  - _Requirements: 2.1,2.2,2.3,2.4,3.1,3.2_
- [ ] 4.2 GossipEngine からの join/leave を EventStream に流す
  - GossipEngine の出力を ClusterTopology に変換し、TopologyUpdated を publish する
  - _Requirements: 1.1,1.2,1.3,4.1,4.4_
- [ ] 4.3 動的トポロジで PubSub/メッセージ配送を検証する
  - 2 ノードで join 後に TopicKind を購読し、publish/subscribe が実ノード間で通ることを確認する
  - _Requirements: 2.2,2.3,4.2,4.3_
- [ ] 4.4 Phase2 統合テスト（Tokio 2ノード）を追加する
  - join/leave/BlockList 反映・metrics 更新・EventStream TopologyUpdated 出力を確認する統合テストを追加
  - _Requirements: 2.1,2.2,2.3,2.4,3.1,3.2,4.1,4.4,5.1,5.3_

- [ ] 5. 観測性・ドキュメント整合を確認する
- [ ] 5.1 metrics 無効時の挙動と EventStream 出力を検証する
  - metrics が無効構成のときに MetricsError::Disabled を返し、イベントは継続することをテストで確認
  - _Requirements: 5.2_
- [ ] 5.2 サンプル手順とログ例を設計に沿って更新する
  - example.md とサンプル冒頭コメントに EventStream 方式、provider 差し替え方法、静的→動的フェーズの実行手順を記載する
  - _Requirements: 5.4_

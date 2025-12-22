# クラスタ利用例（設計段階の計画）

本ドキュメントは、protoactor-go と同等のクラスタ体験を fraktor-rs で提供するための例示的な利用フローを示します。実装完了後に具体的なコードサンプルを更新します。

## 目的
- `ClusterExtensionId` / `ClusterExtensionConfig` を用いて、ActorSystem 上でクラスタを起動する方法を明確にする。
- `ClusterProvider`（etcd/k8s/consul 相当）と Remote core が提供する `BlockListProvider` を注入して、protoactor-go の `Cluster.StartMember/StartClient` と同じ操作感を再現する。
- EventStream から `EventStreamEvent::Cluster` を購読し、Startup/Topology/Shutdown などのイベントを確認できることを示す。

## 想定フロー（実装後の手順）
1. **設定の準備**  
   `ClusterExtensionConfig` にクラスタ名、gossip/pubsub/heartbeat/metrics/timeout などを設定する。
2. **プロバイダの用意**  
   - `ClusterProvider`: メンバーシップ起動/停止を担う実装（例: etcd/k8s/consul 相当）。  
   - `BlockListProvider`: Remote core が提供する実装を取得する。
3. **Extension の登録**  
   `ClusterExtensionId::new(config, provider_arc, block_list_provider_arc, event_stream_arc)` を生成し、`ActorSystemGeneric::extended().register_extension(&id)` を呼ぶ。
4. **起動**  
   `start_member()` または `start_client()` を呼び、起動失敗時は `ClusterEvent::StartupFailed` が EventStream に流れることを確認する。
5. **イベント購読**  
   EventStream を購読し、`EventStreamEvent::Cluster` に含まれる `Startup/StartupFailed/Topology/Shutdown/ShutdownFailed` を観測する。
6. **シャットダウン**  
   `shutdown(true|false)` を呼び、graceful 時は `Shutdown`、失敗時は `ShutdownFailed` が流れることを確認する。

## 更新予定
実装完了後に、上記フローに対応する Rust コード例（examples ディレクトリへの配置を予定）と実行手順を追記します。***

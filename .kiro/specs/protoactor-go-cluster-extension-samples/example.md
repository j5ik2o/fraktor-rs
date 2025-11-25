# クラスタ拡張サンプル ガイド

## 概要

fraktor-rs のクラスタ拡張サンプルは、EventStream 主導のトポロジ通知方式を採用しています。Provider/Gossiper がトポロジイベントを EventStream に publish し、ClusterExtension が自動的に購読して `ClusterCore::on_topology` を呼び出す流れを実装しています。

これは ProtoActor-Go のクラスタ設計と同様の方式です。

## アーキテクチャ

```
┌─────────────────────────────────────────────────────────────┐
│                     ClusterExtension                        │
│  - EventStream を購読                                        │
│  - TopologyUpdated を受信して ClusterCore に適用              │
└─────────────────────────────────────────────────────────────┘
                              ↑
                              │ TopologyUpdated イベント
                              │
┌─────────────────────────────────────────────────────────────┐
│                       EventStream                            │
└─────────────────────────────────────────────────────────────┘
                              ↑
                              │ publish
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│   Provider    │    │   Gossiper    │    │   Observer    │
│ (静的/動的)    │    │ (Phase2)      │    │ (ログ出力)     │
└───────────────┘    └───────────────┘    └───────────────┘
```

## Phase1: 静的トポロジ + EventStream 統合

### InprocSampleProvider（no_std 環境向け）

`InprocSampleProvider` は静的トポロジを EventStream に publish するサンプル Provider です。

```rust
use fraktor_cluster_rs::core::{
    ClusterExtensionId, ClusterExtensionConfig, ClusterTopology, InprocSampleProvider,
};

// 静的トポロジを設定
let static_topology = ClusterTopology::new(
    1,                                           // ハッシュ値
    vec!["node-b".to_string()],                  // joined ノード
    vec![],                                      // left ノード
);

// Provider を作成
let provider = InprocSampleProvider::new(
    event_stream.clone(),
    block_list_provider,
    "node-a",
)
.with_static_topology(static_topology);

// ClusterExtension を登録
let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new()
        .with_advertised_address("node-a")
        .with_metrics_enabled(true),
    ArcShared::new(provider),
    block_list_provider,
    gossiper,
    pubsub,
    identity_lookup,
);

let ext = system.extended().register_extension(&ext_id);
ext.start_member().unwrap();

// start_member() 時に Provider が TopologyUpdated を EventStream に自動 publish
// ClusterExtension が購読しているので自動的に apply_topology が呼ばれる
```

### 実行例（no_std）

```bash
cargo run -p fraktor-cluster-rs --example cluster_extension_no_std --features test-support
```

### 期待される出力

```
=== Cluster Extension No-Std Demo ===
Demonstrates EventStream-based topology with InprocSampleProvider
(No manual on_topology calls - topology is automatically published)

--- Starting cluster members ---
[identity] setup_member: ["grain", "topic"]
[node-a] cluster started (mode=Member)
[node-a] topology updated: joined=["node-b"], left=[]
[node-b] cluster started (mode=Member)
[node-b] topology updated: joined=["node-a"], left=[]

--- Checking metrics after startup ---
[node-a] members=2, virtual_actors=2
[node-b] members=2, virtual_actors=2

--- Sending grain message ---
[grain] recv: hello from node-a

--- Shutting down ---
[node-b] cluster shutdown
[node-a] cluster shutdown

=== Demo complete ===
```

## Phase2: 動的トポロジ + Tokio TCP

### SampleTcpProvider（std 環境向け）

`SampleTcpProvider` は Remoting/TokioTcpTransport と連携し、Transport イベントを自動検知してトポロジを更新するサンプル Provider です。

```rust
use fraktor_cluster_rs::std::sample_tcp_provider::SampleTcpProvider;

// Provider を作成
let provider = SampleTcpProvider::new(
    event_stream.clone(),
    block_list_provider,
    &format!("{}:{}", HOST, port),
)
.with_static_topology(static_topology);

let provider = ArcShared::new(provider);

// Transport イベントの自動検知を開始
// RemotingLifecycleEvent::Connected/Quarantined を監視し、
// 自動的に TopologyUpdated を publish する
SampleTcpProvider::subscribe_remoting_events(&provider);
```

### 実行例（Tokio）

```bash
cargo run -p fraktor-cluster-rs --example cluster_extension_tokio --features std
```

### 期待される出力

```
=== Cluster Extension Tokio Demo ===
Demonstrates EventStream-based topology with SampleTcpProvider

--- Starting cluster members ---
[identity][cluster-node-a] member kinds: ["grain", "topic"]
[pubsub][cluster-node-a] start
[gossip][cluster-node-a] start (no-op in Phase1)
[cluster][cluster-node-a] Startup { address: "127.0.0.1:26050", mode: Member }
[cluster][cluster-node-a] TopologyUpdated { topology: ClusterTopology { hash: 1, ... }, joined: ["127.0.0.1:26051"], left: [], blocked: [] }

--- Checking metrics after startup ---
[node-a] members=2, virtual_actors=2
[node-b] members=2, virtual_actors=2

--- Transport-driven topology updates enabled ---
(Connected/Quarantined events will automatically trigger TopologyUpdated)

--- Sending grain call ---
[hub] recv grain call key=user:va-1 body=hello cluster over tokio tcp
[grain] start
[ok] grain reply: echo:hello cluster over tokio tcp

--- Shutting down ---
[pubsub][cluster-node-b] stop
[gossip][cluster-node-b] stop
[cluster][cluster-node-b] Shutdown { address: "127.0.0.1:26051", mode: Member }

=== Demo complete ===
```

## Provider 差し替え方法

### 差し替え可能な Provider 一覧

| Provider | 環境 | 用途 |
|----------|------|------|
| `InprocSampleProvider` | no_std | 静的トポロジ、テスト用 |
| `SampleTcpProvider` | std/Tokio | 静的 + Transport イベント検知 |
| etcd provider | std | 本番環境（Phase2以降で対応予定） |
| zk provider | std | 本番環境（Phase2以降で対応予定） |
| automanaged provider | std | 本番環境（Phase2以降で対応予定） |

### カスタム Provider の実装

`ClusterProvider` トレイトを実装することで、独自の Provider を作成できます。

```rust
use fraktor_cluster_rs::core::{ClusterProvider, ClusterProviderError};

pub struct CustomProvider {
    event_stream: ArcShared<EventStreamGeneric<TB>>,
    // ...
}

impl ClusterProvider for CustomProvider {
    fn start_member(&self) -> Result<(), ClusterProviderError> {
        // 1. 外部サービス（etcd/zk など）に接続
        // 2. クラスタに参加
        // 3. 初期トポロジを EventStream に publish
        let event = ClusterEvent::TopologyUpdated {
            topology: initial_topology,
            joined: vec![...],
            left: vec![],
            blocked: vec![...],
        };
        let payload = AnyMessageGeneric::new(event);
        let ext_event = EventStreamEvent::Extension {
            name: String::from("cluster"),
            payload,
        };
        self.event_stream.publish(&ext_event);
        Ok(())
    }

    fn start_client(&self) -> Result<(), ClusterProviderError> {
        // クライアントモードでの起動
        Ok(())
    }

    fn shutdown(&self, graceful: bool) -> Result<(), ClusterProviderError> {
        // クリーンアップ処理
        Ok(())
    }
}
```

### トポロジ通知の契約

Provider は以下の契約に従ってトポロジを通知します：

1. **イベント形式**: `ClusterEvent::TopologyUpdated { topology, joined, left, blocked }` を `EventStreamEvent::Extension { name: "cluster", payload }` として publish
2. **重複抑止**: 同一ハッシュのトポロジは publish しない（ClusterCore 側でも抑止）
3. **BlockList**: `BlockListProvider` の結果を `blocked` フィールドに含める
4. **エラー処理**: 起動失敗時は `ClusterEvent::StartupFailed` を EventStream に発火

## イベントの種類

| イベント | 発火タイミング | 内容 |
|----------|----------------|------|
| `Startup` | `start_member()`/`start_client()` 成功時 | アドレスとモード |
| `StartupFailed` | 起動失敗時 | アドレス、モード、失敗理由 |
| `TopologyUpdated` | トポロジ変更時 | トポロジ全体、joined/left/blocked |
| `Shutdown` | `shutdown()` 成功時 | アドレスとモード |
| `ShutdownFailed` | シャットダウン失敗時 | アドレス、モード、失敗理由 |

## metrics 構成

### metrics 有効時

```rust
let config = ClusterExtensionConfig::new()
    .with_metrics_enabled(true);

let ext = system.extended().register_extension(&ext_id);
ext.start_member().unwrap();

// メトリクスを取得
let metrics = ext.metrics().unwrap();
println!("members={}, virtual_actors={}", metrics.members(), metrics.virtual_actors());
```

### metrics 無効時

```rust
let config = ClusterExtensionConfig::new()
    .with_metrics_enabled(false);

// metrics() は MetricsError::Disabled を返す
match ext.metrics() {
    Err(MetricsError::Disabled) => {
        // metrics は未収集
    }
    Ok(metrics) => { /* 通常処理 */ }
}

// 注意: metrics が無効でも EventStream へのイベント発行は継続する
```

## トラブルシューティング

### トポロジが更新されない

1. `start_member()` が呼ばれているか確認
2. EventStream subscriber が正しく登録されているか確認
3. Provider がトポロジを publish しているか確認
4. 同一ハッシュのトポロジが重複送信されていないか確認

### メトリクスが取得できない

1. `with_metrics_enabled(true)` で設定しているか確認
2. `MetricsError::Disabled` が返された場合は metrics が無効

### Transport イベントが検知されない（Phase2）

1. `SampleTcpProvider::subscribe_remoting_events()` が呼ばれているか確認
2. Remoting 拡張が正しく初期化されているか確認
3. Transport が接続/切断イベントを発行しているか確認

## 関連ドキュメント

- [設計ドキュメント](design.md) - アーキテクチャの詳細
- [要件ドキュメント](requirements.md) - 受け入れ条件
- [タスク一覧](tasks.md) - 実装計画

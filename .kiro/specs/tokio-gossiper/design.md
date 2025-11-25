# 技術設計: TokioGossiper

## 概要

本設計は、protoactor-go 互換の Gossiper を Rust + Tokio で実装するための技術アーキテクチャを定義する。既存の no_std 対応 `GossipEngine` を Tokio ランタイム上で駆動し、クラスタメンバー間でメンバーシップ状態を定期的に拡散・同期するコンポーネントを構築する。

### 参照実装

- protoactor-go: `cluster/gossiper.go`, `cluster/gossip_actor.go`, `cluster/gossip_state_management.go`
- 状態マージ戦略: LWW (Last-Writer-Wins) + SequenceNumber ベース

### 設計原則

1. **Hybrid Architecture**: `TokioGossiper`(std) が `GossipEngine`(no_std) をラップ
2. **既存コンポーネント再利用**: `AsyncQueue`（fraktor-utils-rs）を使用
3. **protoactor-go 互換性**: API と動作を protoactor-go に合わせる
4. **Send + Sync**: マルチスレッド環境での安全な共有を保証

### 重要: API スタイルに関する注意

**core トレイトは同期 API、std 実装は内部で非同期駆動**

本プロジェクトでは、組み込み環境（no_std）との互換性を維持するため、core 層のトレイトは `async fn` を使用しません。代わりに以下のパターンを採用します：

| レイヤー | API スタイル | 理由 |
|---------|-------------|------|
| **core (no_std)** | 同期トレイト（`fn`） | 組み込み環境対応、カスタム Future で駆動可能 |
| **std (Tokio)** | 内部で `tokio::spawn` | Tokio ランタイム上で非同期タスクとして動作 |

このパターンは `TokioTcpTransport` などの既存実装と一貫しています：

```rust
// TokioTcpTransport の例（modules/remote/src/std/transport/tokio_tcp.rs:149）
tokio::spawn(async move {
    if let Err(e) = Self::handle_inbound(stream, ...).await {
        eprintln!("Inbound connection error: {e:?}");
    }
});
```

**TokioGossiper も同様のパターンで実装します：**

- `Gossiper` トレイト: 同期メソッド（`fn start`, `fn stop`, `fn set_state` など）
- `TokioGossiper` 実装: 内部で `tokio::spawn` を使用してゴシップループを駆動
- 状態取得: `AsyncQueue` 経由でコマンド送信、レスポンス待機

---

## アーキテクチャ

### コンポーネント構成図

```
┌─────────────────────────────────────────────────────────────────┐
│                         ClusterCore                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    TokioGossiper (std)                    │  │
│  │  ┌─────────────────┐  ┌────────────────────────────────┐ │  │
│  │  │ GossipLoop      │  │ GossipStateStore               │ │  │
│  │  │ (tokio::task)   │  │ (key-value + member states)    │ │  │
│  │  └────────┬────────┘  └───────────────┬────────────────┘ │  │
│  │           │                           │                   │  │
│  │  ┌────────▼──────────────────────────▼─────────────────┐ │  │
│  │  │             GossipEngine (no_std core)              │ │  │
│  │  │  - MembershipTable                                  │ │  │
│  │  │  - Peer management                                  │ │  │
│  │  │  - State machine (Diffusing/Reconciling/Confirmed)  │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────▼───────────────────────────────┐  │
│  │                     AsyncQueue                            │  │
│  │  (fraktor-utils-rs: command/response message passing)     │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │     Remote Transport Layer    │
              │   (GossipRequest/Response)    │
              └───────────────────────────────┘
```

### レイヤー構成

| レイヤー | コンポーネント | 責務 |
|---------|---------------|------|
| Std | `TokioGossiper` | Tokio 統合、ライフサイクル管理、API 提供 |
| Std | `GossipLoop` | 定期的ゴシップ送信、ハートビート管理 |
| Std | `GossipStateStore` | LWW + SeqNo 状態管理、コンセンサスチェック |
| Core | `GossipEngine` | メンバーシップ拡散、状態機械 |
| Utils | `AsyncQueue` | コマンド/レスポンスメッセージパッシング |

---

## コンポーネント詳細設計

### 1. TokioGossiper

#### 構造体定義

```rust
/// Tokio ランタイム上で動作する Gossiper 実装
pub struct TokioGossiper<TB: RuntimeToolbox> {
    /// 設定
    config: TokioGossiperConfig,

    /// 内部状態（ToolboxMutex で保護）
    inner: ToolboxMutex<TB, TokioGossiperInner<TB>>,

    /// ゴシップループ停止シグナル
    shutdown_tx: ToolboxMutex<TB, Option<oneshot::Sender<()>>>,

    /// 実行状態
    state: AtomicU8,  // 0: Stopped, 1: Running, 2: ShuttingDown
}

struct TokioGossiperInner<TB: RuntimeToolbox> {
    /// 状態ストア（LWW + SeqNo）
    state_store: GossipStateStore<TB>,

    /// 既存の GossipEngine
    engine: GossipEngine,

    /// 現在のピアリスト
    peers: Vec<String>,

    /// ブロックリスト
    blocked_members: BTreeSet<String>,

    /// コンセンサスチェッカー
    consensus_checkers: BTreeMap<String, ConsensusChecker>,

    /// EventStream 参照
    event_stream: EventStreamRef<TB>,

    /// シーケンス番号（自ノード用）
    sequence_number: u64,
}
```

#### トレイト実装

```rust
/// Gossiper トレイト（インターフェース定義）
///
/// 注意: core 層のトレイトは同期 API です。
/// std 実装（TokioGossiper）は内部で tokio::spawn を使用して非同期駆動します。
pub trait Gossiper: Send + Sync {
    /// ゴシップを開始
    ///
    /// TokioGossiper では内部で tokio::spawn してゴシップループを起動します。
    fn start(&self) -> Result<(), GossiperError>;

    /// ゴシップを停止
    ///
    /// 進行中のゴシップ送信の完了を待機してから停止します。
    fn stop(&self) -> Result<(), GossiperError>;

    /// 状態を取得（キーに対する全メンバーの値）
    ///
    /// TokioGossiper では AsyncQueue 経由でリクエストを送信し、
    /// レスポンスを待機します。
    fn get_state(&self, key: &str) -> Result<BTreeMap<String, GossipKeyValue>, GossiperError>;

    /// 状態を設定（Fire-and-forget）
    ///
    /// AsyncQueue にコマンドを送信して即座に戻ります。
    fn set_state(&self, key: &str, value: &[u8]);

    /// 状態を設定（完了を待機）
    ///
    /// 設定が内部状態に反映されるまで待機します。
    fn set_state_request(&self, key: &str, value: &[u8]) -> Result<(), GossiperError>;

    /// Map 状態の操作
    fn set_map_state(&self, state_key: &str, map_key: &str, value: &[u8]);
    fn get_map_state(&self, state_key: &str, map_key: &str) -> Option<Vec<u8>>;
    fn remove_map_state(&self, state_key: &str, map_key: &str);
    fn get_map_keys(&self, state_key: &str) -> Vec<String>;

    /// コンセンサスチェック登録
    fn register_consensus_check(
        &self,
        key: &str,
        extractor: Box<dyn Fn(&[u8]) -> Result<u64, GossiperError> + Send + Sync>,
    ) -> ConsensusHandle;

    /// コンセンサスチェック削除
    fn remove_consensus_check(&self, id: &str);

    /// ブロック済みメンバー取得
    fn get_blocked_members(&self) -> Vec<String>;
}
```

#### TokioGossiper 実装パターン

```rust
impl<TB: RuntimeToolbox + 'static> Gossiper for TokioGossiper<TB> {
    fn start(&self) -> Result<(), GossiperError> {
        // 状態チェック: 既に開始済みならエラー
        if self.is_running() {
            return Err(GossiperError::AlreadyStarted);
        }

        // shutdown チャネルを作成
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.lock() = Some(shutdown_tx);

        // tokio::spawn でゴシップループを起動
        let gossiper = self.clone();
        let config = self.config.clone();
        tokio::spawn(async move {
            gossiper.gossip_loop(shutdown_rx, config).await;
        });

        self.set_running(true);
        Ok(())
    }

    fn stop(&self) -> Result<(), GossiperError> {
        // 状態チェック: 開始していないならエラー
        if !self.is_running() {
            return Err(GossiperError::NotStarted);
        }

        // shutdown シグナルを送信
        if let Some(tx) = self.shutdown_tx.lock().take() {
            let _ = tx.send(());
        }

        self.set_running(false);
        Ok(())
    }

    fn get_state(&self, key: &str) -> Result<BTreeMap<String, GossipKeyValue>, GossiperError> {
        // AsyncQueue 経由でリクエスト送信、レスポンス待機
        let inner = self.inner.lock();
        Ok(inner.state_store.get_state(key))
    }

    fn set_state(&self, key: &str, value: &[u8]) {
        // AsyncQueue にコマンド送信（fire-and-forget）
        let mut inner = self.inner.lock();
        inner.state_store.set_local_state(key, value.to_vec(), &mut inner.sequence_number);
    }
}
```

### 2. GossipStateStore

#### 構造体定義

```rust
/// LWW + SequenceNumber ベースの状態ストア
pub struct GossipStateStore<TB: RuntimeToolbox> {
    /// メンバーごとの状態
    /// key: member_id, value: GossipMemberState
    members: BTreeMap<String, GossipMemberState>,

    /// 自ノードの member_id
    local_member_id: String,

    /// Toolbox 参照
    _marker: PhantomData<TB>,
}

/// 単一メンバーの状態
pub struct GossipMemberState {
    /// キーごとの値
    /// key: state_key, value: GossipKeyValue
    values: BTreeMap<String, GossipKeyValue>,
}

/// 状態値（protoactor-go の GossipKeyValue 相当）
pub struct GossipKeyValue {
    /// シリアライズされた値
    pub value: Vec<u8>,

    /// シーケンス番号（LWW 判定用）
    pub sequence_number: u64,

    /// ローカルタイムスタンプ（Unix ミリ秒）
    pub local_timestamp_unix_milliseconds: i64,
}
```

#### 状態マージアルゴリズム

```rust
impl<TB: RuntimeToolbox> GossipStateStore<TB> {
    /// リモート状態をマージし、更新イベントを返す
    pub fn merge_state(&mut self, remote_state: &GossipState) -> Vec<GossipUpdate> {
        let mut updates = Vec::new();
        let now = current_timestamp_millis();

        for (member_id, remote_member_state) in &remote_state.members {
            let local_member = self.members
                .entry(member_id.clone())
                .or_insert_with(GossipMemberState::new);

            for (key, remote_value) in &remote_member_state.values {
                let should_update = match local_member.values.get(key) {
                    Some(local_value) => {
                        // LWW: リモートの SeqNo が大きい場合のみ更新
                        remote_value.sequence_number > local_value.sequence_number
                    }
                    None => true, // ローカルに存在しない場合は追加
                };

                if should_update {
                    let mut updated_value = remote_value.clone();
                    updated_value.local_timestamp_unix_milliseconds = now;
                    local_member.values.insert(key.clone(), updated_value);

                    updates.push(GossipUpdate {
                        member_id: member_id.clone(),
                        key: key.clone(),
                        value: remote_value.value.clone(),
                        seq_number: remote_value.sequence_number,
                    });
                }
            }
        }

        updates
    }

    /// 自ノードの状態を設定
    pub fn set_local_state(&mut self, key: &str, value: Vec<u8>, seq_no: &mut u64) {
        let member = self.members
            .entry(self.local_member_id.clone())
            .or_insert_with(GossipMemberState::new);

        *seq_no += 1;
        member.values.insert(key.to_string(), GossipKeyValue {
            value,
            sequence_number: *seq_no,
            local_timestamp_unix_milliseconds: current_timestamp_millis(),
        });
    }
}
```

### 3. GossipLoop

#### ゴシップループ実装

```rust
/// ゴシップループタスク
async fn gossip_loop<TB: RuntimeToolbox>(
    gossiper: Arc<TokioGossiper<TB>>,
    mut shutdown_rx: oneshot::Receiver<()>,
    config: TokioGossiperConfig,
    cluster: Arc<Cluster<TB>>,
) {
    let mut interval = tokio::time::interval(config.gossip_interval);

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("Gossip loop shutting down");
                break;
            }
            _ = interval.tick() => {
                // 1. 期限切れハートビートをブロック
                gossiper.block_expired_heartbeats().await;

                // 2. gracefully left メンバーをブロック
                gossiper.block_gracefully_left().await;

                // 3. 自ノードのハートビートを設定
                let heartbeat = MemberHeartbeat {
                    actor_statistics: gossiper.get_actor_count(),
                };
                gossiper.set_state(HEARTBEAT_KEY, &heartbeat.encode());

                // 4. 状態を送信
                gossiper.send_state().await;
            }
        }
    }

    // シャットダウン時: gracefully left を設定
    gossiper.set_state(GRACEFULLY_LEFT_KEY, &[]);
}
```

### 4. ネットワーク通信

#### メッセージ定義

```rust
/// ゴシップリクエスト（protoactor-go の GossipRequest 相当）
pub struct GossipRequest {
    /// 送信元 member_id
    pub from_member_id: String,

    /// 状態ペイロード
    pub state: GossipState,
}

/// ゴシップレスポンス
pub struct GossipResponse {
    /// 応答状態（オプション）
    pub state: Option<GossipState>,
}

/// ゴシップ状態全体
pub struct GossipState {
    /// メンバーごとの状態
    pub members: BTreeMap<String, GossipMemberState>,
}
```

#### 送信ロジック

```rust
impl<TB: RuntimeToolbox> TokioGossiper<TB> {
    /// 状態を全ピアに送信
    async fn send_state(&self) {
        let inner = self.inner.lock();
        let members_to_send = self.select_peers_for_gossip(&inner);
        drop(inner);

        for member in members_to_send.iter().take(self.config.gossip_fan_out) {
            let state_delta = self.get_member_state_delta(&member.id);
            if state_delta.has_state {
                self.send_gossip_for_member(member, state_delta).await;
            }
        }
    }

    /// 単一メンバーへのゴシップ送信
    async fn send_gossip_for_member(
        &self,
        member: &Member,
        state_delta: MemberStateDelta,
    ) {
        let request = GossipRequest {
            from_member_id: self.local_member_id.clone(),
            state: state_delta.state,
        };

        let timeout = self.config.gossip_request_timeout;

        match tokio::time::timeout(timeout, self.send_remote(member, request)).await {
            Ok(Ok(response)) => {
                state_delta.commit_offsets();
                if let Some(remote_state) = response.state {
                    self.receive_state(&remote_state);
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("Failed to send gossip to {}: {:?}", member.id, e);
            }
            Err(_) => {
                tracing::warn!("Gossip request to {} timed out", member.id);
            }
        }
    }
}
```

### 5. ハートビート管理

#### 定数定義

```rust
/// ハートビートキー（protoactor-go 互換）
pub const HEARTBEAT_KEY: &str = "heartbeat";

/// gracefully left キー
pub const GRACEFULLY_LEFT_KEY: &str = "gracefully_left";
```

#### ハートビート構造

```rust
/// メンバーハートビート
pub struct MemberHeartbeat {
    /// アクター統計（Kind ごとのアクター数）
    pub actor_statistics: BTreeMap<String, i64>,
}

impl MemberHeartbeat {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }
}
```

#### 期限切れ検出

```rust
impl<TB: RuntimeToolbox> TokioGossiper<TB> {
    /// 期限切れハートビートを持つメンバーをブロック
    async fn block_expired_heartbeats(&self) {
        let heartbeats = self.get_state(HEARTBEAT_KEY).await.unwrap_or_default();
        let now = current_timestamp_millis();
        let expiration = self.config.heartbeat_expiration.as_millis() as i64;

        let mut to_block = Vec::new();

        for (member_id, value) in heartbeats {
            if member_id == self.local_member_id {
                continue; // 自ノードはスキップ
            }

            if !self.is_blocked(&member_id) {
                let elapsed = now - value.local_timestamp_unix_milliseconds;
                if elapsed > expiration {
                    to_block.push(member_id);
                }
            }
        }

        if !to_block.is_empty() {
            tracing::info!("Blocking members due to expired heartbeat: {:?}", to_block);
            self.block_members(&to_block);
        }
    }
}
```

### 6. ClusterTopology イベント対応

```rust
impl<TB: RuntimeToolbox> TokioGossiper<TB> {
    /// ClusterTopology イベントを購読
    fn subscribe_topology_events(&self) {
        let gossiper = self.clone();
        self.event_stream.subscribe(move |event| {
            if let EventStreamEvent::ClusterTopology(topology) = event {
                gossiper.update_cluster_topology(&topology);
            }
        });
    }

    /// トポロジー更新
    fn update_cluster_topology(&self, topology: &ClusterTopology) {
        let mut inner = self.inner.lock();

        // 新規メンバーを追加
        let new_members: Vec<_> = topology.members.iter()
            .filter(|m| !inner.peers.contains(&m.id))
            .collect();

        for member in new_members {
            inner.peers.push(member.id.clone());
        }

        // 離脱メンバーを除外
        let member_ids: BTreeSet<_> = topology.members.iter()
            .map(|m| &m.id)
            .collect();
        inner.peers.retain(|p| member_ids.contains(p));

        // GossipEngine のピアリストも更新
        inner.engine = GossipEngine::new(
            inner.engine.table().clone(),
            inner.peers.clone(),
        );
    }
}
```

### 7. コンセンサスチェック

```rust
/// コンセンサスチェッカー
pub struct ConsensusChecker {
    /// チェック対象キー
    key: String,

    /// 値抽出関数
    extractor: Box<dyn Fn(&[u8]) -> Result<u64, GossiperError> + Send + Sync>,

    /// コンセンサス達成時のコールバック
    callback: Option<Box<dyn FnOnce() + Send + 'static>>,
}

/// コンセンサスハンドル
pub struct ConsensusHandle {
    id: String,
    gossiper: Weak<dyn Gossiper>,
}

impl ConsensusHandle {
    /// コンセンサスが達成されるまで待機
    pub async fn wait_for_consensus(&self, timeout: Duration) -> Result<(), GossiperError> {
        // 実装: 定期的にコンセンサス状態をチェック
    }

    /// コンセンサスチェックを削除
    pub fn cancel(&self) {
        if let Some(gossiper) = self.gossiper.upgrade() {
            gossiper.remove_consensus_check(&self.id);
        }
    }
}
```

### 8. 設定

```rust
/// TokioGossiper 設定
#[derive(Debug, Clone)]
pub struct TokioGossiperConfig {
    /// ゴシップ間隔（デフォルト: 500ms）
    pub gossip_interval: Duration,

    /// リクエストタイムアウト（デフォルト: 2s）
    pub gossip_request_timeout: Duration,

    /// 同時送信先数（デフォルト: 3）
    pub gossip_fan_out: usize,

    /// 1回の送信での最大状態数（デフォルト: 50）
    pub gossip_max_send: usize,

    /// ハートビート期限（デフォルト: 10s）
    pub heartbeat_expiration: Duration,

    /// Gossiper アクター名（デフォルト: "gossip"）
    pub gossip_actor_name: String,
}

impl Default for TokioGossiperConfig {
    fn default() -> Self {
        Self {
            gossip_interval: Duration::from_millis(500),
            gossip_request_timeout: Duration::from_secs(2),
            gossip_fan_out: 3,
            gossip_max_send: 50,
            heartbeat_expiration: Duration::from_secs(10),
            gossip_actor_name: "gossip".to_string(),
        }
    }
}
```

---

## ファイル構成

**注意**: 本プロジェクトでは 2018 エディションのモジュール構成を使用します。`mod.rs` は禁止です。

```
modules/cluster/src/
├── core.rs                        # core モジュールのエントリポイント
├── core/
│   ├── gossip_engine.rs           # 既存（変更なし）
│   ├── gossip_state.rs            # 既存（変更なし）
│   ├── gossip_event.rs            # 既存（変更なし）
│   ├── gossip_outbound.rs         # 既存（変更なし）
│   ├── gossip_key_value.rs        # 新規: GossipKeyValue 構造体
│   ├── gossip_member_state.rs     # 新規: GossipMemberState 構造体
│   ├── gossip_update.rs           # 新規: GossipUpdate イベント
│   ├── gossiper.rs                # 新規: Gossiper トレイト定義
│   ├── gossiper_config.rs         # 新規: 設定構造体
│   └── gossiper_error.rs          # 新規: エラー型
│
├── std.rs                         # std モジュールのエントリポイント（tokio_gossiper を追加）
└── std/
    ├── tokio_gossiper.rs          # 新規: TokioGossiper 実装
    └── tokio_gossiper/
        ├── tests.rs               # 新規: テスト
        ├── gossip_state_store.rs  # 新規: 状態ストア
        ├── gossip_loop.rs         # 新規: ゴシップループ
        ├── consensus_checker.rs   # 新規: コンセンサスチェック
        └── member_heartbeat.rs    # 新規: ハートビート
```

---

## 依存関係

### 既存コンポーネント（再利用）

| コンポーネント | 場所 | 用途 |
|---------------|------|------|
| `GossipEngine` | `modules/cluster/src/core/gossip_engine.rs` | メンバーシップ拡散コア |
| `AsyncQueue` | `modules/utils/src/std/collections/async_queue.rs` | メッセージパッシング |
| `EventStream` | `modules/actor/src/core/event_stream.rs` | イベント配信 |
| `ToolboxMutex` | `modules/utils/src/core/sync/` | スレッドセーフなロック |

### 外部クレート

| クレート | バージョン | 用途 |
|---------|-----------|------|
| `tokio` | 1.x | ランタイム、タイマー、非同期 |
| `postcard` | 1.x | シリアライズ |
| `tracing` | 0.1 | ログ |

---

## テスト計画

### 単体テスト

1. **GossipStateStore テスト**
   - `merge_state`: LWW マージロジック
   - `set_local_state`: シーケンス番号インクリメント
   - エッジケース: 同じ SeqNo、空状態

2. **TokioGossiper テスト**
   - `start/stop`: ライフサイクル
   - `get_state/set_state`: 状態操作
   - 二重開始/停止エラー

3. **ハートビートテスト**
   - 期限切れ検出
   - 自ノード除外
   - ブロックリスト追加

### 統合テスト

1. **マルチノードゴシップ**
   - 3ノードクラスタでの状態収束
   - ノード離脱時の動作

2. **障害シナリオ**
   - タイムアウト処理
   - ネットワーク分断

---

## 実装フェーズ

### Phase 1: 基本構造（R1, R2, R3, R9, R12）
- TokioGossiper 構造体
- Gossiper トレイト
- ゴシップループ
- 設定と初期化
- Graceful shutdown

### Phase 2: 状態管理（R4, R5, R6, R7, R10）
- GossipStateStore
- LWW マージ
- ハートビート
- ClusterTopology 対応
- ブロックメンバー管理

### Phase 3: ネットワーク（R8）
- GossipRequest/Response
- リモート送信
- タイムアウト処理

### Phase 4: コンセンサス（R11）
- ConsensusChecker
- ConsensusHandle
- コンセンサス待機

---

## 制約と前提

1. **protoactor-go 互換**: API 名と動作を可能な限り合わせる
2. **no_std コア維持**: `GossipEngine` は変更しない
3. **AsyncQueue 使用**: 独自キュー実装禁止
4. **Send + Sync**: マルチスレッド安全
5. **LWW + SeqNo**: CRDT ではなく単純な SeqNo 比較

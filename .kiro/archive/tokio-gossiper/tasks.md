# 実装タスク: TokioGossiper

## 概要

本ドキュメントは tokio-gossiper 機能の実装タスクを定義する。設計ドキュメントに基づき、4フェーズに分割して実装を進める。

---

## Phase 1: 基本構造（R1, R2, R3, R9, R12）

### タスク 1.1: Gossiper トレイトの拡張と &mut self 化

**要件**: R1（TokioGossiper 構造体と Gossiper トレイト実装）

**説明**: 既存の `Gossiper` トレイトを拡張し、状態操作メソッドを追加する。ホットパス系メソッドは `&mut self` を使用する。

**ファイル**:
- `modules/cluster/src/core/gossiper.rs`

**実装内容**:
1. 既存の `start(&self)` と `stop(&self)` はライフサイクル系なので `&self` のまま
2. 以下のメソッドを追加（ホットパス系は `&mut self`）:
   - `fn get_state(&self, key: &str) -> Result<...>`（読み取りのみなので `&self`）
   - `fn set_state(&mut self, key: &str, value: &[u8])`
   - `fn set_state_request(&mut self, key: &str, value: &[u8]) -> Result<...>`
   - `fn set_map_state(&mut self, state_key: &str, map_key: &str, value: &[u8])`
   - `fn get_map_state(&self, state_key: &str, map_key: &str) -> Option<Vec<u8>>`
   - `fn remove_map_state(&mut self, state_key: &str, map_key: &str)`
   - `fn get_map_keys(&self, state_key: &str) -> Vec<String>`
   - `fn get_blocked_members(&self) -> Vec<String>`

**テスト**: なし（トレイト定義のみ）

**依存**: なし

---

### タスク 1.2: GossiperError エラー型の作成

**要件**: R1, R12

**説明**: Gossiper 操作で発生するエラーを表すエラー型を作成する。

**ファイル**:
- `modules/cluster/src/core/gossiper_error.rs`
- `modules/cluster/src/core.rs`（モジュール追加）

**実装内容**:
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GossiperError {
    AlreadyStarted,
    NotStarted,
    AlreadyStopped,
    InvalidConfig(&'static str),
    StateNotFound,
    Timeout,
    NetworkError,
}
```

**テスト**: `modules/cluster/src/core/gossiper_error/tests.rs`

**依存**: なし

---

### タスク 1.3: GossiperConfig 設定構造体の作成

**要件**: R12（設定と初期化）

**説明**: TokioGossiper の設定を保持する構造体を作成する。

**ファイル**:
- `modules/cluster/src/core/gossiper_config.rs`
- `modules/cluster/src/core.rs`（モジュール追加）

**実装内容**:
```rust
pub struct GossiperConfig {
    pub gossip_interval_ms: u64,          // デフォルト: 500
    pub gossip_request_timeout_ms: u64,   // デフォルト: 2000
    pub gossip_fan_out: usize,            // デフォルト: 3
    pub gossip_max_send: usize,           // デフォルト: 50
    pub heartbeat_expiration_ms: u64,     // デフォルト: 10000
    pub gossip_actor_name: String,        // デフォルト: "gossip"
}
```

**テスト**: `modules/cluster/src/core/gossiper_config/tests.rs`
- デフォルト値のテスト
- バリデーションのテスト

**依存**: なし

---

### タスク 1.4: GossipKeyValue 構造体の作成

**要件**: R4, R5（状態管理）

**説明**: protoactor-go の GossipKeyValue 相当の構造体を作成する。

**ファイル**:
- `modules/cluster/src/core/gossip_key_value.rs`
- `modules/cluster/src/core.rs`（モジュール追加）

**実装内容**:
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipKeyValue {
    pub value: Vec<u8>,
    pub sequence_number: u64,
    pub local_timestamp_unix_milliseconds: i64,
}
```

**テスト**: `modules/cluster/src/core/gossip_key_value/tests.rs`

**依存**: なし

---

### タスク 1.5: GossipMemberState 構造体の作成

**要件**: R4, R5（状態管理）

**説明**: メンバーごとの状態を保持する構造体を作成する。

**ファイル**:
- `modules/cluster/src/core/gossip_member_state.rs`
- `modules/cluster/src/core.rs`（モジュール追加）

**実装内容**:
```rust
pub struct GossipMemberState {
    values: BTreeMap<String, GossipKeyValue>,
}
```

**テスト**: `modules/cluster/src/core/gossip_member_state/tests.rs`

**依存**: タスク 1.4

---

### タスク 1.6: GossipUpdate イベント構造体の作成

**要件**: R5（状態マージ）

**説明**: 状態更新イベントを表す構造体を作成する。

**ファイル**:
- `modules/cluster/src/core/gossip_update.rs`
- `modules/cluster/src/core.rs`（モジュール追加）

**実装内容**:
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipUpdate {
    pub member_id: String,
    pub key: String,
    pub value: Vec<u8>,
    pub seq_number: u64,
}
```

**テスト**: `modules/cluster/src/core/gossip_update/tests.rs`

**依存**: なし

---

### タスク 1.7: TokioGossiper 構造体の作成

**要件**: R1, R12

**説明**: TokioGossiper の基本構造体を作成する（メソッド実装は後続タスク）。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper.rs`
- `modules/cluster/src/std.rs`（モジュール追加）

**実装内容**:
```rust
pub struct TokioGossiper<TB: RuntimeToolbox> {
    config: GossiperConfig,
    inner: ToolboxMutex<TB, TokioGossiperInner<TB>>,
    shutdown_tx: ToolboxMutex<TB, Option<oneshot::Sender<()>>>,
    state: AtomicU8,
}
```

**テスト**: `modules/cluster/src/std/tokio_gossiper/tests.rs`
- 構造体の生成テスト

**依存**: タスク 1.1, 1.2, 1.3

---

### タスク 1.8: TokioGossiper の start/stop 実装

**要件**: R1, R9（ライフサイクル）

**説明**: ゴシップの開始・停止メソッドを実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper.rs`

**実装内容**:
1. `start(&self)`: `tokio::spawn` でゴシップループを起動
2. `stop(&self)`: shutdown シグナルを送信してループを停止
3. 二重開始/停止のエラーハンドリング

**テスト**: `modules/cluster/src/std/tokio_gossiper/tests.rs`
- start → stop の正常フロー
- 二重 start のエラー
- 未開始 stop のエラー

**依存**: タスク 1.7

---

### タスク 1.9: ゴシップループの実装

**要件**: R2（定期的なゴシップ送信ループ）

**説明**: `tokio::time::interval` を使用した定期的なゴシップループを実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper/gossip_loop.rs`

**実装内容**:
```rust
async fn gossip_loop(
    gossiper: Arc<TokioGossiper<TB>>,
    shutdown_rx: oneshot::Receiver<()>,
    config: GossiperConfig,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(config.gossip_interval_ms));
    loop {
        tokio::select! {
            _ = shutdown_rx => break,
            _ = interval.tick() => {
                // ゴシップ処理（後続タスクで実装）
            }
        }
    }
}
```

**テスト**: `modules/cluster/src/std/tokio_gossiper/tests.rs`
- ループの開始・停止
- interval の動作確認

**依存**: タスク 1.8

---

## Phase 2: 状態管理（R4, R5, R6, R7, R10）

### タスク 2.1: GossipStateStore の作成

**要件**: R4, R5

**説明**: LWW + SequenceNumber ベースの状態ストアを実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper/gossip_state_store.rs`

**実装内容**:
1. `GossipStateStore` 構造体
2. `get_state(&self, key: &str)` メソッド
3. `set_local_state(&mut self, key: &str, value: Vec<u8>, seq_no: &mut u64)` メソッド

**テスト**: `modules/cluster/src/std/tokio_gossiper/gossip_state_store/tests.rs`
- 状態の設定・取得
- シーケンス番号のインクリメント

**依存**: タスク 1.4, 1.5

---

### タスク 2.2: 状態マージ（LWW + SeqNo）の実装

**要件**: R5（状態マージ）

**説明**: リモート状態とローカル状態のマージロジックを実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper/gossip_state_store.rs`

**実装内容**:
```rust
pub fn merge_state(&mut self, remote_state: &GossipState) -> Vec<GossipUpdate> {
    // リモートの SeqNo > ローカルの SeqNo の場合のみ更新
}
```

**テスト**:
- リモート SeqNo が大きい場合: 更新される
- リモート SeqNo が小さい/同じ場合: 無視される
- 新規キーの追加
- GossipUpdate イベントの生成

**依存**: タスク 2.1, 1.6

---

### タスク 2.3: get_state/set_state の実装

**要件**: R4（状態の取得・設定操作）

**説明**: Gossiper トレイトの状態操作メソッドを実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper.rs`

**実装内容**:
1. `get_state(&self, key: &str)`: 内部状態ストアから取得
2. `set_state(&mut self, key: &str, value: &[u8])`: 状態を設定
3. `set_state_request(&mut self, ...)`: 設定完了を待機

**テスト**:
- 状態の設定と取得
- 存在しないキーの取得

**依存**: タスク 2.1

---

### タスク 2.4: MemberHeartbeat 構造体の作成

**要件**: R6（ハートビート管理）

**説明**: メンバーハートビートを表す構造体を作成する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper/member_heartbeat.rs`

**実装内容**:
```rust
pub struct MemberHeartbeat {
    pub actor_statistics: BTreeMap<String, i64>,
}

impl MemberHeartbeat {
    pub fn encode(&self) -> Vec<u8>;
    pub fn decode(bytes: &[u8]) -> Option<Self>;
}
```

**テスト**:
- エンコード/デコードのラウンドトリップ

**依存**: なし

---

### タスク 2.5: ハートビート更新の実装

**要件**: R6（ハートビート管理）

**説明**: ゴシップループ内でハートビートを定期的に更新する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper/gossip_loop.rs`
- `modules/cluster/src/std/tokio_gossiper.rs`

**実装内容**:
1. `HEARTBEAT_KEY` 定数の定義
2. ゴシップループ内で `set_state(HEARTBEAT_KEY, heartbeat.encode())` を呼び出し
3. `get_actor_count()` メソッドの実装

**テスト**:
- ハートビートが定期的に更新されることを確認

**依存**: タスク 2.3, 2.4

---

### タスク 2.6: 期限切れハートビート検出の実装

**要件**: R6（ハートビート管理）

**説明**: HeartbeatExpiration を超えたメンバーを検出してブロックする。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper.rs`

**実装内容**:
```rust
fn block_expired_heartbeats(&mut self) {
    // ハートビート状態を取得
    // 期限切れメンバーを検出
    // ブロックリストに追加（自ノードは除外）
}
```

**テスト**:
- 期限切れメンバーがブロックされる
- 自ノードはブロックされない
- 期限内メンバーはブロックされない

**依存**: タスク 2.5

---

### タスク 2.7: ClusterTopology イベント対応の実装

**要件**: R7（ClusterTopology イベントへの対応）

**説明**: ClusterTopology イベントを購読してピアリストを更新する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper.rs`

**実装内容**:
1. EventStream を購読
2. ClusterTopology イベント受信時にピアリストを更新
3. 新規メンバーの追加、離脱メンバーの除外

**テスト**:
- 新規メンバー追加時のピアリスト更新
- メンバー離脱時のピアリスト更新

**依存**: タスク 1.7

---

### タスク 2.8: ブロックメンバー管理の実装

**要件**: R10（ブロックメンバー管理）

**説明**: ブロックされたメンバーの管理機能を実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper.rs`

**実装内容**:
1. `blocked_members: BTreeSet<String>` の管理
2. `block_members(&mut self, members: &[String])` メソッド
3. `is_blocked(&self, member_id: &str) -> bool` メソッド
4. `get_blocked_members(&self) -> Vec<String>` メソッド
5. `GRACEFULLY_LEFT_KEY` を持つメンバーのブロック

**テスト**:
- メンバーのブロック追加
- ブロック済みメンバーの確認
- 重複ブロックの無視

**依存**: タスク 2.3

---

## Phase 3: ネットワーク（R8）

### タスク 3.1: GossipRequest/Response メッセージの作成

**要件**: R8（ネットワーク通信）

**説明**: ゴシップ通信用のメッセージ構造体を作成する。

**ファイル**:
- `modules/cluster/src/core/gossip_request.rs`
- `modules/cluster/src/core/gossip_response.rs`
- `modules/cluster/src/core.rs`（モジュール追加）

**実装内容**:
```rust
pub struct GossipRequest {
    pub from_member_id: String,
    pub state: GossipState,
}

pub struct GossipResponse {
    pub state: Option<GossipState>,
}

pub struct GossipState {
    pub members: BTreeMap<String, GossipMemberState>,
}
```

**テスト**:
- シリアライズ/デシリアライズ

**依存**: タスク 1.5

---

### タスク 3.2: ゴシップ送信の実装

**要件**: R8（ネットワーク通信）

**説明**: リモートピアへのゴシップ送信を実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper.rs`

**実装内容**:
1. `send_state(&self)` メソッド: FanOut 数のピアを選択して送信
2. `send_gossip_for_member(&self, member, state_delta)` メソッド
3. タイムアウト処理（`GossipRequestTimeout`）

**テスト**:
- 正常送信
- タイムアウト処理

**依存**: タスク 3.1, 2.1

---

### タスク 3.3: ゴシップ受信の実装

**要件**: R8（ネットワーク通信）

**説明**: リモートピアからのゴシップ受信と応答を実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper.rs`

**実装内容**:
1. `receive_state(&mut self, remote_state: &GossipState)` メソッド
2. 状態マージの呼び出し
3. GossipUpdate イベントの EventStream 発行

**テスト**:
- リモート状態の受信とマージ
- イベント発行の確認

**依存**: タスク 2.2, 3.1

---

### タスク 3.4: ゴシップループへのネットワーク統合

**要件**: R2, R8

**説明**: ゴシップループ内でネットワーク送信を統合する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper/gossip_loop.rs`

**実装内容**:
ゴシップループの tick 処理を完成:
1. `block_expired_heartbeats()`
2. `block_gracefully_left()`
3. `set_state(HEARTBEAT_KEY, heartbeat)`
4. `send_state()`

**テスト**:
- 統合テスト: ゴシップループの全フロー

**依存**: タスク 2.6, 3.2

---

## Phase 4: コンセンサス（R11）

### タスク 4.1: ConsensusChecker 構造体の作成

**要件**: R11（コンセンサスチェック）

**説明**: コンセンサスチェッカーを実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper/consensus_checker.rs`

**実装内容**:
```rust
pub struct ConsensusChecker {
    id: String,
    key: String,
    extractor: Box<dyn Fn(&[u8]) -> Result<u64, GossiperError> + Send + Sync>,
}

pub struct ConsensusHandle {
    id: String,
}
```

**テスト**:
- チェッカーの作成
- ID の生成

**依存**: タスク 1.2

---

### タスク 4.2: コンセンサスチェック登録/削除の実装

**要件**: R11

**説明**: コンセンサスチェッカーの登録・削除メソッドを実装する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper.rs`

**実装内容**:
1. `register_consensus_check(&mut self, key, extractor) -> ConsensusHandle`
2. `remove_consensus_check(&mut self, id: &str)`
3. `consensus_checkers: BTreeMap<String, ConsensusChecker>` の管理

**テスト**:
- チェッカーの登録
- チェッカーの削除
- 存在しないチェッカーの削除（何もしない）

**依存**: タスク 4.1

---

### タスク 4.3: コンセンサス判定の実装

**要件**: R11

**説明**: 全メンバーが同一値を持つかどうかを判定する。

**ファイル**:
- `modules/cluster/src/std/tokio_gossiper/consensus_checker.rs`

**実装内容**:
```rust
pub fn check_consensus(
    &self,
    state_store: &GossipStateStore,
    active_members: &[String],
) -> bool {
    // 全メンバーの値を抽出
    // 全て同一かどうかを判定
}
```

**テスト**:
- 全員同一値: true
- 値が異なる: false
- メンバーが値を持たない: false

**依存**: タスク 4.1, 2.1

---

## 完了条件

1. 全タスクのテストがパスすること
2. `./scripts/ci-check.sh all` がエラーなく完了すること
3. clippy 警告がないこと
4. rustdoc がビルドできること

---

## 依存関係グラフ

```
Phase 1:
  1.1 (Gossiper トレイト) ─┐
  1.2 (GossiperError) ────┼─→ 1.7 (TokioGossiper 構造体) → 1.8 (start/stop) → 1.9 (ゴシップループ)
  1.3 (GossiperConfig) ───┘
  1.4 (GossipKeyValue) → 1.5 (GossipMemberState) → 1.6 (GossipUpdate)

Phase 2:
  1.4, 1.5 → 2.1 (GossipStateStore) → 2.2 (状態マージ)
  2.1 → 2.3 (get_state/set_state)
  2.4 (MemberHeartbeat) → 2.5 (ハートビート更新) → 2.6 (期限切れ検出)
  1.7 → 2.7 (ClusterTopology)
  2.3 → 2.8 (ブロックメンバー)

Phase 3:
  1.5 → 3.1 (GossipRequest/Response)
  3.1, 2.1 → 3.2 (ゴシップ送信)
  2.2, 3.1 → 3.3 (ゴシップ受信)
  2.6, 3.2 → 3.4 (ループ統合)

Phase 4:
  1.2 → 4.1 (ConsensusChecker)
  4.1 → 4.2 (登録/削除)
  4.1, 2.1 → 4.3 (コンセンサス判定)
```

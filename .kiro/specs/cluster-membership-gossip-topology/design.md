# 設計ドキュメント

## 概要
本設計は cluster モジュールに Membership/Gossip 基盤を定義し、メンバー状態遷移、失敗検知、隔離、トポロジ更新、メトリクス/イベント連動を一貫した契約で扱う。未起動時の拒否、状態遷移の妥当性検証、同周期の変更集約を備え、運用判断のぶれを減らす。

主な利用者は cluster を組み込む開発者と運用者であり、ノードの起動/停止・障害時の判断を自動化し、EventStream での観測と ClusterCore 反映を確実化する。既存の ClusterCore/LocalClusterProviderGeneric を維持しつつ、core に MembershipCoordinator を導入し、NodeStatus/ClusterEvent/ClusterTopology を拡張する。

### 目標 (Goals)
- Membership/Gossip の起動/停止と未起動時拒否を明確化する
- Join/Alive/Suspect/Dead を含む状態遷移と隔離ルールを一貫させる
- TopologyUpdated の集約生成と ClusterCore 反映、メトリクス更新、EventStream 配信を同期させる
- 現在の Membership スナップショットと隔離一覧を常に参照可能にする

### 非目標 (Non-Goals)
- 外部クラスタ管理サービスの実装や永続ストレージの導入
- 分散合意やリーダー選出の追加
- remoting 以外の新規トランスポート実装

## アーキテクチャ

### 既存アーキテクチャの把握
- `ClusterCore` はトポロジ適用、メトリクス更新、EventStream 発火を担う
- `ClusterExtension` は EventStream を購読し、`TopologyUpdated` を `ClusterCore` へ適用する
- 設計では `ClusterCore::try_apply_topology` を追加し、`ClusterExtension` が失敗時に `TopologyApplyFailed` を EventStream に発火する
- `MembershipTable` と `GossipEngine` が既に存在し、版本管理と差分配布の土台がある
- `PhiFailureDetector` が `remote/core` にあり、到達不能の検知を提供できる
- `IdentityTable` は隔離マップを保持し、解決時に隔離を優先する
- `LocalClusterProviderGeneric` は単純な join/leave で TopologyUpdated を発火する

### ハイレベルアーキテクチャ
```mermaid
graph TB
  MembershipCoordinator --> MembershipTable[MembershipTable]
  MembershipCoordinator --> GossipEngine[GossipEngine]
  MembershipCoordinator --> FailureDetector[PhiFailureDetector]
  MembershipCoordinator --> QuarantineTable[QuarantineTable]
  MembershipCoordinator --> TopologyEmitter[TopologyEmitter]
  MembershipCoordinatorDriver[MembershipCoordinatorDriver] --> MembershipCoordinator
  MembershipCoordinatorDriver --> EventStream[EventStream]
  MembershipCoordinatorDriver --> GossipTransport[GossipTransport]
  TopologyEmitter --> EventStream[EventStream]
  EventStream --> ClusterExtension[ClusterExtension]
  ClusterExtension --> ClusterCore
  GossipTransport --> Remoting[Remoting]
  ClusterCore --> ClusterMetrics[ClusterMetrics]
```
- MembershipCoordinator を core に配置し、no_std で完結する状態機械とイベント生成を担当する
- std 拡張は remoting 受信や transport 連携のみを扱い、core に `cfg` を追加しない
- EventStream 発火はロック外で行う設計とし、デッドロックを避ける
- TopologyUpdated の適用は ClusterExtension を単一入口とする
- MembershipCoordinator は副作用を持たず Outcome を返し、MembershipCoordinatorDriver が EventStream への publish と GossipTransport 送信を担当する
- ClusterExtension は購読済みイベントを ClusterCore に適用し、再 publish は行わない
- ClusterCore は `try_apply_topology` で適用失敗を `TopologyApplyError` として返し、ClusterExtension が失敗イベントを発火する

### 技術スタック / 設計判断
- 時刻は `RuntimeToolbox::clock()` の `TimerInstant` を用い、単調増加のタイムスタンプを付与する
- 期間/間隔は `Duration` で保持し、`TimerInstant` と合成して期限判定する
- 共有が必要な場合のみ `ArcShared<ToolboxMutex<...>>` を使い、入口は `SharedAccess`/`Clone` とする
- 共有ラッパーは `with_read` / `with_write` を入口にし、本体は `&mut self` で更新する
- `TB` を型パラメータとして持つ型は名称末尾を `Generic` とする（例: `XyzSharedGeneric<T, TB>`）
- 共有/ハンドルの判断は `docs/guides/shared_vs_handle.md` に従う
- 失敗検知は `PhiFailureDetector` を再利用し、Suspect/Reachable の効果を状態遷移に反映する
- Gossip は `GossipEngine` と `GossipTransport` に分離し、Transport 依存を core から切り離す
- `MembershipCoordinator` は `TimerInstant` を `u64` ミリ秒へ変換するアダプタを持ち、`PhiFailureDetector` に入力する（単調性を維持）

#### 主要設計判断
- **Decision**: `MembershipCoordinator` を core に導入し、Membership/Gossip/FailureDetector を統合する  
  **Context**: 現状は LocalClusterProviderGeneric が単純な join/leave で TopologyUpdated を発火しており、失敗検知や隔離の契約が不十分  
  **Alternatives**: ClusterCore に直接ロジックを埋め込む / provider ごとに実装する  
  **Selected Approach**: `MembershipCoordinator` に状態機械と集約を集約し、`ClusterCore` は適用と観測に集中する  
  **Rationale**: 1 箇所で状態遷移と集約ルールを管理でき、core/std 境界も守れる  
  **Trade-offs**: 新規モジュールと API が増える

- **Decision**: `NodeStatus` に `Suspect` と `Dead` を追加し、隔離を `QuarantineTable` と分離する  
  **Context**: 現状の `Unreachable` だけでは疑いと確定が区別できず、要件の Suspect/Dead を満たせない  
  **Alternatives**: `Unreachable` のみで状態管理する / `IdentityTable` の隔離だけで運用する  
  **Selected Approach**: Suspect と Dead を状態として明示し、隔離は期限付きテーブルで管理する  
  **Rationale**: 状態遷移の意図が明確になり、無効遷移の検出が容易  
  **Trade-offs**: 既存の `Unreachable` 前提の箇所を更新する必要がある

- **Decision**: TopologyUpdated は同周期の変更を集約し、タイムスタンプと現行メンバー一覧を必須とする  
  **Context**: 重複イベントや変化のない更新が発生しやすく、要件に合致しない  
  **Alternatives**: 逐次イベント発火 / ClusterCore で後処理する  
  **Selected Approach**: `MembershipCoordinator` 内の集約バッファでまとめて発火し、ClusterExtension 経由で `ClusterCore` に適用する  
  **Rationale**: 変更のない周期を除外でき、メトリクス/イベント連動を同期できる  
  **Trade-offs**: バッファ管理のための追加状態が必要

## システムフロー

### 代表フロー: 失敗検知からトポロジ更新
```mermaid
sequenceDiagram
  participant Provider
  participant MembershipCoordinatorDriver
  participant MembershipCoordinator
  participant FailureDetector
  participant TopologyAggregator
  participant GossipTransport
  participant ClusterExtension
  participant ClusterCore
  participant EventStream

  Provider->>MembershipCoordinatorDriver: heartbeat
  MembershipCoordinatorDriver->>MembershipCoordinator: handle_heartbeat
  MembershipCoordinator->>FailureDetector: record_heartbeat
  FailureDetector-->>MembershipCoordinator: effect_or_none
  alt suspect_or_dead
    MembershipCoordinator->>TopologyAggregator: collect_change
    TopologyAggregator-->>MembershipCoordinator: topology_updated
    MembershipCoordinator-->>MembershipCoordinatorDriver: outcome
    MembershipCoordinatorDriver->>EventStream: publish_topology
    EventStream->>ClusterExtension: deliver_topology
    ClusterExtension->>ClusterCore: apply_topology
    alt apply_failed
      ClusterExtension->>EventStream: publish_apply_failed
    end
    MembershipCoordinatorDriver->>EventStream: publish_member_status
    MembershipCoordinatorDriver->>GossipTransport: send_outbound
  else no_change
    MembershipCoordinator-->>MembershipCoordinatorDriver: outcome_empty
  end
```

### 駆動モデル
- `MembershipCoordinator` は副作用を持たない状態機械として扱い、処理結果を `MembershipCoordinatorOutcome` にまとめる
- std 側の `MembershipCoordinatorDriver` が定期的に `poll` を呼び、`GossipTransport::poll_deltas` の結果を `handle_gossip_delta` に投入する
- Driver は `MembershipCoordinatorOutcome` の `topology_event`/`member_events` を EventStream へ publish し、`gossip_outbound` を `GossipTransport` で送信する
- Driver は ClusterProvider から保持され、TickDriver/タイマ駆動で実行される
- Driver は remoting の隔離イベントを `handle_quarantine` に変換し、隔離解除時は明示解除 API を呼び出す
- no_std ではアプリケーションが `poll` を手動で呼び出し、Outcome の publish/送信を外部で処理する

### 入力拒否と状態ガード
- `MembershipCoordinatorState::Stopped` の間は `handle_*`/`poll` は全て `NotStarted` で失敗し、内部状態を変更しない
- `MembershipCoordinatorState::Client` の間は `handle_join`/`handle_leave` を `InvalidState` として拒否する（クライアントは参加/離脱要求の入口にならない）
- `snapshot`/`quarantine_snapshot` は全状態で参照可能とし、未起動時は空スナップショットを返す
- Driver は `start_member`/`start_client` 成功後のみ `handle_*` を呼び、`stop` 後は全入力を中断する

### トポロジ集約ポリシー
- `MembershipCoordinator` は変更をバッファし、`topology_emit_interval` 経過時点で単一の `TopologyUpdated` を生成する
- `poll(now)` は集約ウィンドウの境界判定を行い、変更が存在しない場合はイベントを生成しない
- 集約ウィンドウは `next_topology_emit_at`（`TimerInstant`）で管理し、`now >= next_topology_emit_at` で確定する
- `TopologyUpdated` は `TopologyUpdate.members`（現行のアクティブメンバー一覧）を必須とし、`dead` は `left` と同じくアクティブ集合から除外される

### 状態遷移
```mermaid
stateDiagram-v2
  [*] --> Joining
  Joining --> Up: join_accepted
  Up --> Leaving: leave_requested
  Leaving --> Removed: leave_confirmed
  Up --> Suspect: failure_suspect
  Suspect --> Up: reachable
  Suspect --> Dead: suspect_timeout
  Dead --> Removed: downed
```
- 要件の **Join/Alive** は設計上の **Joining/Up** に対応する（用語のずれは本対応表で吸収する）。

### 合意の定義
- 本設計での「合意」は **強い分散合意ではなく**、Gossip による **最終的整合の収束** を指す。
- 合意の成立条件は、同一 authority の `MembershipTable` が同一 version/epoch に収束した状態とする。
- 再送や差分の収束は `GossipEngine` の delta/anti-entropy に委ね、単一時点の全員一致は要求しない。

## API ブループリント

### 型・トレイト一覧
- `modules/cluster/src/core/membership_coordinator.rs`: `pub struct MembershipCoordinatorGeneric<TB>`  
  Membership/Gossip/FailureDetector を統合する実行時コンポーネント
- `modules/cluster/src/core/membership_coordinator_shared.rs`: `pub struct MembershipCoordinatorSharedGeneric<TB>`  
  `ArcShared<ToolboxMutex<...>>` による共有ラッパー
- `modules/cluster/src/core/membership_coordinator_state.rs`: `pub enum MembershipCoordinatorState`  
  `Stopped | Member | Client`
- `modules/cluster/src/core/membership_coordinator_config.rs`: `pub struct MembershipCoordinatorConfig`  
  閾値、タイムアウト、Gossip 有効化など
- `modules/cluster/src/core/membership_coordinator_outcome.rs`: `pub struct MembershipCoordinatorOutcome`  
  生成されたイベントと送信すべき Gossip の集合
- `modules/cluster/src/core/membership_coordinator_error.rs`: `pub enum MembershipCoordinatorError`  
  未起動/不正状態などの Coordinator 固有エラー
- `modules/cluster/src/std/membership_coordinator_driver.rs`: `pub struct MembershipCoordinatorDriverGeneric<TB, TTransport>`  
  `MembershipCoordinator` を駆動し、EventStream publish と GossipTransport 送信を担当する
- `modules/cluster/src/core/gossip_transport.rs`: `pub trait GossipTransport`  
  Gossip の送受信契約
- `modules/cluster/src/core/quarantine_table.rs`: `pub struct QuarantineTable`  
  期限付き隔離の管理
- `modules/cluster/src/core/quarantine_entry.rs`: `pub struct QuarantineEntry`  
  authority, reason, expires_at を保持する view
- `modules/cluster/src/core/quarantine_event.rs`: `pub enum QuarantineEvent`  
  `Quarantined` と `Cleared` を表すイベント
- `modules/cluster/src/core/topology_update.rs`: `pub struct TopologyUpdate`  
  `TopologyUpdated` のペイロード（members/observed_at を含む）
- `modules/cluster/src/core/node_status.rs`: `pub enum NodeStatus`  
  `Suspect` と `Dead` を追加
- `modules/cluster/src/core/cluster_event.rs`: `pub enum ClusterEvent`  
  `MemberStatusChanged` などを追加、`TopologyUpdated` は `TopologyUpdate` を保持する
- `modules/cluster/src/core/cluster_topology.rs`: `pub struct ClusterTopology`  
  変更集合（joined/left/dead）と hash を保持し、現行メンバー一覧は `TopologyUpdate.members` に分離する
- `modules/cluster/src/core/topology_apply_error.rs`: `pub enum TopologyApplyError`  
  トポロジ適用失敗の理由を表す

### シグネチャ スケッチ
```rust
pub struct MembershipCoordinatorConfig {
  pub phi_threshold: f64,
  pub suspect_timeout: Duration,
  pub dead_timeout: Duration,
  pub quarantine_ttl: Duration,
  pub gossip_enabled: bool,
  pub gossip_interval: Duration,
  pub topology_emit_interval: Duration,
}

pub enum MembershipCoordinatorState {
  Stopped,
  Member,
  Client,
}

pub enum MembershipCoordinatorError {
  NotStarted,
  InvalidState { state: MembershipCoordinatorState },
  Membership(MembershipError),
}

pub enum QuarantineEvent {
  Quarantined { authority: String, reason: String },
  Cleared { authority: String },
}

pub struct MembershipCoordinatorOutcome {
  pub topology_event: Option<ClusterEvent>,
  pub member_events: Vec<ClusterEvent>,
  pub gossip_outbound: Vec<GossipOutbound>,
  pub membership_events: Vec<MembershipEvent>,
  pub quarantine_events: Vec<QuarantineEvent>,
}

pub struct TopologyUpdate {
  pub topology: ClusterTopology,
  pub members: Vec<String>,
  pub joined: Vec<String>,
  pub left: Vec<String>,
  pub dead: Vec<String>,
  pub blocked: Vec<String>,
  pub observed_at: TimerInstant,
}

pub struct MembershipCoordinatorGeneric<TB: RuntimeToolbox + 'static> {
  // fields omitted
}

impl<TB: RuntimeToolbox + 'static> MembershipCoordinatorGeneric<TB> {
  pub fn new(config: MembershipCoordinatorConfig, table: MembershipTable, detector: PhiFailureDetector) -> Self;
  pub fn state(&self) -> MembershipCoordinatorState;
  pub fn start_member(&mut self) -> Result<(), MembershipCoordinatorError>;
  pub fn start_client(&mut self) -> Result<(), MembershipCoordinatorError>;
  pub fn stop(&mut self) -> Result<(), MembershipCoordinatorError>;
  pub fn snapshot(&self) -> MembershipSnapshot;
  pub fn quarantine_snapshot(&self) -> Vec<QuarantineEntry>;

  pub fn handle_join(
    &mut self,
    node_id: String,
    authority: String,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError>;

  pub fn handle_leave(
    &mut self,
    authority: &str,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError>;

  pub fn handle_heartbeat(
    &mut self,
    authority: &str,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError>;
  pub fn handle_gossip_delta(
    &mut self,
    peer: &str,
    delta: MembershipDelta,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError>;

  pub fn handle_quarantine(
    &mut self,
    authority: String,
    reason: String,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError>;

  pub fn poll(&mut self, now: TimerInstant) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError>;
}

pub struct MembershipCoordinatorDriverGeneric<TB: RuntimeToolbox + 'static, TTransport: GossipTransport> {
  // fields omitted
}

impl<TB: RuntimeToolbox + 'static, TTransport: GossipTransport> MembershipCoordinatorDriverGeneric<TB, TTransport> {
  pub fn handle_heartbeat(&mut self, authority: &str, now: TimerInstant);
  pub fn handle_gossip_deltas(&mut self, now: TimerInstant);
  pub fn handle_quarantine(&mut self, authority: &str, reason: &str, now: TimerInstant);
  pub fn poll(&mut self, now: TimerInstant);
}

pub trait GossipTransport {
  fn send(&mut self, outbound: GossipOutbound) -> Result<(), GossipTransportError>;
  fn poll_deltas(&mut self) -> Vec<(String, MembershipDelta)>;
}

pub enum TopologyApplyError {
  NotStarted,
  InvalidTopology { reason: String },
}

impl<TB: RuntimeToolbox + 'static> ClusterCore<TB> {
  pub fn try_apply_topology(&mut self, update: &TopologyUpdate) -> Result<Option<ClusterEvent>, TopologyApplyError>;
}
```

## クラス／モジュール図
```mermaid
classDiagram
  class MembershipCoordinatorGeneric {
    +start_member
    +start_client
    +stop
    +snapshot
    +handle_heartbeat
    +handle_gossip_delta
    +poll
  }
  class MembershipCoordinatorDriverGeneric {
    +handle_heartbeat
    +handle_gossip_deltas
    +poll
  }
  class MembershipTable
  class GossipEngine
  class GossipTransport
  class EventStream
  class PhiFailureDetector
  class QuarantineTable
  class ClusterCore

  MembershipCoordinatorDriverGeneric --> MembershipCoordinatorGeneric
  MembershipCoordinatorDriverGeneric --> GossipTransport
  MembershipCoordinatorDriverGeneric --> EventStream
  MembershipCoordinatorGeneric --> MembershipTable
  MembershipCoordinatorGeneric --> GossipEngine
  MembershipCoordinatorGeneric --> PhiFailureDetector
  MembershipCoordinatorGeneric --> QuarantineTable
```

## クイックスタート / 利用例
```rust
fn membership_coordinator_flow<TB: RuntimeToolbox + 'static>(
  runtime: &mut MembershipCoordinatorGeneric<TB>,
  now: TimerInstant,
) {
  let _ = runtime.start_member();

  let _ = runtime.handle_join("node-a".to_string(), "127.0.0.1:12000".to_string(), now);
  let outcome = runtime.handle_heartbeat("127.0.0.1:12000", now);

  for outbound in outcome.gossip_outbound {
    let _ = outbound;
  }

  let _snapshot = runtime.snapshot();
  let _quarantine = runtime.quarantine_snapshot();
}
```

## 旧→新 API 対応表

| 旧 API / 型 | 新 API / 型 | 置換手順 | 備考 |
| --- | --- | --- | --- |
| `NodeStatus::Unreachable` | `NodeStatus::Suspect` / `NodeStatus::Dead` | 失敗検知で `Suspect` へ遷移し、期限超過で `Dead` へ遷移 | 旧 `Unreachable` 判定は `Dead` 扱いに統一 |
| `ClusterEvent::TopologyUpdated { topology, joined, left, blocked }` | `ClusterEvent::TopologyUpdated { update }` | 既存購読側に `TopologyUpdate` を追加対応 | 変更集合と現行メンバー一覧を同時通知 |
| `LocalClusterProviderGeneric::on_member_join/leave` | `MembershipCoordinatorGeneric::handle_join/handle_leave` | Provider は Coordinator へ委譲し、Driver が集約結果を publish | EventStream 発火は Driver 経由 |
| `ClusterCore::apply_topology_for_external` | `ClusterCore::try_apply_topology` | `TopologyUpdate` を受け取り `Result` を返す API へ置換 | 失敗時は `TopologyApplyFailed` を発火、重複は `Ok(None)` |

## 要件トレーサビリティ

| 要件ID | 要約 | 実装コンポーネント | インターフェイス | 参照フロー |
| --- | --- | --- | --- | --- |
| 1.1 | 起動時に基盤を稼働 | MembershipCoordinator | start_member | sequence |
| 1.3 | 未起動時に拒否 | MembershipCoordinator | handle_* | sequence |
| 2.3 | Suspect 状態 | MembershipCoordinator / NodeStatus | handle_heartbeat | state |
| 3.1 | 隔離イベント | QuarantineTable / ClusterEvent | quarantine | sequence |
| 3.3 | 隔離中の再参加拒否 | MembershipCoordinator / QuarantineTable | handle_join | component |
| 3.5 | 隔離一覧の参照 | QuarantineTable | snapshot | component |
| 4.1 | TopologyUpdated 生成 | MembershipCoordinator | poll | sequence |
| 4.2 | 変更なしは生成しない | MembershipCoordinator | poll | aggregation |
| 4.3 | 同周期の集約 | MembershipCoordinator | poll | aggregation |
| 4.4 | 適用失敗イベント | ClusterExtension / ClusterCore | try_apply_topology | sequence |
| 5.4 | タイムスタンプ付与 | ClusterEvent | TopologyUpdated | sequence |

## コンポーネント & インターフェイス

### MembershipCoordinatorGeneric
- 責務: 状態遷移、失敗検知の反映、Gossip 差分生成、TopologyUpdated 集約、イベント生成
- 入出力: join/leave/heartbeat/gossip_delta を受け取り、ClusterEvent と GossipOutbound を生成
- 依存関係: `MembershipTable`, `GossipEngine`, `PhiFailureDetector`, `QuarantineTable`, `RuntimeToolbox`
- 外部依存の調査結果: Gossip と phi 失敗検知の併用は Akka/Pekko の運用パターンと整合する
- 追加ルール: `handle_join` は `QuarantineTable` を参照し、隔離中は参加を拒否し理由を返す
- 追加ルール: `QuarantineTable` の変更は `QuarantineEvent` として通知し、`IdentityTable` はそれに従って同期する（隔離状態の単一ソースは `QuarantineTable`）
- 追加ルール: `RemoteAuthorityManager` の隔離イベントは Driver が `handle_quarantine` に変換し、`QuarantineTable` に反映する
- 追加ルール: 隔離期限が満了した場合、Driver は `RemoteAuthorityManager` の明示解除 API を呼び出し、再参加を許可する
- 追加ルール: `PhiFailureDetector` の `Suspect` は疑い状態として扱い、隔離は開始しない（Join/Heartbeat を拒否しない）
- 追加ルール: `Suspect` が timeout で `Dead` になった時点を「到達不能」とみなし、隔離を開始して `MemberQuarantined` を発火する
- 追加ルール: `Reachable` は Suspect を解除し、隔離中でなければ通常復帰を許可する
- 追加ルール: 隔離は TTL 満了で解除し、隔離中は join を拒否し、gossip 更新より隔離判定を優先する
- 追加ルール: 隔離の単一ソースは `QuarantineTable` とし、`IdentityTable` は `QuarantineEvent` に従って同期する（直接の隔離判定は行わない）

#### 契約定義
**Component Interface**
```rust
pub trait MembershipCoordinatorPort {
  fn start_member(&mut self) -> Result<(), MembershipCoordinatorError>;
  fn stop(&mut self) -> Result<(), MembershipCoordinatorError>;
  fn snapshot(&self) -> MembershipSnapshot;
  fn poll(&mut self, now: TimerInstant) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError>;
}
```
- 前提条件: `Stopped` は `NotStarted`、`Client` は `handle_join`/`handle_leave` を `InvalidState` で拒否する
- 事後条件: 状態遷移は `NodeStatus` の許可された遷移のみを適用する
- 不変条件: authority は一意、`Dead` と `Removed` はアクティブ集合に含めない

### MembershipCoordinatorDriverGeneric
- 責務: `MembershipCoordinator` を駆動し、EventStream publish と GossipTransport 送信をまとめて行う
- 入出力: `poll`/`handle_*` を呼び出し、`MembershipCoordinatorOutcome` を副作用へ変換する
- 依存関係: `MembershipCoordinatorSharedGeneric`, `GossipTransport`, `EventStreamSharedGeneric`, `RuntimeToolbox`
- 配置: `modules/cluster/src/std`（std 側の駆動ループで実行する）
- no_std では提供せず、アプリケーション側が同等の駆動処理を行う
- 共有理由: std 側のタイマ/駆動系と Provider 側が同一の Coordinator を参照する可能性があるため、共有ラッパーを使う
- 追加ルール: `quarantine_events` を受け取った場合、`IdentityTable` への反映を最優先で行い、その後に EventStream 発火や Gossip 送信を行う

### GossipTransport
- 責務: `GossipOutbound` の送信と受信差分の収集
- 入出力: `send` で delta を送信し、`poll_deltas` で受信差分を返す
- 依存関係: remoting または in-memory transport
- 外部依存の調査結果: ピア間の状態共有は gossip が最小構成で成立する

#### 契約定義
**Component Interface**
```rust
pub trait GossipTransport {
  fn send(&mut self, outbound: GossipOutbound) -> Result<(), GossipTransportError>;
  fn poll_deltas(&mut self) -> Vec<(String, MembershipDelta)>;
}
```
- 前提条件: peer は既知の authority である
- 事後条件: 送信失敗は `GossipTransportError` で通知する

### QuarantineTable
- 責務: 隔離理由と期限の保持、期限満了の解放、一覧取得
- 入出力: `quarantine`, `clear`, `poll_expired`, `snapshot`
- 依存関係: `TimerInstant`
- 外部依存の調査結果: 隔離期間は再参加拒否に用い、期限満了後の復帰を許可する
- 単一ソース: 隔離判定は `QuarantineTable` を唯一の根拠とし、`IdentityTable` にはスナップショットを同期する

### ClusterCore
- 責務: TopologyUpdated を適用し、メトリクス更新と EventStream 発火を統合
- 入出力: `try_apply_topology` で `Result` を返し、失敗時のイベント発火は呼び出し側（ClusterExtension）が行う
- 依存関係: `ClusterMetrics`, `EventStreamSharedGeneric`
- 失敗条件: `NotStarted`（起動前適用）、`InvalidTopology`（joined/left/dead の重複や矛盾）
- 追加ルール: メトリクス収集が無効な場合、メトリクス取得要求は失敗として返す
- 追加ルール: `TopologyUpdate` は `ClusterEvent::TopologyUpdated` の唯一のペイロードとし、`ClusterCore::try_apply_topology` にそのまま渡す
- 追加ルール: `TopologyUpdate.members` を基準にメンバー数を再計算し、`left`/`dead` に含まれる authority を `IdentityLookup`/`PidCache` から無効化する

### ClusterMetrics
- 責務: メンバー数などのメトリクスのスナップショット提供
- 入出力: 取得 API は無効時に失敗を返し続ける（要件5.3の担保）

### イベント契約
- 発行イベント:
  - `ClusterEvent::TopologyUpdated { update }`
  - `ClusterEvent::MemberStatusChanged { node_id, authority, from, to, observed_at }`
  - `ClusterEvent::MemberQuarantined { authority, reason, observed_at }`
  - `ClusterEvent::TopologyApplyFailed { reason, observed_at }`
- 購読イベント:
  - `TopologyUpdated` は EventStream で購読され、ClusterCore に反映される
  - `TopologyApplyFailed` は `try_apply_topology` が失敗した場合に ClusterExtension が発火する
  - 状態遷移イベントは観測用途で購読される

### ドメインモデル
- エンティティ: `Member` (node_id, authority, status), `QuarantineEntry` (authority, reason, expires_at)
- ルール:
  - `Joining -> Up -> Suspect -> Dead` の順で遷移し、`Dead` はアクティブ集合から除外する
  - `Leaving -> Removed` を許可し、`Removed` は再参加までアクティブ集合に含めない
  - `Suspect` の解除は `Reachable` 効果でのみ許可する
  - `Dead` 判定で隔離を開始し、TTL 満了で解除する

## データモデル

### 論理データモデル
- `MembershipTable`: authority をキーにした versioned state
- `QuarantineTable`: authority と期限、理由を保持する
- `ClusterTopology`: 変更集合（joined/left/dead）と hash を保持し、現行メンバー一覧は `TopologyUpdate.members` に分離する

### 物理データモデル
- 永続化は行わず、全てメモリ内保持
- `BTreeMap`/`Vec` による deterministic な順序保持を継続

### データ契約 / 連携
- EventStream へは `ClusterEvent` を payload として送出
- `MembershipSnapshot` は handshake 用の読み取り専用構造とする
- `observed_at` は `RuntimeToolbox::clock()` の `TimerInstant` を使用する

## エラーハンドリング

### エラーストラテジ
- `MembershipCoordinatorError::NotStarted` は即時失敗として返す
- `MembershipCoordinatorError::InvalidState` は `Client` での `handle_join`/`handle_leave` を拒否する
- 入力の検証・遷移違反は `MembershipError` を `MembershipCoordinatorError` 経由で返す
- 無効な状態遷移は `MembershipError::InvalidTransition` として記録
- Gossip 送信失敗は `GossipTransportError` で返し、再送は呼び出し側が判断する
- 隔離中の参加は `MembershipError::Quarantined` とし、`MemberQuarantined` を発火する
- Topology 適用失敗は `TopologyApplyError` とし、`TopologyApplyFailed` を発火する

### エラー分類と応答
- ユーザエラー: 無効な join/leave の要求
- システムエラー: Gossip 送信失敗、Topology 適用失敗
- ビジネスロジックエラー: 不正遷移、隔離中の再参加

### モニタリング
- EventStream へ `MemberStatusChanged` と `MemberQuarantined` を発火
- メトリクスは `ClusterMetrics` を更新し、メンバー数や隔離数を観測する
- メトリクス収集が無効な場合は取得要求を失敗として扱い、無効状態のまま取得を継続しない

## テスト戦略
- ユニットテスト: 未起動時の拒否、Client での join/leave 拒否、状態遷移、無効遷移検出、隔離期限の解放、Gossip 集約
- 統合テスト: `ClusterCore` 反映、EventStream 配信、TopologyUpdated 集約の一貫性
- パフォーマンステスト: 1000 ノード相当の Gossip fan-out と集約遅延を検証

## 追加セクション（必要時のみ）

### セキュリティ
- 隔離理由は外部入力を直接表示しない運用を前提とする
- Gossip payload は既存の remoting 認証方針に従う

### パフォーマンス & スケーラビリティ
- TopologyUpdated を周期集約することでイベント増幅を抑制する
- FailureDetector は観測間隔と閾値を config 化し、環境ごとに調整可能とする

### 移行戦略
```mermaid
graph TB
  Phase1[Phase1 api_update] --> Phase2[Phase2 runtime_wire]
  Phase2 --> Phase3[Phase3 provider_migrate]
  Phase3 --> Phase4[Phase4 cleanup]
```
- Phase1: `NodeStatus` と `ClusterEvent` の拡張を反映
- Phase2: `MembershipCoordinator` を core に追加し、LocalClusterProviderGeneric から利用
- Phase3: std 側 transport 連携を `GossipTransport` に寄せる
- Phase4: 旧 `Unreachable` 前提の分岐を削除

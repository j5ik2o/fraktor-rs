# remote-core-extension Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: Remoting trait

`fraktor_remote_core_rs::extension::Remoting` trait が定義され、リモートサブシステムの lifecycle API を提供する SHALL。god object `RemotingControlHandle` (旧 `fraktor-remote-rs`, 479行) の純粋 lifecycle 責務のみを受け持つ。transport 参照、bridge factory、watcher daemon、heartbeat channels 等の runtime 配線は **一切保持しない**。

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/extension/remoting.rs` を読む
- **THEN** `pub trait Remoting` が定義されている

#### Scenario: lifecycle メソッドのシグネチャ

- **WHEN** `Remoting` trait のメソッド一覧を読む
- **THEN** `start(&mut self) -> Result<(), RemotingError>`、`shutdown(&mut self) -> Result<(), RemotingError>`、`quarantine(&mut self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError>`、`addresses(&self) -> &[Address]` が宣言されている

#### Scenario: async / runtime 依存の不在

- **WHEN** `Remoting` trait の全メソッドを検査する
- **THEN** `async fn` は存在せず、戻り値に `Future` を含まない

#### Scenario: transport / bridge factory / watcher daemon を持たない

- **WHEN** `Remoting` trait のメソッド一覧を検査する
- **THEN** `transport_ref`・`bridge_factory`・`watcher_daemon`・`heartbeat_channels`・`writer`・`reader` 等のランタイム配線を直接操作するメソッドを持たない (それらは Phase B で adapter 側 `StdRemoting` 実装が担う)

### Requirement: RemotingLifecycleState 状態機械

`fraktor_remote_core_rs::extension::RemotingLifecycleState` 型が定義され、`Pending`・`Starting`・`Running`・`ShuttingDown`・`Shutdown` の5状態と `&mut self` ベースの **閉じた遷移メソッド群** を持つ SHALL。状態機械は以下の遷移で閉じられる:

```
Pending --transition_to_start()--> Starting --mark_started()--> Running
   │                                                                │
   │                                                                ▼
   │                                       ShuttingDown <--transition_to_shutdown()
   │                                             │
   │                                             ▼
   │                                          Shutdown
   │                                             ▲
   └────────────── (Pending からも transition_to_shutdown で直接 Shutdown へ) ───┘
```

#### Scenario: 型と状態の存在

- **WHEN** `modules/remote-core/src/extension/lifecycle_state.rs` を読む
- **THEN** `pub struct RemotingLifecycleState` が定義され、内部に `Pending`・`Starting`・`Running`・`ShuttingDown`・`Shutdown` を表現する enum を持つ

#### Scenario: Pending から Starting への遷移

- **WHEN** `Pending` 状態の `RemotingLifecycleState` に `transition_to_start()` を呼ぶ
- **THEN** 戻り値は `Ok(())` で、内部状態が `Starting` に遷移している

#### Scenario: Starting から Running への遷移 (startup 完了)

- **WHEN** `Starting` 状態の `RemotingLifecycleState` に `mark_started()` を呼ぶ
- **THEN** 戻り値は `Ok(())` で、内部状態が `Running` に遷移している

#### Scenario: Starting 以外での mark_started 拒否

- **WHEN** `Pending` 状態の `RemotingLifecycleState` に `mark_started()` を呼ぶ
- **THEN** 戻り値は `Err(RemotingError::InvalidTransition { .. })` である

#### Scenario: 重複 start の拒否

- **WHEN** `Starting` または `Running` 状態の `RemotingLifecycleState` に `transition_to_start()` を呼ぶ
- **THEN** 戻り値は `Err(RemotingError::AlreadyRunning)` または同等のエラーである

#### Scenario: Shutdown 状態での start 拒否

- **WHEN** `Shutdown` 状態の `RemotingLifecycleState` に `transition_to_start()` を呼ぶ
- **THEN** 戻り値は `Err(RemotingError::InvalidTransition { .. })` である

#### Scenario: Running から ShuttingDown へ

- **WHEN** `Running` 状態の `RemotingLifecycleState` に `transition_to_shutdown()` を呼ぶ
- **THEN** 戻り値は `Ok(())` で、内部状態が `ShuttingDown` に遷移している

#### Scenario: ShuttingDown から Shutdown へ (shutdown 完了)

- **WHEN** `ShuttingDown` 状態の `RemotingLifecycleState` に `mark_shutdown()` を呼ぶ
- **THEN** 戻り値は `Ok(())` で、内部状態が `Shutdown` に遷移している

#### Scenario: Pending からの直接 shutdown 許可 (未起動時の graceful terminate)

- **WHEN** `Pending` 状態の `RemotingLifecycleState` に `transition_to_shutdown()` を呼ぶ
- **THEN** 戻り値は `Ok(())` で、内部状態が `Shutdown` に直接遷移する (未起動のまま終了扱い)

#### Scenario: is_running query

- **WHEN** `Running` 状態の `RemotingLifecycleState` に `is_running()` を呼ぶ
- **THEN** `true` が返る (CQS 準拠の `&self` query)

#### Scenario: Starting 状態では is_running が false

- **WHEN** `Starting` 状態の `RemotingLifecycleState` に `is_running()` を呼ぶ
- **THEN** `false` が返る (`mark_started()` が呼ばれるまで Running ではない)

#### Scenario: ensure_running は Running 以外で Err

- **WHEN** `Pending`・`Starting`・`ShuttingDown`・`Shutdown` のいずれかの状態で `ensure_running()` を呼ぶ
- **THEN** 戻り値は `Err(RemotingError::NotStarted)` または同等のエラーである

### Requirement: RemotingLifecycleEvent は actor-core 型を再利用する

`remote-core` は独自の `RemotingLifecycleEvent` enum を新設せず、既存の `fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent` を直接参照する SHALL。これは actor-core の `EventStreamEvent::RemotingLifecycle` バリアントが期待する型と一致させるためであり、型の二重化とドリフトを防ぐ。

#### Scenario: 独自 enum の不在

- **WHEN** `modules/remote-core/src/extension/` 配下を検査する
- **THEN** `lifecycle_event.rs` ファイルは存在しないか、存在する場合でも `pub enum RemotingLifecycleEvent` の新定義を含まない (use re-export のみは許容)

#### Scenario: actor-core 型の import

- **WHEN** `remote-core` の `EventPublisher`・`Association`・`AssociationEffect` 等が lifecycle event を扱うコードを検査する
- **THEN** 型は `fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent` として import されている (または `use` 経由で参照されている)

#### Scenario: actor-core との互換性

- **WHEN** `EventPublisher::publish_lifecycle` に渡す型を確認する
- **THEN** 引数型は `fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent` であり、`EventStreamEvent::RemotingLifecycle(event)` として actor-core の event stream に変換可能である

### Requirement: EventPublisher

`fraktor_remote_core_rs::extension::EventPublisher` 型が定義され、`RemotingLifecycleEvent` を actor system のイベントストリームへ送出する機能を提供する SHALL。core は `fraktor-actor-core-rs` に既に依存しているため、`ActorSystemWeak` を直接保持する形で実装する (独自 trait abstraction は作らない)。

#### Scenario: 型の存在

- **WHEN** `modules/remote-core/src/extension/event_publisher.rs` を読む
- **THEN** `pub struct EventPublisher` が定義されている

#### Scenario: ActorSystemWeak を直接保持

- **WHEN** `EventPublisher` のフィールドを検査する
- **THEN** `system: ActorSystemWeak` (または同等の weak reference) を直接保持し、独自の trait abstraction (`LifecycleEventSink` 等) をラップしていない

#### Scenario: publish メソッド

- **WHEN** `EventPublisher::publish_lifecycle` の定義を読む
- **THEN** `fn publish_lifecycle(&self, event: RemotingLifecycleEvent)` または同等のシグネチャが宣言されている

### Requirement: RemoteAuthoritySnapshot data 型

`fraktor_remote_core_rs::extension::RemoteAuthoritySnapshot` 型が定義され、リモート authority の状態 snapshot を表現する immutable data 型として提供される SHALL。

#### Scenario: 型の存在

- **WHEN** `modules/remote-core/src/extension/remote_authority_snapshot.rs` を読む
- **THEN** `pub struct RemoteAuthoritySnapshot` が定義されている

#### Scenario: immutable accessor のみ

- **WHEN** `RemoteAuthoritySnapshot` の impl ブロックを検査する
- **THEN** 公開メソッドは `&self` の accessor のみで、`&mut self` による状態変更メソッドを持たない

### Requirement: RemotingError 型

`fraktor_remote_core_rs::extension::RemotingError` enum が定義され、lifecycle 遷移失敗や `Remoting` trait メソッドの失敗カテゴリを網羅する SHALL。

#### Scenario: 型の存在

- **WHEN** `modules/remote-core/src/extension/remoting_error.rs` を読む
- **THEN** `pub enum RemotingError` が定義され、`InvalidTransition`・`TransportUnavailable`・`AlreadyRunning`・`NotStarted` 等のバリアントを含む

#### Scenario: core::error::Error の実装

- **WHEN** `RemotingError` の derive または impl ブロックを検査する
- **THEN** `Debug`・`Display`・`core::error::Error` (no_std 互換) が実装されている


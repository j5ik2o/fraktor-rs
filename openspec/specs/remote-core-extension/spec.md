# remote-core-extension Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: Remoting trait

`fraktor_remote_core_rs::domain::extension::Remoting` trait が定義され、リモートサブシステムの lifecycle API を提供する SHALL。god object `RemotingControlHandle` (旧 `fraktor-remote-rs`, 479行) の純粋 lifecycle 責務のみを受け持つ。transport 参照、bridge factory、watcher daemon、heartbeat channels 等の runtime 配線は **一切保持しない**。

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

`fraktor_remote_core_rs::domain::extension::RemotingLifecycleState` 型が定義され、`Pending`・`Starting`・`Running`・`ShuttingDown`・`Shutdown` の5状態と `&mut self` ベースの **閉じた遷移メソッド群** を持つ SHALL。状態機械は以下の遷移で閉じられる:

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

`fraktor_remote_core_rs::domain::extension::EventPublisher` 型が定義され、`RemotingLifecycleEvent` を actor system のイベントストリームへ送出する機能を提供する SHALL。core は `fraktor-actor-core-rs` に既に依存しているため、`ActorSystemWeak` を直接保持する形で実装する (独自 trait abstraction は作らない)。

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

`fraktor_remote_core_rs::domain::extension::RemoteAuthoritySnapshot` 型が定義され、リモート authority の状態 snapshot を表現する immutable data 型として提供される SHALL。

#### Scenario: 型の存在

- **WHEN** `modules/remote-core/src/extension/remote_authority_snapshot.rs` を読む
- **THEN** `pub struct RemoteAuthoritySnapshot` が定義されている

#### Scenario: immutable accessor のみ

- **WHEN** `RemoteAuthoritySnapshot` の impl ブロックを検査する
- **THEN** 公開メソッドは `&self` の accessor のみで、`&mut self` による状態変更メソッドを持たない

### Requirement: RemotingError 型

`fraktor_remote_core_rs::domain::extension::RemotingError` enum が定義され、lifecycle 遷移失敗や `Remoting` trait メソッドの失敗カテゴリを網羅する SHALL。

#### Scenario: 型の存在

- **WHEN** `modules/remote-core/src/extension/remoting_error.rs` を読む
- **THEN** `pub enum RemotingError` が定義され、`InvalidTransition`・`TransportUnavailable`・`AlreadyRunning`・`NotStarted` 等のバリアントを含む

#### Scenario: core::error::Error の実装

- **WHEN** `RemotingError` の derive または impl ブロックを検査する
- **THEN** `Debug`・`Display`・`core::error::Error` (no_std 互換) が実装されている

### Requirement: RemoteEvent enum の存在

`fraktor_remote_core_rs::core::extension::RemoteEvent` enum が定義され、adapter から core への通知種別を closed enum として表現する SHALL。

#### Scenario: RemoteEvent の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_event.rs` を読む
- **THEN** `pub enum RemoteEvent` が定義されている

#### Scenario: 必要なバリアントの宣言

- **WHEN** `RemoteEvent` のバリアント一覧を検査する
- **THEN** 以下のバリアントを **全て** 含み、これら以外を含まない（closed enum、本 change のスコープ）
  - `InboundFrameReceived { authority: TransportEndpoint, frame: alloc::vec::Vec<u8>, now_ms: u64 }`
  - `HandshakeTimerFired { authority: TransportEndpoint, generation: u64, now_ms: u64 }`
  - `OutboundEnqueued { authority: TransportEndpoint, envelope: alloc::boxed::Box<OutboundEnvelope>, now_ms: u64 }`
  - `ConnectionLost { authority: TransportEndpoint, cause: ConnectionLostCause, now_ms: u64 }`
  - `TransportShutdown`

#### Scenario: 本 change スコープ外の variant

- **WHEN** `RemoteEvent` のバリアント一覧を検査する
- **THEN** `OutboundFrameAcked` / `QuarantineTimerFired` / `BackpressureCleared` 等のバリアントは含まれない（本 change で scheduling 経路が確定していないため、必要時に別 change で variant 追加と scheduling 経路を一緒に拡張する）
- **AND** これらの variant は本 change の `RemoteEvent` enum に **追加してはならない**（MUST NOT、closed enum を保ちつつ scope を絞る）

#### Scenario: open hierarchy の不在

- **WHEN** `RemoteEvent` の定義を検査する
- **THEN** `#[non_exhaustive]` および unbounded な generic は宣言されていない（closed enum として固定する）

#### Scenario: generation 型は u64

- **WHEN** `HandshakeTimerFired` バリアントの `generation` フィールド型を検査する
- **THEN** 型は `u64` であり、`HandshakeGeneration` 等の newtype でラップされていない

### Requirement: RemoteEventReceiver trait

`fraktor_remote_core_rs::core::extension::RemoteEventReceiver` trait が定義され、`Remote::run` / `RemoteShared::run` が消費する Port を表現する SHALL。

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_event_receiver.rs` を読む
- **THEN** `pub trait RemoteEventReceiver` が定義されている
- **AND** trait 自体に `Send` supertrait は要求されていない（single-thread executor / no_std 利用を妨げない）

#### Scenario: poll_recv のシグネチャ

- **WHEN** `RemoteEventReceiver::poll_recv` の定義を読む
- **THEN** `fn poll_recv(&mut self, cx: &mut core::task::Context<'_>) -> core::task::Poll<Option<RemoteEvent>>` または同等の poll 型シグネチャで宣言されている
- **AND** `async fn recv` や `fn recv(..) -> impl Future<...>` は存在しない

#### Scenario: tokio 非依存

- **WHEN** `modules/remote-core/src/core/extension/` 配下の RemoteEvent / RemoteEventReceiver 関連 import を検査する
- **THEN** `tokio` クレートへの参照が存在しない

#### Scenario: RemoteEventSink trait の不在

- **WHEN** `modules/remote-core/src/core/extension/` 配下のソースを検査する
- **THEN** `pub trait RemoteEventSink` または同等の adapter→core push 用 trait が定義されていない（adapter 内部 sender で完結し、純増ゼロ方針を維持する）

### Requirement: Remote は CQS core logic 層であり Remote::run を持つ

`Remote` 構造体は CQS 原則を厳格に守る core logic 層 SHALL。状態を変更する method はすべて `&mut self`（Command）、状態を読む method は `&self`（Query）。`Remote::run(&mut self, receiver)` は排他所有時の core event loop として存在する SHALL。`Remote` 自体に共有・並行性責務を持たせてはならない（MUST NOT、内部可変性 / `Arc` / `Mutex` を field に持たない）。

#### Scenario: Remote の CQS 遵守

- **WHEN** `impl Remote` ブロックの method 一覧を検査する
- **THEN** 状態を変更する method（`start` / `shutdown` / `quarantine` / `handle_remote_event` / `set_instrument` / `run` 等）はすべて `&mut self` を取る
- **AND** 状態を読む method（`addresses` / `lifecycle` / `config` 等）はすべて `&self` を取る

#### Scenario: Remote 内部の並行性吸収責務の不在

- **WHEN** `Remote` の field を検査する
- **THEN** `Arc<Mutex<..>>` / `RwLock<..>` / `Cell<..>` / `RefCell<..>` 等の内部可変性を持つ field が存在しない
- **AND** instrument 用の `Box<dyn RemoteInstrument + Send>` 以外に動的ディスパッチ用の field を持たない

#### Scenario: Remote::run の存在と所有権

- **WHEN** `impl Remote` ブロックの method 一覧を検査する
- **THEN** `pub fn run<'a, S: RemoteEventReceiver + ?Sized>(&'a mut self, receiver: &'a mut S) -> RemoteRunFuture<'a, S>` または同等の concrete Future 型を返すシグネチャが存在する
- **AND** `self` consume ではなく `&mut self` を取る（`pub fn run(self, ..)` は存在しない）
- **AND** `async fn run` や `-> impl Future<...>` を public run API に使わない
- **AND** 内部で `Remote::handle_remote_event(event)?` と `Remote::is_terminated()` を使い、event semantics は `Remote` 内に閉じる
- **AND** `RemoteShared::run` に置き換える目的で `Remote::run` を削除してはならない（MUST NOT）

### Requirement: RemoteRunFuture は concrete Future 型

`Remote::run` が返す concrete Future 型として `RemoteRunFuture<'a, S>` が定義される SHALL。`RemoteRunFuture` は `core::future::Future<Output = Result<(), RemotingError>>` を実装し、`Remote` の排他借用と `RemoteEventReceiver` の排他借用を保持する。`Send` 境界を型定義または `run` method の public API に要求してはならない（MUST NOT）。

#### Scenario: RemoteRunFuture の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_run_future.rs` を読む
- **THEN** `pub struct RemoteRunFuture<'a, S: RemoteEventReceiver + ?Sized>` または同等の公開 concrete Future 型が定義されている
- **AND** `impl Future for RemoteRunFuture<'_, S>` が定義されている
- **AND** `Remote::run` の戻り値型としてこの concrete type 名が現れる

#### Scenario: RemoteRunFuture の poll 処理

- **WHEN** `RemoteRunFuture::poll` の実装を検査する
- **THEN** `receiver.poll_recv(cx)` を呼んで event を取得する
- **AND** `Poll::Pending` の場合は `Poll::Pending` を返し、状態遷移を行わない
- **AND** `Poll::Ready(None)` の場合は `Poll::Ready(Err(RemotingError::EventReceiverClosed))` を返す
- **AND** `Poll::Ready(Some(event))` の場合は `remote.handle_remote_event(event)?` を呼び、続けて `remote.is_terminated()` を確認する
- **AND** `remote-core` 内で `async fn` / `async move` / `.await` を使わない

### Requirement: Remote::handle_remote_event は event 1 件分の dispatch を担う Command

`Remote` 構造体に inherent method `pub fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<(), RemotingError>` が定義され、event 1 件分の状態遷移と effect 処理を担当する SHALL。**CQS Command として成功値は `()` のみ**（停止判定 bool を返してはならない、MUST NOT）。停止判定は別の Query method `Remote::is_terminated()` で行う。`Remote` 自体に型パラメータ `<I>` を導入してはならない（MUST NOT、instrument は `Box<dyn RemoteInstrument + Send>` で保持する）。

#### Scenario: handle_remote_event のシグネチャ

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を読む
- **THEN** `impl Remote` ブロックに `pub fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<(), RemotingError>` または同等のシグネチャが宣言されている
- **AND** 成功値は `()` であり、`bool` その他の停止判定値を返していない（CQS Command 準拠）
- **AND** `Remote` 自体には型パラメータ `<I>` が宣言されていない

#### Scenario: TransportShutdown は lifecycle 状態変更で表現する

- **WHEN** `Remote::handle_remote_event` が `RemoteEvent::TransportShutdown` を受信する
- **THEN** lifecycle が未停止なら内部で `lifecycle.transition_to_shutdown_requested()` 等を呼んで状態を変更する（戻り値で停止信号は返さない）
- **AND** lifecycle が既に停止要求済みまたは停止済みなら no-op として `Ok(())` を返す（shutdown wake event として冪等に扱う）
- **AND** 次回 `Remote::is_terminated()` Query が `true` を返すように lifecycle 側で観測可能になる
- **AND** `RemoteShared::run` 側はこの Query を `with_read` で読んでループ終了を判定する

#### Scenario: 復帰不能エラーで Err

- **WHEN** event 処理中に transport が永続的に失敗するなど復帰不能なエラーが発生する
- **THEN** `Remote::handle_remote_event` は `Err(RemotingError::TransportUnavailable)` または同等の variant を返す
- **AND** 戻り値の `Result` を `let _ = ...` で握りつぶす経路は呼び出し側に存在しない

#### Scenario: TransportError から RemotingError への変換

- **WHEN** `Remote::handle_remote_event` 内で `RemoteTransport::send` / `RemoteTransport::schedule_handshake_timeout` 等が `Err(TransportError)` を返す
- **THEN** `Remote::handle_remote_event` は `TransportError` を `RemotingError::TransportUnavailable`（または変換ロジックで対応する `RemotingError` variant）にマップして `?` で伝播する
- **AND** inbound raw frame decode の失敗は `RemotingError::CodecFailed` 等の対応 variant にマップする（未定義なら本 change で `RemotingError` に追加する）
- **AND** マッピングは `Remote::handle_remote_event` または専用 helper で集約され、呼び出し点ごとにアドホックに `match` する経路を作らない

### Requirement: Remote::is_terminated は停止判定 Query

`Remote` 構造体に inherent method `pub fn is_terminated(&self) -> bool` が定義され、ループ継続可否の判定材料を返す SHALL。**CQS Query として状態変更を行わない**（`&self`）。`RemoteShared::run` は per-event の `handle_remote_event` 実行後にこの Query を `with_read` で確認してループ終了を判定する。

#### Scenario: is_terminated のシグネチャ

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を読む
- **THEN** `impl Remote` ブロックに `pub fn is_terminated(&self) -> bool` または同等のシグネチャが宣言されている
- **AND** `&self` を取り、状態を変更しない（CQS Query 準拠）
- **AND** `#[must_use]` 属性が付与されている

#### Scenario: lifecycle 観測

- **WHEN** `Remote::is_terminated()` が呼ばれる
- **THEN** lifecycle が `Terminated` または `ShutdownRequested` のいずれかであれば `true` を返す
- **AND** `Running` / `Starting` / `Idle` のいずれかであれば `false` を返す

### Requirement: RemoteShared は Sharing 層として並行性を吸収する（薄いラッパー原則）

`fraktor_remote_core_rs::core::extension::RemoteShared` 型が定義され、`SharedLock<Remote>` を内包する Sharing 層として並行性責務を吸収する SHALL。`#[derive(Clone)]` で複数 clone 可能、すべての公開 method は `&self` を取る。raw `SharedLock<Remote>` を呼び出し側に露出してはならない（MUST NOT）。

**薄いラッパー原則:** `RemoteShared` は `Remote` が知らない責務（tokio sender、event channel、wake 機構、runtime-specific 概念等）や、`Remote` が持つべき core semantics（event variant 解釈、lifecycle 遷移、effect 実行、transport 呼び出し）を **追加してはならない**（MUST NOT）。すべての公開 method は `with_write` / `with_read` で `Remote` の inherent method にデリゲートするだけに留まる。`run` は例外的な固有責務ではなく、`Remote::run` の共有 wrapper である。

#### Scenario: RemoteShared の存在と Clone

- **WHEN** `modules/remote-core/src/core/extension/remote_shared.rs` を読む
- **THEN** `pub struct RemoteShared` が定義され、`#[derive(Clone)]` または同等の手書き `impl Clone for RemoteShared` を持つ
- **AND** 内部 field は `SharedLock<Remote>` 1 個のみ（`utils-core::sync::SharedLock<T>`）

#### Scenario: 構築 API

- **WHEN** `RemoteShared::new` を検査する
- **THEN** `pub fn new(remote: Remote) -> Self` が定義され、内部で `SharedLock::new_with_driver::<DefaultMutex<_>>(remote)` 相当で構築する
- **AND** `Remote` の所有権は `SharedLock` 内に常駐し、外部から取り出す経路（`into_inner` 等）を公開 API として提供しない

#### Scenario: 公開メソッドはすべて &self

- **WHEN** `impl RemoteShared` ブロックの公開 method 一覧を検査する
- **THEN** constructor (`new`) を除く公開 instance method はすべて `&self` を取り、`self` consume または `&mut self` を取る公開 instance method は存在しない

#### Scenario: raw SharedLock<Remote> を露出しない

- **WHEN** `RemoteShared` の公開 API を検査する
- **THEN** `pub fn inner() -> SharedLock<Remote>` や `pub fn shared_lock() -> SharedLock<Remote>` のような raw lock を返す API が存在しない
- **AND** 内部の `with_write` / `with_read` は `pub(crate)` 以下の visibility に閉じる

#### Scenario: Remote が知らない責務を持たない

- **WHEN** `RemoteShared` の field 構成を検査する
- **THEN** `inner: SharedLock<Remote>` のみが定義されている
- **AND** `event_sender: tokio::sync::mpsc::Sender<...>` / `Box<dyn EventSink>` / wake 用 callback / runtime-specific channel 等の field が **存在しない**
- **AND** core crate（`remote-core`）が `tokio` 等の特定 runtime crate に依存する形になっていない

### Requirement: RemoteShared::run は Remote::run の共有 wrapper

`RemoteShared` に inherent method `pub fn run<'a, S: RemoteEventReceiver + ?Sized>(&'a self, receiver: &'a mut S) -> RemoteSharedRunFuture<'a, S>` が定義され、共有時に `Remote::run` と同じ core event loop semantics を per-event lock で実行する SHALL。各 event の dispatch は `with_write(|remote| remote.handle_remote_event(event))` で行い、ロック区間は event 1 件分のみ。`RemoteShared::run` は event variant を match せず、lifecycle 遷移、effect 実行、transport 呼び出しを実装しない。これらはすべて `Remote` 側に閉じる。

`RemoteShared::run` は `Remote::run` の共有 wrapper だが、`SharedLock` の write guard を取得したまま `Remote::run(&mut remote, receiver)` が返す `RemoteRunFuture` を保持してはならない（MUST NOT）。future が `Poll::Pending` の間も lock を保持すると、他 clone からの `Remoting` method が進行できないためである。`RemoteSharedRunFuture` は receiver の poll と per-event lock orchestration だけを持ち、event 1 件ごとの処理を `Remote::handle_remote_event` と `Remote::is_terminated` にデリゲートする SHALL。

#### Scenario: RemoteShared::run のシグネチャ

- **WHEN** `modules/remote-core/src/core/extension/remote_shared.rs` を読む
- **THEN** `impl RemoteShared` ブロックに `pub fn run<'a, S>(&'a self, receiver: &'a mut S) -> RemoteSharedRunFuture<'a, S>` または同等の concrete Future 型を返すシグネチャが宣言されている
- **AND** `S: RemoteEventReceiver` が trait bound として要求される
- **AND** `async fn run` や `-> impl Future<...>` を public run API に使わない

### Requirement: RemoteSharedRunFuture は concrete Future 型

`RemoteShared::run` が返す concrete Future 型として `RemoteSharedRunFuture<'a, S>` が定義される SHALL。`RemoteSharedRunFuture` は `core::future::Future<Output = Result<(), RemotingError>>` を実装し、`RemoteShared` と `RemoteEventReceiver` の借用を保持する。`Send` 境界を型定義または `run` method の public API に要求してはならない（MUST NOT）。

#### Scenario: RemoteSharedRunFuture の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_shared_run_future.rs` を読む
- **THEN** `pub struct RemoteSharedRunFuture<'a, S: RemoteEventReceiver + ?Sized>` または同等の公開 concrete Future 型が定義されている
- **AND** `impl Future for RemoteSharedRunFuture<'_, S>` が定義されている
- **AND** `RemoteShared::run` の戻り値型としてこの concrete type 名が現れる

#### Scenario: per-event の Command + Query 分離

- **WHEN** `RemoteSharedRunFuture::poll` の実装を検査する
- **THEN** event 1 件あたり次の順で実行する
  1. `receiver.poll_recv(cx)` で event を poll する
  2. `with_write(|remote| remote.handle_remote_event(event))?` で Command 実行（状態変更のみ、戻り値 `()`）
  3. `with_read(|remote| remote.is_terminated())` で Query 確認（停止判定）
- **AND** Query が `true` を返したらループ終了 `Ok(())`
- **AND** `Poll::Pending` の間は lock を取らない
- **AND** `SharedLock` の write guard を保持したまま `Remote::run` が返す `RemoteRunFuture` を保持する経路は存在しない
- **AND** Command の戻り値で停止判定する経路（`Result<bool, _>` 等）は存在しない
- **AND** `RemoteShared::run` 自身が `match event` 等で event variant を解釈する経路は存在しない
- **AND** `RemoteShared::run` 自身が `RemoteTransport` や `Association` を直接触る経路は存在しない

#### Scenario: receiver 枯渇で Err(EventReceiverClosed)

- **WHEN** `RemoteEventReceiver::poll_recv(cx)` が `Poll::Ready(None)` を返す
- **THEN** `RemoteShared::run` は `Err(RemotingError::EventReceiverClosed)` を返してループ終了する

#### Scenario: TransportShutdown 受信時のループ終了経路

- **WHEN** `RemoteEvent::TransportShutdown` が受信される
- **THEN** `Remote::handle_remote_event` 内で lifecycle が未停止なら `lifecycle.transition_to_shutdown_requested()` が呼ばれて状態変更される（CQS Command）
- **AND** lifecycle が既に停止要求済みまたは停止済みなら no-op `Ok(())` になる
- **AND** 続く `with_read(|remote| remote.is_terminated())` で `true` が観測され、`RemoteShared::run` は `Ok(())` を返してループ終了する

#### Scenario: 並行 Remoting メソッドの進行（条件付き保証）

- **WHEN** `RemoteShared::run` が走っている間に、別の clone から `RemoteShared::quarantine` / `shutdown` / `addresses` が呼ばれる
- **THEN** これらは `run` の event 処理間の隙間（`poll_recv(cx)` が `Poll::Pending` を返して lock を持っていない間、または event 1 件処理完了後）で write/read lock を取って進行する
- **AND** **次の前提条件下で** lock の取り合いがあってもデッドロックや無限待機が発生しない
  - `RemoteTransport` の同期 method（`send` / `send_handshake` / `schedule_handshake_timeout` 等）は `RemoteShared` / `Remote` への再入を行わない（remote-core-transport-port capability で要件化、後述）
  - `RemoteTransport` の各 method は bounded 時間内に return する（無限ブロックしない）
  - inbound core wire frame decode は同期で bounded 時間内に return する
- **AND** これらの前提が破られた場合（例: `RemoteTransport::send` 内部で `RemoteShared::shutdown` を呼ぶ）はデッドロックの可能性があり、`RemoteTransport` 実装側の責任とする

### Requirement: Remoting trait は &self ベースで RemoteShared に実装される

`Remoting` trait のすべてのメソッドは `&self` を取る同期 method SHALL。`async fn` および `Future` 戻り値を **追加してはならない**（MUST NOT）。`addresses` の戻り値は owned `Vec<Address>`（read lock 中に clone するため slice 不可）。`impl Remoting for Remote` を **削除** し、`impl Remoting for RemoteShared` を新設する SHALL。

#### Scenario: Remoting trait のシグネチャ

- **WHEN** `modules/remote-core/src/core/extension/remoting.rs` を読む
- **THEN** trait `Remoting` の各メソッドは次のシグネチャを持つ
  - `fn start(&self) -> Result<(), RemotingError>`
  - `fn shutdown(&self) -> Result<(), RemotingError>`
  - `fn quarantine(&self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError>`
  - `fn addresses(&self) -> Vec<Address>`
- **AND** `async fn` および `Future` 戻り値が存在しない

#### Scenario: impl Remoting for Remote の不在

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を検査する
- **THEN** `impl Remoting for Remote` が存在しない（`Remote` は CQS 純粋ロジック層であり `Remoting` port を実装しない）

#### Scenario: impl Remoting for RemoteShared

- **WHEN** `modules/remote-core/src/core/extension/remote_shared.rs` を検査する
- **THEN** `impl Remoting for RemoteShared` が定義され、各メソッドは `with_write` または `with_read` で内部 `Remote` のメソッドにデリゲートする
- **AND** `start` / `shutdown` / `quarantine` は `with_write` 経由
- **AND** `addresses` は `with_read(|remote| remote.addresses().to_vec())` で owned `Vec<Address>` を返す

#### Scenario: Remoting::shutdown の挙動（純デリゲートのみ）

- **WHEN** `RemoteShared::shutdown(&self)` が呼ばれる
- **THEN** `with_write(|remote| remote.shutdown())` で `Remote::shutdown` を呼び lifecycle を terminated に遷移する
- **AND** 既に停止要求済みまたは停止済みの場合は `Remote::shutdown` / `RemoteShared::shutdown` が no-op `Ok(())` になる（shutdown 系 API は冪等）
- **AND** **wake はしない**（`RemoteShared` は `event_sender` を持たない、薄いラッパー原則）
- **AND** `event_sender.send(...).await` や `run_handle.await` を内部で **実行しない**（同期 method、`async fn` を増やさない）
- **AND** `Remote` が知らない責務（tokio sender、event push 等）を `RemoteShared::shutdown` 内で実行しない

#### Scenario: 完了保証は adapter 固有 surface で行う

- **WHEN** run task の完了保証を必要とする呼び出し側がいる
- **THEN** adapter 固有の async surface（例: `RemotingExtensionInstaller::shutdown_and_join`、`Remoting` trait 外）を使う
- **AND** 同期 `Remoting::shutdown` は run task の終了完了まで保証したように **見せない**
- **AND** `Remoting::shutdown` 単独呼び出しは「lifecycle terminated に遷移するだけ」のセマンティクスに留まる（run task は次の event 受信時に lifecycle terminated を観測してループ終了する）

### Requirement: 別 Driver 型を新設しない

`Remote::run` / `RemoteShared::run` のために `RemoteDriver` / `RemoteDriverHandle` / `RemoteDriverOutcome` 等の新規型を core 側に追加してはならない（MUST NOT）。排他所有時の event loop は `Remote::run(&mut self, ..) -> RemoteRunFuture<'_, S>`、共有時の lock orchestration は `RemoteShared::run(&self, ..) -> RemoteSharedRunFuture<'_, S>`、終了結果は各 concrete Future の `Output = Result<(), RemotingError>` で表現する。

#### Scenario: RemoteDriver 型の不在

- **WHEN** `modules/remote-core/src/core/` 配下を検査する
- **THEN** `pub struct RemoteDriver` または `pub mod driver` が定義されていない

#### Scenario: RemoteDriverHandle 型の不在

- **WHEN** `modules/remote-core/src/core/` 配下を検査する
- **THEN** `pub struct RemoteDriverHandle` が定義されていない

#### Scenario: RemoteDriverOutcome enum の不在

- **WHEN** `modules/remote-core/src/core/` 配下を検査する
- **THEN** `pub enum RemoteDriverOutcome` が定義されていない（`Result<(), RemotingError>` で「正常終了 / 異常終了」を表現する）

### Requirement: AssociationEffect::StartHandshake は Remote::handle_remote_event で 2 ステップ処理される

`Remote::handle_remote_event` 内で `AssociationEffect::StartHandshake { authority, timeout, generation }` を次の **2 ステップ** で処理する SHALL。adapter 側の effect application からは該当分岐を削除する。

#### Scenario: ステップ 1 — handshake request frame の送出

- **WHEN** `Remote::handle_remote_event` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を見つける
- **THEN** 該当 association の local / remote address から `HandshakePdu::Req(HandshakeReq::new(local, remote))` を構築する
- **AND** 続いて `RemoteTransport::send_handshake` で送出する
- **AND** `Result` を `?` で伝播する（`let _ =` で握りつぶさない）

#### Scenario: ステップ 2 — handshake timer の予約

- **WHEN** ステップ 1 の send が成功して戻る
- **THEN** `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)` を呼ぶ
- **AND** 戻り値の `Result` を `?` で伝播する
- **AND** adapter 側はこの呼出を契機に tokio task で sleep を起動し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation, now_ms }` を内部 sender 経由で receiver に push する責務を持つ（詳細は `remote-core-transport-port` capability および `remote-adaptor-std-io-worker` capability で要件化）

#### Scenario: 順序保証

- **WHEN** ステップ 1 とステップ 2 の呼出順序を検査する
- **THEN** ステップ 1（`send`）の戻り値を確認してからステップ 2（`schedule_handshake_timeout`）を呼ぶ
- **AND** ステップ 1 が `Err` の場合、ステップ 2 は呼ばれない

### Requirement: RemoteEvent::OutboundEnqueued 処理

`Remote::handle_remote_event` は `RemoteEvent::OutboundEnqueued { authority, envelope, now_ms }` を受信した際、該当 association に envelope を enqueue し、続けて outbound drain（next_outbound 処理）を実行する SHALL。`envelope` は `Box<OutboundEnvelope>` とし、`RemoteTransport::send` の失敗時 envelope 返却経路と型の大きさを揃える。

#### Scenario: enqueue と drain の連鎖

- **WHEN** `Remote::handle_remote_event` が `RemoteEvent::OutboundEnqueued { authority, envelope, now_ms }` を受信する
- **THEN** `AssociationRegistry` から `authority` 対応の `Association` を取得し、`Association::enqueue(*envelope, now_ms)` を呼ぶ
- **AND** 続けて outbound drain helper（`next_outbound` → `RemoteTransport::send`）を起動し、可能な限り queue を消化する

#### Scenario: 内部可変性回避

- **WHEN** adapter 側の enqueue 経路（local actor からの tell 等）を検査する
- **THEN** adapter は `AssociationRegistry` を直接 mutate せず、`RemoteEvent::OutboundEnqueued` を内部 sender に push する
- **AND** `AssociationRegistry` の所有権は本 change の主経路では `Remote` に集約されており、adapter 側から raw shared handle 経由で直接 mutate しない

### Requirement: Installer は RemoteShared を保持し外部公開する

adapter 側の `RemotingExtensionInstaller` は `RemoteShared` を `OnceLock<RemoteShared>` として保持し、`installer.remote() -> Result<RemoteShared, RemotingError>` で外部公開しなければならない（MUST）。raw `SharedLock<Remote>` / `Arc<Mutex<Remote>>` / `Arc<Remote>` を field として保持してはならない（MUST NOT）。`installer.remote()` の戻り値型は `RemoteShared` または `Result<RemoteShared, _>` であり、raw `SharedLock<Remote>` を返してはならない（MUST NOT）。

#### Scenario: installer の field 構成

- **WHEN** `RemotingExtensionInstaller` の field を検査する
- **THEN** `transport: std::sync::Mutex<Option<TcpRemoteTransport>>` / `config: RemoteConfig` / `remote_shared: std::sync::OnceLock<RemoteShared>` / `event_sender: std::sync::OnceLock<tokio::sync::mpsc::Sender<RemoteEvent>>` / `event_receiver: std::sync::Mutex<Option<TokioMpscRemoteEventReceiver>>` / `run_handle: std::sync::Mutex<Option<JoinHandle<Result<(), RemotingError>>>>` 程度のみを保持する（`ExtensionInstaller::install(&self)` 契約と整合させるため、書き換え可能 field は内部可変性で包む）
- **AND** raw `SharedLock<Remote>` / `Arc<Mutex<Remote>>` / `Arc<Remote>` の field が存在しない
- **AND** `cached_addresses: Vec<Address>` のような addresses cache field を持たない（`RemoteShared::addresses` で source of truth から取得するため）

#### Scenario: 公開 getter のシグネチャ

- **WHEN** `RemotingExtensionInstaller::remote` の戻り値型を検査する
- **THEN** `pub fn remote(&self) -> Result<RemoteShared, RemotingError>`（または adapter error へ変換可能な同等の `Result<RemoteShared, _>`）を返す
- **AND** raw `SharedLock<Remote>` を返す API が公開されていない

#### Scenario: install と Remote::start と spawn の分離

- **WHEN** `RemotingExtensionInstaller::install(&self, system)` の挙動を検査する
- **THEN** `transport` field の `Mutex` を取り、`Option::take()` で transport を取り出す
- **AND** `Remote::with_instrument` で `Remote` を構築 → `RemoteShared::new(remote)` で `RemoteShared` を構築
- **AND** `tokio::sync::mpsc::channel` で event channel を作成し、`event_sender` (`OnceLock`) と `event_receiver` (`Mutex<Option<_>>`) に保存
- **AND** `remote_shared` を `OnceLock::set` で保存（重複 install は `ALREADY_INSTALLED` エラー）
- **AND** `Remote::start` を呼ばない（外部から `installer.remote()?.start()` で呼ぶ）
- **AND** run task を `tokio::spawn` で起動しない（明示 API `installer.spawn_run_task()` を別途呼ぶ）

#### Scenario: spawn 経路（明示 API、&self 契約）

- **WHEN** `installer.spawn_run_task(&self) -> Result<(), RemotingError>` が呼ばれる
- **THEN** `event_receiver` の `Mutex` を取り、`Option::take()` で receiver を取得する（既に take 済の場合は `RemotingError::AlreadyRunning` を返す）
- **AND** `remote_shared.get().cloned()` で `RemoteShared` clone を取得（未 install なら `RemotingError::NotStarted` を返す）
- **AND** `tokio::spawn(async move { run_target.run(&mut receiver).await })` 相当で起動
- **AND** 戻り値の `JoinHandle` を `run_handle` (`Mutex<Option<_>>`) に保存
- **AND** メソッドは `&self` のみを取る（`&mut self` ではない、`ExtensionInstaller` 契約と整合）

### Requirement: 外部制御 surface（adapter 固有 surface との責務分担）

run task の制御経路は次の責務分担で構成される SHALL。

- `Remoting` trait（`RemoteShared` 実装、core 提供）— 4 同期 method（`start` / `shutdown` / `quarantine` / `addresses`）。**lifecycle 状態遷移のみ**を担い、tokio の wake は行わない（`RemoteShared` は `event_sender` を持たない、薄いラッパー原則）
- `Sender<RemoteEvent>`（adapter installer が保持） — `RemoteEvent` を adapter 内部で push（I/O ワーカー / handshake timer task / RemoteActorRef が clone 共有）。`shutdown_and_join` 内で `try_send(TransportShutdown)` の wake にも使う
- `JoinHandle<Result<(), RemotingError>>`（adapter installer が保持）— `installer.shutdown_and_join().await` で完了観測
- `RemotingExtensionInstaller::shutdown_and_join(&self) -> impl Future<Output = Result<(), RemotingError>>` — adapter 固有 async surface、wake (`event_sender.try_send`) + 完了観測 (`run_handle.await`) を 1 step で行う

#### Scenario: 外部制御の手段

- **WHEN** run task に対する外部制御手段を検査する
- **THEN** adapter 内部には以下の **2 つだけ** が存在する
  - `Sender<RemoteEvent>`（installer が clone 保持）
  - `JoinHandle<Result<(), RemotingError>>`
- **AND** これら以外で run task の `Remote` に触れる経路（直接 method 呼出、raw shared state 経由）を作らない（`RemoteShared` の `Remoting` trait API は許容される）

#### Scenario: addresses クエリは RemoteShared 経由で source of truth から返す

- **WHEN** `Remoting::addresses()`（`RemoteShared::addresses` 経由）が呼ばれる
- **THEN** `RemoteShared::addresses(&self)` が `with_read(|remote| remote.addresses().to_vec())` で内部 `Remote` から owned `Vec<Address>` を返す
- **AND** installer 側の `cached_addresses` を経由しない（キャッシュを持たない）
- **AND** `transport.start()` の戻り値を installer 側 cache に保存する経路や、addresses cache 専用の新規 API は採用しない

### Requirement: adapter 固有 shutdown_and_join での wake + 完了観測

run task の wake と完了観測を 1 step で行う adapter 固有の async surface `RemotingExtensionInstaller::shutdown_and_join(&self) -> impl Future<Output = Result<(), RemotingError>>` を提供する SHALL。**`&self` を取る**（`self` consume ではない、`ExtensionInstaller::install(&self)` 契約と整合し、actor system に登録された installer を一時的に shutdown 用に借りられるようにする）。`RemoteShared::shutdown` は **wake せず**、`event_sender` を持たない。

#### Scenario: shutdown_and_join のシグネチャ

- **WHEN** `RemotingExtensionInstaller::shutdown_and_join` の戻り値型を検査する
- **THEN** `pub async fn shutdown_and_join(&self) -> Result<(), RemotingError>` または `pub fn shutdown_and_join(&self) -> impl Future<Output = Result<(), RemotingError>> + '_` を返す
- **AND** `self` consume ではない（`&self` を取る、actor system に登録されたまま使える）

#### Scenario: shutdown_and_join の手順

- **WHEN** `installer.shutdown_and_join().await` が呼ばれる
- **THEN** 次の 3 ステップを順次実行する
  1. `self.remote_shared.get().ok_or(RemotingError::NotStarted)?.shutdown()` を呼ぶ（lifecycle terminated 遷移、`RemoteShared::shutdown` の純デリゲート。既に停止要求済みまたは停止済みなら no-op `Ok(())`）
  2. `self.event_sender.get()` で `Sender` を取得し、`try_send(RemoteEvent::TransportShutdown)` で wake（同期 try_send、`await` しない。`TransportShutdown` handler は既に停止要求済み/停止済みなら no-op）
  3. `self.run_handle.lock().take()` で `JoinHandle` を取得し、存在すれば `await` で run task の終了を観測する
- **AND** ステップ 3 の戻り値 `Result<Result<(), RemotingError>, JoinError>` を `Ok(Ok(())) → Ok(())` / `Ok(Err(e)) → Err(e)` / `Err(_) → Err(RemotingError::TransportUnavailable)` に変換して呼出元に返す

#### Scenario: shutdown と try_send の失敗扱い（握りつぶし禁止に従う）

- **WHEN** ステップ 1 の `RemoteShared::shutdown` が `Err` を返す
- **THEN** その `Err` は呼出元に伝播する（`?` または `match` で）
- **AND** 既に停止要求済みまたは停止済みの場合は、`RemoteShared::shutdown` 自体が no-op `Ok(())` を返すため、error を idempotent として握りつぶす分岐を作らない
- **WHEN** ステップ 2 の `event_sender.try_send` が `Err(TrySendError::Full)` または `Err(TrySendError::Closed)` を返す
- **THEN** best-effort wake のため log 記録（`tracing::debug!` 等）に留め、shutdown_and_join 自体は継続する（直前コメントで「Full: event 待ちが滞留、次回 is_terminated() 観測で停止 / Closed: receiver drop 済、handle.await で観測される」と明記）
- **AND** `let _ = ...` での無言握りつぶしを行わない（`if let Err(e) = ...` で error 値を log に渡す）
- **WHEN** ステップ 3 の `JoinHandle::await` が `Err(JoinError)` を返す
- **THEN** `tracing::error!` で log 記録した上で `RemotingError::TransportUnavailable` に変換して伝播する

#### Scenario: RemoteShared::shutdown は wake しない

- **WHEN** `Remoting::shutdown`（`RemoteShared::shutdown` 経由）が単独で呼ばれる
- **THEN** lifecycle terminated 遷移のみを行う（`with_write(|r| r.shutdown())` の純デリゲート）
- **AND** `event_sender.try_send` を内部で呼ばない（`RemoteShared` は `event_sender` を持たない）
- **AND** run task は次の event 受信時に `Remote::handle_remote_event` 末尾で lifecycle terminated を観測してループ終了する。`poll_recv(cx)` が `Poll::Pending` のまま event が来なければ即座には停止しない

#### Scenario: 同期 shutdown の制約

- **WHEN** `Remoting::shutdown`（`RemoteShared::shutdown` 経由）が呼ばれる
- **THEN** `event_sender.send(...).await` または `run_handle.await` を内部で実行しない
- **AND** `Remoting` trait には `async fn` および `Future` 戻り値を追加しない
- **AND** run task の終了完了まで保証したように見せない（完了保証が必要なら `installer.shutdown_and_join().await` を使う）

### Requirement: Codec 経路の明文化

`Remote::handle_remote_event` は inbound 側で adapter から渡された core wire frame bytes を既存 core wire codec（`EnvelopeCodec` / `HandshakeCodec` / `ControlCodec` / `AckCodec`）で復号してから `Association` に渡す SHALL。outbound 側は現行 port 境界を維持し、`Association::next_outbound` の戻り値である `OutboundEnvelope` をそのまま `RemoteTransport::send` に渡す SHALL。core 側で `Codec<OutboundEnvelope>` / `Codec<InboundEnvelope>` を新設して raw bytes を `RemoteTransport::send` に渡してはならない（MUST NOT）。

#### Scenario: inbound decode の経路

- **WHEN** `Remote::handle_remote_event` が `RemoteEvent::InboundFrameReceived { authority, frame, now_ms }` を受信する
- **THEN** core wire frame header の kind に応じて `EnvelopeCodec` / `HandshakeCodec` / `ControlCodec` / `AckCodec` のいずれかで復号する
- **AND** 復号した PDU を該当 association の dispatch 経路に渡し、state transition に必要な時刻には `now_ms` を使う

#### Scenario: outbound encode の経路

- **WHEN** `Remote::handle_remote_event` が `Association::next_outbound()` で `OutboundEnvelope` を取得する
- **THEN** `RemoteTransport::send(envelope)` を呼ぶ
- **AND** core 側で raw bytes 化しない（wire encode は transport adapter の責務）

### Requirement: outbound watermark backpressure の発火経路

`Remote::handle_remote_event` は outbound enqueue / dequeue のたびに `Association::total_outbound_len()` を `RemoteConfig::outbound_high_watermark` / `outbound_low_watermark` と比較し、watermark 境界をエッジで跨いだ時にのみ `Association::apply_backpressure(BackpressureSignal::Apply)` または `Release` を発火する SHALL。境界を跨がない通常の enqueue / dequeue では発火しない。

#### Scenario: high watermark で Apply（エッジでのみ発火）

- **WHEN** `Remote::handle_remote_event` が outbound enqueue 直後に `Association::total_outbound_len()` を確認し、enqueue 前は `outbound_high_watermark` 以下、enqueue 後は超過になった（境界を跨いだ）
- **THEN** `Remote::handle_remote_event` は `Association::apply_backpressure(BackpressureSignal::Apply)` を呼ぶ
- **AND** 該当 instrument の `record_backpressure(.., BackpressureSignal::Apply, ..)` が呼ばれる
- **AND** 既に超過状態で連続 enqueue した場合、2 回目以降は `apply_backpressure` を呼ばない

#### Scenario: low watermark で Release（エッジでのみ発火）

- **WHEN** `Remote::handle_remote_event` が outbound dequeue 直後に `Association::total_outbound_len()` を確認し、dequeue 前は `outbound_low_watermark` 以上で Apply 状態、dequeue 後は下回った（境界を跨いだ）
- **THEN** `Remote::handle_remote_event` は `Association::apply_backpressure(BackpressureSignal::Release)` を呼ぶ
- **AND** 該当 instrument の `record_backpressure(.., BackpressureSignal::Release, ..)` が呼ばれる
- **AND** 既に Release 済み状態で連続 dequeue した場合、2 回目以降は `apply_backpressure` を呼ばない

#### Scenario: 設定値の経路

- **WHEN** `RemoteConfig` のフィールドを検査する
- **THEN** `pub outbound_high_watermark: usize` と `pub outbound_low_watermark: usize` が宣言され、`outbound_low_watermark < outbound_high_watermark` を validation する

### Requirement: 戻り値の握りつぶし禁止

`RemoteRunFuture` / `RemoteSharedRunFuture` 内で `RemoteEventReceiver::poll_recv(cx)` の `Poll<Option<RemoteEvent>>` は明示的に扱う SHALL。`Remote::handle_remote_event` 内で `Result` 戻り値（`RemoteTransport::*`、`Codec::*` 等）を `let _ = ...` で握りつぶしてはならない（MUST NOT）。

#### Scenario: 戻り値の明示的扱い

- **WHEN** `Remote::handle_remote_event` の実装ソースを検査する
- **THEN** `let _ = ...` による `Result` 握りつぶしが存在しない
- **AND** 失敗は `?` で伝播するか、`match` で観測可能な経路（log / metric / instrument）に分岐する


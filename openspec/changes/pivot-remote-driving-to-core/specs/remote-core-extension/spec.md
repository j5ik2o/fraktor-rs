## ADDED Requirements

### Requirement: RemoteEvent enum の存在

`fraktor_remote_core_rs::core::extension::RemoteEvent` enum が定義され、adapter から core への通知種別を closed enum として表現する SHALL。

#### Scenario: RemoteEvent の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_event.rs` を読む
- **THEN** `pub enum RemoteEvent` が定義されている

#### Scenario: 必要なバリアントの宣言

- **WHEN** `RemoteEvent` のバリアント一覧を検査する
- **THEN** 以下のバリアントを **全て** 含み、これら以外を含まない（closed enum、本 change のスコープ）
  - `InboundFrameReceived { authority: TransportEndpoint, frame: alloc::vec::Vec<u8> }`
  - `HandshakeTimerFired { authority: TransportEndpoint, generation: u64 }`
  - `OutboundEnqueued { authority: TransportEndpoint, envelope: OutboundEnvelope }`
  - `ConnectionLost { authority: TransportEndpoint, cause: ConnectionLostCause }`
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

`fraktor_remote_core_rs::core::extension::RemoteEventReceiver` trait が定義され、`RemoteShared::run` が消費する Port を表現する SHALL。

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_event_receiver.rs` を読む
- **THEN** `pub trait RemoteEventReceiver: Send` が定義されている

#### Scenario: recv のシグネチャ

- **WHEN** `RemoteEventReceiver::recv` の定義を読む
- **THEN** `fn recv(&mut self) -> impl core::future::Future<Output = Option<RemoteEvent>> + Send + '_` または `async fn recv(&mut self) -> Option<RemoteEvent>` 形式で宣言されている

#### Scenario: tokio 非依存

- **WHEN** `modules/remote-core/src/core/extension/` 配下の RemoteEvent / RemoteEventReceiver 関連 import を検査する
- **THEN** `tokio` クレートへの参照が存在しない

#### Scenario: RemoteEventSink trait の不在

- **WHEN** `modules/remote-core/src/core/extension/` 配下のソースを検査する
- **THEN** `pub trait RemoteEventSink` または同等の adapter→core push 用 trait が定義されていない（adapter 内部 sender で完結し、純増ゼロ方針を維持する）

### Requirement: Remote は CQS 純粋ロジック層であり run を持たない

`Remote` 構造体は CQS 原則を厳格に守る純粋ロジック層 SHALL。状態を変更する method はすべて `&mut self`（Command）、状態を読む method は `&self`（Query）。`Remote` 自体に並行性責務を持たせてはならない（MUST NOT、内部可変性 / `Arc` / `Mutex` を field に持たない）。`Remote` には `run` method を **持たせない**（MUST NOT、`run` は `RemoteShared` 側に置く）。

#### Scenario: Remote の CQS 遵守

- **WHEN** `impl Remote` ブロックの method 一覧を検査する
- **THEN** 状態を変更する method（`start` / `shutdown` / `quarantine` / `handle_remote_event` / `set_instrument` 等）はすべて `&mut self` を取る
- **AND** 状態を読む method（`addresses` / `lifecycle` / `config` 等）はすべて `&self` を取る

#### Scenario: Remote 内部の並行性吸収責務の不在

- **WHEN** `Remote` の field を検査する
- **THEN** `Arc<Mutex<..>>` / `RwLock<..>` / `Cell<..>` / `RefCell<..>` 等の内部可変性を持つ field が存在しない
- **AND** instrument 用の `Box<dyn RemoteInstrument + Send>` 以外に動的ディスパッチ用の field を持たない

#### Scenario: Remote::run の不在

- **WHEN** `impl Remote` ブロックの method 一覧を検査する
- **THEN** `pub async fn run<..>(..)` または `pub fn run<..>(..)` という名の method が存在しない（`run` は `RemoteShared::run` として実装される）

### Requirement: Remote::handle_remote_event は event 1 件分の dispatch を担う

`Remote` 構造体に inherent method `pub fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<bool, RemotingError>` が定義され、event 1 件分の状態遷移と effect 処理を担当する SHALL。戻り値が `true` ならループ終了を意味する（`TransportShutdown` 受信または lifecycle terminated 観測時）。`Remote` 自体に型パラメータ `<I>` を導入してはならない（MUST NOT、instrument は `Box<dyn RemoteInstrument + Send>` で保持する）。

#### Scenario: handle_remote_event のシグネチャ

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を読む
- **THEN** `impl Remote` ブロックに `pub fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<bool, RemotingError>` または同等のシグネチャが宣言されている
- **AND** `Remote` 自体には型パラメータ `<I>` が宣言されていない

#### Scenario: TransportShutdown で true

- **WHEN** `Remote::handle_remote_event` が `RemoteEvent::TransportShutdown` を受信する
- **THEN** 戻り値は `Ok(true)` であり、`RemoteShared::run` 側はこれを観測してループ終了する

#### Scenario: 復帰不能エラーで Err

- **WHEN** event 処理中に transport が永続的に失敗するなど復帰不能なエラーが発生する
- **THEN** `Remote::handle_remote_event` は `Err(RemotingError::TransportUnavailable)` または同等の variant を返す
- **AND** 戻り値の `Result` を `let _ = ...` で握りつぶす経路は呼び出し側に存在しない

#### Scenario: TransportError から RemotingError への変換

- **WHEN** `Remote::handle_remote_event` 内で `RemoteTransport::send` / `RemoteTransport::schedule_handshake_timeout` 等が `Err(TransportError)` を返す
- **THEN** `Remote::handle_remote_event` は `TransportError` を `RemotingError::TransportUnavailable`（または変換ロジックで対応する `RemotingError` variant）にマップして `?` で伝播する
- **AND** `Codec::encode` / `Codec::decode` の失敗は `RemotingError::CodecFailed` 等の対応 variant にマップする
- **AND** マッピングは `Remote::handle_remote_event` または専用 helper で集約され、呼び出し点ごとにアドホックに `match` する経路を作らない

### Requirement: RemoteShared は Sharing 層として並行性を吸収する（薄いラッパー原則）

`fraktor_remote_core_rs::core::extension::RemoteShared` 型が定義され、`SharedLock<Remote>` を内包する Sharing 層として並行性責務を吸収する SHALL。`#[derive(Clone)]` で複数 clone 可能、すべての公開 method は `&self` を取る。raw `SharedLock<Remote>` を呼び出し側に露出してはならない（MUST NOT）。

**薄いラッパー原則:** `RemoteShared` は `Remote` が知らない責務（tokio sender、event channel、wake 機構、runtime-specific 概念等）を **追加してはならない**（MUST NOT）。すべての公開 method は `with_write` / `with_read` で `Remote` の inherent method にデリゲートするだけに留まる（例外: `run` の per-event lock ループ、これは `with_write` の合成）。

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
- **THEN** すべての公開 method は `&self`（または `self` consume だが `&self` が圧倒的多数）であり、`&mut self` を取る公開 method が存在しない

#### Scenario: raw SharedLock<Remote> を露出しない

- **WHEN** `RemoteShared` の公開 API を検査する
- **THEN** `pub fn inner() -> SharedLock<Remote>` や `pub fn shared_lock() -> SharedLock<Remote>` のような raw lock を返す API が存在しない
- **AND** 内部の `with_write` / `with_read` は `pub(crate)` 以下の visibility に閉じる

#### Scenario: Remote が知らない責務を持たない

- **WHEN** `RemoteShared` の field 構成を検査する
- **THEN** `inner: SharedLock<Remote>` のみが定義されている
- **AND** `event_sender: tokio::sync::mpsc::Sender<...>` / `Box<dyn EventSink>` / wake 用 callback / runtime-specific channel 等の field が **存在しない**
- **AND** core crate（`remote-core`）が `tokio` 等の特定 runtime crate に依存する形になっていない

### Requirement: RemoteShared::run は per-event lock の長期実行ループ

`RemoteShared` に inherent method `pub async fn run<S: RemoteEventReceiver>(&self, receiver: &mut S) -> Result<(), RemotingError>` が定義され、event loop の主導権を core 側に集約する SHALL。各 event の dispatch は `with_write(|remote| remote.handle_remote_event(event))` で行い、ロック区間は event 1 件分のみ。これにより `Clone` で配った他の `RemoteShared` から並行に `Remoting` メソッドを呼べる。

#### Scenario: RemoteShared::run のシグネチャ

- **WHEN** `modules/remote-core/src/core/extension/remote_shared.rs` を読む
- **THEN** `impl RemoteShared` ブロックに `pub async fn run<S>(&self, receiver: &mut S) -> Result<(), RemotingError>` または同等のシグネチャが宣言されている
- **AND** `S: RemoteEventReceiver` が trait bound として要求される

#### Scenario: per-event lock

- **WHEN** `RemoteShared::run` の実装を検査する
- **THEN** event 1 件あたり `with_write(|remote| remote.handle_remote_event(event))` で 1 回の write lock を取り、戻り値が `true` ならループ終了する
- **AND** lock を `await` 越しに保持しない（`receiver.recv().await` 中はロックを取らない）

#### Scenario: receiver 枯渇で Err(EventReceiverClosed)

- **WHEN** `RemoteEventReceiver::recv` が `None` を返す
- **THEN** `RemoteShared::run` は `Err(RemotingError::EventReceiverClosed)` を返してループ終了する

#### Scenario: handle_remote_event の戻り値 true でループ終了

- **WHEN** `Remote::handle_remote_event` が `Ok(true)` を返す
- **THEN** `RemoteShared::run` は `Ok(())` を返してループ終了する

#### Scenario: 並行 Remoting メソッドの進行

- **WHEN** `RemoteShared::run` が走っている間に、別の clone から `RemoteShared::quarantine` / `shutdown` / `addresses` が呼ばれる
- **THEN** これらは `run` の event 処理間の隙間（`receiver.recv().await` 中、または event 1 件処理完了後）で write/read lock を取って進行する
- **AND** lock の取り合いは存在するが、デッドロックや無限待機は発生しない

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
- **AND** **wake はしない**（`RemoteShared` は `event_sender` を持たない、薄いラッパー原則）
- **AND** `event_sender.send(...).await` や `run_handle.await` を内部で **実行しない**（同期 method、`async fn` を増やさない）
- **AND** `Remote` が知らない責務（tokio sender、event push 等）を `RemoteShared::shutdown` 内で実行しない

#### Scenario: 完了保証は adapter 固有 surface で行う

- **WHEN** run task の完了保証を必要とする呼び出し側がいる
- **THEN** adapter 固有の async surface（例: `RemotingExtensionInstaller::shutdown_and_join`、`Remoting` trait 外）を使う
- **AND** 同期 `Remoting::shutdown` は run task の終了完了まで保証したように **見せない**
- **AND** `Remoting::shutdown` 単独呼び出しは「lifecycle terminated に遷移するだけ」のセマンティクスに留まる（run task は次の event 受信時に lifecycle terminated を観測してループ終了する）

### Requirement: 別 Driver 型を新設しない

`RemoteShared::run` の責務を担う `RemoteDriver` / `RemoteDriverHandle` / `RemoteDriverOutcome` 等の新規型を core 側に追加してはならない（MUST NOT）。これらの責務は `RemoteShared::run` の inherent method と `Remoting` trait と `Result<(), RemotingError>` で表現する。

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
- **AND** adapter 側はこの呼出を契機に tokio task で sleep を起動し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を内部 sender 経由で receiver に push する責務を持つ（詳細は `remote-core-transport-port` capability および `remote-adaptor-std-io-worker` capability で要件化）

#### Scenario: 順序保証

- **WHEN** ステップ 1 とステップ 2 の呼出順序を検査する
- **THEN** ステップ 1（`send`）の戻り値を確認してからステップ 2（`schedule_handshake_timeout`）を呼ぶ
- **AND** ステップ 1 が `Err` の場合、ステップ 2 は呼ばれない

### Requirement: RemoteEvent::OutboundEnqueued 処理

`Remote::handle_remote_event` は `RemoteEvent::OutboundEnqueued { authority, envelope }` を受信した際、該当 association に envelope を enqueue し、続けて outbound drain（next_outbound 処理）を実行する SHALL。

#### Scenario: enqueue と drain の連鎖

- **WHEN** `Remote::handle_remote_event` が `RemoteEvent::OutboundEnqueued { authority, envelope }` を受信する
- **THEN** `AssociationRegistry` から `authority` 対応の `Association` を取得し、`Association::enqueue(envelope)` を呼ぶ
- **AND** 続けて outbound drain helper（`next_outbound` → `Codec::encode` → `RemoteTransport::send`）を起動し、可能な限り queue を消化する

#### Scenario: 内部可変性回避

- **WHEN** adapter 側の enqueue 経路（local actor からの tell 等）を検査する
- **THEN** adapter は `AssociationRegistry` を直接 mutate せず、`RemoteEvent::OutboundEnqueued` を内部 sender に push する
- **AND** `AssociationRegistry` の所有権は本 change の主経路では `Remote` に集約されており、adapter 側から raw shared handle 経由で直接 mutate しない

### Requirement: Installer は RemoteShared を保持し外部公開する

adapter 側の `RemotingExtensionInstaller` は `RemoteShared` を field として保持し、`installer.remote() -> RemoteShared` で外部公開しなければならない（MUST）。raw `SharedLock<Remote>` / `Arc<Mutex<Remote>>` / `Arc<Remote>` を field として保持してはならない（MUST NOT）。`installer.remote()` の戻り値型は `RemoteShared` であり、raw `SharedLock<Remote>` を返してはならない（MUST NOT）。

#### Scenario: installer の field 構成

- **WHEN** `RemotingExtensionInstaller` の field を検査する
- **THEN** `remote_shared: RemoteShared` / `event_sender: tokio::sync::mpsc::Sender<RemoteEvent>` / `event_receiver: Option<TokioMpscRemoteEventReceiver>` / `run_handle: Option<JoinHandle<Result<(), RemotingError>>>` 程度のみを保持する
- **AND** raw `SharedLock<Remote>` / `Arc<Mutex<Remote>>` / `Arc<Remote>` の field が存在しない
- **AND** `cached_addresses: Vec<Address>` のような addresses cache field を持たない（`RemoteShared::addresses` で source of truth から取得するため）

#### Scenario: 公開 getter のシグネチャ

- **WHEN** `RemotingExtensionInstaller::remote` の戻り値型を検査する
- **THEN** `pub fn remote(&self) -> RemoteShared`（または `Result<RemoteShared, _>`）を返す
- **AND** raw `SharedLock<Remote>` を返す API が公開されていない

#### Scenario: install と Remote::start と spawn の分離

- **WHEN** `RemotingExtensionInstaller::install` の挙動を検査する
- **THEN** `RemoteShared::new(remote)` で `RemoteShared` を構築する
- **AND** `Remote::start` を呼ばない（外部から `installer.remote().start()` で呼ぶ）
- **AND** run task を `tokio::spawn` で起動しない（明示 API `installer.spawn_run_task()` 等を別途呼ぶ）

#### Scenario: spawn 経路（明示 API）

- **WHEN** `installer.spawn_run_task()` 等の明示 API が呼ばれる
- **THEN** `let run_target = self.remote_shared.clone();` の後 `tokio::spawn(async move { run_target.run(&mut receiver).await })` 相当で起動する
- **AND** spawn 後も installer は `remote_shared` を保持し続け、外部から `installer.remote()` で取得できる
- **AND** `event_receiver` を `take()` で消費し、`run_handle` を `Some` に保存する

### Requirement: 外部制御 surface（adapter 固有 surface との責務分担）

run task の制御経路は次の責務分担で構成される SHALL。

- `Remoting` trait（`RemoteShared` 実装、core 提供）— 4 同期 method（`start` / `shutdown` / `quarantine` / `addresses`）。**lifecycle 状態遷移のみ**を担い、tokio の wake は行わない（`RemoteShared` は `event_sender` を持たない、薄いラッパー原則）
- `Sender<RemoteEvent>`（adapter installer が保持） — `RemoteEvent` を adapter 内部で push（I/O ワーカー / handshake timer task / RemoteActorRef が clone 共有）。`shutdown_and_join` 内で `try_send(TransportShutdown)` の wake にも使う
- `JoinHandle<Result<(), RemotingError>>`（adapter installer が保持）— `installer.shutdown_and_join().await` で完了観測
- `RemotingExtensionInstaller::shutdown_and_join(self) -> impl Future<Output = Result<(), RemotingError>>` — adapter 固有 async surface、wake (`event_sender.try_send`) + 完了観測 (`run_handle.await`) を 1 step で行う

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
- **AND** `transport.start()` の戻り値を直接キャッシュに使う経路や `Remote::start` 等の新規 API は採用しない

### Requirement: adapter 固有 shutdown_and_join での wake + 完了観測

run task の wake と完了観測を 1 step で行う adapter 固有の async surface `RemotingExtensionInstaller::shutdown_and_join(self) -> impl Future<Output = Result<(), RemotingError>>` を提供する SHALL。`RemoteShared::shutdown` は **wake せず**、`event_sender` を持たない。

#### Scenario: shutdown_and_join の手順

- **WHEN** `installer.shutdown_and_join().await` が呼ばれる
- **THEN** 次の 3 ステップを順次実行する
  1. `self.remote_shared.shutdown()` を呼ぶ（`RemoteShared::shutdown` 経由で lifecycle terminated 遷移、純デリゲート）
  2. `self.event_sender.try_send(RemoteEvent::TransportShutdown)` で wake（同期 try_send、`await` しない、Full / Closed 失敗は無視）
  3. `self.run_handle.take().unwrap().await` で run task の終了を観測する
- **AND** ステップ 3 の戻り値 `Result<Result<(), RemotingError>, JoinError>` を `Ok(Ok(())) → Ok(())` / `Ok(Err(e)) → Err(e)` / `Err(_) → Err(RemotingError::TransportUnavailable)` に変換して呼出元に返す

#### Scenario: RemoteShared::shutdown は wake しない

- **WHEN** `Remoting::shutdown`（`RemoteShared::shutdown` 経由）が単独で呼ばれる
- **THEN** lifecycle terminated 遷移のみを行う（`with_write(|r| r.shutdown())` の純デリゲート）
- **AND** `event_sender.try_send` を内部で呼ばない（`RemoteShared` は `event_sender` を持たない）
- **AND** run task は次の event 受信時に `Remote::handle_remote_event` 末尾で lifecycle terminated を観測してループ終了する。`recv().await` で blocked のまま event が来なければ即座には停止しない

#### Scenario: 同期 shutdown の制約

- **WHEN** `Remoting::shutdown`（`RemoteShared::shutdown` 経由）が呼ばれる
- **THEN** `event_sender.send(...).await` または `run_handle.await` を内部で実行しない
- **AND** `Remoting` trait には `async fn` および `Future` 戻り値を追加しない
- **AND** run task の終了完了まで保証したように見せない（完了保証が必要なら `installer.shutdown_and_join().await` を使う）

### Requirement: Codec 経路の明文化

`Remote::handle_remote_event` は inbound 側で raw frame を `Codec::decode` で復号してから `Association` に渡し、outbound 側で `Association::next_outbound` の戻り値を `Codec::encode` で raw bytes 化してから `RemoteTransport` に渡す SHALL。

#### Scenario: inbound decode の経路

- **WHEN** `Remote::handle_remote_event` が `RemoteEvent::InboundFrameReceived { authority, frame }` を受信する
- **THEN** `Codec<InboundEnvelope>::decode(&frame)` で復号する
- **AND** 復号した `InboundEnvelope` を該当 association の dispatch 経路に渡す

#### Scenario: outbound encode の経路

- **WHEN** `Remote::handle_remote_event` が `Association::next_outbound()` で `OutboundEnvelope` を取得する
- **THEN** `Codec<OutboundEnvelope>::encode(&envelope)` で raw bytes 化する
- **AND** その raw bytes を `RemoteTransport::send` または同等の API に渡す

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

`Remote::handle_remote_event` 内で `RemoteEventReceiver::recv`（戻り値 `Option`）以外の `Result` 戻り値（`RemoteTransport::*`、`Codec::*` 等）を `let _ = ...` で握りつぶしてはならない（MUST NOT）。

#### Scenario: 戻り値の明示的扱い

- **WHEN** `Remote::handle_remote_event` の実装ソースを検査する
- **THEN** `let _ = ...` による `Result` 握りつぶしが存在しない
- **AND** 失敗は `?` で伝播するか、`match` で観測可能な経路（log / metric / instrument）に分岐する

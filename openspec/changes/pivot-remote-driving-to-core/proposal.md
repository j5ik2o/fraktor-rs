## Why

`remote-core` は Pekko Artery 互換の状態機械、Port trait、wire model を no_std で備えるが、現在は駆動の主導権が `remote-adaptor-std` 側の tokio task 群に握られている。`outbound_loop` が 1ms ポーリングで `Association::next_outbound` を回し、`inbound_dispatch` が `accept_handshake_*` を直接呼び、`HandshakeDriver` が timeout を駆動する。`AssociationEffect::StartHandshake` は adaptor 側で無視されており、core が表明した意図が adapter に伝わらない設計欠陥が残っている。

これにより以下の問題が生じる。

- 組み込み / WASM 等で std を使えない環境では tokio task 群を移植する必要があり、core 主導という設計意図が機能しない。
- `remote-core` には 604 行の `RemoteInstrument` / `RemotingFlightRecorder` が定義されているが、`Association` から一度も呼ばれておらず、配信失敗・handshake 進捗・quarantine が観測不能である。
- Pekko Artery が保証する system message の飢餓回避は既存の system / user 2 キュー分離で成立しているが、双方向の watermark backpressure と handshake generation 管理が未実装である。
- `Remote` から `Codec`、`RemoteTransport`、association registry を駆動する経路が暗黙で、event loop の lifecycle（起動 / 停止）が文書化されていない。

正式リリース前の今、Port & Adapter の純度を上げ、駆動主導権を core 側に反転する。adapter は I/O と event 通知だけを担当する形に退化させる。

## 設計方針

**二層構造を採用し、CQS 純粋ロジック層と並行性吸収層を分離する。** `Remote` 自体は `&mut self` / `&self` で CQS を守るピュアロジックに保ち、並行性は `RemoteShared(SharedLock<Remote>)` ラッパーが吸収する。`Remoting` trait は `RemoteShared` 側に実装され、すべて `&self` ベースに揃う。

具体的に追加する公開要素は次の **3 つ** とする（純増 +3）。

- `RemoteEvent` enum（adapter が core に通知するイベント種別、closed enum）
- `RemoteEventReceiver` trait（core が adapter から event を pull する 1 メソッド trait）
- `RemoteShared` 型（`SharedLock<Remote>` の Sharing 層ラッパー、`Clone`、`impl Remoting`、`run` を提供）

これ以外の機能（駆動ループの中身、handshake generation 管理、watermark backpressure、instrument 配線）は **既存型のメソッド・フィールド追加** で実現する。

## What Changes

### 1. `RemoteShared::run` で駆動主導権を core に集約する（二層構造で `Remote` のCQSを保つ）

`Remote` 構造体には `run` を持たせず、CQS 純粋ロジック（`&mut self` for Command, `&self` for Query）に保つ。代わりに event 1 件分の dispatch を担う `Remote::handle_remote_event(&mut self, event: RemoteEvent) -> Result<bool, RemotingError>` を inherent method として追加する。戻り値が `true` ならループ終了を意味する（`TransportShutdown` 受信または lifecycle terminated 観測時）。

並行性吸収層として `RemoteShared(SharedLock<Remote>)` を新設する。`#[derive(Clone)]` で複数 clone 可能、`SharedLock` 内部で `Remote` の所有権を保持する。

```rust
impl RemoteShared {
    pub async fn run<S: RemoteEventReceiver>(&self, receiver: &mut S) -> Result<(), RemotingError> {
        loop {
            let Some(event) = receiver.recv().await else {
                return Err(RemotingError::EventReceiverClosed);
            };
            // per-event lock: ロック区間は handle_remote_event 1 回分のみ
            let done = self.with_write(|remote| remote.handle_remote_event(event))?;
            if done { return Ok(()); }
        }
    }
}
```

per-event lock により、`run` task と並行して他の `RemoteShared` clone から `Remoting` メソッド（`start` / `shutdown` / `quarantine` / `addresses`）を呼べる。event 処理間の隙間で進行する。

`Remote::handle_remote_event` 内で receiver からイベントを dispatch し、対応する `Association` メソッドを呼んで effect 列を実行する。`AssociationEffect::StartHandshake` を復活させ、`RemoteTransport` 経由で handshake を開始する。

新規 core 型（`RemoteDriver` / `RemoteDriverHandle` / `RemoteDriverOutcome`）は **作らない**。`RemoteShared::run` の終了結果は `Result<(), RemotingError>` で表現する。run task の停止要求は `Remoting::shutdown`（lifecycle terminated 遷移 + `event_sender.try_send(TransportShutdown)` の best-effort wake）で行い、完了観測は adapter 固有の async wait surface で `JoinHandle::await` する。同期 `Remoting::shutdown` の内部で `await` してはならない。

### 2. `RemoteEvent` を closed enum、`RemoteEventReceiver` を 1 メソッド trait として追加する

`RemoteEvent` は core が adapter から受け取る closed enum。本 change のスコープでは次の 5 variant **のみ** を含み、scheduling 経路が確定していない event は別 change で variant 追加と一緒に拡張する。

- `InboundFrameReceived { authority, frame }` — TCP 受信 frame
- `HandshakeTimerFired { authority, generation: u64 }` — handshake timeout 満了
- `OutboundEnqueued { authority, envelope }` — local actor からの送信要求（後述 #10）
- `ConnectionLost { authority, cause }` — 接続切断
- `TransportShutdown` — 全体停止指示

本 change で **追加しない** variant: `OutboundFrameAcked` / `QuarantineTimerFired` / `BackpressureCleared`（必要時に scheduling 経路と一緒に別 change で導入）。

`RemoteEventReceiver::recv` は `&mut self` で `Option<RemoteEvent>` を非同期に返す。adapter は `tokio::sync::mpsc::Receiver` 等で実装する。

**adapter→core push 用の `RemoteEventSink` trait は core に追加しない**。adapter は内部で sender / receiver pair を保持し、receiver 側だけを `RemoteEventReceiver` として core に渡す。sender 側は adapter 内部の I/O ワーカー / handshake timer task / RemoteActorRef が clone 共有するため、core から見ない。

### 3. handshake timer 予約は `RemoteTransport::schedule_handshake_timeout` で表出する

handshake timer の予約責務は **adapter 側** に置くが、core から adapter に予約を依頼する経路は既存 `RemoteTransport` trait に新 method を追加して表現する。

```rust
// 既存 trait への追加 (modification of remote-core-transport-port)
pub trait RemoteTransport {
    // ...既存 method...
    fn send_handshake(&mut self, remote: &Address, pdu: HandshakePdu) -> Result<(), TransportError>;
    fn schedule_handshake_timeout(
        &mut self,
        authority: &TransportEndpoint,
        timeout: Duration,
        generation: u64,
    ) -> Result<(), TransportError>;
}
```

`Remote::handle_remote_event` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を処理する手順：

1. `HandshakePdu::Req(HandshakeReq::new(local, remote))` を構築し、`RemoteTransport::send_handshake` で送出
2. 続けて新 method `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)` を呼ぶ

adapter 側 `schedule_handshake_timeout` 実装は `tokio::spawn(sleep + push HandshakeTimerFired)` を行う。

**棄却した代替**:

- `Timer` Port 新設 — 純増増える、すでに棄却済み
- `RemoteTransport::initiate_handshake(authority, timeout, generation, frame_bytes)` 統合形 — `send` との責務混在
- 新 effect `ScheduleHandshakeTimeout` + 新 trait — 純増 2 個増える

quarantine timer 等、handshake 以外の timer 系経路は本 change のスコープ外とし、別 change で variant 追加 + scheduling 経路の MODIFIED を一緒に行う。

### 4. `RemoteInstrument` を `Box<dyn>` で `Remote` に配線する（ジェネリクス採用しない）

`Remote` は型パラメータを持たず、`instrument: Box<dyn RemoteInstrument + Send>` フィールドで instrument を保持する。

ジェネリクス `Remote<I: RemoteInstrument = ()>` を採用しない理由：

- 参照実装（Apache Pekko の `RemoteInstrument` abstract class、protoactor-go の interface）はいずれも virtual / dyn dispatch を採用しており、production 規模で問題なく動いている
- hot path での vtable lookup は ~1-2ns 程度であり、tokio mpsc send / codec encode / mutex acquisition 等のコストに対して noise レベル
- ジェネリクスを採用するとテスト・showcase・cluster adapter まで `<I>` が伝播し、ユーザー API が複雑化する
- 実行時に instrument を差し替えできなくなる
- `tracing-rs` / `metrics` / `opentelemetry-rs` 等の Rust 観測ライブラリも dyn 経由が通例

既定 instrument は `pub(crate) struct NoopInstrument` を内部定義し、`Remote::new` で `Box::new(NoopInstrument)` を割り当てる。**`NoopInstrument` は `pub(crate)` で外部公開せず**、ユーザーは `Remote::new` を呼ぶだけで no-op 既定が得られる。

`Remote::with_instrument(transport, config, event_publisher, instrument: Box<dyn RemoteInstrument + Send>)` および `Remote::set_instrument(&mut self, instrument: Box<dyn RemoteInstrument + Send>)` を公開し、ユーザーは構築時または構築後に instrument を差し替えられる。

複数 instrument の合成は **ユーザー責務** とする（独自 composite struct を定義して `RemoteInstrument` を実装）。core 側で tuple impl などの composite ヘルパは提供しない（YAGNI、Pekko の `Vector[RemoteInstrument]` 同等の構造はユーザーが必要に応じて書く）。

`Remote::associate` / `accept_handshake_*` / `quarantine` / `next_outbound` / inbound dispatch / `apply_backpressure` から instrument の対応 method を呼ぶ経路を確定する（呼び出し点は `remote-core-association-state-machine` capability で要件化）。`Association` メソッドは `&mut dyn RemoteInstrument` を引数で受け取り、型パラメータは導入しない。

### 5. system message 飢餓回避は既存の system / user 2 キュー分離で維持する

`Association::SendQueue` は既存仕様（`remote-core-association-state-machine` capability）で system priority と user priority の 2 キュー分離を持ち、system 優先で取り出す挙動が規定されている。本 change ではこの構造を維持し、Pekko Artery の Control / Ordinary 分離と同等の飢餓回避を継続する。

新規 query として `Association::total_outbound_len(&self) -> usize`（system + user の合計長、deferred は除く）のみを追加する。

### 6. 双方向 watermark backpressure を導入する（既存 BackpressureSignal を流用）

`RemoteConfig` に `outbound_high_watermark` / `outbound_low_watermark` を追加し、queue 長が high を超えると `Association::apply_backpressure(BackpressureSignal::Apply)` を発火、low を下回ると `BackpressureSignal::Release` を発火する。

**新規 variant（`Engaged` / `Released`）は追加せず、既存 `Apply` / `Release` をそのまま使う**。signal は `RemoteInstrument::record_backpressure` 経由で観測可能とする。

### 7. handshake generation を inline `u64` で管理する

`Association` に `handshake_generation: u64` フィールドを追加し、`Handshaking` 状態に入るたびに +1 する。`AssociationEffect::StartHandshake { authority, generation }` と `RemoteEvent::HandshakeTimerFired { authority, generation }` で同じ `u64` を運び、古い timeout の発火を `Remote::handle_remote_event` 側で識別して破棄する。

**`HandshakeGeneration` newtype は新設せず、`u64` を直接使う**（外部公開境界での意味付けは rustdoc に依存し、型レベルでは追加しない）。

### 8. adaptor task を I/O ワーカーに退化させる

`remote-adaptor-std` の以下を削除する。

- `outbound_loop.rs`（1ms ポーリングで `next_outbound` を回す tokio task）
- `handshake_driver.rs`（timeout を tokio sleep で駆動する task）

`inbound_dispatch.rs` は `RemoteEvent::InboundFrameReceived` を adapter 内部 sender に push する I/O ワーカーに退化させる。`Association` の状態遷移メソッドを直接呼ぶ責務を `Remote::handle_remote_event`（`RemoteShared::run` の `with_write` 区間内で実行される）に移す。

`effect_application.rs` の `StartHandshake` ignore 分岐を削除する（`Remote::handle_remote_event` が処理するため adapter ではすでに通らない）。

`RemotingExtensionInstaller` は `RemoteShared` を field として保持し、`installer.remote() -> RemoteShared` で外部公開する。raw `SharedLock<Remote>` は露出しない（後述 #9 参照）。

### 9. `RemoteShared(SharedLock<Remote>)` を正式 surface とし、`Remoting` を `RemoteShared` に実装する

`Remote::run(self, ..)` で run task が単独所有する設計は採らない。理由は次の通り:

- `installer.remote() -> SharedLock<Remote>` の現状実装の移行先がない（破壊的変更の経路として `RemoteShared` が必要）
- `Remoting::quarantine` を run task と並行して呼ぶ手段が消える（`Remote` を消費すると外部から触れない）
- `RemoteEvent` の closed enum に `quarantine` 用 variant を追加すると本 change のスコープを超える

代わりに `RemoteShared(SharedLock<Remote>)` を新設し、`run` を `RemoteShared::run(&self, ..)` の per-event lock ループとして置く。`Remoting` trait は `RemoteShared` 側に実装する。

#### `Remoting` trait シグネチャの変更（破壊的）

`Remoting` trait の全メソッドを `&self` ベースへ変更する。`addresses` は内部 read lock のため `&[Address]` ではなく owned `Vec<Address>` を返す。

| メソッド | 旧 | 新 |
|----------|----|----|
| `start` | `fn start(&mut self) -> Result<(), RemotingError>` | `fn start(&self) -> Result<(), RemotingError>` |
| `shutdown` | `fn shutdown(&mut self) -> Result<(), RemotingError>` | `fn shutdown(&self) -> Result<(), RemotingError>` |
| `quarantine` | `fn quarantine(&mut self, ..) -> Result<(), RemotingError>` | `fn quarantine(&self, ..) -> Result<(), RemotingError>` |
| `addresses` | `fn addresses(&self) -> &[Address]` | `fn addresses(&self) -> Vec<Address>` |

すべて同期 method（`async fn` / `Future` 戻り値は追加しない）。`impl Remoting for Remote` は **削除** する。`impl Remoting for RemoteShared` を新設し、内部で `with_write` / `with_read` してデリゲートする。

#### Installer の構造

```text
RemotingExtensionInstaller {
    remote_shared: RemoteShared,                          // Remoting API として外部公開
    event_sender: tokio::sync::mpsc::Sender<RemoteEvent>, // try_send による best-effort wake
    run_handle:   JoinHandle<Result<(), RemotingError>>,  // async wait surface で await
}

pub fn remote(&self) -> RemoteShared {
    self.remote_shared.clone()  // raw SharedLock<Remote> は露出しない
}
```

#### `Remoting::shutdown` の挙動

`RemoteShared::shutdown(&self)` は同期 method:

1. `with_write(|r| r.shutdown())` で `Remote::shutdown` を呼び lifecycle を terminated に遷移
2. `event_sender.try_send(RemoteEvent::TransportShutdown)` で run loop を best-effort wake（失敗は無視、次の event 処理で lifecycle 観測されてループは正常終了する）

`Remoting::shutdown` は `remoting.rs` の契約通り `async fn` / `Future` を返さないため、`event_sender.send(...).await` や `run_handle.await` を内部で実行しない。完了観測は adapter 固有の async wait surface で `run_handle.await` を行う。同期 API が run task の終了完了まで保証したように見せてはならない。

#### `Remoting::addresses()` の実装

`RemoteShared::addresses(&self)` は `with_read(|r| r.addresses().to_vec())` で内部 `Remote` から source of truth を返す。**キャッシュは持たない**（読み取りのたびに read lock + clone）。listening address の動的更新（NAT / port reassignment）にも将来追従できる。

### 10. tokio ベース `RemoteEventReceiver` 実装と `OutboundEnqueued` enqueue 経路を追加する

`remote-adaptor-std` に tokio mpsc 受信側を `RemoteEventReceiver` として実装した型を 1 つ追加する。送信側 sender clone は adapter 内部の I/O ワーカー / handshake timer task / RemoteActorRef が保持する（adapter 内部のため公開 API ではない）。

local actor から remote ref への tell で発生する outbound enqueue は、新 variant `RemoteEvent::OutboundEnqueued { authority, envelope }` を adapter 内部 sender に push することで `RemoteShared::run` を起こす。

```text
local actor.tell → adapter RemoteActorRef
                   → OutboundEnvelope 構築
                   → Sender::send(RemoteEvent::OutboundEnqueued { authority, envelope })
                   → RemoteShared::run が event 受信
                     → with_write(|remote| remote.handle_remote_event(event))
                       → Association::enqueue(envelope)
                       → outbound drain (next_outbound → Codec::encode → Transport::send)
```

これにより `outbound_loop` 削除後の wake 問題（peer が silent な間 outbound queue が drain されない）が解消する。`AssociationRegistry` の所有権は `Remote` に集約され（`Remote` は `RemoteShared` の `SharedLock` 内に常駐）、`Remote` 自身は内部可変性を持たず CQS を維持する。並行性は `RemoteShared` の `with_write` / `with_read` で吸収する。

zero-copy / per-authority channel 分離は本 change のスコープ外とし、別 change での最適化余地として残す。

## Capabilities

### Modified Capabilities

- **`remote-core-extension`**
  - `Remote` に `fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<bool, RemotingError>` を追加（event 1 件分の dispatch、戻り値 true でループ終了）
  - `RemoteShared(SharedLock<Remote>)` 型を新設（`Clone`、`run` を提供、`impl Remoting`）
  - `RemoteShared` に `pub async fn run<S: RemoteEventReceiver>(&self, receiver: &mut S) -> Result<(), RemotingError>` を追加（per-event lock ループ）
  - `RemoteEvent` enum と `RemoteEventReceiver` trait を core 公開 API として追加
  - `RemoteEvent` の variant は `InboundFrameReceived` / `HandshakeTimerFired` / `OutboundEnqueued` / `ConnectionLost` / `TransportShutdown` の 5 つに限定（closed enum）
  - `Remote::handle_remote_event` は `AssociationEffect::StartHandshake` を 2 ステップ（send_handshake + schedule_handshake_timeout）で処理
  - `Remote::handle_remote_event` は `RemoteEvent::OutboundEnqueued` を受けて `Association::enqueue` + drain
  - `Remoting` trait の全メソッドを `&self` ベースへ変更（破壊的）。`addresses` は `Vec<Address>` 戻り値。すべて同期 method（`async fn` を増やさない）
  - `impl Remoting for Remote` を **削除**、`impl Remoting for RemoteShared` を新設（`with_write` / `with_read` で内部 `Remote` にデリゲート）
  - `Remote` 自身は CQS 純粋ロジックを保つ（`run` を持たない、内部可変性を持たない）

- **`remote-core-transport-port`**
  - `RemoteTransport` trait に `fn schedule_handshake_timeout(&mut self, authority: &TransportEndpoint, timeout: Duration, generation: u64) -> Result<(), TransportError>` を追加（同期 method、`async fn` は使わない）
  - 他の timer 系 method（quarantine timer 等）は本 change で追加しない（必要時に別 change）

- **`remote-core-instrument`**
  - `Remote` は型パラメータを持たず、`Box<dyn RemoteInstrument + Send>` で instrument を保持する
  - 既定 instrument は `pub(crate) struct NoopInstrument`（`Remote::new` 内部で `Box::new(NoopInstrument)` を割り当てる、外部公開しない）
  - `Remote::with_instrument(...)` および `Remote::set_instrument(...)` で差し替え可能
  - tuple composite / `() impl` は提供しない（複数 instrument 合成はユーザー責務）
  - `Arc<dyn RemoteInstrument>` を hot path で clone しない（所有 `Box<dyn>` 経由）

- **`remote-core-association-state-machine`**
  - instrument hook を `associate` / `handshake_accepted` / `handshake_timed_out` / `quarantine` / `next_outbound` / inbound dispatch / `apply_backpressure` から呼ぶ
  - 既存の system / user 2 キュー分離は維持する
  - watermark 連動のため `total_outbound_len(&self)` クエリを追加する
  - `handshake_generation: u64` フィールドを追加する（newtype は作らない）
  - `AssociationEffect::StartHandshake { authority, generation }` のセマンティクスを「`Remote::handle_remote_event` で実行（`RemoteShared::run` の `with_write` 区間内）」と明示し、adapter 無視を禁止する
  - `BackpressureSignal` の variant は既存 `Apply` / `Release` を維持する（新 variant 追加なし）

- **`remote-adaptor-std-io-worker`**
  - `outbound_loop` / `handshake_driver` を REMOVED
  - `inbound_dispatch` は `RemoteEvent::InboundFrameReceived` を adapter 内部 sender に push する I/O ワーカーに退化
  - tokio mpsc 受信側を `RemoteEventReceiver` として実装した型を 1 つ追加
  - `RemotingExtensionInstaller` は `RemoteShared` を field として保持し、`installer.remote() -> RemoteShared` で外部公開する。raw `SharedLock<Remote>` を露出しない
  - `RemotingExtensionInstaller::install` で `RemoteShared::new(remote)` を構築し、clone 1 個を spawn 用に渡す。spawn 経路は `tokio::spawn(async move { remote_shared_clone.run(&mut receiver).await })`
  - `Remoting::addresses()` は `RemoteShared::addresses()` 経由で内部 `Remote` から source of truth を返す（キャッシュは持たない）
  - 同期 `Remoting::shutdown` の中では `await` しない。`Remote::shutdown` を `with_write` で呼んで lifecycle terminated 遷移し、`event_sender.try_send(TransportShutdown)` で best-effort wake する
  - 完了観測は adapter 固有の async wait surface で `JoinHandle::await` を行う
  - `RemoteTransport::schedule_handshake_timeout` 実装で tokio task の sleep + push を行う
  - adapter 側 RemoteActorRef 等の outbound 経路は `RemoteEvent::OutboundEnqueued` を adapter 内部 sender に push する（`AssociationRegistry` の直接 mutate を禁止）
  - `effect_application.rs` の `StartHandshake` ignore 分岐を削除

### New Capabilities

なし（既存 capability への変更で完結）。

## Impact

**影響を受けるコード:**

- `modules/remote-core/src/core/extension/remote.rs`（`handle_remote_event` inherent method 追加、`Box<dyn RemoteInstrument + Send>` field、`with_instrument` / `set_instrument` 追加、`impl Remoting for Remote` 削除）
- `modules/remote-core/src/core/extension/remote_shared.rs`（新規、`RemoteShared(SharedLock<Remote>)`、`run` / `impl Remoting for RemoteShared`）
- `modules/remote-core/src/core/extension/remoting.rs`（trait シグネチャを `&self` ベースへ変更、`addresses` を `Vec<Address>` 戻り値へ）
- `modules/remote-core/src/core/extension/remote_event.rs`（新規、closed enum 5 variant）
- `modules/remote-core/src/core/extension/remote_event_receiver.rs`（新規、1 メソッド trait）
- `modules/remote-core/src/core/transport/remote_transport.rs`（既存 trait に `schedule_handshake_timeout` method 追加）
- `modules/remote-core/src/core/association/base.rs`（`&mut dyn RemoteInstrument` 引数追加、watermark 連動、handshake_generation field、total_outbound_len）
- `modules/remote-core/src/core/association/effect.rs`（`StartHandshake { authority, timeout, generation: u64 }` 拡張、rustdoc 更新）
- `modules/remote-core/src/core/association/registry.rs`（instrument 参照経路、queue 分離追従）
- `modules/remote-core/src/core/instrument/`（`pub(crate) NoopInstrument` 内部定義、`RemotingFlightRecorder` への `RemoteInstrument` impl 追加）
- `modules/remote-core/src/core/config/`（`outbound_high_watermark` / `outbound_low_watermark`）
- `modules/remote-adaptor-std/src/std/outbound_loop.rs`（削除）
- `modules/remote-adaptor-std/src/std/handshake_driver.rs`（削除）
- `modules/remote-adaptor-std/src/std/inbound_dispatch.rs`（I/O ワーカーへ縮退、`InboundFrameReceived` push のみ）
- `modules/remote-adaptor-std/src/std/effect_application.rs`（`StartHandshake` ignore 削除）
- `modules/remote-adaptor-std/src/std/tokio_remote_event_receiver.rs`（新規、tokio mpsc 受信ラッパ）
- `modules/remote-adaptor-std/src/std/extension_installer/remoting_extension_installer.rs`（`RemoteShared` 保持、`installer.remote() -> RemoteShared`、spawn 経路、shutdown プロトコル）
- `modules/remote-adaptor-std/src/std/`（adapter 側 `RemoteTransport::schedule_handshake_timeout` 実装で `tokio::spawn(sleep)`）
- `modules/remote-adaptor-std/src/std/`（RemoteActorRef 等の outbound 経路を `OutboundEnqueued` push に切替）

**ファイル収支試算:**

- core: 新規 3（`remote_event.rs` / `remote_event_receiver.rs` / `remote_shared.rs`）、削除 0
- adapter: 新規 1（`tokio_remote_event_receiver.rs`）、削除 2（`outbound_loop.rs` / `handshake_driver.rs`）
- 合計 net delta: **+2 ファイル**（新規 4、削除 2）。新規 trait / 型の純増は **3 個**（`RemoteEvent` enum + `RemoteEventReceiver` trait + `RemoteShared` 型）。`RemoteTransport::schedule_handshake_timeout` は既存 trait への method 追加のため新規型カウント外。

**公開 API 影響:**

- `Remote` は型パラメータを持たないまま、`Box<dyn RemoteInstrument + Send>` フィールドを内部に追加する（型シグネチャは変わらない）。`Remote::with_instrument` / `Remote::set_instrument` を新規 public API として追加する。
- `Remote::handle_remote_event` を inherent method として追加する。
- `RemoteShared` 型を新規 public として追加する（`Clone`、`new`、`run`、`impl Remoting`）。
- `Remoting` trait のシグネチャを破壊的変更:
  - 全メソッドを `&self` へ
  - `addresses` の戻り値を `&[Address]` から `Vec<Address>` へ
  - `async fn` / `Future` 戻り値は引き続き禁止
- `impl Remoting for Remote` を削除（`impl Remoting for RemoteShared` に移管）。`Remote::start` / `shutdown` / `quarantine` / `addresses` は inherent method として残す（`RemoteShared` がデリゲートで使う）。
- `RemoteEvent` enum と `RemoteEventReceiver` trait を新規 public 型として追加する。
- `RemoteEvent` の variant は本 change スコープで **5 つに限定**（`InboundFrameReceived` / `HandshakeTimerFired` / `OutboundEnqueued` / `ConnectionLost` / `TransportShutdown`）。
- `RemoteTransport` 既存 trait に `send_handshake(&mut self, &Address, HandshakePdu) -> Result<(), TransportError>` と `schedule_handshake_timeout(&mut self, &TransportEndpoint, Duration, u64) -> Result<(), TransportError>` を追加（破壊的変更、既存実装は新 method 実装が必須）。
- `AssociationEffect::StartHandshake` の意味論が「adapter が無視」から「`Remote::handle_remote_event` が send_handshake + schedule_handshake_timeout の 2 ステップで実行」へ変わる。variant に `timeout: Duration` / `generation: u64` を追加。
- `RemoteConfig` に `outbound_high_watermark` / `outbound_low_watermark` を追加する。
- `RemotingExtensionInstaller::remote()` の戻り値を `SharedLock<Remote>` から `RemoteShared` へ変更（破壊的、利用側は `RemoteShared` の `Remoting` trait 経由で start / shutdown / quarantine / addresses を呼ぶ）。
- adapter 側の `outbound_loop` / `handshake_driver` 公開関数は削除される。これは前 change `hide-remote-adaptor-runtime-internals` で internal 化済みのため外部 API 影響は無い。

**挙動影響:**

- 1ms ポーリングが消え、event 駆動になる。outbound throughput と CPU 消費が改善する。
- system message が ordinary message に飢餓されないことが既存仕様で保証されている状態が継続する。
- queue が watermark を超えると backpressure signal が発火し、計測可能になる。
- handshake / quarantine / send / receive の全イベントが instrument に通知される。
- handshake timeout の古い発火が `Remote::handle_remote_event` 側で破棄され、generation 不一致による誤遷移が発生しない。

## Non-goals

- payload serialization の完成、wire protocol の再設計
- large message queue の追加（control / ordinary 分離のみ維持）
- inbound 側 ack window / 動的 receive buffer の調整
- cluster adaptor、persistence adaptor の駆動見直し
- failure detector の駆動経路変更（heartbeat は別 change で扱う）
- `Codec<T>` trait 自体のシグネチャ変更
- 後方互換 shim、deprecated alias、旧 API 残置
- 新規 Driver 型 / Handle 型 / Outcome enum / Timer trait / Sink trait / Generation newtype の導入（純増最小化のため。`RemoteShared` は二層構造の Sharing 層として唯一例外で +1 する）
- `Remote` の型パラメータ化、tuple composite `RemoteInstrument` 実装、`() impl RemoteInstrument` の提供（ユーザー API 単純化と参照実装整合のため、dyn dispatch を採用）

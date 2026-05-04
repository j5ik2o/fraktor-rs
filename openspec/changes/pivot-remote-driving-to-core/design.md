## Context

現状の駆動経路は次の形である。

```text
remote-adaptor-std
├─ inbound_dispatch (tokio task)
│   └─ assoc.accept_handshake_request / accept_handshake_response を直接呼ぶ
├─ outbound_loop (tokio task, 1ms ポーリング)
│   └─ assoc.next_outbound / gate / recover を直接呼ぶ
└─ handshake_driver (tokio sleep tasks)
    └─ assoc.handshake_timed_out を直接呼ぶ

remote-core
├─ Association                    state machine 本体
├─ AssociationEffect              core が表明する意図
│   └─ StartHandshake             ★ adaptor で無視されている
├─ RemoteInstrument               ★ どこからも呼ばれていない
└─ RemotingFlightRecorder         ★ snapshot 経路もない
```

目指す経路は次の形である（**二層構造を採用。`Remote` は CQS 純粋ロジック、`RemoteShared` が並行性を吸収して `Remoting` を提供**）。

```text
remote-core
├─ Remote (Logic 層、CQS 純粋、並行性を知らない)
│   ├─ start / shutdown / quarantine        ← &mut self (Command)
│   ├─ addresses                            ← &self    (Query)
│   └─ handle_remote_event                  ← &mut self (event 1 件分の dispatch)
│       ├─ Association メソッドへ dispatch
│       ├─ Codec で encode / decode
│       ├─ AssociationEffect 実行 (StartHandshake 含む)
│       ├─ outbound queue (system / user) を消化
│       └─ watermark で apply_backpressure を発火
├─ RemoteShared(SharedLock<Remote>) (Sharing 層、並行性吸収、Clone)
│   ├─ run (&self, async)                   ← per-event with_write ループ
│   └─ impl Remoting                        ← すべて &self、内部で with_write/with_read
├─ RemoteEvent                              ← 新規 enum (closed)
├─ RemoteEventReceiver                      ← 新規 trait (1 メソッド)
└─ RemoteTransport                          → adapter (既存)

remote-adaptor-std
├─ inbound I/O ワーカー
│   └─ raw frame を RemoteEvent として adapter 内部 sender に push
├─ handshake timer task (per association)
│   └─ tokio::time::sleep → adapter 内部 sender に HandshakeTimerFired push
├─ TokioMpscRemoteEventReceiver           tokio mpsc 受信側ラッパ
└─ RemotingExtensionInstaller
    ├─ RemoteShared を保持（clone 1 個を run 用に spawn）
    └─ installer.remote() -> RemoteShared (raw SharedLock<Remote> を露出しない)
```

`hide-remote-adaptor-runtime-internals` で std adapter の public surface は既に縮小されているため、本 change は内部実装の駆動主導権を反転することに集中できる。

## Goals / Non-Goals

**Goals:**

- 駆動主導権を `remote-core` に集約し、`remote-adaptor-std` は I/O とイベント通知だけを担当する形にする。
- `RemoteInstrument` を hot path で機能させ、`RemotingFlightRecorder` を観測手段として稼働させる。
- system message が ordinary message に飢餓されない既存保証を維持する。
- outbound queue の watermark を超えたら backpressure signal を発火し、計測可能にする。
- `AssociationEffect::StartHandshake` を「adapter で無視」状態から「`Remote::handle_remote_event` で実行（`RemoteShared::run` の `with_write` 区間内）」状態に戻す。
- handshake timeout の古い発火を `u64` generation で識別し、誤った状態遷移を防ぐ。
- **新規型・新規 trait の純増を最小化する**（純増 3 個: `RemoteEvent` enum、`RemoteEventReceiver` trait、`RemoteShared` ラッパー型）。`RemoteShared` は二層構造の Sharing 層として必須なため、当初の「純増 2 個」目標から +1 する。

**Non-Goals:**

- wire protocol、`Codec<T>` trait シグネチャ、payload serialization の再設計。
- inbound 側 ack window、動的 receive buffer。
- large message queue の追加（control / ordinary 分離のみ維持）。
- failure detector / heartbeat の駆動経路変更（別 change）。
- cluster adaptor、persistence adaptor、stream adaptor の駆動見直し。
- 後方互換 shim、deprecated alias の残置。
- **新規 Driver / Handle / Outcome / Timer / Sink / Generation newtype の導入**（純増ゼロ最優先のため。`RemoteShared` は二層構造のために唯一例外として +1 する）。

## Decisions

### Decision 1: 駆動ループは `RemoteShared::run` に置き、`Remote` は CQS 純粋ロジックを保つ

第一案では `RemoteDriver<S, K, T, I, C>` という 5 ジェネリクスの新規型を導入していたが、これは「Association を駆動するループを所有する」という責務以上のことをしておらず、既存型に統合できる。本 change では **二層構造** を採用し、駆動ループを `RemoteShared` の inherent method として置く。`Remote` 自体には `run` を持たせない。

**二層の役割分離:**

| 層 | 型 | 並行性責務 | メソッドシグネチャ |
|----|----|------------|--------------------|
| Logic | `Remote` | 知らない | CQS 純粋（`&mut self` for Command, `&self` for Query） |
| Sharing | `RemoteShared(SharedLock<Remote>)` | 吸収する | すべて `&self`（内部で `with_write` / `with_read`） |

```rust
impl RemoteShared {
    pub async fn run<S>(&self, receiver: &mut S) -> Result<(), RemotingError>
    where
        S: RemoteEventReceiver,
    {
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

**この設計の利点:**

- `Remote` 側はロックを知らずに済み、CQS（`&mut self` / `&self`）が型シグネチャに直接表れる
- `RemoteShared` は `Clone` 可能なので、`run` task と同時に他の clone から `start` / `shutdown` / `quarantine` / `addresses` を呼べる
- per-event lock により、event 処理間の隙間で他の `Remoting` メソッドが進行できる
- `Remote` 自体は型パラメータを持たない（Decision 4 参照、instrument は `Box<dyn RemoteInstrument + Send>` フィールド）

**棄却した代替案:**

- *`RemoteDriver` 新規型*: ジェネリクス連鎖（`<S, K, T, I, C>`）が拡散し、ユーザー API が複雑化する。`RemoteShared` の inherent method 化で責務を統合する方が利用しやすい（registry + transport + lifecycle 駆動）。
- *`RemoteDriverHandle` / `RemoteDriverOutcome`*: `Result<(), RemotingError>` で「正常終了 / 異常終了 / 強制停止」を表現できるため、新規 Outcome enum は冗長。停止制御は `Remoting::shutdown` が `Remote::shutdown` 経由で lifecycle を terminated に遷移させ、`RemoteShared::run` ループが lifecycle を観測して終了することで実現する。
- *`Remote::run(self, ..)` で run task が単独所有*: `Remoting::quarantine` のような「run と並行して呼ばれる必要がある」mutation メソッドの経路がなくなる。`installer.remote()` が現状 raw `SharedLock<Remote>` を露出しており、移行先がない。本 change ではこれを採用しない。

### Decision 2: `RemoteEvent` を closed enum（5 variant に限定）、`RemoteEventReceiver` を 1 メソッド trait にする

`RemoteEvent` は core が adapter 側から受け取る closed enum。本 change のスコープでは scheduling 経路が確定している variant のみを含める。

```rust
pub enum RemoteEvent {
    /// TCP 受信 frame
    InboundFrameReceived { authority: TransportEndpoint, frame: alloc::vec::Vec<u8> },
    /// handshake timeout 満了 (RemoteTransport::schedule_handshake_timeout 経路)
    HandshakeTimerFired  { authority: TransportEndpoint, generation: u64 },
    /// local actor からの送信要求 (RemoteActorRef → adapter sender 経路)
    OutboundEnqueued     { authority: TransportEndpoint, envelope: OutboundEnvelope },
    /// 接続切断
    ConnectionLost       { authority: TransportEndpoint, cause: ConnectionLostCause },
    /// 全体停止指示 (Remoting::shutdown 経路)
    TransportShutdown,
}
```

**本 change で追加しない variant**: `OutboundFrameAcked` / `QuarantineTimerFired` / `BackpressureCleared`。これらは scheduling 経路が確定していないため、必要時に **別 change** で variant 追加と `RemoteTransport::schedule_*` method 追加を一緒に行う。closed enum を保ちつつスコープを絞る方針。

`RemoteEventReceiver` は `RemoteShared::run` が消費する側のみ。

```rust
pub trait RemoteEventReceiver: Send {
    fn recv(&mut self) -> impl core::future::Future<Output = Option<RemoteEvent>> + Send + '_;
}
```

`async fn` を trait に直書きせず `-> impl Future` にして dyn 互換性は意図的に外す（`RemoteShared::run` 側はジェネリクス、adapter 側は具象実装で完結する）。

**`RemoteEventSink` trait は core に追加しない**。adapter は内部で `tokio::sync::mpsc::channel` を作り、`Sender` clone を I/O ワーカー / handshake timer task に配り、`Receiver` を `TokioMpscRemoteEventReceiver` でラップして `RemoteShared::run` に渡す。sender 側は adapter 内部の責務であり、core から見る必要がない。これにより new trait を 1 つ削減できる。

### Decision 3: Timer Port を core に追加せず、handshake timer は `RemoteTransport::schedule_handshake_timeout` で表出する

第一案では `Timer` trait（`schedule` / `cancel` / `TimerToken`）を新規追加していたが、これも純増ゼロ方針で削減できる。

ただし、`AssociationEffect::StartHandshake { authority, timeout, generation }` を `Remote::handle_remote_event` が処理する際、core 側から adapter に「timer を予約せよ」と指示する経路が必要。これを **既存 `RemoteTransport` trait の method 追加** で表現する。

```rust
// 既存 trait への method 追加 (modification of remote-core-transport-port)
pub trait RemoteTransport {
    // ...既存 method...
    fn schedule_handshake_timeout(
        &mut self,
        authority: &TransportEndpoint,
        timeout: core::time::Duration,
        generation: u64,
    ) -> Result<(), TransportError>;
}
```

`Remote::handle_remote_event` の StartHandshake 処理（疑似コード）:

```text
when AssociationEffect::StartHandshake { authority, timeout, generation } を見つける:
    1. let request = HandshakePdu::Req(HandshakeReq::new(assoc.local().clone(), assoc.remote().clone()));
       self.transport.send_handshake(assoc.remote(), request)?;
    2. self.transport.schedule_handshake_timeout(&authority, timeout, generation)?;  // 新
```

adapter 側 `schedule_handshake_timeout` 実装:

```text
fn schedule_handshake_timeout(&mut self, authority, timeout, generation) -> Result<()> {
    let sender = self.event_sender.clone();
    tokio::spawn(async move {
        tokio::time::sleep(timeout).await;
        let _ = sender.send(RemoteEvent::HandshakeTimerFired { authority, generation }).await;
        // ↑ adapter 内部の閉じた経路。古い generation の発火は Remote::handle_remote_event 側で破棄される
    });
    Ok(())
}
```

これにより：

- core 側に Timer Port を新設しない（純増ゼロ維持）
- handshake timer の予約契約が `RemoteTransport` の 1 method として明示される（`AssociationEffect::StartHandshake` 処理経路の曖昧さ解消）
- `utils-core::DelayProvider` への依存を作らない（actor-core scheduler との結合を避ける）

**スコープ限定**: 本 change では `schedule_handshake_timeout` の **handshake 用 1 method のみ** を追加する。quarantine timer / large message ack timer 等の汎用 scheduling 経路は本 change の対象外とし、必要時に別 change で `RemoteEvent` の variant 追加と `RemoteTransport::schedule_*` method 追加を一緒に行う。

**棄却した代替案:**

- *Core 側 `Timer` trait + adapter 実装*: schedule / cancel / TimerToken を core 公開 API に増やす。本質的に「handshake event を遅延配信する」というスコープでは過剰一般化。
- *`utils-core::DelayProvider` 経由*: actor-core scheduler との結合があり、layer 整合性を崩す。
- *`RemoteTransport::initiate_handshake(authority, timeout, generation, frame_bytes)` 統合形*: `send` との責務混在。`Codec::encode` の経路を transport の中に隠す形になり、Codec Port の見通しが悪化する。
- *新 effect `ScheduleHandshakeTimeout`*: `StartHandshake` から自然に派生する 2 ステップ目を独立 effect にすると、Association の effect 列が冗長になる。`StartHandshake` 自身が「send_handshake + schedule」の 2 ステップを意味する効果と定義する方が圧縮的。

### Decision 4: `RemoteInstrument` を `Box<dyn>` で配線する（ジェネリクス採用しない、`&mut self` 維持）

既存仕様（`remote-core-instrument` capability）で `RemoteInstrument` は `&mut self` 系 hook を持つ。本 change はこのシグネチャを維持し、内部可変性（`Cell` / `SpinSyncMutex`）を instrument に持たせる必要を作らない。

```rust
pub trait RemoteInstrument {
    fn on_send(&mut self, envelope: &OutboundEnvelope);
    fn on_receive(&mut self, envelope: &InboundEnvelope);
    fn record_handshake(&mut self, authority: &TransportEndpoint, phase: HandshakePhase, now_ms: u64);
    fn record_quarantine(&mut self, authority: &TransportEndpoint, reason: QuarantineReason, now_ms: u64);
    fn record_backpressure(&mut self, authority: &TransportEndpoint, signal: BackpressureSignal, correlation: Option<u64>, now_ms: u64);
}
```

`Remote` は型パラメータを持たず、`Box<dyn RemoteInstrument + Send>` を内部フィールドで保持する。

```rust
pub struct Remote {
    // ...既存フィールド...
    instrument: alloc::boxed::Box<dyn RemoteInstrument + Send>,
}

impl Remote {
    pub fn new(transport: T, config: RemoteConfig, event_publisher: EventPublisher) -> Self
    where T: RemoteTransport + Send + 'static
    {
        Self {
            /* ... */
            instrument: alloc::boxed::Box::new(NoopInstrument), // pub(crate) ZST、外部公開しない
        }
    }

    pub fn with_instrument<T>(
        transport: T,
        config: RemoteConfig,
        event_publisher: EventPublisher,
        instrument: alloc::boxed::Box<dyn RemoteInstrument + Send>,
    ) -> Self
    where T: RemoteTransport + Send + 'static
    {
        Self { /* ... */ instrument }
    }

    pub fn set_instrument(&mut self, instrument: alloc::boxed::Box<dyn RemoteInstrument + Send>) {
        self.instrument = instrument;
    }
}

// 内部 ZST、pub(crate) で公開しない
pub(crate) struct NoopInstrument;

impl RemoteInstrument for NoopInstrument {
    fn on_send(&mut self, _: &OutboundEnvelope) {}
    fn on_receive(&mut self, _: &InboundEnvelope) {}
    fn record_handshake(&mut self, _: &TransportEndpoint, _: HandshakePhase, _: u64) {}
    fn record_quarantine(&mut self, _: &TransportEndpoint, _: QuarantineReason, _: u64) {}
    fn record_backpressure(&mut self, _: &TransportEndpoint, _: BackpressureSignal, _: Option<u64>, _: u64) {}
}
```

`Remote::handle_remote_event` での instrument 借用は `&mut *self.instrument`（`Box::deref_mut` 経由で `&mut dyn RemoteInstrument`）として取得し、`Association` の状態遷移メソッドに渡す。`RemoteShared::run` がループの外側、`Remote::handle_remote_event` がイベント1件分の dispatch を担当する。

```rust
impl Remote {
    /// 単一 event の dispatch。&mut self （CQS Command）。
    /// 戻り値が true ならループ終了（TransportShutdown 受信または lifecycle 終了）。
    pub fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<bool, RemotingError> {
        let instrument: &mut dyn RemoteInstrument = &mut *self.instrument;
        // ... event 種別ごとの dispatch ...
        match event {
            RemoteEvent::TransportShutdown => Ok(true),
            // ...
        }
    }
}

impl RemoteShared {
    pub async fn run<S: RemoteEventReceiver>(&self, receiver: &mut S) -> Result<(), RemotingError> {
        loop {
            let Some(event) = receiver.recv().await else {
                return Err(RemotingError::EventReceiverClosed);
            };
            let done = self.with_write(|remote| remote.handle_remote_event(event))?;
            if done { return Ok(()); }
        }
    }
}
```

**ジェネリクス `Remote<I: RemoteInstrument = ()>` を棄却した理由:**

- 参照実装（Apache Pekko の `RemoteInstrument` abstract class、protoactor-go の interface）は virtual / dyn dispatch で実装されており、production で問題なく稼働している。
- vtable lookup の cost（~1-2ns）は tokio mpsc send（~100ns）/ codec encode（数十ns〜μs）/ mutex acquisition（~10ns）と比較して noise レベル。zero-cost を狙う動機が弱い。
- `Remote<I>` を採用すると `<I>` がテスト・showcase・cluster adapter まで伝播し、ユーザー API が複雑化する。
- 実行時に instrument を差し替えできず、`set_instrument` 相当の API を持てない。
- Rust の他の観測ライブラリ（`tracing-rs` の `Subscriber`、`metrics` の `Recorder`、`opentelemetry-rs` 等）も dyn dispatch を採用している。

**tuple composite `(A, B)` / `(A, B, C)` impl を棄却した理由:**

- 複数 instrument 合成はユーザー側で composite struct を自作すれば足り、ライブラリ側で提供する必要がない（YAGNI）。
- Pekko も `Vector[RemoteInstrument]` を内部 helper として持つだけで、ユーザー API としての tuple composite は提供していない。
- tuple impl はジェネリクス前提のため、`Remote` を非ジェネリクス化した本 decision と整合しない。

**`impl RemoteInstrument for ()` を棄却した理由:**

- `Box<dyn>` ベース設計では `()` impl は不要（`Box::new(())` をデフォルトにすると semantic に「instrument が無い」を表現できず、`NoopInstrument` ZST の方が意図が明確）。
- `pub(crate) NoopInstrument` は内部実体として隔離されており、公開 API 純増ゼロ条件を満たす。

**`&mut dyn RemoteInstrument` の伝播経路:**

- `Remote` の `&mut self` 経由で `&mut *self.instrument` を取得し、`&mut dyn RemoteInstrument` として扱う。
- `Remote::handle_remote_event` 内部で local 変数 `let instrument: &mut dyn RemoteInstrument = &mut *self.instrument;` として保持する（`RemoteShared::run` は `with_write` クロージャ内でこの method を呼ぶ）。
- `Association` の状態遷移メソッドは `instrument: &mut dyn RemoteInstrument` 引数で受け取り、型パラメータを導入しない。

**split-borrow 前提:**

`Remote::handle_remote_event` 内部で `&mut *self.instrument` を保持したまま `self.handle_*` 系の inherent helper を呼ぶと、`self` 全体の再 `&mut` 借用が発生して借用衝突する。実装側では次のいずれかで split borrow を確保する。

1. event 種別ごとの処理を free function（または associated function）に切り出し、`registry` / `transport` / `codec` / `instrument` を個別 `&mut` 引数で受け取る形にする
2. `handle_remote_event` 先頭で `let Self { registry, transport, codec, instrument, .. } = self;` と destructuring し、field 単位で別々の `&mut` 借用を作って helper に渡す

Rust の field 単位 borrow split は安全に成立するため、この前提さえ守れば設計に問題はない。`RemoteShared::run` 側はロックを受け持つだけで、借用衝突の問題は `Remote::handle_remote_event` 内部に閉じる。

### Decision 5: 既存の system / user 2 キュー分離を維持し、`total_outbound_len()` を追加する

既存仕様（`remote-core-association-state-machine` capability の "SendQueue priority logic" 要件）で `SendQueue` は system priority と user priority の 2 キュー分離を持ち、system 優先で取り出す。これは Pekko Artery の Control / Ordinary 分離と同等の飢餓回避をすでに提供している。本 change はこの構造を維持する。

新規追加は次の 1 点のみ:

```rust
impl Association {
    pub fn total_outbound_len(&self) -> usize {
        // SendQueue の system + user の合計長
        // deferred queue は含めない（handshake 完了で flush される一時バッファのため）
    }
}
```

### Decision 6: 既存 `BackpressureSignal::Apply` / `Release` を流用する

第一案では `BackpressureSignal::Engaged` / `Released` を新 variant として追加していたが、既存の `Apply` / `Release` でセマンティクスを満たすため新 variant は不要。

watermark 連動の発火経路は `Remote::handle_remote_event` 内（具体的には `OutboundEnqueued` 処理の drain helper）で `Association::total_outbound_len()` を `outbound_high_watermark` / `outbound_low_watermark` と比較し、状態遷移時に `Association::apply_backpressure(BackpressureSignal::Apply)` または `Release` を呼ぶ。`Association` 側は手動 `apply_backpressure` 呼び出しと watermark 経由の自動呼び出しを区別しない。

### Decision 7: `handshake_generation` を `u64` で inline 表現する

第一案では `HandshakeGeneration` newtype を導入していたが、`u64` を直接使う方が型数を 1 個削減できる。

```rust
pub struct Association {
    // ...
    handshake_generation: u64,
}

impl Association {
    fn enter_handshaking(&mut self) {
        self.handshake_generation = self.handshake_generation.wrapping_add(1);
        // ...
    }

    fn current_generation(&self) -> u64 {
        self.handshake_generation
    }
}

pub enum AssociationEffect {
    StartHandshake { authority: TransportEndpoint, timeout: Duration, generation: u64 },
    // ...
}

pub enum RemoteEvent {
    HandshakeTimerFired { authority: TransportEndpoint, generation: u64 },
    // ...
}

// Remote::handle_remote_event 側の staleness 判定（疑似コード）
fn is_stale_handshake_timer(current: u64, event_generation: u64) -> bool {
    // `wrapping_add(1)` を使う以上、大小比較ではなく等値比較で判定する。
    // u64::MAX → 0 の wrap でも漏れずに「現行 generation 以外＝ stale」と扱える。
    current != event_generation
}
```

`u64` の意味付けは rustdoc に依存し、型安全性は若干落ちるが、`Association` 内部および `Remote::handle_remote_event` の判定経路でしか使わないため過剰設計を避ける。

### Decision 8: lifecycle 制御は `Remoting` trait 経由で `RemoteShared` に集約する

`RemoteDriverHandle` / `RemoteDriverOutcome` を導入せず、既存 `Remoting` trait の `start` / `shutdown` / `quarantine` / `addresses` で lifecycle を制御する。`Remoting` の実装は **`RemoteShared` 側** に集約し、`Remote` 自身は `impl Remoting for Remote` を持たない（Decision 10 参照）。

- `Remoting::start`: `RemoteShared::start` が `with_write(|r| r.start())` で内部 `Remote::start` を呼ぶ。tokio task として `RemoteShared::run` を spawn する経路は adapter 側の `RemotingExtensionInstaller` が担当する（installer が `RemoteShared` の clone を 1 個 spawn 用に持ち、別 clone を `Remoting` API 用に外部公開する）。
- `Remoting::shutdown`: `RemoteShared::shutdown` が `with_write(|r| r.shutdown())` で `Remote` の lifecycle を terminated に遷移させる純デリゲートのみ。**wake はしない**（`RemoteShared` は `event_sender` を持たない、薄いラッパー原則）。run task の即時停止と完了観測は adapter 固有の `installer.shutdown_and_join().await` で行う（Decision 10 参照）。
- `Remoting::quarantine`: `RemoteShared::quarantine` が `with_write(|r| r.quarantine(...))` で内部 `Remote::quarantine` を呼ぶ。`run` task と並行して呼ばれてよい（per-event lock の隙間で進行する）。
- `Remoting::addresses`: `RemoteShared::addresses` が `with_read(|r| r.addresses().to_vec())` で source of truth から owned `Vec<Address>` を返す。キャッシュは持たない。

`RemoteShared::run` の戻り値 `Result<(), RemotingError>` は次の意味で使う。

- `Ok(())`: `TransportShutdown` 受信または lifecycle terminated 観測による正常終了
- `Err(RemotingError::EventReceiverClosed)`: `TransportShutdown` 受信前に receiver が閉じた異常終了
- `Err(RemotingError::TransportUnavailable)`: 復帰不能エラー（transport 永続失敗、association registry 破損 等）

### Decision 9: AssociationEffect::StartHandshake の adapter 無視を禁止し、`Remote::handle_remote_event` で 2 ステップ処理する

`AssociationEffect::StartHandshake` のセマンティクスを「`Remote::handle_remote_event` 経路で **send_handshake + schedule_handshake_timeout** の 2 ステップ実行」と明示する。adapter 側 `effect_application::apply_effects_in_place` から該当分岐を削除する。

```rust
// Remote::handle_remote_event の effect 処理（疑似コード）
fn handle_effect(&mut self, effect: AssociationEffect) -> Result<(), RemotingError> {
    match effect {
        AssociationEffect::StartHandshake { authority, timeout, generation } => {
            // ステップ 1: handshake request frame の送出
            let request = HandshakePdu::Req(HandshakeReq::new(assoc.local().clone(), assoc.remote().clone()));
            self.transport.send_handshake(assoc.remote(), request)
                .map_err(RemotingError::TransportUnavailable)?;

            // ステップ 2: handshake timer の予約
            self.transport
                .schedule_handshake_timeout(&authority, timeout, generation)
                .map_err(RemotingError::TransportUnavailable)?;

            Ok(())
        }
        // ...
    }
}
```

**順序保証**: ステップ 1（`send`）が `Err` の場合、ステップ 2 は呼ばれない（`?` で早期 return）。これにより「frame は送ったが timer 予約に失敗」「frame 送出失敗なのに timer だけ走る」といった状態不整合を避ける。

**adapter 側の閉ループ**: adapter は `schedule_handshake_timeout` 受領を契機に tokio task で sleep を起動し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で receiver に push する。古い generation の発火は `Remote::handle_remote_event` 側で `!=` 判定により破棄される（adapter 側でキャンセル責務を負わない、Decision 7 と整合）。

**adapter 側 `effect_application` の責務縮減**: `StartHandshake` 分岐を削除し、`SendEnvelopes` / `DiscardEnvelopes` / `PublishLifecycle` 等の I/O 直結 effect のみを扱う。状態遷移を伴う effect は `Remote::handle_remote_event` 側に集約される（実行は `RemoteShared::run` の `with_write` 区間内で行われる）。

### Decision 10: `RemoteShared(SharedLock<Remote>)` を正式 surface とし、`Remoting` trait は `RemoteShared` に実装する

#### 二層構造の採用

`Remote` は CQS 純粋ロジック層として `&mut self` で状態を変更し、`&self` で状態を読む。並行性の責務は **`RemoteShared(SharedLock<Remote>)` ラッパー** が吸収する。

```rust
// Logic 層: 並行性を知らない
pub struct Remote {
    // ... 既存フィールド ...
    instrument: alloc::boxed::Box<dyn RemoteInstrument + Send>,
}

impl Remote {
    pub fn start(&mut self) -> Result<(), RemotingError> { ... }       // Command
    pub fn shutdown(&mut self) -> Result<(), RemotingError> { ... }    // Command
    pub fn quarantine(&mut self, ..) -> Result<(), RemotingError> { ... }  // Command
    pub fn addresses(&self) -> &[Address] { ... }                       // Query
    pub fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<bool, RemotingError> { ... }  // Command
}

// Sharing 層: 並行性を吸収（薄いラッパーに徹する、Remote が知らない責務は持たない）
#[derive(Clone)]
pub struct RemoteShared {
    inner: SharedLock<Remote>,
}

impl RemoteShared {
    pub fn new(remote: Remote) -> Self {
        Self {
            inner: SharedLock::new_with_driver::<DefaultMutex<_>>(remote),
        }
    }

    pub(crate) fn with_write<R>(&self, f: impl FnOnce(&mut Remote) -> R) -> R {
        self.inner.with_write(f)
    }

    pub(crate) fn with_read<R>(&self, f: impl FnOnce(&Remote) -> R) -> R {
        self.inner.with_read(f)
    }

    /// per-event lock の長期実行ループ。&self なので Clone と並行に呼べる。
    pub async fn run<S: RemoteEventReceiver>(&self, receiver: &mut S) -> Result<(), RemotingError> {
        loop {
            let Some(event) = receiver.recv().await else {
                return Err(RemotingError::EventReceiverClosed);
            };
            let done = self.with_write(|remote| remote.handle_remote_event(event))?;
            if done { return Ok(()); }
        }
    }
}

// Remoting trait は RemoteShared 側に実装。すべて with_write / with_read で
// Remote の inherent method にデリゲートするだけ（Remote が知らない責務を追加しない）。
impl Remoting for RemoteShared {
    fn start(&self) -> Result<(), RemotingError> {
        self.with_write(|remote| remote.start())
    }
    fn shutdown(&self) -> Result<(), RemotingError> {
        self.with_write(|remote| remote.shutdown())
    }
    fn quarantine(&self, addr: &Address, uid: Option<u64>, reason: QuarantineReason)
        -> Result<(), RemotingError>
    {
        self.with_write(|remote| remote.quarantine(addr, uid, reason))
    }
    fn addresses(&self) -> Vec<Address> {
        self.with_read(|remote| remote.addresses().to_vec())
    }
}
```

**`RemoteShared` の薄さ原則:** `RemoteShared` は `SharedLock<Remote>` の薄いラッパーであり、`Remote` が知らない責務（tokio sender、wake 機構、event channel 等）を **追加してはならない**（MUST NOT）。これは依存方向（`remote-core` が `tokio` を知らない）と Decision 2（`RemoteEventSink` を core に追加しない）の対称性を保つため。

#### `Remoting` trait シグネチャの変更（破壊的変更）

`Remoting` trait は **すべて `&self` ベース** へ変更する。並行性の吸収責任を実装側に任せる port にするためである。

| メソッド | 旧シグネチャ | 新シグネチャ |
|----------|--------------|--------------|
| `start` | `fn start(&mut self) -> Result<(), RemotingError>` | `fn start(&self) -> Result<(), RemotingError>` |
| `shutdown` | `fn shutdown(&mut self) -> Result<(), RemotingError>` | `fn shutdown(&self) -> Result<(), RemotingError>` |
| `quarantine` | `fn quarantine(&mut self, ..) -> Result<(), RemotingError>` | `fn quarantine(&self, ..) -> Result<(), RemotingError>` |
| `addresses` | `fn addresses(&self) -> &[Address]` | `fn addresses(&self) -> Vec<Address>` |

すべて同期 method（`async fn` / `Future` 戻り値は追加しない）。`addresses` は内部 read lock のため slice ではなく owned `Vec` を返す。

`impl Remoting for Remote` は **削除** する（CLAUDE.md「後方互換は不要」に基づく）。

#### Installer の構造

```text
RemotingExtensionInstaller::install(actor_system) {
    let remote = Remote::with_instrument(transport, config, event_publisher, instrument);
    let remote_shared = RemoteShared::new(remote);
    // capacity は実装 PR で確定（本 change では RemoteConfig に capacity 用の新フィールドを追加しない、Open Questions 参照）
    let (event_sender, mpsc_rx) = tokio::sync::mpsc::channel(EVENT_CHANNEL_CAPACITY);
    let event_receiver = TokioMpscRemoteEventReceiver::new(mpsc_rx);

    // install 時点では run task を spawn しない（外部から remote.start() を呼んでから別途起動）
    Self {
        remote_shared,    // Remoting API として外部公開
        event_sender,     // adapter 内部、shutdown_and_join で wake に使う
        event_receiver,   // run 起動時に消費（Option で保持）
        run_handle: None, // run 起動時に Some に
    }
}

pub fn remote(&self) -> RemoteShared {
    self.remote_shared.clone()
}

/// run task を tokio::spawn する明示 API（install と分離）。
pub fn spawn_run_task(&mut self) -> Result<(), RemotingError> {
    let receiver = self.event_receiver.take().ok_or(RemotingError::AlreadyRunning)?;
    let run_target = self.remote_shared.clone();
    let handle = tokio::spawn(async move {
        let mut receiver = receiver;
        run_target.run(&mut receiver).await
    });
    self.run_handle = Some(handle);
    Ok(())
}

/// adapter 固有の async surface: 停止要求 wake + run task 完了観測を 1 step で行う。
pub async fn shutdown_and_join(mut self) -> Result<(), RemotingError> {
    // 1. lifecycle を terminated に遷移（Remoting::shutdown 経由）
    let _ = self.remote_shared.shutdown();
    // 2. wake — try_send なので await しない、Full / Closed 失敗は無視
    let _ = self.event_sender.try_send(RemoteEvent::TransportShutdown);
    // 3. run task 完了を観測（adapter 固有 async surface のためここで await）
    let Some(handle) = self.run_handle.take() else { return Ok(()); };
    match handle.await {
        Ok(Ok(()))    => Ok(()),
        Ok(Err(e))    => Err(e),
        Err(_join)    => Err(RemotingError::TransportUnavailable),
    }
}
```

`installer.remote()` は **`RemoteShared` を返す**（raw `SharedLock<Remote>` は露出しない）。呼び出し側は `RemoteShared` を `Remoting` trait の API として扱う。

#### 外部制御 surface の責務分担

| surface | 配置 | 用途 |
|---------|------|------|
| `Remoting` trait（`RemoteShared` 実装） | core | lifecycle 操作（同期、4 メソッド） |
| `Sender<RemoteEvent>` | adapter 内部 | I/O ワーカー / handshake timer / RemoteActorRef が `TransportShutdown` 等を push |
| `JoinHandle<Result<(), RemotingError>>` | adapter installer 内部 | run task 完了観測 |
| `installer.shutdown_and_join().await` | adapter | 停止 wake + 完了観測の adapter 固有 async surface |

#### `Remoting::shutdown` の意味論（薄いラッパー徹底）

`RemoteShared::shutdown(&self)` は `Remote::shutdown` への純デリゲートのみ：

```text
RemoteShared::shutdown(&self) {
    self.with_write(|remote| remote.shutdown())   // lifecycle terminated に遷移するだけ
}
```

**wake はしない。`event_sender` を持たない。** `Remote` が知らない責務（tokio sender、event push）を `RemoteShared` に追加することは、Decision 2（`RemoteEventSink` を core に追加しない）の対称性および「`RemoteShared` は薄いラッパー」原則に反する。

そのため `Remoting::shutdown` 単独呼び出しは「lifecycle 状態を terminated に遷移する」セマンティクスに留まる。run task は次の event 受信時に lifecycle terminated を観測してループ終了するが、`recv().await` で blocked のまま event が来なければ即座には停止しない。

#### graceful shutdown は `installer.shutdown_and_join` を使う

run task の即時停止と完了観測が必要な場合は、adapter 固有の `installer.shutdown_and_join().await` を使う。これは:

1. `Remoting::shutdown`（lifecycle 遷移）を呼ぶ
2. `event_sender.try_send(TransportShutdown)` で wake（同期 try_send、await しない）
3. `run_handle.await` で完了観測（adapter 固有 async surface のためここで await）

の 3 ステップを 1 コールにまとめる。`Remoting` trait の同期 API は run task の終了完了まで保証したように **見せてはならない**（MUST NOT）。完了保証が必要な呼び出し側は `shutdown_and_join` を使う。

**棄却した代替案:**

- *raw `Arc<Mutex<Remote>>` / raw `SharedLock<Remote>` を installer field として保持*: `*Shared` 型で API を制限せずに `Remote` 本体を共有すると、CQS の command/query 境界と event loop の所有者が曖昧になる。`RemoteShared` で API を `Remoting` trait に閉じる本決定で解消される。
- *`Remote::run(self, ..)` で run task が単独所有*: `Remoting::quarantine` を run と並行して呼ぶ経路がなくなる（`Remote` を消費すると外部から触れない）。`installer.remote()` の現状実装（`SharedLock<Remote>` を露出）の移行先がない。本決定では run loop が per-event lock で動くため、`Remoting::quarantine` 等は lock の隙間で進行できる。
- *`Remoting` trait を `&mut self` のまま維持*: `RemoteShared` が `&mut self` で trait を実装しようとすると、`Clone` で配った clone から同時に `&mut self` を取れなくなる（`Arc<RemoteShared>` の中で borrow するなら `&mut RemoteShared` が要るが、そもそも `RemoteShared` は内部ロックで並行性を吸収する型なので、外側 `&mut` は意味的に不適切）。`&self` ベースに揃える方が二層構造の意図と合致する。

### Decision 11: outbound enqueue は `RemoteEvent::OutboundEnqueued` で表現する

`outbound_loop.rs` 削除後、local actor から outbound queue への enqueue 経路で `RemoteShared::run` を起こす wake event が必要。これを新 variant `RemoteEvent::OutboundEnqueued { authority, envelope }` で表現する。

```text
local actor.tell(remote_ref, msg)
  → adapter RemoteActorRef
    → OutboundEnvelope::user(msg, ...)
    → adapter event_sender.send(RemoteEvent::OutboundEnqueued { authority, envelope })

RemoteShared::run loop:
  receiver.recv().await:
    Some(event) => self.with_write(|remote| remote.handle_remote_event(event))?

Remote::handle_remote_event の RemoteEvent::OutboundEnqueued 分岐:
    let assoc = self.registry.get_mut(&authority);
    assoc.enqueue(envelope, &mut *self.instrument);
    self.drain_outbound(&authority)?;        // next_outbound → encode → send
```

**棄却した代替案:**

- *`RemoteEvent::OutboundEnqueued { authority }` (signal のみ、envelope は別経路)*: `AssociationRegistry` を adapter から直接 mutate するため `Mutex` / `RwLock` / lock-free queue が必要。core に内部可変性を持ち込む。本 change の二層構造では `Remote` 自身が CQS 純粋であり、内部可変性は持たない（並行性は `RemoteShared` 側に閉じる）。
- *別 channel での envelope 配送*: `RemoteShared::run` 内で複数 channel を `select!` する形になり、ループの複雑性が上がる。`RemoteEvent` enum 1 本に集約する方が見通しがよい。
- *enqueue を effect で表現*: enqueue は core から adapter への要求ではなく adapter から core への通知方向のため、effect ではなく event が適切。

**コスト**: `OutboundEnvelope` が event channel を経由するため、payload が大きい場合に move / copy コストが発生する。本 change ではシンプルさを優先し、zero-copy / per-authority channel 分離 / ring buffer 化等の最適化は別 change の余地として残す（Open Questions 参照）。

## Risks / Trade-offs

### Risk 1: `Box<dyn RemoteInstrument>` の vtable オーバーヘッド

hot path（`on_send` / `on_receive` / `record_*`）で 1-2ns の vtable lookup が常時発生する。`Remote<I>` で zero-cost にする選択肢もあったが、Decision 4 で参照実装（Pekko / protoactor-go）と Rust 観測ライブラリ（tracing / metrics）の慣行に倣い、API の単純化を優先して dyn dispatch を採用した。

**緩和策:** ベンチマークで vtable の影響を測定し、tokio mpsc send / codec encode / mutex acquisition のコストに対して noise 範囲であることを実装 PR で確認する。問題が顕在化したら、その時点でジェネリクス化を再検討する（YAGNI）。`NoopInstrument` 既定の場合は分岐予測が効きやすく、コンパイラが devirtualize する余地もある。

### Risk 2: `Remote::run` の長期保有 → 二層構造（`RemoteShared`）で解決

`Remote::run` を `&mut self` で長期保有すると、`Remoting::quarantine` 等の他の `&mut self` メソッドを並行して呼べなくなる。一方で `Remote::run(self, ..)` で consume すると、`Remoting` trait を実装する経路が消えてしまい、`installer.remote()` の現状実装（`SharedLock<Remote>` 露出）の移行先がない。

**解決策（Decision 10 で確定）:** 二層構造を採用する。`Remote` は CQS 純粋ロジックを保ち、`run` を持たない。並行性責務は `RemoteShared(SharedLock<Remote>)` が吸収し、`RemoteShared::run` は per-event lock の長期実行ループとして動く。`Remoting` trait は `RemoteShared` 側に実装され、すべて `&self` ベースに揃う。`Clone` で配った clone から `start` / `shutdown` / `quarantine` / `addresses` を並行に呼べる（per-event lock の隙間で進行する）。

外部制御は `Remoting` trait（`RemoteShared` 実装、4 同期 method、wake しない）と、adapter 固有 surface の 2 系統に分かれる。adapter 側の `installer.shutdown_and_join().await` が wake（`event_sender.try_send(TransportShutdown)`）と完了観測（`run_handle.await`）を 1 step で担う。`RemoteShared` は `event_sender` を持たない（Decision 10 の薄いラッパー原則）。`Remoting::addresses()` は `RemoteShared::addresses()` が `with_read` で source of truth から owned `Vec<Address>` を返すため、cached_addresses を持たない。

### Risk 3: handshake timer の責務分担 → `RemoteTransport::schedule_handshake_timeout` で確定

第一案では timer 予約の責務分担が曖昧だった。

**解決策（Decision 9 で確定）:** 既存 `RemoteTransport` trait に `schedule_handshake_timeout(&mut self, &TransportEndpoint, Duration, u64) -> Result<(), TransportError>` を追加（既存 capability `remote-core-transport-port` への MODIFIED）。`Remote::handle_remote_event` は `AssociationEffect::StartHandshake` を「`send` → `schedule_handshake_timeout`」の 2 ステップで処理する。adapter 実装は `schedule_handshake_timeout` で tokio task を spawn する。残る曖昧さなし。

### Risk 4: 設計純化と Pekko 互換性のトレードオフ

`Timer` Port を作らないことで、組み込み / WASM 等での migration 時に「sleep 抽象を adapter ごとに再実装する」必要が出る。Pekko の `Scheduler` のような汎用 Timer 抽象は提供しない。

**緩和策:** 必要が生じた段階で別 change として `Timer` Port を追加すれば良い。現時点では adapter が tokio sleep で十分であり、YAGNI を優先する。

### Risk 5: `with_write` 区間中の同期 I/O による並行性吸収の限界

`Remote::handle_remote_event` 内で `RemoteTransport::send`（既存仕様で同期 method）を呼ぶと、その間 `RemoteShared::with_write` の write lock が解放されない。`Clone` で配った他の `RemoteShared` から `Remoting::quarantine` 等を並行に呼んでも、送信完了まで待たされる。

**スコープ:** 本 change の責務ではなく、既存 `RemoteTransport` trait が同期である設計上の限界。`per-event lock` で吸収できる並行性は「event 処理間の隙間」と「同期 I/O 完了後の隙間」に限られる。

**緩和策:**

- TCP send は通常 buffer 書込みで即返るため、blocking が長期化するケースは限定的（peer が極度に slow consumer の場合のみ）
- 現状の Pekko / protoactor-go も同等の構造（state machine への dispatch は単一 task で順次実行）
- 将来 `RemoteTransport::send` を async 化する場合は別 change で対応する。本 change のスコープでは accept する

## Migration Plan

### Phase 1: instrument 配線基盤の整備（破壊的変更を含む）

- `pub(crate) struct NoopInstrument` を内部定義し、`impl RemoteInstrument for NoopInstrument` を追加（外部公開しない）
- `Remote` に `instrument: Box<dyn RemoteInstrument + Send>` フィールドを追加
- `Remote::new(...)` を更新し、内部で `Box::new(NoopInstrument)` を割り当てる
- `Remote::with_instrument(...)` および `Remote::set_instrument(...)` を新規 public API として追加
- `RemotingFlightRecorder: impl RemoteInstrument` を追加
- 既存テスト・showcase の `Remote::new` 呼出は型シグネチャ変更なしで動作（フィールド追加のみのため）

### Phase 2: Association 配線

- `Association::associate` / `handshake_accepted` / `handshake_timed_out` / `quarantine` / `apply_backpressure` のシグネチャに `instrument: &mut dyn RemoteInstrument` を追加
- `Association::total_outbound_len` を追加
- `Association` に `handshake_generation: u64` フィールドを追加
- `AssociationEffect::StartHandshake { authority, timeout, generation }` に generation を追加（既存変数名は維持）

### Phase 3: core 側 Port の追加

- `RemoteEvent` enum を `core/extension/remote_event.rs` に追加
- `RemoteEventReceiver` trait を `core/extension/remote_event_receiver.rs` に追加

### Phase 4: `Remote::handle_remote_event` と `RemoteShared::run` 実装

- `Remote::handle_remote_event(&mut self, event: RemoteEvent) -> Result<bool, RemotingError>` の inherent method 実装（戻り値 true でループ終了）
- event match の dispatch 表
- effect 列処理（`StartHandshake` / `SendEnvelopes` / `DiscardEnvelopes` / `PublishLifecycle`）
- watermark 連動 backpressure 発火
- `RemoteShared` 型の新設（`SharedLock<Remote>` ラッパー、`#[derive(Clone)]`）
- `RemoteShared::run<S: RemoteEventReceiver>(&self, receiver: &mut S) -> Result<(), RemotingError>` の inherent method 実装（per-event `with_write` ループ）

### Phase 5: `Remoting` trait のシグネチャ変更と `impl Remoting for RemoteShared`

- `Remoting` trait の全メソッドを `&self` ベースへ変更（破壊的変更）
- `addresses` は `&[Address]` から `Vec<Address>` （owned）へ戻り値変更
- `impl Remoting for Remote` を **削除**（`Remote` は CQS 純粋ロジックに戻す）
- `impl Remoting for RemoteShared` を新設（各メソッドは `with_write` / `with_read` で内部 `Remote` にデリゲート）

### Phase 6: adapter 側 I/O ワーカー化

- `inbound_dispatch` を `RemoteEvent::InboundFrameReceived` push のみに退化
- `outbound_loop` を削除
- `handshake_driver` を削除（handshake timer は `StartHandshake` 実行時に adapter 側 I/O ワーカーが per-association tokio task として確保）
- `effect_application` から `StartHandshake` ignore 削除

### Phase 7: adapter 側 `RemoteEventReceiver` 実装と spawn 経路

- `tokio_remote_event_receiver.rs` 新設（tokio mpsc 受信側ラッパ）
- `RemotingExtensionInstaller` のフィールドを `remote_shared: RemoteShared` / `event_sender: Sender<RemoteEvent>` / `event_receiver: Option<TokioMpscRemoteEventReceiver>` / `run_handle: Option<JoinHandle<...>>` に揃える
- `RemotingExtensionInstaller::install` で `RemoteShared::new(remote)` を構築する（**run task の spawn は行わない**、`Remote::start` も呼ばない）
- `installer.remote() -> RemoteShared` で `RemoteShared` の clone を返す（raw `SharedLock<Remote>` を露出しない）
- 外部から `installer.remote().start()` を呼んで `Remote::start`（transport listening 確立）してから、`installer.spawn_run_task()` 等の明示 API で run task を起動する（install と start と spawn を分離する β パターン）
- `RemoteShared::shutdown` は `with_write(|r| r.shutdown())` の純デリゲートのみ（**wake しない、event_sender を持たない**）
- 停止 wake と完了観測は adapter 固有の `installer.shutdown_and_join().await` に集約（lifecycle 遷移 + `event_sender.try_send(TransportShutdown)` + `run_handle.await` の 3 ステップ）

### Phase 8: テスト・検証

- `cargo test -p fraktor-remote-core-rs` / `-p fraktor-remote-adaptor-std-rs` / `-p fraktor-cluster-adaptor-std-rs` green
- handshake / quarantine / watermark backpressure / instrument 通知の integration test
- showcase（`showcases/std/remote_lifecycle/` 等）が新 API で起動

各 Phase は独立して merge 可能な PR に分割する（tasks.md 参照）。

## Open Questions

1. **`RemoteEvent::InboundFrameReceived` および `OutboundEnqueued` のペイロード所有権** — `alloc::vec::Vec<u8>` / `OutboundEnvelope` を move で渡すか、`Arc<[u8]>` / pooled buffer で zero-copy にするか。本 change では move で簡素に進め、zero-copy / per-authority channel 分離 / ring buffer 化等の最適化は別 change で扱う。
2. **adapter 内部 mpsc channel の bounded / unbounded 選択** — adapter 内部実装詳細であり capability spec では規定しない。実装 PR で判断する（既定 bounded、capacity は `RemoteConfig` から読む方向）。bounded で `try_send` 失敗時の挙動（caller への error 伝播 / drop / 待機）も実装 PR で確定する。
3. **quarantine timer / OutboundFrameAcked / BackpressureCleared の scheduling 経路** — 本 change のスコープ外。必要時に別 change で `RemoteEvent` の variant 追加と `RemoteTransport::schedule_*` method 追加を一緒に行う。
4. **`Remoting::addresses()` の動的更新** — adapter 起動後に listening address が変わるケース（NAT / virtual network / port reassignment）の取扱い。本 change では「起動時にキャッシュした `Vec<Address>` を返すだけ」で固定し、動的更新は別 change で扱う。
5. **並行 change 名 `hide-remote-adaptor-runtime-internals` の rename** — 同名の active change が `openspec/changes/hide-remote-adaptor-runtime-internals/` に存在し、change 名に禁止サフィックス "Runtime" を含む。ただし当該 change が変更する capability は `remote-adaptor-std-public-surface` / `remote-core-package` であり、本 change で rename した `remote-adaptor-std-io-worker`（旧 `remote-adaptor-std-runtime`）とは衝突しない。change 名そのものの rename は当該 change 自身または別 change で扱うべきスコープ。

## References

- 並行 change: `openspec/changes/hide-remote-adaptor-runtime-internals/`（adapter public surface 縮小、※change 名に "Runtime" を含むが capability 名は別系統で衝突なし、Open Questions #5 参照）
- 既存 capability: `openspec/specs/remote-core-extension/spec.md`、`openspec/specs/remote-core-association-state-machine/spec.md`、`openspec/specs/remote-core-instrument/spec.md`、`openspec/specs/remote-core-transport-port/spec.md`、`openspec/specs/remote-adaptor-std-io-worker/spec.md`
- 参照実装: `references/pekko/Association.scala`、`references/protoactor-go/`

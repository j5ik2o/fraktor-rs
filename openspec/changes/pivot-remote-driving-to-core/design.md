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

目指す経路は次の形である（**新規型はゼロ。`Remote::run` という inherent method 1 つと、Port 2 つ追加のみ**）。

```text
remote-core
├─ Remote::run (async, inherent method)
│   ├─ poll RemoteEventSource           ← Port (新規)
│   ├─ Association メソッドへ dispatch
│   ├─ Codec で encode / decode
│   ├─ AssociationEffect 実行 (StartHandshake 含む)
│   ├─ outbound queue (system / user) を消化
│   └─ watermark で apply_backpressure を発火
├─ RemoteEvent                            ← 新規 enum (closed)
├─ RemoteEventSource                      ← 新規 trait (1 メソッド)
└─ RemoteTransport                        → adapter (既存)

remote-adaptor-std
├─ inbound I/O ワーカー
│   └─ raw frame を RemoteEvent として adapter 内部 sender に push
├─ handshake timer task (per association)
│   └─ tokio::time::sleep → adapter 内部 sender に HandshakeTimerFired push
├─ TokioMpscEventSource                   tokio mpsc 受信側ラッパ
└─ Remote::run の spawn (RemotingExtensionInstaller から)
```

`hide-remote-adaptor-runtime-internals` で std adapter の public surface は既に縮小されているため、本 change は内部実装の駆動主導権を反転することに集中できる。

## Goals / Non-Goals

**Goals:**

- 駆動主導権を `remote-core` に集約し、`remote-adaptor-std` は I/O とイベント通知だけを担当する形にする。
- `RemoteInstrument` を hot path で機能させ、`RemotingFlightRecorder` を観測手段として稼働させる。
- system message が ordinary message に飢餓されない既存保証を維持する。
- outbound queue の watermark を超えたら backpressure signal を発火し、計測可能にする。
- `AssociationEffect::StartHandshake` を「adapter で無視」状態から「`Remote::run` で実行」状態に戻す。
- handshake timeout の古い発火を `u64` generation で識別し、誤った状態遷移を防ぐ。
- **新規型・新規 trait の純増を最小化する**（純増 2 個まで: `RemoteEvent` enum と `RemoteEventSource` trait のみ）。

**Non-Goals:**

- wire protocol、`Codec<T>` trait シグネチャ、payload serialization の再設計。
- inbound 側 ack window、動的 receive buffer。
- large message queue の追加（control / ordinary 分離のみ維持）。
- failure detector / heartbeat の駆動経路変更（別 change）。
- cluster adaptor、persistence adaptor、stream adaptor の駆動見直し。
- 後方互換 shim、deprecated alias の残置。
- **新規 Driver / Handle / Outcome / Timer / Sink / Generation newtype の導入**（純増ゼロ最優先のため）。

## Decisions

### Decision 1: `Remote::run` を inherent method として追加し、新規 Driver 型を作らない

第一案では `RemoteDriver<S, K, T, I, C>` という 5 ジェネリクスの新規型を導入していたが、これは「Association を駆動するループを所有する」という責務以上のことをしておらず、`Remote` 構造体に統合できる。

```rust
impl Remote {
    pub async fn run<S>(&mut self, source: &mut S) -> Result<(), RemotingError>
    where
        S: RemoteEventSource,
    {
        loop {
            match source.recv().await {
                None => return Ok(()),                       // source 枯渇 = 正常終了
                Some(RemoteEvent::TransportShutdown) => return Ok(()),
                Some(event) => self.handle_event(event)?,
            }
        }
    }
}
```

`Remote` 自体は型パラメータを持たない（Decision 4 参照）。instrument は `Box<dyn RemoteInstrument + Send>` フィールドで保持するため、`run` 側のシグネチャは `S` のみがジェネリクス。

**棄却した代替案:**

- *`RemoteDriver` 新規型*: ジェネリクス連鎖（`<S, K, T, I, C>`）が拡散し、ユーザー API が複雑化する。`Remote` の inherent method 化で責務を統合する方が利用しやすい（registry + transport + lifecycle 駆動）。
- *`RemoteDriverHandle` / `RemoteDriverOutcome`*: `Result<(), RemotingError>` で「正常終了 / 異常終了 / 強制停止」を表現できるため、新規 Outcome enum は冗長。停止制御は既存 `Remoting::shutdown` が adapter 内部 sender 経由で `TransportShutdown` を push することで実現する。

### Decision 2: `RemoteEvent` を closed enum（5 variant に限定）、`RemoteEventSource` を 1 メソッド trait にする

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

`RemoteEventSource` は `Remote::run` が消費する側のみ。

```rust
pub trait RemoteEventSource: Send {
    fn recv(&mut self) -> impl core::future::Future<Output = Option<RemoteEvent>> + Send + '_;
}
```

`async fn` を trait に直書きせず `-> impl Future` にして dyn 互換性は意図的に外す（`Remote::run` 側はジェネリクス、adapter 側は具象実装で完結する）。

**`RemoteEventSink` trait は core に追加しない**。adapter は内部で `tokio::sync::mpsc::channel` を作り、`Sender` clone を I/O ワーカー / handshake timer task に配り、`Receiver` を `TokioMpscEventSource` でラップして `Remote::run` に渡す。sender 側は adapter 内部の責務であり、core から見る必要がない。これにより new trait を 1 つ削減できる。

### Decision 3: Timer Port を core に追加せず、handshake timer は `RemoteTransport::schedule_handshake_timeout` で表出する

第一案では `Timer` trait（`schedule` / `cancel` / `TimerToken`）を新規追加していたが、これも純増ゼロ方針で削減できる。

ただし、`AssociationEffect::StartHandshake { authority, timeout, generation }` を `Remote::run` が処理する際、core 側から adapter に「timer を予約せよ」と指示する経路が必要。これを **既存 `RemoteTransport` trait の method 追加** で表現する。

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

`Remote::run` の StartHandshake 処理（疑似コード）:

```text
when AssociationEffect::StartHandshake { authority, timeout, generation } を見つける:
    1. let frame = self.codec.encode(handshake_request_envelope(&authority))?;
       self.transport.send(frame)?;                                      // 既存
    2. self.transport.schedule_handshake_timeout(&authority, timeout, generation)?;  // 新
```

adapter 側 `schedule_handshake_timeout` 実装:

```text
fn schedule_handshake_timeout(&mut self, authority, timeout, generation) -> Result<()> {
    let sender = self.event_sender.clone();
    tokio::spawn(async move {
        tokio::time::sleep(timeout).await;
        let _ = sender.send(RemoteEvent::HandshakeTimerFired { authority, generation }).await;
        // ↑ adapter 内部の閉じた経路。古い generation の発火は Remote::run 側で破棄される
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
- *新 effect `ScheduleHandshakeTimeout`*: `StartHandshake` から自然に派生する 2 ステップ目を独立 effect にすると、Association の effect 列が冗長になる。`StartHandshake` 自身が「send + schedule」の 2 ステップを意味する効果と定義する方が圧縮的。

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

`Remote::run` での instrument 借用は `&mut *self.instrument`（`Box::deref_mut` 経由で `&mut dyn RemoteInstrument`）として取得し、`Association` の状態遷移メソッドに渡す。

```rust
impl Remote {
    pub async fn run<S: RemoteEventSource>(&mut self, source: &mut S) -> Result<(), RemotingError> {
        loop {
            match source.recv().await {
                None => return Ok(()),
                Some(RemoteEvent::TransportShutdown) => return Ok(()),
                Some(event) => {
                    let instrument: &mut dyn RemoteInstrument = &mut *self.instrument;
                    self.handle_event(event, instrument)?;
                }
            }
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
- `Remote::run` ループ内で local 変数 `let instrument: &mut dyn RemoteInstrument = &mut *self.instrument;` として保持する。
- `Association` の状態遷移メソッドは `instrument: &mut dyn RemoteInstrument` 引数で受け取り、型パラメータを導入しない。

**split-borrow 前提:**

`&mut *self.instrument` を保持したまま `self.handle_event(...)` のような inherent method を呼ぶと、`self` 全体の再 `&mut` 借用が発生して借用衝突する。実装側では次のいずれかで split borrow を確保する。

1. event 処理を free function（または associated function）に切り出し、`registry` / `transport` / `codec` / `instrument` を個別 `&mut` 引数で受け取る形にする
2. ループ先頭で `let Self { registry, transport, codec, instrument, .. } = self;` と destructuring し、field 単位で別々の `&mut` 借用を作って helper に渡す

Rust の field 単位 borrow split は安全に成立するため、この前提さえ守れば設計に問題はない。

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

watermark 連動の発火経路は `Remote::run` 内で `Association::total_outbound_len()` を `outbound_high_watermark` / `outbound_low_watermark` と比較し、状態遷移時に `Association::apply_backpressure(BackpressureSignal::Apply)` または `Release` を呼ぶ。`Association` 側は手動 `apply_backpressure` 呼び出しと watermark 経由の自動呼び出しを区別しない。

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

// Remote::run 側の staleness 判定（疑似コード）
fn is_stale_handshake_timer(current: u64, event_generation: u64) -> bool {
    // `wrapping_add(1)` を使う以上、大小比較ではなく等値比較で判定する。
    // u64::MAX → 0 の wrap でも漏れずに「現行 generation 以外＝ stale」と扱える。
    current != event_generation
}
```

`u64` の意味付けは rustdoc に依存し、型安全性は若干落ちるが、`Association` 内部および `Remote::run` の判定経路でしか使わないため過剰設計を避ける。

### Decision 8: lifecycle 制御は既存 `Remoting::start` / `shutdown` で完結する

`RemoteDriverHandle` / `RemoteDriverOutcome` を導入せず、既存 `Remoting` trait の `start` / `shutdown` で lifecycle を制御する。

- `Remoting::start`: tokio task として `Remote::run` を spawn する経路は adapter 側の `RemotingExtensionInstaller` が担当する（adapter は `Remote` の所有権を持つため、`run` を spawn できる）。
- `Remoting::shutdown`: adapter 内部 sender 経由で `RemoteEvent::TransportShutdown` を push し、`Remote::run` ループの `match` で `Ok(())` を返してループ終了する。adapter 側は spawn した task の `JoinHandle` を待つ。

`Remote::run` の戻り値 `Result<(), RemotingError>` は次の意味で使う。

- `Ok(())`: source 枯渇または `TransportShutdown` 受信による正常終了
- `Err(RemotingError::TransportUnavailable)`: 復帰不能エラー（transport 永続失敗、association registry 破損 等）

### Decision 9: AssociationEffect::StartHandshake の adapter 無視を禁止し、`Remote::run` で 2 ステップ処理する

`AssociationEffect::StartHandshake` のセマンティクスを「`Remote::run` 経路で **send + schedule_handshake_timeout** の 2 ステップ実行」と明示する。adapter 側 `effect_application::apply_effects_in_place` から該当分岐を削除する。

```rust
// Remote::run の effect 処理（疑似コード）
fn handle_effect(&mut self, effect: AssociationEffect) -> Result<(), RemotingError> {
    match effect {
        AssociationEffect::StartHandshake { authority, timeout, generation } => {
            // ステップ 1: handshake request frame の送出
            let frame = self.codec.encode(handshake_request_envelope(&authority))
                .map_err(RemotingError::CodecFailed)?;
            self.transport.send(frame)
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

**adapter 側の閉ループ**: adapter は `schedule_handshake_timeout` 受領を契機に tokio task で sleep を起動し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で source に push する。古い generation の発火は `Remote::run` 側で `!=` 判定により破棄される（adapter 側でキャンセル責務を負わない、Decision 7 と整合）。

**adapter 側 `effect_application` の責務縮減**: `StartHandshake` 分岐を削除し、`SendEnvelopes` / `DiscardEnvelopes` / `PublishLifecycle` 等の I/O 直結 effect のみを扱う。状態遷移を伴う effect は `Remote::run` 側に集約される。

### Decision 10: `Remote` の所有権を run task に move する（共有可変性なし）

`Remote::run(&mut self, ...)` は長期実行 task として `&mut self` を保持し続けるため、`Arc<Mutex<Remote>>` 等で外部から共有すると常にロック衝突する。これを避けるため、`Remote` の所有権を spawn した tokio task に **move で渡す** ことを必須とする。

```text
RemotingExtensionInstaller::install(actor_system) {
    let remote = Remote::with_instrument(transport, config, event_publisher, instrument);
    // capacity は実装 PR で確定（本 change では RemoteConfig に capacity 用の新フィールドを追加しない、Open Questions 参照）
    let (sender, receiver) = tokio::sync::mpsc::channel(EVENT_CHANNEL_CAPACITY);
    let mut source = TokioMpscRemoteEventSource::new(receiver);
    let cached_addresses = remote.addresses().to_vec();  // 起動直後にキャッシュ（Remote::addresses() 一本）

    let run_handle = tokio::spawn(async move {
        // remote と source の所有権はここに move
        let mut remote = remote;
        remote.run(&mut source).await
    });

    // installer は sender / run_handle / cached_addresses のみ保持
    Self {
        event_sender: sender,
        run_handle,
        cached_addresses,
    }
}
```

外部制御 surface は次の **2 つだけ**:

- `Sender<RemoteEvent>` — `Remoting::shutdown` で `TransportShutdown` を push、I/O ワーカー / handshake timer task / RemoteActorRef が clone 共有
- `JoinHandle<Result<(), RemotingError>>` — `Remoting::shutdown` で await

`Remoting::addresses()` は installer の `cached_addresses` から返し、run 中の `Remote` には触らない。これにより `&mut self` 衝突が原理的に発生しない。

`Remoting::shutdown` の手順:

```text
1. event_sender.send(RemoteEvent::TransportShutdown).await?;
2. run_handle.await {
       Ok(Ok(()))    => Ok(()),                                  // 正常終了
       Ok(Err(e))    => Err(e),                                  // run の Err 伝播
       Err(join_err) => Err(RemotingError::TransportUnavailable),// task panic 等
   }
```

**棄却した代替案:**

- *`Arc<Mutex<Remote>>` で共有*: run 中は常時ロック中で、shutdown / addresses が必ず blocking。デッドロック懸念。
- *`AShared<Remote>` パターン*: 共有可変性を core に持ち込む。fraktor の `AShared` 原則は「どうしても共有が必要な場合の最終手段」であり、本ケースは run task 単独所有で済むため不要。

### Decision 11: outbound enqueue は `RemoteEvent::OutboundEnqueued` で表現する

`outbound_loop.rs` 削除後、local actor から outbound queue への enqueue 経路で `Remote::run` を起こす wake event が必要。これを新 variant `RemoteEvent::OutboundEnqueued { authority, envelope }` で表現する。

```text
local actor.tell(remote_ref, msg)
  → adapter RemoteActorRef
    → OutboundEnvelope::user(msg, ...)
    → adapter event_sender.send(RemoteEvent::OutboundEnqueued { authority, envelope })

Remote::run loop:
  match source.recv().await {
    Some(RemoteEvent::OutboundEnqueued { authority, envelope }) => {
      let assoc = self.registry.get_mut(&authority);
      assoc.enqueue(envelope, &mut *self.instrument);
      self.drain_outbound(&authority)?;        // next_outbound → encode → send
    }
    // ...
  }
```

**棄却した代替案:**

- *`RemoteEvent::OutboundEnqueued { authority }` (signal のみ、envelope は別経路)*: `AssociationRegistry` を adapter から直接 mutate するため `Mutex` / `RwLock` / lock-free queue が必要。core に内部可変性を持ち込む。fraktor の「内部可変性禁止 / `&mut self` 原則」に反する。
- *別 channel での envelope 配送*: `Remote::run` 内で複数 channel を `select!` する形になり、ループの複雑性が上がる。`RemoteEvent` enum 1 本に集約する方が見通しがよい。
- *enqueue を effect で表現*: enqueue は core から adapter への要求ではなく adapter から core への通知方向のため、effect ではなく event が適切。

**コスト**: `OutboundEnvelope` が event channel を経由するため、payload が大きい場合に move / copy コストが発生する。本 change ではシンプルさを優先し、zero-copy / per-authority channel 分離 / ring buffer 化等の最適化は別 change の余地として残す（Open Questions 参照）。

## Risks / Trade-offs

### Risk 1: `Box<dyn RemoteInstrument>` の vtable オーバーヘッド

hot path（`on_send` / `on_receive` / `record_*`）で 1-2ns の vtable lookup が常時発生する。`Remote<I>` で zero-cost にする選択肢もあったが、Decision 4 で参照実装（Pekko / protoactor-go）と Rust 観測ライブラリ（tracing / metrics）の慣行に倣い、API の単純化を優先して dyn dispatch を採用した。

**緩和策:** ベンチマークで vtable の影響を測定し、tokio mpsc send / codec encode / mutex acquisition のコストに対して noise 範囲であることを実装 PR で確認する。問題が顕在化したら、その時点でジェネリクス化を再検討する（YAGNI）。`NoopInstrument` 既定の場合は分岐予測が効きやすく、コンパイラが devirtualize する余地もある。

### Risk 2: `Remote::run` の長期保有 → 所有権 move で解決

`Remote::run` は `&mut self` を保持し続けるため、`Arc<Mutex<Remote>>` 等で外部から共有すると常時ロック衝突する。

**解決策（Decision 10 で確定）:** `Remote` の所有権を spawn した tokio task に **move で渡す**。外部制御は `Sender<RemoteEvent>` と `JoinHandle<Result<(), RemotingError>>` の 2 surface のみ。`Remoting::addresses()` は installer の起動時キャッシュから返す。これにより `&mut self` 衝突が原理的に発生しない。

### Risk 3: handshake timer の責務分担 → `RemoteTransport::schedule_handshake_timeout` で確定

第一案では timer 予約の責務分担が曖昧だった。

**解決策（Decision 9 で確定）:** 既存 `RemoteTransport` trait に `schedule_handshake_timeout(&mut self, &TransportEndpoint, Duration, u64) -> Result<(), TransportError>` を追加（既存 capability `remote-core-transport-port` への MODIFIED）。`Remote::run` は `AssociationEffect::StartHandshake` を「`send` → `schedule_handshake_timeout`」の 2 ステップで処理する。adapter 実装は `schedule_handshake_timeout` で tokio task を spawn する。残る曖昧さなし。

### Risk 4: 設計純化と Pekko 互換性のトレードオフ

`Timer` Port を作らないことで、組み込み / WASM 等での migration 時に「sleep 抽象を adapter ごとに再実装する」必要が出る。Pekko の `Scheduler` のような汎用 Timer 抽象は提供しない。

**緩和策:** 必要が生じた段階で別 change として `Timer` Port を追加すれば良い。現時点では adapter が tokio sleep で十分であり、YAGNI を優先する。

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
- `RemoteEventSource` trait を `core/extension/remote_event_source.rs` に追加

### Phase 4: `Remote::run` 実装

- `Remote::run<S: RemoteEventSource>(&mut self, source: &mut S) -> Result<(), RemotingError>` の inherent method 実装
- event match の dispatch 表
- effect 列処理（`StartHandshake` / `SendEnvelopes` / `DiscardEnvelopes` / `PublishLifecycle`）
- watermark 連動 backpressure 発火

### Phase 5: adapter 側 I/O ワーカー化

- `inbound_dispatch` を `RemoteEvent::InboundFrameReceived` push のみに退化
- `outbound_loop` を削除
- `handshake_driver` を削除（handshake timer は `StartHandshake` 実行時に adapter 側 I/O ワーカーが per-association tokio task として確保）
- `effect_application` から `StartHandshake` ignore 削除

### Phase 6: adapter 側 `RemoteEventSource` 実装と spawn 経路

- `tokio_remote_event_source.rs` 新設（tokio mpsc 受信側ラッパ）
- `RemotingExtensionInstaller` から `Remote::run` を tokio task として spawn する経路を追加
- 停止時 `Remoting::shutdown` で adapter 内部 sender 経由 `TransportShutdown` push → `run` task の join

### Phase 7: テスト・検証

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

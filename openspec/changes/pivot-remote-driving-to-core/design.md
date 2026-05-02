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

目指す経路は次の形である。

```text
remote-core
├─ RemoteDriver
│   ├─ poll RemoteEventSource
│   ├─ Association メソッドへ dispatch
│   ├─ Codec で encode / decode
│   ├─ AssociationEffect 実行 (StartHandshake 含む)
│   ├─ outbound queue (control / ordinary) を消化
│   └─ watermark で apply_backpressure を発火
├─ RemoteDriverHandle             start / shutdown / outcome
├─ RemoteEventSource              ← adapter
├─ RemoteEventSink                → adapter
├─ Timer                          → adapter
├─ Codec                          → adapter (既存)
└─ RemoteTransport                → adapter (既存)

remote-adaptor-std
├─ inbound I/O ワーカー
│   └─ raw frame を RemoteEvent として sink に push
├─ TokioMpscEventSource           tokio mpsc 実装
├─ TokioMpscEventSink
├─ TokioTimer
└─ Driver 起動 (RemotingExtensionInstaller から spawn)
```

`hide-remote-adaptor-runtime-internals` で std adapter の public surface は既に縮小されているため、本 change は内部実装の駆動主導権を反転することに集中できる。

## Goals / Non-Goals

**Goals:**

- 駆動主導権を `remote-core` に集約し、`remote-adaptor-std` は I/O とイベント通知だけを担当する形にする。
- `RemoteInstrument` を hot path で機能させ、`RemotingFlightRecorder` を観測手段として稼働させる。
- system message が ordinary message に飢餓されない保証を Pekko Artery 互換の形で表現する。
- outbound queue の watermark を超えたら backpressure signal を発火し、計測可能にする。
- Driver の lifecycle（起動 / 正常停止 / 異常終了）を型で表現する。
- `AssociationEffect::StartHandshake` を「adapter で無視」状態から「Driver で実行」状態に戻す。

**Non-Goals:**

- wire protocol、`Codec<T>` trait シグネチャ、payload serialization の再設計。
- inbound 側 ack window、動的 receive buffer。
- large message queue の追加（control / ordinary 分離のみ）。
- failure detector / heartbeat の駆動経路変更（別 change）。
- cluster adaptor、persistence adaptor、stream adaptor の駆動見直し。
- 後方互換 shim、deprecated alias の残置。

## Decisions

### Decision 1: `RemoteDriver` を `remote-core` に新設し、駆動主導権を反転する

`modules/remote-core/src/core/driver/` 配下に以下を置く。

```rust
pub struct RemoteDriver<S, K, T, I, C>
where
    S: RemoteEventSource,
    K: Timer,
    T: RemoteTransport,
    I: RemoteInstrument,
    C: Codec<InboundEnvelope>,
{
    registry: AssociationRegistry,
    transport: T,
    timer: K,
    instrument: I,
    codec: C,
    config: RemoteConfig,
}

impl<S, K, T, I, C> RemoteDriver<S, K, T, I, C> { /* ... */
    pub async fn run(mut self, mut source: S) -> RemoteDriverOutcome {
        loop {
            match source.recv().await {
                None => return RemoteDriverOutcome::SourceExhausted,
                Some(RemoteEvent::TransportShutdown) => {
                    return RemoteDriverOutcome::Shutdown { reason: ... };
                }
                Some(event) => {
                    if let Err(error) = self.handle(event) {
                        return RemoteDriverOutcome::Aborted { error };
                    }
                    self.drive_outbound();
                }
            }
        }
    }
}
```

`run` は所有権を取り、終了時に `RemoteDriverOutcome` を返す（CQS 違反にならないよう、内部状態の所有を返さず outcome のみを返す）。

Driver は `&mut self` ベースのループで動き、内部可変性（`Arc<Mutex<...>>` 等）を Driver 自身が持つことは原則禁止する。`AssociationRegistry` を所有し、外部から共有しない。

### Decision 2: `RemoteEvent` を closed enum、`RemoteEventSource` / `RemoteEventSink` を Port にする

`RemoteEvent` は core が adapter 側から受け取る closed enum。

```rust
pub enum RemoteEvent {
    InboundFrameReceived { authority: Authority, frame: WireFrameBytes },
    OutboundFrameAcked   { authority: Authority, sequence: u64 },
    HandshakeTimerFired  { authority: Authority, generation: HandshakeGeneration },
    QuarantineTimerFired { authority: Authority },
    ConnectionLost       { authority: Authority, cause: ConnectionLostCause },
    TransportShutdown,
    BackpressureCleared  { authority: Authority },
}
```

`RemoteEventSource` は Driver が消費する側、`RemoteEventSink` は adapter が push する側。

```rust
pub trait RemoteEventSource {
    fn recv(&mut self) -> impl Future<Output = Option<RemoteEvent>> + Send;
}

pub trait RemoteEventSink: Send + Sync {
    fn push(&self, event: RemoteEvent) -> Result<(), RemoteEventDispatchError>;
}
```

`async fn` を trait に直書きせず `-> impl Future` にして dyn 互換性は意図的に外す（Driver 側はジェネリクス、adapter 側は具象実装で完結する）。

### Decision 3: `RemoteInstrument` をジェネリクス + composite で配線する（`&mut self` 維持）

既存仕様（`remote-core-instrument` capability）で `RemoteInstrument` は `&mut self` 系 hook を持つ。本 change はこのシグネチャを維持し、内部可変性（`Cell` / `SpinSyncMutex`）を instrument に持たせる必要を作らない。

```rust
pub trait RemoteInstrument {
    fn on_send(&mut self, envelope: &OutboundEnvelope);
    fn on_receive(&mut self, envelope: &InboundEnvelope);
    fn record_handshake(&mut self, authority: &Authority, phase: HandshakePhase, now_ms: u64);
    fn record_quarantine(&mut self, authority: &Authority, reason: QuarantineReason, now_ms: u64);
    fn record_backpressure(&mut self, authority: &Authority, signal: BackpressureSignal, correlation: Option<u64>, now_ms: u64);
}
```

`Remote<I: RemoteInstrument = NoopInstrument>` 化し、Driver も同じ型パラメータを伝播する。`NoopInstrument` は何もしない実装で、デフォルト `I` で zero-cost に振る舞う。

複数 instrument の合成は tuple impl で行う。`&mut self` でも `self.0` と `self.1` は disjoint なフィールドなので順次借用が成立する。

```rust
impl<A, B> RemoteInstrument for (A, B)
where A: RemoteInstrument, B: RemoteInstrument
{
    fn on_send(&mut self, e: &OutboundEnvelope) {
        self.0.on_send(e);
        self.1.on_send(e);
    }
    /* ... */
}
```

`RemotingFlightRecorder` は `RemoteInstrument` を実装する具象型として残し、ユーザーは `(FlightRecorder, MyMetrics)` のように tuple 合成できる。

`&mut I` の伝播経路は次のとおり。

- `Remote<I>` の `&mut self` 経由で `&mut self.instrument: &mut I` を取得する。
- Driver は `RemoteDriver<S, K, T, I, C>` の `&mut self` ループで `&mut self.instrument` を保持する。
- Driver は `Association` の状態遷移メソッドに `&mut I` を引数として渡す（`Association` は instrument を field 保持しない）。

### Decision 4: 既存の system / user 2 キュー分離を維持し、`total_outbound_len()` を追加する

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

これは Driver が watermark 判定に使うクエリで、CQS 準拠の `&self` query。`OutboundQueueSet` のような新型は導入しない（既存 `SendQueue` の構造変更を避ける）。

### Decision 5: Watermark backpressure を導入する

`RemoteConfig` に追加する。

```rust
pub struct RemoteConfig {
    /* ... existing ... */
    pub outbound_high_watermark: usize, // default: 1024
    pub outbound_low_watermark: usize,  // default: 512
}
```

`Association` 内に backpressure 状態を持つ。

```rust
enum BackpressureState { Released, Engaged }
```

`enqueue` 後に `total_len > high && state == Released` なら `Engaged` に遷移させ、`apply_backpressure(Engaged)` を発火する。`pop_next` 後に `total_len < low && state == Engaged` なら `Released` に遷移させ、`apply_backpressure(Released)` を発火する。

`BackpressureSignal` は既存型を再利用するか、`Engaged` / `Released` を表す closed enum に再定義する（design 後半で確定）。

### Decision 6: `AssociationEffect::StartHandshake` を Driver で実行する

`AssociationEffect::StartHandshake { endpoint }` は Driver で `RemoteTransport::initiate_handshake(endpoint)` に dispatch する。adapter 側の `effect_application::apply_effects_in_place` から `StartHandshake` ignore 分岐を削除する。

`RemoteTransport` に initiate_handshake が無ければ追加する。signature は以下を想定する。

```rust
pub trait RemoteTransport {
    fn initiate_handshake(&self, endpoint: &Endpoint) -> Result<(), TransportError>;
    /* ... */
}
```

Driver は `Effect::StartHandshake` を実行した後、`Timer::schedule(handshake_timeout)` で timeout を予約する。timeout 発火時は `RemoteEvent::HandshakeTimerFired` が source に流れ、Driver が `Association::handshake_timed_out` を呼ぶ。

### Decision 7: Driver lifecycle と `RemoteDriverOutcome`

```rust
pub struct RemoteDriverHandle {
    sink: Box<dyn RemoteEventSink>,
    join: BoxFuture<'static, RemoteDriverOutcome>,
}

impl RemoteDriverHandle {
    pub fn shutdown(&self) -> Result<(), RemoteEventDispatchError> {
        self.sink.push(RemoteEvent::TransportShutdown)
    }
    pub async fn outcome(self) -> RemoteDriverOutcome { self.join.await }
}

pub enum RemoteDriverOutcome {
    Shutdown { reason: ShutdownReason },
    SourceExhausted,
    Aborted { error: RemoteDriverError },
}
```

adapter 側は Driver を tokio task として spawn し、`RemoteDriverHandle` を `RemotingExtensionInstaller` に保持させる。actor system 停止時に `shutdown()` を呼び、`outcome()` を待つ。

### Decision 8: Codec 経路を Driver に明示する

Driver は inbound 側で raw `WireFrameBytes` を受け、`Codec::decode` で `InboundEnvelope` に復号してから `Association::accept_handshake_*` / inbound dispatch に渡す。outbound 側は `Association::next_outbound` で得た `OutboundEnvelope` を `Codec::encode` で raw bytes 化してから `RemoteTransport::send_frame` を呼ぶ。

`Codec<T>` のシグネチャは変えない。Driver が `Codec<InboundEnvelope>` と `Codec<OutboundEnvelope>` を保持する形で、対応する型ごとに実装が選ばれる。

adapter 側の TCP layer は raw bytes と frame boundary だけを扱い、payload type を知らない。これは前 change `hide-remote-adaptor-runtime-internals` の方針と整合する。

### Decision 9: `Timer` Port を追加する

```rust
pub trait Timer: Send + Sync {
    fn schedule(&self, delay: Duration, event: RemoteEvent) -> TimerToken;
    fn cancel(&self, token: TimerToken);
}
```

adapter 側は tokio `sleep_until` で実装する。Driver は handshake / quarantine timer を `Timer::schedule` で予約し、トークンを `Association` 状態に保持する（再ハンドシェイク時に cancel する）。

heartbeat / failure detector tick は本 change の対象外（別 change で扱う）。

### Decision 10: ジェネリクス vs dyn の選択

| 対象 | 選択 | 理由 |
|---|---|---|
| `RemoteInstrument` | ジェネリクス | hot path（全 envelope）で dyn dispatch を避ける |
| `RemoteEventSource` | ジェネリクス | Driver が所有、複数実装を混在させない |
| `RemoteEventSink` | dyn (`Box<dyn>`) | adapter 側で `RemoteDriverHandle` に保持し、`shutdown()` の signature 安定性を優先 |
| `Timer` | ジェネリクス | Driver が所有、混在させない |
| `RemoteTransport` | dyn (既存) | 既存の `Box<dyn RemoteTransport>` を踏襲 |
| `Codec<T>` | ジェネリクス | hot path の encode / decode で dyn 越し dispatch を避ける |

`RemoteTransport` は既存設計で dyn なので踏襲する。本 change で `Box<dyn>` から ジェネリクスに変更する利益は薄く、影響範囲が大きいため別 change で扱う。

## Alternatives Considered

### Alternative A: instrument を `AssociationEffect::Instrument(InstrumentEvent)` 経由で adapter 実行

却下。`on_send` / `on_receive` は全 envelope に掛かる hot path で、Effect Vec への append + adapter 側 dispatch のオーバーヘッドが無視できない。Effect 経由は handshake / quarantine 等の低頻度イベントに限定する案も検討したが、配線が二系統になり認知負荷が増えるため採用しない。

### Alternative B: 既存 `Remoting` trait に `drive` メソッドを追加して Driver を統合する

却下。`Remoting` は薄い lifecycle trait で、駆動ループを持つ責務は別。trait を肥大化させると `Remoting` の意味が「lifecycle + driving + ...」と複合化し、責務境界が崩れる。

### Alternative C: queue 細分化を Pekko Artery 完全互換（Control / Ordinary / LargeMessage）にする

却下（現時点）。既存の system / user 2 キュー分離で飢餓回避は満たされている。Large message queue は frame size 上限と分割再送ロジックを伴うため独立 change で扱う。本 change はこの分離を維持し、追加するのは watermark 連動の発火経路と `total_outbound_len` クエリのみとする。

### Alternative D: 1ms ポーリングを残したまま instrument だけ配線する

却下。ユーザー要件「Port & Adapter 前提の core 主導駆動」を満たさない。instrument 配線は派生的な恩恵で、本質的には駆動主導権の反転が中心。

### Alternative E: tokio mpsc を core 側に直接埋め込む

却下。core は no_std 維持が原則（cfg-std-forbid lint）。`RemoteEventSource` Port を切って adapter 実装に閉じ込める。

## Risks / Trade-offs

- Driver のジェネリクス連鎖（`RemoteDriver<S, K, T, I, C>`）が monomorphization コストを増やす。ただし remote crate は単一バイナリで型確定するため二項展開は抑えられる。
- `Remote<I = NoopInstrument>` のジェネリクス化により、`Remote` を保持する周辺型（`RemotingExtensionInstaller` など）にも `<I>` が伝播する。default 型で見かけ上は `Remote` と書けるが、型推論が複雑になる箇所は明示型注釈が必要。
- adapter 側 task 削除に伴い、既存の reconnect / restart 制御が `RemoteEvent::ConnectionLost` 駆動に変わる。再接続ロジックが Driver 側に移るため、`reconnect_backoff_policy` / `restart_counter` の所有権が core 側に来る。これらは純粋な値オブジェクトなので core への移動は妥当。
- `RemoteEvent` を closed enum にすると adapter 側で新規 event 種を増やす際に core 側 enum 拡張が必要になる。これは Pekko Artery の `InboundEnvelope` / `OutboundEnvelope` と同じ性質で、open hierarchy より型安全。
- watermark 既定値（1024 / 512）は経験値。設定可能にしているので運用時に調整できる。
- `RemoteEventSource` の `async fn` を `-> impl Future` で表現するとトレイトオブジェクト化できず、複数 source 実装の動的差し替えはコンパイル時固定となる。これは意図的な制約。

## Migration Plan

1. 既存 `AssociationEffect::StartHandshake` の adapter 側 ignore 分岐をコメントアウトせずに、Driver 経路を新設してから一括差し替える。
2. Driver と adapter 側 I/O ワーカーは新規ファイルとして追加し、既存 `outbound_loop` / `handshake_driver` / `inbound_dispatch` は最後に削除する。
3. `Remote` ジェネリクス化は `Remote<I = NoopInstrument>` のデフォルト型で既存呼び出し点をできる限り維持する。明示型注釈が必要な箇所は同一 commit で更新する。
4. 既存 integration test は public API 経由で動かし続ける（Driver が裏で動く形）。internal test は Driver を直接構築する形に書き換える。
5. `rtk cargo test -p fraktor-remote-core-rs`、`rtk cargo test -p fraktor-remote-adaptor-std-rs`、最後に `rtk ./scripts/ci-check.sh ai all` で確認する。

## Open Questions

- `RemoteInstrument` の `on_send` / `on_receive` を `&self` にする場合、内部状態を持つ instrument が `SpinSyncMutex` を抱える形になる。これは fraktor-rs immutability policy の例外（計測責務に閉じる）として明文化するが、reviewer 確認が必要。
- `RemoteEventSource::recv` の signature は `-> impl Future<Output = Option<RemoteEvent>>` か `async fn recv(&mut self) -> Option<RemoteEvent>` か。RPITIT が安定して使えるなら後者を選ぶ（Rust 1.75 以降）。
- `Timer::schedule` が返す `TimerToken` の cancel idempotency 保証（複数回 cancel しても安全か、既に発火したトークンに対する cancel 挙動）。
- `BackpressureSignal` を既存 enum と統合するか、`Engaged` / `Released` 専用 enum を新設するか。
- `RemoteConfig::outbound_high_watermark` / `outbound_low_watermark` の既定値（1024 / 512）は適切か。Pekko Artery 既定値との整合を確認する。
- adapter から sink に push したイベントが Driver で処理されるまでの遅延が、handshake タイムアウト等の判定に影響しないか（実測が必要）。

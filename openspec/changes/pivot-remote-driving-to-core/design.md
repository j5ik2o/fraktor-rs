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
impl<I: RemoteInstrument> Remote<I> {
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

**棄却した代替案:**

- *`RemoteDriver` 新規型*: ジェネリクス連鎖（`<S, K, T, I, C>`）が拡散し、ユーザー API が複雑化する。`Remote<I>` で I だけ表面化する設計の方が利用しやすく、責務も同じ（registry + transport + lifecycle 駆動）。
- *`RemoteDriverHandle` / `RemoteDriverOutcome`*: `Result<(), RemotingError>` で「正常終了 / 異常終了 / 強制停止」を表現できるため、新規 Outcome enum は冗長。停止制御は既存 `Remoting::shutdown` が adapter 内部 sender 経由で `TransportShutdown` を push することで実現する。

### Decision 2: `RemoteEvent` を closed enum、`RemoteEventSource` を 1 メソッド trait にする

`RemoteEvent` は core が adapter 側から受け取る closed enum。

```rust
pub enum RemoteEvent {
    InboundFrameReceived { authority: TransportEndpoint, frame: alloc::vec::Vec<u8> },
    OutboundFrameAcked   { authority: TransportEndpoint, sequence: u64 },
    HandshakeTimerFired  { authority: TransportEndpoint, generation: u64 },
    QuarantineTimerFired { authority: TransportEndpoint },
    ConnectionLost       { authority: TransportEndpoint, cause: ConnectionLostCause },
    TransportShutdown,
    BackpressureCleared  { authority: TransportEndpoint },
}
```

`RemoteEventSource` は `Remote::run` が消費する側のみ。

```rust
pub trait RemoteEventSource: Send {
    fn recv(&mut self) -> impl core::future::Future<Output = Option<RemoteEvent>> + Send + '_;
}
```

`async fn` を trait に直書きせず `-> impl Future` にして dyn 互換性は意図的に外す（`Remote::run` 側はジェネリクス、adapter 側は具象実装で完結する）。

**`RemoteEventSink` trait は core に追加しない**。adapter は内部で `tokio::sync::mpsc::channel` を作り、`Sender` clone を I/O ワーカー / handshake timer task に配り、`Receiver` を `TokioMpscEventSource` でラップして `Remote::run` に渡す。sender 側は adapter 内部の責務であり、core から見る必要がない。これにより new trait を 1 つ削減できる。

### Decision 3: Timer Port を core に追加しない

第一案では `Timer` trait（`schedule` / `cancel` / `TimerToken`）を新規追加していたが、これも純増ゼロ方針で削減できる。

handshake timeout / quarantine timer は **adapter 側の tokio task が `tokio::time::sleep` し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を内部 sender 経由で source に push する** ことで実現する。core から見ると単に「ある時刻に event が来る」状態であり、専用 Port は不要。

```text
adapter:
   when AssociationEffect::StartHandshake { authority, timeout, generation } を実行:
     1. RemoteTransport::send で handshake request frame 送信
     2. tokio::spawn(async move {
          tokio::time::sleep(timeout).await;
          let _ = sender.send(RemoteEvent::HandshakeTimerFired { authority, generation });
        });
```

`utils-core::DelayProvider` は actor-core の scheduler に紐づいており、remote 用途で再利用するには結合が強い。adapter 内部の tokio sleep で完結させる方が層分離として健全。

**棄却した代替案:**

- *Core 側 Timer trait + adapter 実装*: schedule / cancel / TimerToken を core 公開 API に増やす。本質的に「event を遅延配信する」という責務は adapter で閉じられるため不要。
- *`utils-core::DelayProvider` 経由*: actor-core scheduler との結合があり、layer 整合性を崩す。

### Decision 4: `RemoteInstrument` をジェネリクス + tuple composite で配線する（`&mut self` 維持、`NoopInstrument` 型を作らない）

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

`Remote<I: RemoteInstrument = ()>` 化する。**`NoopInstrument` 型は新設せず、`impl RemoteInstrument for ()` を提供して `()` を no-op 既定として使う**。

```rust
impl RemoteInstrument for () {
    fn on_send(&mut self, _: &OutboundEnvelope) {}
    fn on_receive(&mut self, _: &InboundEnvelope) {}
    fn record_handshake(&mut self, _: &TransportEndpoint, _: HandshakePhase, _: u64) {}
    fn record_quarantine(&mut self, _: &TransportEndpoint, _: QuarantineReason, _: u64) {}
    fn record_backpressure(&mut self, _: &TransportEndpoint, _: BackpressureSignal, _: Option<u64>, _: u64) {}
}
```

`Remote<()>` がデフォルトで zero-cost に振る舞う。これにより新規 ZST 型 `NoopInstrument` を追加せずに済む。

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

`RemotingFlightRecorder` は `RemoteInstrument` を実装する具象型として残し、ユーザーは `Remote<(RemotingFlightRecorder, MyMetrics)>` のように tuple 合成できる。

`&mut I` の伝播経路は次のとおり。

- `Remote<I>` の `&mut self` 経由で `&mut self.instrument: &mut I` を取得する。
- `Remote::run` ループで `&mut self.instrument` を保持する。
- `Association` の状態遷移メソッドに `&mut I` を引数として渡す（`Association` は instrument を field 保持しない）。

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

### Decision 9: AssociationEffect::StartHandshake の adapter 無視を禁止し、`Remote::run` で実行する

`AssociationEffect::StartHandshake` のセマンティクスを「`Remote::run` 経路で `RemoteTransport` 経由の handshake 開始 + adapter 内部 sender への timer task spawn」と明示する。adapter 側 `effect_application::apply_effects_in_place` から該当分岐を削除する。

```rust
// Remote::run の effect 処理（疑似コード）
fn handle_effect(&mut self, effect: AssociationEffect) -> Result<(), RemotingError> {
    match effect {
        AssociationEffect::StartHandshake { authority, timeout, generation } => {
            self.transport.send(handshake_request_envelope(&authority))?;
            // adapter 側で timer をスケジュールするための情報は、
            // 将来的に別の effect (例: ScheduleEvent) として表出させる必要がある。
            // ただし current scope では transport.send の延長で adapter 側 I/O ワーカーが
            // 内部 sender 経由で HandshakeTimerFired を push する責務を持つ
            // (詳細は adapter 側 capability で規定)。
            Ok(())
        }
        // ...
    }
}
```

**Open issue (実装段階で詰める)**: handshake timer の予約タイミングを effect で表出させるか、`RemoteTransport::initiate_handshake(authority, timeout, generation)` のような形で transport 契約に折り込むか。本 change は capability spec で「adapter は generation 付きの timer を adapter 内部で確保する責務を持つ」と書き、具体的な経路は実装 PR で決める。

## Risks / Trade-offs

### Risk 1: `Remote<I>` のジェネリクス伝播

`Remote<I>` から `Association` メソッドへ `&mut I` を渡す経路がコード全体に伝播する。既存呼出箇所（テスト、showcase、cluster adaptor 等）への影響を実装 PR で評価する。`I = ()` をデフォルト型にすることで、明示しない呼出は `Remote<()>` として動作する。

**緩和策:** `Remote::new(...)` のシグネチャを `Remote<()>` 既定で互換に保ち、instrument 指定時は `Remote::with_instrument(transport, config, event_publisher, instrument)` 等の builder method を提供する。

### Risk 2: `Remote::run` の長期保有

`Remote::run` は `&mut self` を保持し続けるため、ループ実行中は `Remote` の他のメソッド（`addresses`、`shutdown`）を同時に呼べない。既存 `Remoting` 実装は同期メソッドのみで、shutdown が `&mut self` を要求するため衝突する。

**緩和策:** adapter 側で `Remote` を `Arc<Mutex<Remote<I>>>` で保持し、`run` は別 task で動作させ、`shutdown` は adapter 内部 sender 経由の `TransportShutdown` push で間接的に終了させる。`Remote::shutdown` を呼ぶ前に `run` task が終了しているよう順序保証する。

### Risk 3: handshake timer の責務分担曖昧さ

`AssociationEffect::StartHandshake` を実行する際の timer 予約責務が、`Remote::run` と adapter 側 `RemoteTransport` のどちらに帰属するかが Decision 9 で曖昧。実装 PR で transport API を拡張するか、effect variant を増やすかの判断が残る。

**緩和策:** capability spec では「adapter が generation 付き timer を確保する」MUST 要件を書くにとどめ、API 形は実装 PR の判断とする。本 change の artifact レビューでは、その分担に「曖昧さ残置を許容する」ことを明示しレビュー対象から外す。

### Risk 4: 設計純化と Pekko 互換性のトレードオフ

`Timer` Port を作らないことで、組み込み / WASM 等での migration 時に「sleep 抽象を adapter ごとに再実装する」必要が出る。Pekko の `Scheduler` のような汎用 Timer 抽象は提供しない。

**緩和策:** 必要が生じた段階で別 change として `Timer` Port を追加すれば良い。現時点では adapter が tokio sleep で十分であり、YAGNI を優先する。

## Migration Plan

### Phase 1: instrument 配線基盤の整備（破壊的変更を含む）

- `impl RemoteInstrument for ()` を追加
- `(A, B)` / `(A, B, C)` の tuple composite を追加
- `Remote<I: RemoteInstrument = ()>` ジェネリクス化（既存呼出は `()` で吸収）
- `RemotingFlightRecorder: impl RemoteInstrument`
- 既存テスト・showcase の `Remote::new` 呼出を `()` で吸収

### Phase 2: Association 配線

- `Association::associate` / `handshake_accepted` / `handshake_timed_out` / `quarantine` / `apply_backpressure` のシグネチャに `instrument: &mut I` を追加
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

1. **`AssociationEffect::StartHandshake` における handshake timer 予約の責務分担** — `RemoteTransport::initiate_handshake` を transport API に追加するか、別の `AssociationEffect::ScheduleEvent { delay, on_expire }` variant を追加するか。本 change の artifact レビューでは曖昧さを許容し、実装 PR で確定する。
2. **`RemoteEvent::InboundFrameReceived` のフレーム所有権** — `alloc::vec::Vec<u8>` で渡すか、参照で渡すか。current scope では `Vec<u8>` で簡素に進める（zero-copy 最適化は別 change）。
3. **adapter 内部 mpsc channel の bounded / unbounded 選択** — adapter 内部実装詳細であり capability spec では規定しない。実装 PR で判断する（既定 bounded、capacity は `RemoteConfig` から読む方向）。

## References

- 前段 change: `openspec/changes/hide-remote-adaptor-runtime-internals/`（adapter public surface 縮小）
- 既存 capability: `openspec/specs/remote-core-extension/spec.md`、`openspec/specs/remote-core-association-state-machine/spec.md`、`openspec/specs/remote-core-instrument/spec.md`、`openspec/specs/remote-adaptor-std-runtime/spec.md`
- 参照実装: `references/pekko/Association.scala`、`references/protoactor-go/`

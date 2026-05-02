## Why

`remote-core` は Pekko Artery 互換の状態機械、Port trait、wire model を no_std で備えるが、現在は駆動の主導権が `remote-adaptor-std` 側の tokio task 群に握られている。`outbound_loop` が 1ms ポーリングで `Association::next_outbound` を回し、`inbound_dispatch` が `accept_handshake_*` を直接呼び、`HandshakeDriver` が timeout を駆動する。`AssociationEffect::StartHandshake` は adaptor 側で無視されており、core が表明した意図が adapter に伝わらない設計欠陥が残っている。

これにより以下の問題が生じる。

- 組み込み / WASM 等で std を使えない環境では tokio task 群を移植する必要があり、core 主導という設計意図が機能しない。
- `remote-core` には 604 行の `RemoteInstrument` / `RemotingFlightRecorder` が定義されているが、`Association` から一度も呼ばれておらず、配信失敗・handshake 進捗・quarantine が観測不能である。
- Pekko Artery が保証する system message の飢餓回避は既存の system / user 2 キュー分離で成立しているが、双方向の watermark backpressure と handshake generation 管理が未実装である。
- `Remote` から `Codec`、`RemoteTransport`、association registry を駆動する経路が暗黙で、event loop の lifecycle（起動 / 停止）が文書化されていない。

正式リリース前の今、Port & Adapter の純度を上げ、駆動主導権を core 側に反転する。adapter は I/O と event 通知だけを担当する形に退化させる。

## 設計方針

**純増ゼロを最優先とし、既存型・既存 Port への配置換えで主導権反転を実現する。** 新規責務を増やすのではなく、現在 adapter にある駆動責務を core 側の既存型に吸収させる。新規 Port は「core が必須で持てない adapter→core push 経路」一つだけに絞る。

具体的に追加する公開要素は次の **2 つだけ** とする。

- `RemoteEvent` enum（adapter が core に通知するイベント種別、closed enum）
- `RemoteEventSource` trait（core が adapter から event を pull する 1 メソッド trait）

これ以外の機能（駆動ループ、handshake generation 管理、watermark backpressure、instrument 配線）は **既存型のメソッド・フィールド追加** で実現する。

## What Changes

### 1. `Remote::run` で駆動主導権を core に集約する（新規型を作らない）

`Remote` 構造体に inherent method として `pub async fn run<S: RemoteEventSource>(&mut self, source: &mut S) -> Result<(), RemotingError>` を追加する。`Remoting` trait は同期 lifecycle 専用のままにし、async event loop は `Remote` の inherent method として trait 契約を侵食しない。

run の中で source からイベントを受信し、対応する `Association` メソッドへ dispatch して effect 列を実行する。`AssociationEffect::StartHandshake` を復活させ、run 経路から `RemoteTransport` 経由で handshake を開始する。

新規型（`RemoteDriver` / `RemoteDriverHandle` / `RemoteDriverOutcome`）は **作らない**。lifecycle 制御は既存 `Remoting::start` / `shutdown` と `Result<(), RemotingError>` で表現する。

### 2. `RemoteEvent` を closed enum、`RemoteEventSource` を 1 メソッド trait として追加する

`RemoteEvent` は core が adapter から受け取る closed enum。`InboundFrameReceived` / `HandshakeTimerFired` / `ConnectionLost` / `TransportShutdown` 等の必要バリアントだけを持つ。

`RemoteEventSource::recv` は `&mut self` で `Option<RemoteEvent>` を非同期に返す。adapter は `tokio::sync::mpsc::Receiver` 等で実装する。

**adapter→core push 用の `RemoteEventSink` trait は core に追加しない**。adapter は内部で sender / receiver pair を保持し、receiver 側だけを `RemoteEventSource` として core に渡す。sender 側は adapter 内部の I/O ワーカーが共有するため、core から見ない。

### 3. Timer Port を新設しない（adapter 内部責務）

handshake timeout / quarantine timer 等の遅延発火は adapter が tokio task で実現し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を内部 sender 経由で source に push する。core は専用 Timer Port を持たず、event 入力だけで時間に依存する状態遷移を駆動する。

これにより、`utils-core` の既存 `DelayProvider` / `MonotonicClock` を再利用するための新規 trait 追加も不要になる。

### 4. `RemoteInstrument` を `Box<dyn>` で `Remote` に配線する（ジェネリクス採用しない）

`Remote` は型パラメータを持たず、`instrument: Box<dyn RemoteInstrument + Send>` フィールドで instrument を保持する。

ジェネリクス `Remote<I: RemoteInstrument = ()>` を採用しない理由：

- 参照実装（Apache Pekko の `RemoteInstrument` abstract class、protoactor-go の interface）はいずれも virtual / dyn dispatch を採用しており、production 規模で問題なく動いている
- hot path での vtable lookup は ~1-2ns 程度であり、tokio mpsc send / codec encode / mutex acquisition 等のコストに対して noise レベル
- ジェネリクスを採用するとテスト・showcase・cluster adapter まで `<I>` が伝播し、ユーザー API が複雑化する
- runtime での instrument 差し替えができなくなる
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

`Association` に `handshake_generation: u64` フィールドを追加し、`Handshaking` 状態に入るたびに +1 する。`AssociationEffect::StartHandshake { authority, generation }` と `RemoteEvent::HandshakeTimerFired { authority, generation }` で同じ `u64` を運び、古い timeout の発火を `Remote::run` 側で識別して破棄する。

**`HandshakeGeneration` newtype は新設せず、`u64` を直接使う**（外部公開境界での意味付けは rustdoc に依存し、型レベルでは追加しない）。

### 8. adaptor task を I/O ワーカーに退化させる

`remote-adaptor-std` の以下を削除する。

- `outbound_loop.rs`（1ms ポーリングで `next_outbound` を回す tokio task）
- `handshake_driver.rs`（timeout を tokio sleep で駆動する task）

`inbound_dispatch.rs` は `RemoteEvent::InboundFrameReceived` を adapter 内部 sender に push する I/O ワーカーに退化させる。`Association` の状態遷移メソッドを直接呼ぶ責務を `Remote::run` に移す。

`effect_application.rs` の `StartHandshake` ignore 分岐を削除する（`Remote::run` が処理するため adapter ではすでに通らない）。

`RemotingExtensionInstaller` から `Remote::run` を tokio task として spawn し、停止時は既存 `Remoting::shutdown` を呼んで `Remote::run` 側のループ終了を待つ。

### 9. tokio ベース `RemoteEventSource` 実装を追加する

`remote-adaptor-std` に tokio mpsc 受信側を `RemoteEventSource` として実装した型を 1 つ追加する。送信側 sender clone は adapter 内部の I/O ワーカー / handshake timer task が保持する（adapter 内部のため公開 API ではない）。

## Capabilities

### Modified Capabilities

- **`remote-core-extension`**
  - `Remote` に `async fn run<S: RemoteEventSource>(&mut self, source: &mut S) -> Result<(), RemotingError>` を追加
  - `RemoteEvent` enum と `RemoteEventSource` trait を core 公開 API として追加
  - `Remoting` trait は既存通り同期 lifecycle 専用（async fn を増やさない）

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
  - `AssociationEffect::StartHandshake { authority, generation }` のセマンティクスを「`Remote::run` で実行」と明示し、adapter 無視を禁止する
  - `BackpressureSignal` の variant は既存 `Apply` / `Release` を維持する（新 variant 追加なし）

- **`remote-adaptor-std-runtime`**
  - `outbound_loop` / `handshake_driver` を REMOVED
  - `inbound_dispatch` は `RemoteEvent` を adapter 内部 sender に push する I/O ワーカーに退化
  - tokio mpsc 受信側を `RemoteEventSource` として実装した型を 1 つ追加
  - `RemotingExtensionInstaller` から `Remote::run` を tokio task として spawn する経路を追加
  - `effect_application.rs` の `StartHandshake` ignore 分岐を削除

### New Capabilities

なし（純増ゼロ）。

## Impact

**影響を受けるコード:**

- `modules/remote-core/src/core/extension/remote.rs`（ジェネリクス化、`run` 追加、instrument 配線）
- `modules/remote-core/src/core/extension/remote_event.rs`（新規、closed enum）
- `modules/remote-core/src/core/extension/remote_event_source.rs`（新規、1 メソッド trait）
- `modules/remote-core/src/core/association/base.rs`（instrument 引数追加、watermark 連動、handshake_generation field、total_outbound_len）
- `modules/remote-core/src/core/association/effect.rs`（`StartHandshake` rustdoc 更新、generation を含めるなら variant 拡張）
- `modules/remote-core/src/core/association/registry.rs`（instrument 参照経路、queue 分離追従）
- `modules/remote-core/src/core/instrument/`（`pub(crate) NoopInstrument` 内部定義、`RemotingFlightRecorder` への `RemoteInstrument` impl 追加）
- `modules/remote-core/src/core/config/`（`outbound_high_watermark` / `outbound_low_watermark`）
- `modules/remote-adaptor-std/src/std/outbound_loop.rs`（削除）
- `modules/remote-adaptor-std/src/std/handshake_driver.rs`（削除）
- `modules/remote-adaptor-std/src/std/inbound_dispatch.rs`（I/O ワーカーへ縮退）
- `modules/remote-adaptor-std/src/std/effect_application.rs`（`StartHandshake` ignore 削除）
- `modules/remote-adaptor-std/src/std/tokio_remote_event_source.rs`（新規、tokio mpsc 受信ラッパ）
- `modules/remote-adaptor-std/src/std/extension_installer.rs`（`Remote::run` spawn 経路）

**ファイル収支試算:**

- core: 新規 2（`remote_event.rs` / `remote_event_source.rs`）、削除 0
- adapter: 新規 1（`tokio_remote_event_source.rs`）、削除 2（`outbound_loop.rs` / `handshake_driver.rs`）
- 合計 net delta: **+1 ファイル**（新規 3、削除 2）。ただし新規 trait / 型の純増は **2 個**（`RemoteEvent` enum + `RemoteEventSource` trait）に抑制。

**公開 API 影響:**

- `Remote` は型パラメータを持たないまま、`Box<dyn RemoteInstrument + Send>` フィールドを内部に追加する（型シグネチャは変わらない）。`Remote::with_instrument` / `Remote::set_instrument` を新規 public API として追加する。
- `AssociationEffect::StartHandshake` の意味論が「adapter が無視」から「`Remote::run` が実行」へ変わる。
- adapter 側の `outbound_loop` / `handshake_driver` 公開関数は削除される。これは前 change `hide-remote-adaptor-runtime-internals` で internal 化済みのため外部 API 影響は無い。
- `RemoteEvent` enum と `RemoteEventSource` trait を新規 public 型として追加する。
- `RemoteConfig` に `outbound_high_watermark` / `outbound_low_watermark` を追加する。
- `Remote::run` を inherent method として追加する。`Remoting` trait は既存 4 メソッドのまま（async fn を増やさない）。

**挙動影響:**

- 1ms ポーリングが消え、event 駆動になる。outbound throughput と CPU 消費が改善する。
- system message が ordinary message に飢餓されないことが既存仕様で保証されている状態が継続する。
- queue が watermark を超えると backpressure signal が発火し、計測可能になる。
- handshake / quarantine / send / receive の全イベントが instrument に通知される。
- handshake timeout の古い発火が `Remote::run` 側で破棄され、generation 不一致による誤遷移が発生しない。

## Non-goals

- payload serialization の完成、wire protocol の再設計
- large message queue の追加（control / ordinary 分離のみ維持）
- inbound 側 ack window / 動的 receive buffer の調整
- cluster adaptor、persistence adaptor の駆動見直し
- failure detector の駆動経路変更（heartbeat は別 change で扱う）
- `Codec<T>` trait 自体のシグネチャ変更
- 後方互換 shim、deprecated alias、旧 API 残置
- 新規 Driver 型 / Handle 型 / Outcome enum / Timer trait / Sink trait / Generation newtype の導入（純増ゼロを優先するため）
- `Remote` の型パラメータ化、tuple composite `RemoteInstrument` 実装、`() impl RemoteInstrument` の提供（ユーザー API 単純化と参照実装整合のため、dyn dispatch を採用）

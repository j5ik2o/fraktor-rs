# remote-core-instrument Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: RemoteInstrument trait

`fraktor_remote_core_rs::domain::instrument::RemoteInstrument` trait が定義され、リモート通信のイベントフックを提供する SHALL。Pekko `RemoteInstrument` (Scala) に対応する。

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/instrument/remote_instrument.rs` を読む
- **THEN** `pub trait RemoteInstrument` が定義されている

#### Scenario: フックメソッドの存在

- **WHEN** `RemoteInstrument` の定義を読む
- **THEN** `on_send(&mut self, envelope: &OutboundEnvelope)`・`on_receive(&mut self, envelope: &InboundEnvelope)` または同等のフックメソッドが宣言されている

### Requirement: transport 実装非依存

`RemoteInstrument` trait および関連型は `tokio`・特定の transport 実装・特定の async runtime に依存しない SHALL。計装 (instrumentation) は transport と無関係に成立する責務である。

#### Scenario: feature gate の不在

- **WHEN** `modules/remote-core/src/instrument/` 配下のすべての `.rs` ファイルを検査する
- **THEN** `#[cfg(feature = "tokio-transport")]` または同等の transport 実装ゲートが存在しない

#### Scenario: tokio 非依存

- **WHEN** `modules/remote-core/src/instrument/` 配下のすべての import を検査する
- **THEN** `tokio` クレートへの参照が存在しない

### Requirement: RemotingFlightRecorder 型

`fraktor_remote_core_rs::domain::instrument::RemotingFlightRecorder` 型が定義され、リモート通信イベントの ring buffer ベースの記録を提供する SHALL。Pekko `RemotingFlightRecorder` に対応する。

#### Scenario: 型の存在

- **WHEN** `modules/remote-core/src/instrument/flight_recorder.rs` または同等のファイルを読む
- **THEN** `pub struct RemotingFlightRecorder` が定義されている

#### Scenario: 容量指定コンストラクタ

- **WHEN** `RemotingFlightRecorder::new` の定義を読む
- **THEN** `fn new(capacity: usize) -> Self` または同等のシグネチャが宣言されている

#### Scenario: snapshot メソッド

- **WHEN** `RemotingFlightRecorder::snapshot` の定義を読む
- **THEN** `fn snapshot(&self) -> RemotingFlightRecorderSnapshot` または同等のシグネチャが宣言されている (`&self` の query、CQS 準拠)

### Requirement: alloc ベースの ring buffer

`RemotingFlightRecorder` は内部に `alloc::collections::VecDeque<T>` または `alloc::vec::Vec<T>` ベースの ring buffer を持ち、容量を超えた古いイベントを自動的に破棄する SHALL。`heapless::Vec` は使わない (alloc が利用可能なため)。

#### Scenario: 容量超過時の破棄

- **WHEN** `capacity = 3` で `RemotingFlightRecorder` を作成し、5個のイベントを記録する
- **THEN** `snapshot()` が返すイベント数は3で、最も古い2個のイベントは含まれていない

#### Scenario: heapless 不依存

- **WHEN** `modules/remote-core/Cargo.toml` を検査する
- **THEN** `heapless` が依存に含まれていない

### Requirement: 時刻入力の引数化 (monotonic millis)

`RemotingFlightRecorder` のイベント記録メソッドは時刻を **monotonic millis** として引数で受け取り、`Instant::now()` や `SystemTime::now()` を内部で呼ばない SHALL。flight recorder の snapshot はイベントの発生順序を保つために単調増加する時刻軸を必要とするため、wall clock は使わない。wall clock 情報が必要な場合 (ログ突き合わせ等) は adapter 側で別途タイムスタンプを付与する。

#### Scenario: record_event の時刻引数

- **WHEN** `RemotingFlightRecorder::record_*` 系メソッドの定義を読む
- **THEN** いずれも時刻入力 (`now_ms: u64 /* monotonic millis */`) を引数として持つ

#### Scenario: doc comment の monotonic 明示

- **WHEN** `RemotingFlightRecorder::record_*` 系メソッドの rustdoc を読む
- **THEN** `now` パラメータが **monotonic millis** であることが明示されている

#### Scenario: Instant 直接呼び出しの不在

- **WHEN** `modules/remote-core/src/instrument/` 配下のすべての `.rs` ファイルを検査する
- **THEN** `Instant::now()`・`SystemTime::now()`・`std::time::` の参照が存在しない

### Requirement: FlightRecorderEvent 型

`fraktor_remote_core_rs::domain::instrument::FlightRecorderEvent` 型 (enum または trait) が定義され、記録対象のイベント種別 (送信、受信、ハンドシェイク、quarantine、backpressure 等) を表現する SHALL。

#### Scenario: イベント型の存在

- **WHEN** `modules/remote-core/src/instrument/` 配下を検査する
- **THEN** `pub enum FlightRecorderEvent` または `pub trait FlightRecorderEvent` が定義されている

#### Scenario: backpressure イベントの記録

- **WHEN** `RemotingFlightRecorder::record_backpressure(authority, signal, correlation_id, now)` を呼んだ後に `snapshot()` を呼ぶ
- **THEN** snapshot に該当 backpressure イベントが含まれる

### Requirement: Remote は RemoteInstrument を Box<dyn> で保持する

`fraktor_remote_core_rs::extension::Remote` は `Box<dyn RemoteInstrument + Send>` フィールドで instrument を保持しなければならない（MUST）。`Remote` 構造体に型パラメータ `<I>` を導入してはならない（MUST NOT）。

#### Scenario: Remote のシグネチャ

- **WHEN** `modules/remote-core/src/extension/remote.rs` を読む
- **THEN** `pub struct Remote { /* ... */ }` が宣言され、ジェネリクス `<I: RemoteInstrument>` を持たない
- **AND** `instrument: alloc::boxed::Box<dyn RemoteInstrument + Send>` フィールドを保持している

#### Scenario: ジェネリクス採用しない理由（spec 内 rationale）

- **WHEN** `Remote` に `<I: RemoteInstrument>` を導入したい衝動が生じる
- **THEN** 参照実装（Apache Pekko の `RemoteInstrument` abstract class、protoactor-go の interface）が virtual / dyn dispatch を採用していること、および hot path のオーバーヘッド差がネットワーク I/O・codec・mpsc send に対して noise レベルであることを根拠に却下する
- **AND** ジェネリクスによる型パラメータ伝播コスト（テスト・showcase・cluster adapter 等）の方が大きい

#### Scenario: hot path での Arc 経由の不在

- **WHEN** `Remote` から instrument の `on_send` / `on_receive` を呼び出す経路を検査する
- **THEN** `Arc<dyn RemoteInstrument>` 経由でなく、所有 `Box<dyn RemoteInstrument + Send>` 経由で `&mut self.instrument` を取得する
- **AND** Arc clone のコストが hot path に発生しない

### Requirement: 内部 NoopInstrument は pub(crate) でデフォルト実体として保持する

`Remote` の既定 instrument として、すべての method が空実装の `NoopInstrument` 型を `pub(crate)` で内部定義し、`Remote::new` 構築時に `Box::new(NoopInstrument)` を割り当てる SHALL。`NoopInstrument` を `pub` で外部公開してはならない（MUST NOT）。

#### Scenario: NoopInstrument の宣言

- **WHEN** `modules/remote-core/src/instrument/noop_instrument.rs` または `modules/remote-core/src/instrument/` 配下を読む
- **THEN** `pub(crate) struct NoopInstrument;` または同等の ZST が定義されている
- **AND** `impl RemoteInstrument for NoopInstrument` がすべての method を空実装で提供する

#### Scenario: 公開 API への露出禁止

- **WHEN** `modules/remote-core/src/instrument.rs` および `modules/remote-core/src/lib.rs` の `pub use` 経路を検査する
- **THEN** `NoopInstrument` を `pub use` または `pub` で公開していない
- **AND** ユーザーは `Remote::new(...)` で構築するだけで no-op 既定が得られ、`NoopInstrument` を import する必要がない

#### Scenario: Remote::new の既定 instrument

- **WHEN** `Remote::new(transport, config, event_publisher)` を呼ぶ
- **THEN** 内部で `Box::new(NoopInstrument)` を `instrument` フィールドに割り当てる
- **AND** 構築直後に instrument 経由のフックを呼んでも副作用が発生しない

### Requirement: instrument を差し替えるための公開 API

`Remote` は構築時または構築後に instrument を差し替える公開 API を提供する SHALL。

#### Scenario: with_instrument 構築

- **WHEN** `Remote::with_instrument(transport, config, event_publisher, instrument)` を呼ぶ
- **THEN** `instrument: Box<dyn RemoteInstrument + Send>` を `instrument` フィールドに割り当てて `Remote` を構築する

#### Scenario: set_instrument による差し替え

- **WHEN** 既存の `Remote` インスタンスに `set_instrument(instrument: Box<dyn RemoteInstrument + Send>)` を呼ぶ
- **THEN** 既存 instrument は drop され、新 instrument が割り当てられる
- **AND** event loop 実行中（`run` 進行中）でない時のみ呼ぶ前提を rustdoc に明記する

### Requirement: tuple composite と () impl は提供しない

`(A, B)` / `(A, B, C)` 等の tuple に対する `RemoteInstrument` 実装、および `()` に対する `RemoteInstrument` 実装は core 側で提供してはならない（MUST NOT）。複数 instrument の合成はユーザーが composite struct を自作する責務とする。

#### Scenario: tuple impl の不在

- **WHEN** `modules/remote-core/src/instrument/` 配下のソースを検査する
- **THEN** `impl<A, B> RemoteInstrument for (A, B)` および `impl<A, B, C> RemoteInstrument for (A, B, C)` が定義されていない

#### Scenario: () impl の不在

- **WHEN** `modules/remote-core/src/instrument/` 配下のソースを検査する
- **THEN** `impl RemoteInstrument for ()` が定義されていない（`Box<dyn>` ベース設計のため不要）

#### Scenario: ユーザー自作 composite の使用例

- **WHEN** ユーザーが複数 instrument を併用したい
- **THEN** `pub struct MyComposite { recorder: RemotingFlightRecorder, metrics: MyMetrics }` のような独自 struct を定義し、`impl RemoteInstrument for MyComposite` で各 method を順次 dispatch する形にする
- **AND** core 側ライブラリは composite ヘルパを提供しない（YAGNI、必要な場合はユーザーが独自 composite を書く）

### Requirement: RemotingFlightRecorder は RemoteInstrument を実装する

`RemotingFlightRecorder` は `RemoteInstrument` trait を実装し、`Box::new(RemotingFlightRecorder::new())` 形で `Remote::with_instrument` に渡せなければならない（MUST）。既存の record 系メソッド（`record_handshake` / `record_quarantine` / `record_backpressure` / `record_send` / `record_receive`）は `RemoteInstrument` 経由で間接的に発火されてもよい。

#### Scenario: RemoteInstrument 実装の存在

- **WHEN** `modules/remote-core/src/instrument/flight_recorder.rs` を読む
- **THEN** `impl RemoteInstrument for RemotingFlightRecorder` が定義されている
- **AND** `on_send` / `on_receive` / handshake / quarantine / backpressure 系の通知が内部 ring buffer に追加される

#### Scenario: with_instrument での利用

- **WHEN** `Remote::with_instrument(transport, config, event_publisher, Box::new(RemotingFlightRecorder::new(...)))` を呼ぶ
- **THEN** `Remote` が `RemotingFlightRecorder` を所有し、event loop 中の hook 発火が ring buffer に蓄積される

### Requirement: instrument 配線の Remote::handle_remote_event 透過

`Remote::handle_remote_event` は `Remote` の `&mut self` 経由で `&mut *self.instrument: &mut dyn RemoteInstrument` を取得し、Association メソッドへ渡す SHALL。

#### Scenario: Remote::handle_remote_event 内での instrument 借用

- **WHEN** `Remote::handle_remote_event` のループ実装を検査する
- **THEN** `self.instrument` への `&mut dyn RemoteInstrument` 参照が確保され、`Association` 関連メソッド呼び出しに渡される
- **AND** `Arc<dyn>` clone は発生しない

#### Scenario: 別 Driver 型を作らない

- **WHEN** `modules/remote-core/src/` 配下のソースを検査する
- **THEN** `pub struct RemoteDriver` または同等の Driver 型が定義されていない（純増ゼロ方針、`Remote::handle_remote_event` がその責務を負う）

### Requirement: instrument hook 呼出は association state machine からトリガされる

`RemoteInstrument` の各 method は `Association` の状態遷移または送受信メソッドからトリガされなければならない（MUST）。具体的な呼出点は `remote-core-association-state-machine` capability の要件で規定する。

#### Scenario: Remote::handle_remote_event と instrument の独立性

- **WHEN** `Remote::handle_remote_event` から instrument 直接呼出を検査する
- **THEN** `Remote::handle_remote_event` は `Association` を経由して instrument を呼ぶか、または Association メソッドの戻り値（effect）を介して間接的にトリガする経路を持つ
- **AND** `Remote::handle_remote_event` が状態遷移コンテキストを持たずに instrument 単体で何かを記録することはない（state-aware であるべき記録は association 経由）


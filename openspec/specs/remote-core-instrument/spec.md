# remote-core-instrument Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: RemoteInstrument trait

`fraktor_remote_core_rs::instrument::RemoteInstrument` trait が定義され、リモート通信のイベントフックを提供する SHALL。Pekko `RemoteInstrument` (Scala) に対応する。

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

`fraktor_remote_core_rs::instrument::RemotingFlightRecorder` 型が定義され、リモート通信イベントの ring buffer ベースの記録を提供する SHALL。Pekko `RemotingFlightRecorder` に対応する。

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

`fraktor_remote_core_rs::instrument::FlightRecorderEvent` 型 (enum または trait) が定義され、記録対象のイベント種別 (送信、受信、ハンドシェイク、quarantine、backpressure 等) を表現する SHALL。

#### Scenario: イベント型の存在

- **WHEN** `modules/remote-core/src/instrument/` 配下を検査する
- **THEN** `pub enum FlightRecorderEvent` または `pub trait FlightRecorderEvent` が定義されている

#### Scenario: backpressure イベントの記録

- **WHEN** `RemotingFlightRecorder::record_backpressure(authority, signal, correlation_id, now)` を呼んだ後に `snapshot()` を呼ぶ
- **THEN** snapshot に該当 backpressure イベントが含まれる


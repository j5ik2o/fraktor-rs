## ADDED Requirements

### Requirement: Remote は RemoteInstrument を Box<dyn> で保持する

`fraktor_remote_core_rs::core::extension::Remote` は `Box<dyn RemoteInstrument + Send>` フィールドで instrument を保持しなければならない（MUST）。`Remote` 構造体に型パラメータ `<I>` を導入してはならない（MUST NOT）。

#### Scenario: Remote のシグネチャ

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を読む
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

- **WHEN** `modules/remote-core/src/core/instrument/noop_instrument.rs` または `modules/remote-core/src/core/instrument/` 配下を読む
- **THEN** `pub(crate) struct NoopInstrument;` または同等の ZST が定義されている
- **AND** `impl RemoteInstrument for NoopInstrument` がすべての method を空実装で提供する

#### Scenario: 公開 API への露出禁止

- **WHEN** `modules/remote-core/src/core/instrument.rs` および `modules/remote-core/src/lib.rs` の `pub use` 経路を検査する
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

- **WHEN** `modules/remote-core/src/core/instrument/` 配下のソースを検査する
- **THEN** `impl<A, B> RemoteInstrument for (A, B)` および `impl<A, B, C> RemoteInstrument for (A, B, C)` が定義されていない

#### Scenario: () impl の不在

- **WHEN** `modules/remote-core/src/core/instrument/` 配下のソースを検査する
- **THEN** `impl RemoteInstrument for ()` が定義されていない（`Box<dyn>` ベース設計のため不要）

#### Scenario: ユーザー自作 composite の使用例

- **WHEN** ユーザーが複数 instrument を併用したい
- **THEN** `pub struct MyComposite { recorder: RemotingFlightRecorder, metrics: MyMetrics }` のような独自 struct を定義し、`impl RemoteInstrument for MyComposite` で各 method を順次 dispatch する形にする
- **AND** core 側ライブラリは composite ヘルパを提供しない（YAGNI、必要な場合はユーザーが独自 composite を書く）

### Requirement: RemotingFlightRecorder は RemoteInstrument を実装する

`RemotingFlightRecorder` は `RemoteInstrument` trait を実装し、`Box::new(RemotingFlightRecorder::new())` 形で `Remote::with_instrument` に渡せなければならない（MUST）。既存の record 系メソッド（`record_handshake` / `record_quarantine` / `record_backpressure` / `record_send` / `record_receive`）は `RemoteInstrument` 経由で間接的に発火されてもよい。

#### Scenario: RemoteInstrument 実装の存在

- **WHEN** `modules/remote-core/src/core/instrument/flight_recorder.rs` を読む
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

- **WHEN** `modules/remote-core/src/core/` 配下のソースを検査する
- **THEN** `pub struct RemoteDriver` または同等の Driver 型が定義されていない（純増ゼロ方針、`Remote::handle_remote_event` がその責務を負う）

### Requirement: instrument hook 呼出は association state machine からトリガされる

`RemoteInstrument` の各 method は `Association` の状態遷移または送受信メソッドからトリガされなければならない（MUST）。具体的な呼出点は `remote-core-association-state-machine` capability の要件で規定する。

#### Scenario: Remote::handle_remote_event と instrument の独立性

- **WHEN** `Remote::handle_remote_event` から instrument 直接呼出を検査する
- **THEN** `Remote::handle_remote_event` は `Association` を経由して instrument を呼ぶか、または Association メソッドの戻り値（effect）を介して間接的にトリガする経路を持つ
- **AND** `Remote::handle_remote_event` が状態遷移コンテキストを持たずに instrument 単体で何かを記録することはない（state-aware であるべき記録は association 経由）

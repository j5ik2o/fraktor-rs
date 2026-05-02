## ADDED Requirements

### Requirement: Remote が RemoteInstrument をジェネリクスで保持する

`fraktor_remote_core_rs::core::extension::Remote` は型パラメータ `I: RemoteInstrument`（既定型 `NoopInstrument`）で instrument を保持しなければならない（MUST）。`Arc<dyn RemoteInstrument>` を hot path で経由してはならない（MUST NOT）。

#### Scenario: Remote のジェネリクス署名

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を読む
- **THEN** `pub struct Remote<I: RemoteInstrument = NoopInstrument>` または同等のジェネリクス署名が宣言されている
- **AND** `instruments: I` または `instrument: I` フィールドを保持している

#### Scenario: hot path での dyn 経由の不在

- **WHEN** `Remote` から instrument の `on_send` / `on_receive` を呼び出す経路を検査する
- **THEN** `Box<dyn RemoteInstrument>` または `Arc<dyn RemoteInstrument>` を経由しない
- **AND** ジェネリクスによる static dispatch でメソッドが解決される

### Requirement: tuple ベースの composite RemoteInstrument

`RemoteInstrument` は tuple 経由で複数 instrument を合成できなければならない（MUST）。`(A, B)` および `(A, B, C)` まで少なくとも tuple impl が提供される。

#### Scenario: 2 要素 tuple の合成

- **WHEN** `(A, B)` 型に対する `RemoteInstrument` impl を検査する
- **THEN** `A: RemoteInstrument`、`B: RemoteInstrument` を bound として `RemoteInstrument` 実装が存在する
- **AND** 各 method 呼び出しは `self.0` → `self.1` の順で順次 dispatch する

#### Scenario: 3 要素 tuple の合成

- **WHEN** `(A, B, C)` 型に対する `RemoteInstrument` impl を検査する
- **THEN** `A: RemoteInstrument`、`B: RemoteInstrument`、`C: RemoteInstrument` を bound として `RemoteInstrument` 実装が存在する

#### Scenario: tuple 経由でも `&mut self` 借用検査が成立する

- **WHEN** `(A, B): RemoteInstrument` の `on_send` 実装が `self.0.on_send(env); self.1.on_send(env);` を呼ぶ
- **THEN** Rust の借用検査でコンパイルが通る（`&mut self.0` と `&mut self.1` は disjoint）

### Requirement: NoopInstrument の存在

`fraktor_remote_core_rs::core::instrument::NoopInstrument` 型が定義され、`RemoteInstrument` のすべての method を no-op として実装する SHALL。これは `Remote<I>` の既定型である。

#### Scenario: NoopInstrument の存在

- **WHEN** `modules/remote-core/src/core/instrument/noop_instrument.rs` を読む
- **THEN** `pub struct NoopInstrument;` または同等の zero-sized type が定義されている
- **AND** `impl RemoteInstrument for NoopInstrument` が存在し、すべての method 本体が空である

#### Scenario: zero-cost であることの担保

- **WHEN** `Remote<NoopInstrument>` を構築して `on_send` / `on_receive` 等の経路を辿る
- **THEN** `NoopInstrument` の method 呼び出しは monomorphization で消去され、ランタイム側で命令が残らないことを期待する（最適化レベルが release のとき）

### Requirement: instrument 配線の Driver 透過

`RemoteDriver` は型パラメータ `I: RemoteInstrument` を受け取り、Driver 内で association メソッドに instrument 参照を渡す SHALL。Driver から `&mut I` を `Association` 関連メソッドへ渡す経路が確立されている。

#### Scenario: Driver と Remote の I 一致

- **WHEN** `RemoteDriver<S, K, T, I, C>` と `Remote<I>` の型パラメータを検査する
- **THEN** 同じ `I` 型が両者で一貫している
- **AND** ユーザーは `Remote::<MyInstrument>::new(...)` で構築すれば Driver 構築でも同じ `I` が要求される

#### Scenario: instrument 参照の渡し方

- **WHEN** Driver の outbound 駆動経路（`next_outbound` 呼出）を検査する
- **THEN** `&mut I` または `&I` の参照が `Association` 関連メソッドまたはローカルラッパーに渡される
- **AND** instrument が `Arc<dyn>` でラップされていない

### Requirement: RemotingFlightRecorder は RemoteInstrument を実装する

`RemotingFlightRecorder` は `RemoteInstrument` trait を実装し、tuple 合成可能でなければならない（MUST）。既存の record 系メソッド（`record_handshake` / `record_quarantine` / `record_backpressure` / `record_send` / `record_receive`）は `RemoteInstrument` 経由で間接的に発火されてもよい。

#### Scenario: RemoteInstrument 実装の存在

- **WHEN** `modules/remote-core/src/core/instrument/flight_recorder.rs` を読む
- **THEN** `impl RemoteInstrument for RemotingFlightRecorder` が定義されている
- **AND** `on_send` / `on_receive` / handshake / quarantine / backpressure 系の通知が内部 ring buffer に追加される

#### Scenario: tuple 合成での共存

- **WHEN** `(RemotingFlightRecorder, MyMetricsInstrument)` を `Remote::with_instrument` または builder に渡す
- **THEN** `Remote<(RemotingFlightRecorder, MyMetricsInstrument)>` が構築でき、両方の instrument に通知が分配される

### Requirement: instrument hook 呼出は association state machine からトリガされる

`RemoteInstrument` の各 method は `Association` の状態遷移または送受信メソッドからトリガされなければならない（MUST）。具体的な呼出点は `remote-core-association-state-machine` capability の要件で規定する。

#### Scenario: Driver と instrument の独立性

- **WHEN** Driver から instrument 直接呼出を検査する
- **THEN** Driver は `Association` を経由して instrument を呼ぶか、または Association メソッドの戻り値（effect）を介して間接的にトリガする経路を持つ
- **AND** Driver が状態遷移コンテキストを持たずに instrument 単体で何かを記録することはない（state-aware であるべき記録は association 経由）

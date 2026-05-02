## ADDED Requirements

### Requirement: Remote が RemoteInstrument をジェネリクスで保持する

`fraktor_remote_core_rs::core::extension::Remote` は型パラメータ `I: RemoteInstrument`（既定型 `()`）で instrument を保持しなければならない（MUST）。`Arc<dyn RemoteInstrument>` を hot path で経由してはならない（MUST NOT）。

#### Scenario: Remote のジェネリクス署名

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を読む
- **THEN** `pub struct Remote<I: RemoteInstrument = ()>` または同等のジェネリクス署名が宣言されている
- **AND** `instrument: I` フィールドを保持している

#### Scenario: hot path での dyn 経由の不在

- **WHEN** `Remote` から instrument の `on_send` / `on_receive` を呼び出す経路を検査する
- **THEN** `Box<dyn RemoteInstrument>` または `Arc<dyn RemoteInstrument>` を経由しない
- **AND** ジェネリクスによる static dispatch でメソッドが解決される

### Requirement: () 型が RemoteInstrument の no-op 既定実装である

`()` 型に対して `impl RemoteInstrument for ()` が提供され、すべての method が no-op として実装されなければならない（MUST）。`NoopInstrument` 等の専用 ZST 型は新設してはならない（MUST NOT）。

#### Scenario: () 型の RemoteInstrument 実装の存在

- **WHEN** `modules/remote-core/src/core/instrument/` 配下を検査する
- **THEN** `impl RemoteInstrument for ()` が定義されている
- **AND** すべての method（`on_send`、`on_receive`、`record_handshake`、`record_quarantine`、`record_backpressure`）の本体が空である

#### Scenario: NoopInstrument 型の不在

- **WHEN** `modules/remote-core/src/core/instrument/` 配下のソースを検査する
- **THEN** `pub struct NoopInstrument` または `pub type NoopInstrument` が宣言されていない（純増ゼロ方針）

#### Scenario: zero-cost であることの担保

- **WHEN** `Remote<()>` を構築して `on_send` / `on_receive` 等の経路を辿る
- **THEN** `()` の method 呼び出しは monomorphization で消去され、ランタイム側で命令が残らないことを期待する（最適化レベルが release のとき）

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

### Requirement: RemotingFlightRecorder は RemoteInstrument を実装する

`RemotingFlightRecorder` は `RemoteInstrument` trait を実装し、tuple 合成可能でなければならない（MUST）。既存の record 系メソッド（`record_handshake` / `record_quarantine` / `record_backpressure` / `record_send` / `record_receive`）は `RemoteInstrument` 経由で間接的に発火されてもよい。

#### Scenario: RemoteInstrument 実装の存在

- **WHEN** `modules/remote-core/src/core/instrument/flight_recorder.rs` を読む
- **THEN** `impl RemoteInstrument for RemotingFlightRecorder` が定義されている
- **AND** `on_send` / `on_receive` / handshake / quarantine / backpressure 系の通知が内部 ring buffer に追加される

#### Scenario: tuple 合成での共存

- **WHEN** `(RemotingFlightRecorder, MyMetricsInstrument)` を `Remote::with_instrument` または builder に渡す
- **THEN** `Remote<(RemotingFlightRecorder, MyMetricsInstrument)>` が構築でき、両方の instrument に通知が分配される

### Requirement: instrument 配線の Remote::run 透過

`Remote::run` は `Remote<I>` の `&mut self` 経路で `&mut self.instrument: &mut I` を保持し、Association メソッドへ instrument 参照を渡す SHALL。

#### Scenario: Remote::run 内での instrument 借用

- **WHEN** `Remote::run` のループ実装を検査する
- **THEN** `self.instrument` への `&mut` 参照が確保され、`Association` 関連メソッド呼び出しに渡される
- **AND** instrument が `Arc<dyn>` でラップされていない

#### Scenario: 別 Driver 型を作らない

- **WHEN** `modules/remote-core/src/core/` 配下のソースを検査する
- **THEN** `pub struct RemoteDriver` または同等の Driver 型が定義されていない（純増ゼロ方針、`Remote::run` がその責務を負う）

### Requirement: instrument hook 呼出は association state machine からトリガされる

`RemoteInstrument` の各 method は `Association` の状態遷移または送受信メソッドからトリガされなければならない（MUST）。具体的な呼出点は `remote-core-association-state-machine` capability の要件で規定する。

#### Scenario: Remote::run と instrument の独立性

- **WHEN** `Remote::run` から instrument 直接呼出を検査する
- **THEN** `Remote::run` は `Association` を経由して instrument を呼ぶか、または Association メソッドの戻り値（effect）を介して間接的にトリガする経路を持つ
- **AND** `Remote::run` が状態遷移コンテキストを持たずに instrument 単体で何かを記録することはない（state-aware であるべき記録は association 経由）

# remote-core-watcher-state Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: WatcherState 型

`fraktor_remote_core_rs::domain::watcher::WatcherState` 型が定義され、RemoteWatcher の状態 (誰が誰を watch しているか、最後の heartbeat 時刻、quarantine 判定) を保持する SHALL。Pekko `RemoteWatcher` (Scala, 342行) のロジック部分に対応する。

#### Scenario: WatcherState の存在

- **WHEN** `modules/remote-core/src/watcher/watcher_state.rs` を読む
- **THEN** `pub struct WatcherState` が定義されている

#### Scenario: 状態の保持

- **WHEN** `WatcherState` のフィールドを検査する
- **THEN** watch 対象の集合 (`watching`)、各リモートノードの最後の heartbeat 時刻、failure detector instance を保持している

### Requirement: actor / scheduler / async の不在

`WatcherState` および関連型は actor framework・scheduler・async runtime に依存しない SHALL。Pekko `RemoteWatcher` は Akka actor だが、core には状態遷移ロジックのみを置き、actor 化と scheduling は adapter (Phase B) に委ねる。

#### Scenario: ActorRef を持たない

- **WHEN** `WatcherState` のフィールドを検査する
- **THEN** `ActorRef`・`Sender<T>`・`Receiver<T>` 等の actor / channel 型を持たない

#### Scenario: tokio / async-std の不在

- **WHEN** `modules/remote-core/src/watcher/` 配下のすべての import を検査する
- **THEN** `tokio`・`async_std`・`futures` 等の async runtime クレートへの参照が存在しない

#### Scenario: async fn の不在

- **WHEN** `modules/remote-core/src/watcher/` 配下のすべての関数定義を検査する
- **THEN** `async fn` が存在しない

### Requirement: WatcherCommand と WatcherEffect

`fraktor_remote_core_rs::domain::watcher::WatcherCommand` enum と `WatcherEffect` enum が定義され、`WatcherState` への入力 (Watch / Unwatch / Heartbeat 受信) と出力 (Heartbeat 送信指示 / Terminated 通知 / Quarantine 通知) を表現する SHALL。`now` を含むすべてのバリアントは **monotonic millis** (`u64` に comment/rustdoc で明示) で時刻を保持する。

#### Scenario: WatcherCommand の存在

- **WHEN** `modules/remote-core/src/watcher/watcher_command.rs` を読む
- **THEN** `pub enum WatcherCommand` が定義され、`Watch { target, watcher }`・`Unwatch { target, watcher }`・`HeartbeatReceived { from, now: u64 /* monotonic millis */ }`・`HeartbeatTick { now: u64 /* monotonic millis */ }` 等のバリアントを含む

#### Scenario: monotonic millis の明示

- **WHEN** `WatcherCommand` の `now` を含むバリアントの doc comment を読む
- **THEN** `now` が **monotonic millis** (wall clock ではない) であることが明示されている — `PhiAccrualFailureDetector` と同一時刻ソースを使う必要があるため

#### Scenario: WatcherEffect の存在

- **WHEN** `modules/remote-core/src/watcher/watcher_effect.rs` を読む
- **THEN** `pub enum WatcherEffect` が定義され、`SendHeartbeat { to }`・`NotifyTerminated { target, watchers }`・`NotifyQuarantined { node }` 等のバリアントを含む

### Requirement: 純関数としての handle メソッド

`WatcherState::handle` メソッド (または同等の状態遷移メソッド) は `&mut self` を取り、`WatcherCommand` を受けて `Vec<WatcherEffect>` を返す純関数として実装される SHALL。

#### Scenario: handle メソッドのシグネチャ

- **WHEN** `WatcherState::handle` の定義を読む
- **THEN** `fn handle(&mut self, command: WatcherCommand) -> Vec<WatcherEffect>` または同等のシグネチャが宣言されている

#### Scenario: 時刻入力の引数化

- **WHEN** `WatcherState::handle` に `WatcherCommand::HeartbeatTick` を渡す呼び出しを検査する
- **THEN** 時刻は command 内に含まれ、`WatcherState` 自身は `Instant::now()` を呼ばない

### Requirement: failure detector との連携

`WatcherState` は `PhiAccrualFailureDetector` を内部に保持し、heartbeat 受信時に `heartbeat(now)` を呼び、tick 時に `is_available(now)` で生存判定する SHALL。

#### Scenario: heartbeat 受信時の failure detector 更新

- **WHEN** `WatcherState::handle(WatcherCommand::HeartbeatReceived { from: node_a, now: 1000 })` を呼ぶ
- **THEN** `node_a` に対応する `PhiAccrualFailureDetector::heartbeat(1000)` が内部で呼ばれる

#### Scenario: tick 時の生存判定

- **WHEN** ある node が長時間 heartbeat を送ってこない状態で `WatcherState::handle(WatcherCommand::HeartbeatTick { now: 99999 })` を呼ぶ
- **THEN** 戻り値の effect 列に `WatcherEffect::NotifyTerminated { target: ..., watchers: ... }` または `WatcherEffect::NotifyQuarantined { node: ... }` が含まれる


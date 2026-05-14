## ADDED Requirements

### Requirement: WatcherState emits remote DeathWatch system-message effects

`WatcherState` は remote watch lifecycle を表す effect として、heartbeat だけでなく remote DeathWatch system message 送信指示を返す SHALL。effect は actor-core delivery や transport send を直接実行してはならない（MUST NOT）。

#### Scenario: watch emits remote watch effect

- **WHEN** `WatcherState::handle(WatcherCommand::Watch { target, watcher })` を呼ぶ
- **THEN** state は `(target, watcher)` を tracking する
- **AND** 戻り値には target へ remote `Watch` system message を送るための effect が含まれる

#### Scenario: unwatch emits remote unwatch effect

- **GIVEN** `WatcherState` が `(target, watcher)` を tracking している
- **WHEN** `WatcherState::handle(WatcherCommand::Unwatch { target, watcher })` を呼ぶ
- **THEN** state は `(target, watcher)` の tracking を解除する
- **AND** 戻り値には target へ remote `Unwatch` system message を送るための effect が含まれる

#### Scenario: rewatch contains target and watcher identity

- **GIVEN** `WatcherState` が remote node 上の `target` を `watcher` のために tracking している
- **WHEN** 同じ remote node から新しい actor-system UID を持つ heartbeat response を受信する
- **THEN** 戻り値の rewatch effect は `target` と `watcher` の両方の actor path を含む
- **AND** std adaptor はその情報だけで remote `Watch` system message を再発行できる

### Requirement: terminated effects stay idempotent

`WatcherState` は failure detector が同じ remote node を継続して unavailable と判定しても、同じ watch pair に対する `NotifyTerminated` を繰り返し返してはならない（MUST NOT）。heartbeat または heartbeat response を再受信した場合のみ、次回の unavailable 判定で再通知できる。

#### Scenario: repeated tick emits one termination notification

- **GIVEN** `WatcherState` が `(target, watcher)` を tracking している
- **AND** target の remote node が unavailable と判定される状態である
- **WHEN** `HeartbeatTick` を複数回処理する
- **THEN** `NotifyTerminated { target, watchers }` は最初の unavailable 判定でだけ返る

#### Scenario: heartbeat clears notified marker

- **GIVEN** `WatcherState` が remote node の unavailable 判定で `NotifyTerminated` を返した後である
- **WHEN** 同じ remote node から heartbeat または heartbeat response を受信する
- **THEN** state は通知済み marker を解除する
- **AND** 後続の unavailable 判定では新しい `NotifyTerminated` を返せる

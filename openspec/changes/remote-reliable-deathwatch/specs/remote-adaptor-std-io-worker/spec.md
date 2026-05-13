## ADDED Requirements

### Requirement: std watcher task applies WatcherEffect

std adaptor は `WatcherState` を所有して timer と command queue で駆動する watcher task を持つ SHALL。task は `WatcherEffect` を transport control、remote system message、actor-core DeathWatch delivery、event stream notification のいずれかへ変換する。

#### Scenario: heartbeat effect sends control pdu

- **WHEN** `WatcherState::handle` が `WatcherEffect::SendHeartbeat { to }` を返す
- **THEN** watcher task は `ControlPdu::Heartbeat` を対象 remote node へ送る
- **AND** 送信失敗は log または returned error path で観測できる

#### Scenario: watch effect enqueues system priority envelope

- **WHEN** `WatcherState::handle` が remote `Watch` system message の送信 effect を返す
- **THEN** watcher task は target actor path を recipient、watcher actor path を sender metadata とする system priority envelope を enqueue する
- **AND** envelope は ACK/NACK redelivery state の対象になる

#### Scenario: notify terminated sends actor-core system message

- **WHEN** `WatcherState::handle` が `NotifyTerminated { target, watchers }` を返す
- **THEN** watcher task は各 local watcher へ `SystemMessage::DeathWatchNotification(target_pid)` を送る
- **AND** target pid は remote actor path から local actor system 上の remote actor ref pid へ解決される

#### Scenario: quarantine notification is observable

- **WHEN** `WatcherState::handle` が `NotifyQuarantined { node }` を返す
- **THEN** watcher task は actor-core event stream または明示 error path に remote node quarantine を通知する
- **AND** notification を silent drop しない

### Requirement: inbound remote system message path rehydrates local pid

std inbound delivery bridge は remote DeathWatch 系 system message を actor-core へ渡す前に、envelope の actor path metadata から受信側 actor system の pid を解決する SHALL。wire 上の送信元 node local pid を actor-core にそのまま渡してはならない（MUST NOT）。

#### Scenario: inbound watch resolves remote watcher pid

- **GIVEN** remote node から recipient `target_path`、sender `watcher_path` を持つ `Watch` system envelope を受信した
- **WHEN** inbound delivery bridge が actor-core へ配送する
- **THEN** bridge は `watcher_path` を受信側の remote actor ref pid へ materialize または解決する
- **AND** `target_path` の local actor へ `SystemMessage::Watch(resolved_watcher_pid)` を送る

#### Scenario: inbound unwatch resolves remote watcher pid

- **GIVEN** remote node から recipient `target_path`、sender `watcher_path` を持つ `Unwatch` system envelope を受信した
- **WHEN** inbound delivery bridge が actor-core へ配送する
- **THEN** bridge は `watcher_path` を受信側の remote actor ref pid へ materialize または解決する
- **AND** `target_path` の local actor へ `SystemMessage::Unwatch(resolved_watcher_pid)` を送る

#### Scenario: inbound deathwatch notification resolves local watcher

- **GIVEN** remote node から recipient `watcher_path`、sender `target_path` を持つ `DeathWatchNotification` system envelope を受信した
- **WHEN** inbound delivery bridge が actor-core へ配送する
- **THEN** bridge は `watcher_path` を local actor pid へ解決する
- **AND** `target_path` を受信側の remote actor ref pid へ materialize または解決する
- **AND** local watcher へ `SystemMessage::DeathWatchNotification(resolved_target_pid)` を送る

### Requirement: retry driver uses core ACK/NACK effects

std retry driver は core association が返す ACK/NACK / resend effects を実行する SHALL。sequence state は std 側で二重に持ってはならない（MUST NOT）。

#### Scenario: resend effect sends retained system envelope

- **WHEN** core association が sequence number 付き system envelope の resend effect を返す
- **THEN** retry driver は同じ remote authority へ同じ system priority envelope を再送する
- **AND** retry driver は新しい sequence number を割り当てない

#### Scenario: ack pdu is routed into association

- **WHEN** TCP inbound dispatch が `AckPdu` を受信する
- **THEN** std run loop は `Remote::handle_remote_event` 経由で core association へ ACK を適用する
- **AND** ACK 後に返った resend / drop effects を retry driver が実行する

#### Scenario: retry timer is monotonic

- **WHEN** retry driver が pending system envelope の resend timeout を判定する
- **THEN** driver は monotonic millis を core に渡す
- **AND** wall clock に依存しない

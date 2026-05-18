## ADDED Requirements

### Requirement: WatcherState は address termination effect を emit する

`WatcherState` は remote node が unavailable と判定されたとき、remote actor ごとの `NotifyTerminated` に加えて、node-level address termination を表す `WatcherEffect` を返す SHALL。effect は unavailable remote node の address、reason metadata、`HeartbeatTick` の `now` に由来する monotonic millis timestamp を含む MUST。effect は actor-core event stream への publish を直接実行してはならず（MUST NOT）、std adaptor が適用する副作用指示に留める MUST。

#### Scenario: unavailable node は address termination effect を emit する

- **GIVEN** `WatcherState` が remote node 上の target / watcher pair を tracking している
- **AND** failure detector がその node を unavailable と判定する状態である
- **WHEN** `WatcherState::handle(WatcherCommand::HeartbeatTick { now })` を呼ぶ
- **THEN** 戻り値の effect 列に address termination effect が含まれる
- **AND** effect は unavailable と判定された remote node の address を含む
- **AND** effect は reason metadata と `now` と同じ monotonic millis timestamp を含む

#### Scenario: address termination effect は terminated notifications と一緒に emit される

- **GIVEN** `WatcherState` が remote node 上の複数 target を tracking している
- **AND** failure detector がその node を unavailable と判定する状態である
- **WHEN** heartbeat tick を処理する
- **THEN** 戻り値には対象 actor の watcher へ通知する `NotifyTerminated` effects が含まれる
- **AND** 同じ node に対する address termination effect も含まれる

### Requirement: address termination effect は idempotent に保たれる

`WatcherState` は failure detector が同じ remote node を継続して unavailable と判定しても、同じ failure epoch に対する address termination effect を繰り返し返してはならない（MUST NOT）。heartbeat または heartbeat response を再受信した場合のみ、次回の unavailable 判定で新しい address termination effect を返せる SHALL。

#### Scenario: repeated tick は address termination effect を一度だけ emit する

- **GIVEN** `WatcherState` が remote node 上の target を tracking している
- **AND** target の remote node が unavailable と判定される状態である
- **WHEN** `HeartbeatTick` を複数回処理する
- **THEN** address termination effect は最初の unavailable 判定でだけ返る

#### Scenario: heartbeat は address termination marker を解除する

- **GIVEN** `WatcherState` が remote node の unavailable 判定で address termination effect を返した後である
- **WHEN** 同じ remote node から heartbeat または heartbeat response を受信する
- **THEN** state は address termination 通知済み marker を解除する
- **AND** 後続の unavailable 判定では新しい address termination effect を返せる

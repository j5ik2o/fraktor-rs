## MODIFIED Requirements

### Requirement: remote watch hook forwards DeathWatchNotification

remote watch hook は actor-core から渡された remote watcher pid と terminated target pid を actor path へ解決し、remote-bound `DeathWatchNotification` を system priority envelope として enqueue する前に std flush gate へ渡す SHALL。remote watcher pid を解決できない場合、hook は notification を消費してはならない（MUST NOT）。

remote watcher pid は解決できたが terminated target pid を local actor path へ解決できない場合、hook は notification を消費する SHALL。ただし invalid actor path metadata を持つ notification を enqueue してはならず（MUST NOT）、解決失敗を log または test-observable error path で観測可能にする MUST。

#### Scenario: remote notification is gated before enqueue

- **GIVEN** watcher pid が provider の remote pid/path registry に存在する
- **AND** terminated target pid が local actor path として解決できる
- **WHEN** actor-core が `SystemMessage::DeathWatchNotification(target)` を watcher pid へ送る
- **THEN** remote watch hook は recipient を watcher path、sender metadata を target path とする pending notification を std flush gate へ渡す
- **AND** hook は同じ call stack で `RemoteEvent::OutboundEnqueued` を送らない
- **AND** hook は `true` を返す

#### Scenario: flush outcome enqueues remote notification

- **GIVEN** remote watch hook が std flush gate に pending notification を渡している
- **WHEN** flush gate が matching `BeforeDeathWatchNotification` flush completed / timed-out / failed outcome を観測する
- **THEN** flush gate は pending notification を `RemoteEvent::OutboundEnqueued` の system priority envelope として enqueue する
- **AND** notification は一度だけ enqueue される

#### Scenario: unresolved remote watcher does not consume notification

- **WHEN** remote watch hook が watcher pid を remote actor path へ解決できない
- **THEN** hook は `false` を返す
- **AND** notification を std flush gate または remote outbound lane へ渡さない

#### Scenario: unresolved local target does not send invalid notification

- **GIVEN** watcher pid が provider の remote pid/path registry に存在する
- **WHEN** remote watch hook が terminated target pid を local actor path へ解決できない
- **THEN** hook は `true` を返す
- **AND** invalid actor path metadata を持つ notification を std flush gate または remote outbound lane へ渡さない
- **AND** failure は log または test-observable error path で観測できる

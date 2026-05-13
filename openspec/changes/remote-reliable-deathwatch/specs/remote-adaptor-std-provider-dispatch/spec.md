## ADDED Requirements

### Requirement: provider registers remote watch hook

std remote actor-ref provider installer は actor-core に remote watch hook を登録する SHALL。hook は provider が materialize した remote actor ref の pid/path mapping を使い、remote-bound watch/unwatch/notification を std watcher task または remote outbound lane へ渡す。

#### Scenario: installer registers hook

- **WHEN** `StdRemoteActorRefProviderInstaller::install` が remote-aware provider を actor system に登録する
- **THEN** installer は同じ actor system に remote watch hook を登録する
- **AND** hook は provider が保持する remote pid/path mapping を参照できる

#### Scenario: remote actor ref materialization records mapping

- **WHEN** `StdRemoteActorRefProvider::actor_ref(remote_path)` が remote actor ref を materialize する
- **THEN** provider は生成した synthetic remote pid と `remote_path` の対応を registry に記録する
- **AND** registry は remote watch hook から参照できる

#### Scenario: local actor ref is not recorded as remote mapping

- **WHEN** `StdRemoteActorRefProvider::actor_ref(local_path)` が local provider に委譲される
- **THEN** provider は local actor の pid/path を remote pid/path registry に記録しない

### Requirement: remote watch hook forwards watch commands

remote watch hook は actor-core から渡された target pid と watcher pid を actor path へ解決し、remote target に対する watch/unwatch command を std watcher task へ渡す SHALL。解決できない場合は hook が `false` を返し、actor-core の既存 fallback を維持する MUST。

#### Scenario: remote watch is forwarded

- **GIVEN** target pid が provider の remote pid/path registry に存在する
- **AND** watcher pid が local actor path として解決できる
- **WHEN** actor-core が `SystemMessage::Watch(watcher)` を target pid へ送る
- **THEN** remote watch hook は `WatcherCommand::Watch { target, watcher }` 相当を std watcher task へ渡す
- **AND** hook は `true` を返す

#### Scenario: remote unwatch is forwarded

- **GIVEN** target pid が provider の remote pid/path registry に存在する
- **AND** watcher pid が local actor path として解決できる
- **WHEN** actor-core が `SystemMessage::Unwatch(watcher)` を target pid へ送る
- **THEN** remote watch hook は `WatcherCommand::Unwatch { target, watcher }` 相当を std watcher task へ渡す
- **AND** hook は `true` を返す

#### Scenario: unresolved mapping does not consume

- **WHEN** remote watch hook が target pid または watcher pid を actor path へ解決できない
- **THEN** hook は `false` を返す
- **AND** actor-core は既存 fallback を実行できる

### Requirement: remote watch hook forwards DeathWatchNotification

remote watch hook は actor-core から渡された remote watcher pid と terminated target pid を actor path へ解決し、remote-bound `DeathWatchNotification` を system priority envelope として enqueue する SHALL。remote watcher pid を解決できない場合、hook は notification を消費してはならない（MUST NOT）。

#### Scenario: remote notification is forwarded

- **GIVEN** watcher pid が provider の remote pid/path registry に存在する
- **AND** terminated target pid が local actor path として解決できる
- **WHEN** actor-core が `SystemMessage::DeathWatchNotification(target)` を watcher pid へ送る
- **THEN** remote watch hook は recipient を watcher path、sender metadata を target path とする remote-bound notification を enqueue する
- **AND** hook は `true` を返す

#### Scenario: unresolved remote watcher does not consume notification

- **WHEN** remote watch hook が watcher pid を remote actor path へ解決できない
- **THEN** hook は `false` を返す
- **AND** notification を remote outbound lane へ enqueue しない

#### Scenario: unresolved local target does not send invalid notification

- **GIVEN** watcher pid が provider の remote pid/path registry に存在する
- **WHEN** remote watch hook が terminated target pid を local actor path へ解決できない
- **THEN** hook は invalid actor path metadata を持つ notification を enqueue しない
- **AND** failure は log または test-observable error path で観測できる

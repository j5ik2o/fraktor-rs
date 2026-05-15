# actor-core-remote-deathwatch Specification

## Purpose
TBD - created by archiving change remote-reliable-deathwatch. Update Purpose after archive.
## Requirements
### Requirement: remote-bound system message hook

actor-core は remote adaptor が remote-bound `SystemMessage::Watch`、`SystemMessage::Unwatch`、`SystemMessage::DeathWatchNotification` を消費できる hook を提供する SHALL。hook が `true` を返した場合、actor-core は missing local cell fallback を実行してはならない（MUST NOT）。

#### Scenario: remote watch hook consumes watch

- **WHEN** actor-core が local cell を持たない target pid へ `SystemMessage::Watch(watcher)` を送る
- **AND** installed remote hook がその target pid を remote actor として認識して `true` を返す
- **THEN** actor-core は watcher へ即時 `DeathWatchNotification` を送らない
- **AND** `send_system_message` は成功として返る

#### Scenario: remote hook does not consume local fallback

- **WHEN** actor-core が local cell を持たない target pid へ `SystemMessage::Watch(watcher)` を送る
- **AND** installed remote hook が `false` を返す
- **THEN** actor-core は既存 fallback として watcher へ `DeathWatchNotification(target)` を送る

#### Scenario: remote notification hook consumes deathwatch notification

- **WHEN** actor-core が local cell を持たない watcher pid へ `SystemMessage::DeathWatchNotification(target)` を送る
- **AND** installed remote hook がその watcher pid を remote actor として認識して `true` を返す
- **THEN** actor-core は notification を silent drop しない
- **AND** remote adaptor へ remote-bound notification が渡される

### Requirement: inbound remote DeathWatch notification uses local actor-core path

remote adaptor から inbound `DeathWatchNotification` を受けた actor-core は、local watcher pid に通常の `SystemMessage::DeathWatchNotification(target)` を送る SHALL。watcher actor の `watching` 状態と `terminated_queued` dedup は既存 actor-core の規則を使う MUST。

#### Scenario: inbound notification reaches watcher

- **GIVEN** local actor `watcher` が remote actor `target` を watch 済みである
- **WHEN** remote adaptor が `target` の終了を actor-core へ通知する
- **THEN** actor-core は `watcher` に `SystemMessage::DeathWatchNotification(target_pid)` を送る
- **AND** `watcher` は既存 DeathWatch 経路で termination を観測する

#### Scenario: duplicate notification is deduplicated

- **GIVEN** local actor `watcher` が remote actor `target` を watch 済みである
- **WHEN** remote adaptor が同じ `target` の終了通知を複数回 actor-core へ渡す
- **THEN** actor-core は既存 `terminated_queued` / watching dedup により user-visible termination を重複配送しない

#### Scenario: unwatch suppresses stale notification

- **GIVEN** local actor `watcher` が remote actor `target` を unwatch 済みである
- **WHEN** remote adaptor が古い `target` の終了通知を actor-core へ渡す
- **THEN** actor-core は `watcher` の user handler へ termination を配送しない


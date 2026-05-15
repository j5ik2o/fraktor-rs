## ADDED Requirements

### Requirement: address termination is separate from actor DeathWatch

actor-core は address termination event を remote node-level failure signal として扱い、actor-level `SystemMessage::DeathWatchNotification` と混同してはならない（MUST NOT）。remote adaptor が remote node failure を観測した場合、local watcher への DeathWatch notification は既存 dedup / unwatch suppression を通り、address termination は event stream へ publish される SHALL。

#### Scenario: address termination does not bypass DeathWatch dedup

- **GIVEN** local actor `watcher` が remote actor `target` を watch 済みである
- **AND** remote adaptor が `target` の remote node failure を観測する
- **WHEN** actor-core が inbound remote DeathWatch notification を処理する
- **THEN** `watcher` の existing watching state と `terminated_queued` dedup が適用される
- **AND** address termination event は user actor への `DeathWatchNotification` を直接生成しない

#### Scenario: unwatch suppresses actor notification but not address event

- **GIVEN** local actor `watcher` が remote actor `target` を unwatch 済みである
- **WHEN** `target` の remote node に対する address termination event が publish される
- **THEN** `watcher` の user handler へ stale termination は配送されない
- **AND** event stream subscriber は node-level address termination event を受信できる

### Requirement: actor-core exposes address termination subchannel

actor-core event stream は address termination event 用の concrete `ClassifierKey` を提供する SHALL。subscriber が address termination key で購読した場合、address termination events だけを受信し、他の remoting lifecycle / authority / DeathWatch 関連 event を受信してはならない（MUST NOT）。

#### Scenario: address termination subscriber receives only address events

- **GIVEN** subscriber が address termination classifier key で event stream を購読している
- **WHEN** address termination、remoting lifecycle、remote authority event が publish される
- **THEN** subscriber は address termination event だけを受信する

#### Scenario: all subscriber receives address termination

- **GIVEN** subscriber が `ClassifierKey::All` で event stream を購読している
- **WHEN** address termination event が publish される
- **THEN** subscriber はその event を受信する

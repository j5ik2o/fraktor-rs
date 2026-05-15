## ADDED Requirements

### Requirement: remote watcher publishes address termination

std remote watcher は remote node が failure detector により unavailable と判定されたとき、actor-core event stream へ address termination event を publish する SHALL。event は actor-core owned の authority string、termination reason、monotonic millis observation timestamp を含み、actor-level `DeathWatchNotification` とは別の node-level signal として扱われる MUST。

#### Scenario: remote node failure publishes address termination

- **GIVEN** local actor が remote node 上の actor を watch 済みである
- **AND** remote watcher failure detector がその node を unavailable と判定する
- **WHEN** std watcher task が watcher effects を適用する
- **THEN** actor-core event stream に address termination event が publish される
- **AND** event の authority は unavailable と判定された remote node を指す
- **AND** event の reason は watcher effect の reason metadata から構築される
- **AND** event の monotonic millis observation timestamp は watcher effect の timestamp と一致する

#### Scenario: actor termination notification remains separate

- **GIVEN** remote node failure により address termination event が publish される
- **WHEN** 同じ failure 判定が watched remote actor に対応している
- **THEN** local watcher には既存 DeathWatch 経路で `DeathWatchNotification` が配送される
- **AND** address termination event は `DeathWatchNotification` の代替として扱われない

### Requirement: address termination publication is idempotent per failure epoch

std remote watcher は同じ remote node が unavailable と判定され続ける間、address termination event を重複 publish してはならない（MUST NOT）。同じ node から heartbeat または heartbeat response を再受信した後に再び unavailable と判定された場合は、新しい failure epoch として再 publish できる SHALL。

#### Scenario: repeated heartbeat ticks publish once

- **GIVEN** remote watcher が remote node を watch している
- **AND** failure detector がその node を unavailable と判定する
- **WHEN** heartbeat tick が複数回処理される
- **THEN** address termination event は最初の unavailable 判定で一度だけ publish される

#### Scenario: heartbeat recovery allows later publication

- **GIVEN** remote watcher が remote node の unavailable 判定で address termination event を publish 済みである
- **WHEN** 同じ node から heartbeat または heartbeat response を再受信する
- **AND** その後に同じ node が再び unavailable と判定される
- **THEN** 新しい address termination event が publish される

### Requirement: remote deployment reacts to address termination

remote deployment watcher / daemon は address termination event を購読し、該当 remote authority に紐づく pending deployment request、remote-created child tracking、late response state を cleanup する SHALL。cleanup は watcher task から deployment module を直接呼び出すのではなく、actor-core event stream の `AddressTerminated` subchannel 経由で接続される MUST。deployment cleanup は address termination event の monotonic millis observation timestamp を使い、pending request の開始時刻より古い replayed event で新しい pending request を失敗させてはならない（MUST NOT）。

#### Scenario: pending deployment fails on address termination

- **GIVEN** origin node が remote deployment create request の response を待っている
- **WHEN** target remote authority の address termination event が publish される
- **THEN** pending deployment は remote address termination として失敗する
- **AND** caller は timeout ではなく address termination に由来する failure を観測できる

#### Scenario: stale deployment response is rejected after cleanup

- **GIVEN** address termination event により pending deployment state が cleanup 済みである
- **WHEN** 同じ correlation id の late create success または create failure response が到着する
- **THEN** response dispatcher は late response を stale response として扱う
- **AND** cleanup 済み deployment を成功に戻してはならない（MUST NOT）

#### Scenario: replayed old termination does not fail new deployment

- **GIVEN** address termination event が event stream buffer に残っている
- **AND** pending deployment request がその event の monotonic millis observation timestamp より後に開始された
- **WHEN** deployment watcher / daemon が buffered address termination event を受信する
- **THEN** その pending deployment request は address termination failure として扱われない

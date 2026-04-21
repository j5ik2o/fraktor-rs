# pekko-eventstream-subchannel Specification

## Purpose
TBD - created by archiving change 2026-04-20-pekko-eventstream-subchannel. Update Purpose after archive.
## Requirements
### Requirement: ClassifierKey は EventStreamEvent の全 variant に対応しなければならない

`ClassifierKey` enum は `EventStreamEvent` の全 14 variant に対応する具象 key と、全 variant を束ねる `All` を提供しなければならない（MUST）。`for_event(&EventStreamEvent)` は具象 variant のみ返し、`All` を返してはならない。

#### Scenario: 14 variant が全て具象 key を持つ

- **WHEN** `ClassifierKey::for_event(&event)` を各 `EventStreamEvent` variant で呼び出す
- **THEN** `Lifecycle / Log / DeadLetter / Extension / Mailbox / MailboxPressure / UnhandledMessage / AdapterFailure / Serialization / RemoteAuthority / RemotingBackpressure / RemotingLifecycle / SchedulerTick / TickDriver` のいずれかが返る
- **AND** `All` は返らない

#### Scenario: 主要 4 variant の対応関係

- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::Lifecycle(_))` を呼ぶ
- **THEN** `ClassifierKey::Lifecycle` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::Log(_))` を呼ぶ
- **THEN** `ClassifierKey::Log` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::DeadLetter(_))` を呼ぶ
- **THEN** `ClassifierKey::DeadLetter` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::Extension { .. })` を呼ぶ
- **THEN** `ClassifierKey::Extension` が返る

### Requirement: subscribe_with_key は key にマッチする event のみを配送しなければならない

`EventStreamShared::subscribe_with_key(key, subscriber)` で購読した subscriber は、`publish` された event の `ClassifierKey::for_event(event)` が `key` と一致する場合、もしくは `key == ClassifierKey::All` の場合にのみ配送を受けなければならない（MUST）。**replay（新規購読時に過去の buffered event を通知する挙動）も同じフィルタを適用する** — key にマッチしない buffered event を replay してはならない（MUST）。

#### Scenario: 具象 key は同一 variant のみ受信

- **GIVEN** `subscribe_with_key(ClassifierKey::Log, &subscriber)` で購読
- **WHEN** `Lifecycle` / `Log` / `DeadLetter` の 3 variant を順に publish する
- **THEN** subscriber は `Log` variant 1 件のみ受信する

#### Scenario: ClassifierKey::All は全 variant を受信

- **GIVEN** `subscribe_with_key(ClassifierKey::All, &subscriber)` で購読
- **WHEN** 異なる variant (例: `Log`, `Lifecycle`, `Extension`) を 1 件ずつ publish する
- **THEN** subscriber は 3 件全てを受信する

#### Scenario: 複数購読者の fan-out は独立する

- **GIVEN** `Lifecycle` / `Log` / `All` の 3 購読者が登録されている
- **WHEN** `Lifecycle` / `Log` / `DeadLetter` をそれぞれ publish する
- **THEN** `Lifecycle` 購読者は `Lifecycle` 1 件、`Log` 購読者は `Log` 1 件、`All` 購読者は 3 件を受信する

#### Scenario: replay は key にマッチする buffered event のみを返す

- **GIVEN** `EventStreamShared` に Log 1 件と Lifecycle 1 件が buffered 済み
- **WHEN** 新規 subscriber が `subscribe_with_key(ClassifierKey::Log, subscriber)` で登録される
- **THEN** subscriber が replay として受け取る snapshot は Log 1 件のみ
- **AND** Lifecycle event は replay されない

#### Scenario: ClassifierKey::All の replay は全 buffered event を返す

- **GIVEN** `EventStreamShared` に Log / Lifecycle / DeadLetter がそれぞれ 1 件 buffered 済み
- **WHEN** 新規 subscriber が `subscribe_with_key(ClassifierKey::All, subscriber)` で登録される
- **THEN** subscriber が replay として受け取る snapshot は 3 件全て

### Requirement: 既存 subscribe は ClassifierKey::All 相当として互換性を保持しなければならない

`EventStreamShared::subscribe(subscriber)` は `subscribe_with_key(ClassifierKey::All, subscriber)` の **糖衣構文** として定義されなければならない（MUST）。これは key 指定を省略した通常購読のための API ergonomics であり、後方互換性の保持を目的としない。

#### Scenario: subscribe は subscribe_with_key(All, ...) と同じ結果になる

- **GIVEN** `EventStreamShared::subscribe(&subscriber)` で購読
- **WHEN** 任意の `EventStreamEvent` を publish する
- **THEN** subscriber は `subscribe_with_key(ClassifierKey::All, &subscriber)` で登録した場合と完全に同じ挙動で全 event を受信する
- **AND** 内部実装は 2 つの API を同一コードパスで処理する（別実装による挙動乖離を防ぐ）

### Requirement: EventStream の publish_prepare は subchannel 対応でなければならない

`EventStream::publish_prepare(event)` は `ClassifierKey::for_event(event)` に基づいて配送対象購読者を絞り込んだ snapshot を返さなければならない（MUST）。`snapshot_for(key)` は `key == ClassifierKey::All` または `subscriber.key == key` または `subscriber.key == ClassifierKey::All` の購読者を含む。

#### Scenario: publish_prepare は subchannel 別に絞り込む

- **GIVEN** 具象 key `Log` で登録された subscriber A と `ClassifierKey::All` で登録された subscriber B
- **WHEN** `EventStream::publish_prepare(&EventStreamEvent::Log(..))` を呼ぶ
- **THEN** 返る snapshot に A と B が含まれる
- **AND** 別 variant (`EventStreamEvent::Lifecycle(..)`) の publish では B のみが含まれる

#### Scenario: subscribe_with_key は id と replay snapshot を返す

- **WHEN** `EventStream::subscribe_with_key(key, subscriber)` を呼ぶ
- **THEN** 戻り値は `(u64, Vec<EventStreamEvent>)` で、既存の `subscribe` と同じく subscriber id と buffered event snapshot を返す

#### Scenario: subscribe_no_replay は ClassifierKey::All 相当

- **WHEN** `EventStream::subscribe_no_replay(subscriber)` を呼ぶ
- **THEN** subscriber は `ClassifierKey::All` として登録される（本 change では `subscribe_no_replay_with_key` 等の key 指定版は追加しない）


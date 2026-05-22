# pekko-eventstream-subchannel Specification

## Purpose
TBD - created by archiving change 2026-04-20-pekko-eventstream-subchannel. Update Purpose after archive.
## Requirements
### Requirement: ClassifierKey は EventStreamEvent の全 variant に対応しなければならない

`ClassifierKey` enum は `EventStreamEvent` の全 15 variant に対応する具象 key と、全 variant を束ねる `All` を提供しなければならない（MUST）。`for_event(&EventStreamEvent)` は具象 variant のみ返し、`All` を返してはならない。

#### Scenario: 15 variant が全て具象 key を持つ

- **WHEN** `ClassifierKey::for_event(&event)` を各 `EventStreamEvent` variant で呼び出す
- **THEN** `Lifecycle / Log / DeadLetter / Extension / Mailbox / MailboxPressure / UnhandledMessage / AdapterFailure / Serialization / RemoteAuthority / RemotingBackpressure / RemotingLifecycle / AddressTerminated / SchedulerTick / TickDriver` のいずれかが返る
- **AND** `All` は返らない

#### Scenario: 主要 5 variant の対応関係

- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::Lifecycle(_))` を呼ぶ
- **THEN** `ClassifierKey::Lifecycle` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::Log(_))` を呼ぶ
- **THEN** `ClassifierKey::Log` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::DeadLetter(_))` を呼ぶ
- **THEN** `ClassifierKey::DeadLetter` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::AddressTerminated(_))` を呼ぶ
- **THEN** `ClassifierKey::AddressTerminated` が返る
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

### Requirement: subscribe replay は return 前に同期通知されなければならない

`EventStreamShared::subscribe_with_key(key, subscriber)` は、登録時点で key にマッチする buffered events の replay snapshot を確定し、`EventStreamSubscription` を返す前に対象 subscriber へ同期通知しなければならない（MUST）。この契約は subscribe 呼び出しと同じ thread の happens-before に限定され、subscribe 実行中に別 thread から実行される `publish()` との厳密な replay/live 順序は保証してはならない（MUST NOT）。

`subscribe_no_replay(subscriber)` は replay snapshot を持たない live-only registration であり、この replay 同期通知契約の対象外である。ただし、`EventStreamSubscription` を返す前に live registration は完了していなければならない（MUST）。buffered event の同期観測を必要とする caller は `subscribe_with_key` または `subscribe` を使用する。

#### Scenario: subscribe return 後は replay が観測済み

- **GIVEN** `EventStreamShared` に Log event が buffered 済みである
- **WHEN** subscriber が `subscribe_with_key(ClassifierKey::Log, subscriber)` で登録され、その呼び出しが return する
- **THEN** subscriber は buffered Log event を既に受信している

#### Scenario: concurrent publish との replay/live 順序は未規定

- **GIVEN** subscriber が `subscribe_with_key` で登録中である
- **WHEN** 別 thread が同じ subscriber に match する live event を `publish()` する
- **THEN** implementation は replay callback と live callback の厳密な `buffered -> live` 順序を保証しない

### Requirement: publish は callback 完了を同期観測できなければならない

`EventStreamShared::publish(event)` は、panic が発生しない限り、配送対象 snapshot に含まれる subscriber callback を同期実行し、すべての対象 callback が完了してから return しなければならない（MUST）。配送対象 snapshot は `publish()` の event stream lock 区間で確定され、callback 実行中の subscribe / unsubscribe によって当該 publish の対象集合を変更してはならない（MUST NOT）。

#### Scenario: publish return 後は callback が完了済み

- **GIVEN** subscriber が `ClassifierKey::Log` で購読済みである
- **WHEN** caller が Log event を `publish()` し、その呼び出しが return する
- **THEN** subscriber callback はその Log event を観測済みである

#### Scenario: callback 中の購読変更は当該 publish の snapshot を変更しない

- **GIVEN** publish target snapshot が確定済みである
- **WHEN** subscriber callback が event stream の subscribe または unsubscribe を実行する
- **THEN** その変更は進行中の publish の配送対象集合を変更しない

### Requirement: publish 中の subscriber panic は subscription lifecycle を自動変更してはならない

`EventStreamShared::publish(event)` は subscriber callback panic を catch / isolate してはならない（MUST NOT）。panic は `publish()` 呼び出し元へ伝播しなければならず（MUST）、panic した subscriber の subscription は自動解除してはならない（MUST NOT）。panic 発生後に、panic した subscriber より後続の subscriber へ同じ event が配送されることは保証しない。

#### Scenario: publish 中の subscriber panic は呼び出し元へ伝播する

- **GIVEN** subscriber callback が Log event で panic する
- **WHEN** caller が Log event を `publish()` する
- **THEN** panic は `publish()` の呼び出し元へ伝播する

#### Scenario: panic した subscriber は自動解除されない

- **GIVEN** subscriber callback が最初の Log event で panic する
- **AND** caller がその panic を捕捉して処理を継続する
- **WHEN** caller が次の Log event を `publish()` する
- **THEN** 同じ subscriber は明示的に unsubscribe されていない限り次の Log event も受信する

#### Scenario: panic 後の後続 subscriber 配送は保証されない

- **GIVEN** panic する subscriber と別の subscriber が同じ key で購読済みである
- **WHEN** panic する subscriber の callback が `publish()` 中に panic する
- **THEN** implementation は同じ event を後続 subscriber へ配送することを保証しない


# mailbox-prepend-deque-contract Specification

## Purpose
TBD - created by archiving change mailbox-prepend-requires-deque. Update Purpose after archive.
## Requirements
### Requirement: Mailbox prepend SHALL require caller-resolved deque capability

generic な mailbox prepend 呼び出しは deque-capable queue を事前に解決した caller だけが使える形で提供されなければならない（MUST）。non-deque queue に対する drain-and-requeue fallback を実行してはならない（MUST NOT）。

#### Scenario: Caller cannot use prepend without deque capability
- **GIVEN** mailbox の user queue が deque-capable ではない
- **WHEN** internal caller が prepend を行おうとする
- **THEN** caller は prepend API 呼び出し前に non-deque を検知する
- **AND** drain-and-requeue fallback は使われない

#### Scenario: Empty prepend remains a no-op
- **GIVEN** mailbox に prepend する message batch が空である
- **WHEN** deque 専用 prepend API を呼び出す
- **THEN** result は `Ok(())` である
- **AND** deque capability の有無に依存して failure しない

#### Scenario: Deque prepend preserves front insertion ordering
- **GIVEN** mailbox の user queue が deque-capable である
- **WHEN** caller が deque 専用 prepend API で複数 message を prepend する
- **THEN** batch の先頭 message が mailbox の先頭に観測される
- **AND** 既存 pending message より前に replay される

### Requirement: Drain-and-requeue fallback SHALL not remain on production path

actor-core の production path は `prepend_via_drain_and_requeue(...)` に依存してはならない（MUST NOT）。Phase III 完了後、`ActorCell::unstash_*` を含む production caller はすべて deque contract を満たすか、prepend 前に失敗しなければならない（MUST）。

#### Scenario: ActorCell unstash never reaches fallback
- **GIVEN** `ActorCell::unstash_message`、`unstash_messages`、`unstash_messages_with_limit` が production caller である
- **WHEN** actor が stash replay を行う
- **THEN** deque-capable mailbox では prepend が成功する
- **AND** non-deque mailbox では prepend 前に deterministic failure となる

### Requirement: Caller inventory SHALL remain deque-safe

current production caller は deque-capable queue を前提に prepend を使うか、prepend 前に失敗しなければならない（MUST）。新しい production caller が追加される場合も同じ contract を満たさなければならない（MUST）。

注: この requirement は test だけでなく型システムでも構造的に強制される。新しい deque 専用 prepend API は `Mailbox::user_deque()` または同等の peek accessor を通じた事前解決を前提とし、deque capability を解決しない caller は compile できない。

#### Scenario: ActorCell caller stays deque-safe
- **GIVEN** `ActorCell::unstash_message`、`unstash_messages`、`unstash_messages_with_limit` が production caller である
- **WHEN** caller inventory を確認する
- **THEN** それらの caller は deque-capable queue を前提に prepend を使う
- **AND** non-deque queue では prepend 前に失敗する


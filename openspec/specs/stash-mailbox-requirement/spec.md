# stash-mailbox-requirement Specification

## Purpose
TBD - created by archiving change stash-requires-deque-mailbox. Update Purpose after archive.
## Requirements
### Requirement: Stash-capable actor props SHALL declare deque mailbox requirement explicitly

stash を利用する actor を構成する API は、deque-capable mailbox requirement を明示できなければならない（MUST）。typed/untyped の両方で stash 用 mailbox convenience API を提供し、その API は `MailboxRequirement::for_stash()` と同等の requirement を適用しなければならない（MUST）。

#### Scenario: Untyped stash mailbox helper applies deque requirement
- **GIVEN** caller が untyped actor 用の `Props` を構成している
- **WHEN** caller が `Props::with_stash_mailbox()` を適用する
- **THEN** resulting `Props` は `MailboxRequirement::for_stash()` と同等の mailbox requirement を持つ

#### Scenario: Typed stash mailbox helper applies deque requirement
- **GIVEN** caller が typed actor 用の `TypedProps` を構成している
- **WHEN** caller が `TypedProps::with_stash_mailbox()` を適用する
- **THEN** underlying untyped `Props` は `MailboxRequirement::for_stash()` と同等の mailbox requirement を持つ

### Requirement: Stash SHALL fail deterministically when mailbox is not deque-capable

`ActorCell::stash_message_with_limit` は、mailbox が deque-capable prepend を提供できない場合に stash を受け付けてはならない（MUST NOT）。この場合は deterministic な recoverable error を返し、新しい stashed message を追加してはならない（MUST NOT）。

#### Scenario: stash rejects non-deque mailbox before buffering
- **GIVEN** actor が deque-capable mailbox requirement を持たないまま実行されている
- **WHEN** actor が `stash_message_with_limit` を実行する
- **THEN** result は recoverable error になる
- **AND** 新しい message は stash buffer に追加されない

### Requirement: Unstash SHALL fail deterministically when mailbox is not deque-capable

`ActorCell::unstash_message`、`unstash_messages`、`unstash_messages_with_limit` は、mailbox が deque-capable prepend を提供できない場合に silent fallback してはならない（MUST NOT）。この場合は deterministic な recoverable error を返し、既存の stashed messages を失ってはならない（MUST NOT）。

#### Scenario: unstash_message rejects non-deque mailbox
- **GIVEN** actor が既に stashed message を保持している
- **AND** actor は deque-capable mailbox requirement を持たないまま実行されている
- **WHEN** actor が `unstash_message` を実行する
- **THEN** result は recoverable error になる
- **AND** unstash 対象 message は stashed state に残る

#### Scenario: unstash_messages rejects non-deque mailbox
- **GIVEN** actor が複数の stashed messages を保持している
- **AND** actor は deque-capable mailbox requirement を持たないまま実行されている
- **WHEN** actor が `unstash_messages` を実行する
- **THEN** result は recoverable error になる
- **AND** stashed messages は失われない

#### Scenario: unstash_messages_with_limit rejects non-deque mailbox
- **GIVEN** actor が複数の stashed messages を保持している
- **AND** actor は deque-capable mailbox requirement を持たないまま実行されている
- **WHEN** actor が `unstash_messages_with_limit` を実行する
- **THEN** result は recoverable error になる
- **AND** wrap 前の stash 順序を保ったまま messages は復元される

### Requirement: Supported stash path SHALL preserve prepend ordering semantics

deque mailbox requirement を満たした stash actor では、unstash により replay された messages は既に mailbox に積まれている pending messages より前に処理されなければならない（MUST）。

#### Scenario: Typed unstash replays before queued messages
- **GIVEN** typed actor が `TypedProps::with_stash_mailbox()` を使って spawn されている
- **WHEN** actor が message を stash した後に mailbox 上へ別 message を enqueue してから unstash する
- **THEN** unstashed message は既存 queued message より前に観測される

#### Scenario: Classic unstash replays before queued messages
- **GIVEN** classic actor が `Props::with_stash_mailbox()` を使って spawn されている
- **WHEN** actor が message を stash した後に mailbox 上へ別 message を enqueue してから unstash する
- **THEN** unstashed message は既存 queued message より前に観測される

#### Scenario: Empty unstash remains a no-op even without deque capability
- **GIVEN** actor は stashed messages を保持していない
- **AND** actor は deque-capable mailbox requirement を持たないまま実行されている
- **WHEN** actor が `unstash_message` または `unstash_messages` を実行する
- **THEN** result は `Ok(0)` である
- **AND** stash contract violation error は返らない

### Requirement: `bounded + stash` SHALL fail deterministically

`bounded` mailbox policy と stash mailbox requirement の組み合わせは、silent fallback してはならない（MUST NOT）。Phase III では bounded deque mailbox を導入しないため、この構成は deterministic な configuration failure を返さなければならない（MUST）。

#### Scenario: Bounded typed stash support is rejected
- **GIVEN** caller が bounded mailbox policy を選んでいる
- **WHEN** caller が `TypedProps::with_stash_mailbox()` を組み合わせて actor を構成する
- **THEN** spawn または mailbox configuration validation は deterministic に失敗する

#### Scenario: Bounded untyped stash support is rejected
- **GIVEN** caller が bounded mailbox policy を選んでいる
- **WHEN** caller が `Props::with_stash_mailbox()` を組み合わせて actor を構成する
- **THEN** spawn または mailbox configuration validation は deterministic に失敗する


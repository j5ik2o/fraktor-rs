## MODIFIED Requirements

### Requirement: `Dispatchers` の primary entry id は `pekko.actor.default-dispatcher` である

fraktor-rs の `Dispatchers` registry は、default dispatcher の primary entry id として **Pekko 原典と整合する `"pekko.actor.default-dispatcher"`** を使用しなければならない (MUST)。以下の契約を全て満たすこと:

- `pub const DEFAULT_DISPATCHER_ID: &str = "pekko.actor.default-dispatcher"` (Pekko `Dispatchers.DefaultDispatcherId` (`Dispatchers.scala:160-164`) と同値)
- `ensure_default` / `ensure_default_inline` / `replace_default_inline` は primary entry を `DEFAULT_DISPATCHER_ID` 下に登録する
- legacy id `"default"` は **登録しない** (entry でも alias でもない)。fraktor-rs 独自の短縮表記 `"default"` は Pekko / Akka のどちらにも存在せず、本 change で完全退役する
- `pekko.actor.internal-dispatcher` は引き続き `DEFAULT_DISPATCHER_ID` への alias として自動登録される (Pekko `InternalDispatcherId` 互換のため)
- `pekko.actor.default-dispatcher` 自体は alias ではなく entry そのものであるため、alias として登録してはならない (`register_alias` の `AliasConflictsWithEntry` 判定で自然に拒否されるが、ensure_default 内で `if_absent` helper により idempotency を保証する)

#### Scenario: ensure_default_inline 直後に Pekko primary id が entry として解決される

- **GIVEN** `Dispatchers::new()` に対して `ensure_default_inline()` を呼んだ直後
- **WHEN** `resolve("pekko.actor.default-dispatcher")` を呼ぶ
- **THEN** primary entry 由来の `Ok(MessageDispatcherShared)` が返る
- **AND** `canonical_id("pekko.actor.default-dispatcher")` が `Ok("pekko.actor.default-dispatcher")` を返す (alias を辿らず entry 直接)

#### Scenario: ensure_default_inline 直後に legacy `"default"` は未登録 (完全退役)

- **GIVEN** `Dispatchers::new()` に対して `ensure_default_inline()` を呼んだ直後
- **WHEN** `resolve("default")` を呼ぶ
- **THEN** `Err(DispatchersError::Unknown("default"))` が返る (fraktor-rs 独自の legacy 短縮表記 `"default"` は本 change で完全退役したため、entry でも alias でも無い)
- **AND** `canonical_id("default")` も `Err(DispatchersError::Unknown("default"))` を返す

#### Scenario: ensure_default_inline 直後に pekko.actor.internal-dispatcher も primary entry に解決される

- **GIVEN** `Dispatchers::new()` に対して `ensure_default_inline()` を呼んだ直後
- **WHEN** `resolve("pekko.actor.internal-dispatcher")` を呼ぶ
- **THEN** primary entry 由来の `Ok(MessageDispatcherShared)` が返る (alias 経由)

#### Scenario: ensure_default_inline は冪等 (idempotent) で internal alias を重複登録しない

- **GIVEN** `Dispatchers::new()` に対して `ensure_default_inline()` を 2 回連続で呼ぶ
- **WHEN** 呼び出し完了後
- **THEN** `aliases` table には `"pekko.actor.internal-dispatcher"` のみ存在し (legacy `"default"` は無い)、重複登録エラーは発生しない
- **AND** `resolve("pekko.actor.internal-dispatcher")` が primary entry を返す

---

### Requirement: `Mailboxes` の primary entry id は `pekko.actor.default-mailbox` である

fraktor-rs の `Mailboxes` registry は、default mailbox の primary entry id として **Pekko 原典と整合する `"pekko.actor.default-mailbox"`** を使用しなければならない (MUST)。以下の契約を全て満たすこと:

- `const DEFAULT_MAILBOX_ID: &str = "pekko.actor.default-mailbox"` (Pekko `Mailboxes.DefaultMailboxId` (`Mailboxes.scala:58`) と同値、private const)
- `Mailboxes::ensure_default()` は primary entry を `DEFAULT_MAILBOX_ID` 下に登録する
- `Mailboxes` registry には alias chain 機構が **ない** (Dispatchers と非対称)。legacy `"default"` は entry として登録されず、`Mailboxes::resolve("default")` は `Err(MailboxRegistryError::Unknown(...))` を返す

**補足**: Mailboxes alias は設計上必要性が低い (`Props::mailbox_config()` で inline 指定が主流で、`Props::mailbox_id()` による registry lookup は稀) ため、本 change では alias 機構を追加しない。必要性が出た場合は別 change `pekko-mailbox-alias-chain` で対応する。

#### Scenario: ensure_default 直後に Pekko primary mailbox id が entry として解決される

- **GIVEN** `Mailboxes::new()` に対して `ensure_default()` を呼んだ直後
- **WHEN** `resolve("pekko.actor.default-mailbox")` を呼ぶ
- **THEN** default `MailboxConfig` の `Ok(MailboxConfig)` が返る

#### Scenario: ensure_default 後の legacy `"default"` は Mailboxes では resolve できない

- **GIVEN** `Mailboxes::new()` に対して `ensure_default()` を呼んだ直後
- **WHEN** `resolve("default")` を呼ぶ
- **THEN** `Err(MailboxRegistryError::Unknown("default"))` が返る (Mailboxes に alias 機構がないため)
- **NOTE**: これは本 change による破壊的変更。ただし `Mailboxes::resolve` の呼び出しは `ActorCell::create` の `Props::mailbox_id()` 経路のみで、`Props::mailbox_id("default")` を明示指定する production callsite は fraktor-rs 内に存在しない

---

### Requirement: typed `Dispatchers` facade は kernel primary id 定数を直接参照する

`modules/actor-core/src/core/typed/dispatchers.rs` の typed `Dispatchers` facade は、`DispatcherSelector::Default` / `SameAsParent` の解決時に **kernel const `crate::core::kernel::dispatch::dispatcher::DEFAULT_DISPATCHER_ID`** を直接参照しなければならない (MUST)。

- 従来の `const REGISTERED_DEFAULT_DISPATCHER_ID: &str = "default"` は削除される
- 値が 2 箇所で定義されることを避け、kernel の primary id flip に typed facade が自動追従することを保証する

#### Scenario: typed Default selector が kernel primary id を経由して resolve される

- **GIVEN** `ActorSystem::new_empty()` でデフォルトの dispatcher registry が構築された typed facade
- **WHEN** `Dispatchers::lookup(&DispatcherSelector::Default)` を呼ぶ
- **THEN** primary entry 由来の `Ok(MessageDispatcherShared)` が返る
- **AND** resolved id は `"pekko.actor.default-dispatcher"` である (旧 `"default"` ではない)

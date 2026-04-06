## ADDED Requirements

### Requirement: dispatcher は 1 : N で actor を収容する lifecycle を提供する

dispatcher は Pekko の `MessageDispatcher` と同じく、単一 instance で複数 actor を同時に収容する lifecycle を提供しなければならない (MUST)。`attach` / `detach` は inhabitants カウンタを通じて dispatcher 自身の終了タイミングを決定する。

#### Scenario: 複数 actor が同一 dispatcher に共存する
- **WHEN** `MessageDispatcherShared` に異なる 2 体以上の actor を `attach` する
- **THEN** すべての `attach` は成功する（`PinnedDispatcher` を除く）
- **AND** `inhabitants` は attach した actor 数と等しい
- **AND** すべての actor の mailbox が同じ `ExecutorShared` を経由して submit される
- **AND** dispatcher は 1 actor の mailbox への参照を field として保持しない

#### Scenario: attach は register_actor hook → create_mailbox → register_for_execution を順に呼ぶ
- **WHEN** `MessageDispatcherShared::attach(&self, actor)` を呼ぶ（内部で `with_write` 経由で trait の `attach(&mut self, actor)` default impl に委譲）
- **THEN** trait default impl が `self.register_actor(actor)` を呼ぶ
- **AND** `register_actor` の default impl は `self.core_mut().add_inhabitants(1)` を呼び `inhabitants` を 1 加算する（`PinnedDispatcher` は owner check も行う）
- **AND** `self.create_mailbox(actor, ...)` で作られた mailbox が actor に設定される
- **AND** ロック解放後に `MessageDispatcherShared::register_for_execution(&mbox, false, true)` が呼ばれる

#### Scenario: detach は unregister_actor hook を呼んで inhabitants 減算と auto-shutdown を予約する
- **WHEN** `MessageDispatcherShared::detach(&self, actor)` を呼ぶ（内部で `with_write` 経由で trait の `detach(&mut self, actor)` default impl に委譲）
- **THEN** trait default impl が `self.unregister_actor(actor)` を呼ぶ
- **AND** `unregister_actor` の default impl は `self.core_mut().add_inhabitants(-1)` を呼び `inhabitants` を 1 減算する
- **AND** 続けて `self.core_mut().schedule_shutdown_if_sensible()` を呼ぶ
- **AND** 全 actor が detach された（`inhabitants == 0`）場合、`shutdown_schedule` が `UNSCHEDULED → SCHEDULED` へ遷移する
- **AND** `PinnedDispatcher` は `unregister_actor` を override して owner を `None` に戻してから default 処理を呼ぶ

#### Scenario: 全 detach 後に shutdown_timeout 経過で executor が自動停止する
- **WHEN** 全 actor が `detach` された後、`shutdown_timeout` で指定された時間が経過する
- **THEN** dispatcher は自身の `ExecutorShared::shutdown()` を呼ぶ
- **AND** `shutdown_schedule` は `UNSCHEDULED` に戻る（次回 attach 時に再利用可能）

#### Scenario: shutdown 予約中の再 attach が shutdown をキャンセルする
- **WHEN** shutdown 予約中（`shutdown_schedule == SCHEDULED`）に新しい actor が `attach` される
- **THEN** `shutdown_schedule` は `SCHEDULED → RESCHEDULED` へ遷移する
- **AND** shutdown アクションが発火するときに `RESCHEDULED` を検知して cancel する
- **AND** dispatcher は停止せず新しい actor を収容する

### Requirement: attach / detach は trait default impl として提供され具象は override しない

`MessageDispatcher::attach` と `MessageDispatcher::detach` は、trait の default impl として提供され、具象 dispatcher 型は override しない規律で運用されなければならない (MUST NOT be overridden)。これは Pekko の `MessageDispatcher.attach`（`final`）/ `detach`（`final`）と等価である。具象型が差分を表現するための拡張点は `register_actor` / `unregister_actor` / `dispatch` / `register_for_execution` / `create_mailbox` の各 hook である。

#### Scenario: attach / detach は trait の default impl として提供される
- **WHEN** `MessageDispatcher` trait の定義を確認する
- **THEN** `attach(&mut self, actor)` と `detach(&mut self, actor)` が trait default impl として宣言されている
- **AND** trait doc に具象型がこれらを override してはならない旨が明記されている
- **AND** `DefaultDispatcher` / `PinnedDispatcher` は `attach` / `detach` を override しない
- **AND** `PinnedDispatcher` の owner check は `register_actor` hook の override で表現される
- **AND** 将来の `BalancingDispatcher` も `attach` / `detach` を override せず、`register_actor` / `unregister_actor` / `dispatch` / `create_mailbox` の override で team 機能を実現する

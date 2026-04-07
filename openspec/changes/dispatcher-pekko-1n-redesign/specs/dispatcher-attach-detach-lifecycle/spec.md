## ADDED Requirements

### Requirement: dispatcher は 1 : N で actor を収容する lifecycle を提供する

dispatcher は Pekko の `MessageDispatcher` と同じく、単一 instance で複数 actor を同時に収容する lifecycle を提供しなければならない (MUST)。`attach` / `detach` は inhabitants カウンタを通じて dispatcher 自身の終了タイミングを決定する。

#### Scenario: 複数 actor が同一 dispatcher に共存する
- **WHEN** `MessageDispatcherShared` に異なる 2 体以上の actor を `attach` する
- **THEN** すべての `attach` は成功する（`PinnedDispatcher` を除く）
- **AND** `inhabitants` は attach した actor 数と等しい
- **AND** すべての actor の mailbox が同じ `ExecutorShared` を経由して submit される
- **AND** dispatcher は 1 actor の mailbox への参照を field として保持しない

#### Scenario: attach は MessageDispatcherShared が register_actor hook → create_mailbox → register_for_execution を順に orchestrate する
- **WHEN** `MessageDispatcherShared::attach(&self, actor)` を呼ぶ
- **THEN** `MessageDispatcherShared` は `with_write` の中で `self.register_actor(actor)` を呼ぶ
- **AND** `register_actor` の default impl は `self.core_mut().mark_attach()` を呼び `inhabitants` を 1 加算する（`PinnedDispatcher` は owner check も行う）
- **AND** 同じ `with_write` の中で `self.create_mailbox(actor, ...)` で作られた mailbox が actor に設定される
- **AND** ロック解放後に `MessageDispatcherShared::register_for_execution(&mbox, false, true)` が呼ばれる

#### Scenario: attach は mailbox overflow strategy と executor の blocking 対応を検証する
- **WHEN** `MessageDispatcherShared::attach(&self, actor)` を呼ぶ
- **THEN** `with_write` の中で mailbox install より前に actor の mailbox config と executor の compatibility を検証する
- **AND** actor の mailbox overflow strategy が `MailboxOverflowStrategy::Block` の場合は `self.executor().supports_blocking()` を確認する
- **AND** `supports_blocking()` が `false` のとき `SpawnError::InvalidMailboxConfig` を返し、`register_actor` / `create_mailbox` / mailbox install は実行しない

#### Scenario: detach は unregister_actor hook を呼んで inhabitants 減算と auto-shutdown を予約する
- **WHEN** `MessageDispatcherShared::detach(&self, actor)` を呼ぶ
- **THEN** `MessageDispatcherShared` は `with_write` の中で `self.unregister_actor(actor)` を呼ぶ
- **AND** `unregister_actor` の default impl は `self.core_mut().mark_detach()` を呼び `inhabitants` を 1 減算する（戻り値なし、純粋 command）
- **AND** 同じ `with_write` の中で detached mailbox は terminal 状態へ遷移し、clean up される
- **AND** 続けて `self.core_mut().schedule_shutdown_if_sensible()` が呼ばれ、その戻り値 `ShutdownSchedule` をローカル変数へ copy する。全 actor が detach された（`inhabitants == 0`）場合のみ `shutdown_schedule` が `UNSCHEDULED → SCHEDULED` へ遷移する
- **AND** ロック解放後、copy した値が `SCHEDULED` のときだけ `MessageDispatcherShared` は `actor.scheduler()` から取得した handle に delayed shutdown closure を登録する
- **AND** lock 解放後の状態再観測は行わない（race window を作らない）
- **AND** `PinnedDispatcher` は `unregister_actor` を override して owner を `None` に戻してから default 処理を呼ぶ

#### Scenario: 全 detach 後に shutdown_timeout 経過で executor が自動停止する
- **WHEN** 全 actor が `detach` された後、`shutdown_timeout` で指定された時間が経過する
- **THEN** dispatcher は自身の `ExecutorShared::shutdown()` を呼ぶ
- **AND** `shutdown_schedule` は `UNSCHEDULED` に戻る（次回 attach 時に再利用可能）

#### Scenario: delayed shutdown の system scheduler は detach 引数の actor から辿る
- **WHEN** `MessageDispatcherShared::detach(&self, actor)` が delayed shutdown を登録する
- **THEN** scheduler handle は `actor.scheduler()` 経由で取得する
- **AND** `DispatcherCore` / `MessageDispatcherShared` は system scheduler への参照を field として保持しない

#### Scenario: shutdown 予約中の再 attach が shutdown をキャンセルする
- **WHEN** shutdown 予約中（`shutdown_schedule == SCHEDULED`）に新しい actor が `attach` される
- **THEN** `register_actor` の実行中に `shutdown_schedule` は `SCHEDULED → RESCHEDULED` へ遷移する
- **AND** shutdown アクションが発火するときに `RESCHEDULED` を検知して cancel する
- **AND** dispatcher は停止せず新しい actor を収容する

### Requirement: attach / detach は MessageDispatcherShared の orchestration として提供される

`attach` と `detach` は lock 解放後の scheduling / shutdown 予約を伴うため、trait default impl ではなく `MessageDispatcherShared` の orchestration として提供されなければならない (MUST)。具象型が差分を表現するための拡張点は `register_actor` / `unregister_actor` / `dispatch` / `create_mailbox` の各 hook である。`register_for_execution` は hook ではなく shared wrapper 固有の CAS + executor submit ロジックである。

#### Scenario: attach / detach は shared wrapper の public API として提供される
- **WHEN** `MessageDispatcher` trait の定義を確認する
- **THEN** `attach` / `detach` は `MessageDispatcherShared` の public API として提供されている
- **AND** `MessageDispatcher` trait には orchestration を表す `attach` / `detach` メソッドが存在しない
- **AND** `PinnedDispatcher` の owner check は `register_actor` hook の override で表現される
- **AND** 将来の `BalancingDispatcher` も `attach` / `detach` を override せず、`register_actor` / `unregister_actor` / `dispatch` / `create_mailbox` の override で team 機能を実現する
- **AND** その `register_actor` / `unregister_actor` override は default の inhabitants 更新を維持した上で team registry 更新を追加する
- **AND** `register_for_execution` は trait hook ではなく `MessageDispatcherShared` 内部ロジックとして維持される
- **AND** `MessageDispatcherShared` 自身は Balancing 専用の team 探索や候補 mailbox 合成を行わない

#### Scenario: 並走期間中の legacy ActorCell create 経路は mailbox を即時 install する
- **WHEN** 旧 dispatcher 経路を使う `ActorCell::create` が mailbox を eager 生成する
- **THEN** 生成直後に `install_mailbox` を即時呼ぶ
- **AND** その責務は legacy create 経路側にある
- **AND** 新 dispatcher 経路では `MessageDispatcherShared::attach` が mailbox install を担当する

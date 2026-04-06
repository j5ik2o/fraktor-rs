## ADDED Requirements

### Requirement: `DispatcherCore` は pub struct として dispatcher 共通 state を集約する

dispatcher の共通 state と private helper は pub struct `DispatcherCore` に集約し、fraktor 外部からも `MessageDispatcher` 独自実装のベースとして利用可能にしなければならない (MUST)。

#### Scenario: DispatcherCore は pub struct として定義される
- **WHEN** `core::kernel::dispatch::dispatcher::DispatcherCore` の可視性を確認する
- **THEN** `pub struct DispatcherCore` として公開されている
- **AND** `DispatcherCore::new` が pub コンストラクタとして公開されている

#### Scenario: DispatcherCore は dispatcher の共通 state をすべて保持する
- **WHEN** `DispatcherCore` の field を確認する
- **THEN** 次の field が存在する:
  - `id: String`
  - `throughput: NonZeroUsize`
  - `throughput_deadline: Option<Duration>`
  - `shutdown_timeout: Duration`
  - `executor: ExecutorShared`
  - `inhabitants: i64`
  - `shutdown_schedule: ShutdownSchedule` (enum、Atomic ではない)
- **AND** フィールドに `AtomicI64` / `AtomicU8` / `Mutex<T>` / `UnsafeCell<T>` などの内部可変性は存在しない

#### Scenario: DispatcherCore は CQS 準拠の pub メソッドを提供する
- **WHEN** `DispatcherCore` の impl を確認する
- **THEN** 次の query メソッドが `&self` で提供されている: `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants`, `executor`（`&ExecutorShared` を返す）
- **AND** 次の command メソッドが `&mut self` で提供されている: `add_inhabitants(&mut self, delta: i64) -> i64`, `schedule_shutdown_if_sensible(&mut self)`, `shutdown(&mut self)`
- **AND** `add_inhabitants` の戻り値 `i64` は CQS 許容例外として扱われる（Pekko `addInhabitants: Long` と等価で、`schedule_shutdown_if_sensible` の判定に合成後の値を使う必要があるため）
- **AND** `DispatcherCore::shutdown(&mut self)` は内部で `self.executor.shutdown()` を呼び、`shutdown_schedule` を UNSCHEDULED に戻す
- **AND** command メソッドを `&self` + 内部可変性で偽装する実装は存在しない

#### Scenario: 具象 dispatcher 型は DispatcherCore を field として保持する
- **WHEN** `DefaultDispatcher` と `PinnedDispatcher` の struct 定義を確認する
- **THEN** 両方が `core: DispatcherCore` を field として保持する（`ArcShared<DispatcherCore>` のような多所有化はしない）
- **AND** 共通状態を重複して別フィールドで持たない
- **AND** 多所有の共有は `MessageDispatcherShared` 経由で dispatcher 全体ごと行われる

#### Scenario: DispatcherCore は別名ラッパと同時に公開されない
- **WHEN** dispatcher 関連の公開型を確認する
- **THEN** `DispatcherShared` / `DispatchShared` は存在しない
- **AND** `DispatcherCore` を wrap して同じ状態を別名で公開する型は存在しない

### Requirement: `register_for_execution` は `MessageDispatcherShared` 上で Pekko `registerForExecution` の契約を満たす

`MessageDispatcherShared::register_for_execution` は Pekko `Dispatcher.registerForExecution` と同じ契約で動作しなければならない (MUST)。mailbox の `canBeScheduledForExecution` / `setAsScheduled` 相当の CAS に成功した場合のみ executor へ submit し、ロック区間を最小化しつつ結果を返す。`MessageDispatcher` trait 側の `register_for_execution(&mut self, ...)` は CAS 判定のみを担当し、実際の closure 組み立てと executor への submit は `MessageDispatcherShared` 側で orchestrate する。

#### Scenario: スケジュール可能かつ CAS 成功時のみ executor へ submit する
- **WHEN** `MessageDispatcherShared::register_for_execution(&self, mbox: &ArcShared<Mailbox>, has_message_hint: bool, has_system_hint: bool)` を呼ぶ
- **THEN** mailbox の `can_be_scheduled_for_execution(hints)` が `false` を返す場合、戻り値 `false` で即 return する
- **AND** `set_as_scheduled` の CAS に失敗した場合、戻り値 `false` で即 return する
- **AND** CAS に成功した場合のみ `ExecutorShared::execute(task)` を呼ぶ
- **AND** submit に成功したら `true` を返す

#### Scenario: ロック区間は最小化される
- **WHEN** `MessageDispatcherShared::register_for_execution` の実装を確認する
- **THEN** mailbox の CAS 評価はロック外（`with_read` / `with_write` 外）で実行される
- **AND** throughput / throughput_deadline / executor_shared の取得は 1 度の `with_read` で完結する
- **AND** closure 構築と `ExecutorShared::execute` 呼び出しはロック解放後に実行される
- **AND** `ActorRefSenderShared::send` と同じく、ロックを保持したまま副作用 closure を実行しない（再入デッドロック防止）

#### Scenario: submit された task は mailbox.run を駆動する
- **WHEN** `register_for_execution` が `ExecutorShared::execute` に渡す task を確認する
- **THEN** task は `mbox.run(throughput, throughput_deadline)` を呼ぶ
- **AND** 終端で `set_as_idle` を呼び、さらに captured `MessageDispatcherShared` の clone 経由で `self_clone.register_for_execution(&mbox, false, false)` を呼ぶ（Pekko の `Mailbox.run` の finally に相当）
- **AND** panic が発生しても `set_as_idle` と再スケジュール経路が実行されるよう guard される

#### Scenario: RejectedExecution の扱いは 1 回だけリトライする
- **WHEN** `ExecutorShared::execute` が reject された場合
- **THEN** 1 回だけ再試行する
- **AND** 2 回目も失敗した場合はエラーをログに残し、`set_as_idle` を呼んで状態を復旧する
- **AND** これは Pekko `Dispatcher.registerForExecution` のリトライ戦略と一致する

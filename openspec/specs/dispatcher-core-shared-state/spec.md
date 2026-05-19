# dispatcher-core-shared-state Specification

## Purpose
TBD - created by archiving change dispatcher-pekko-1n-redesign. Update Purpose after archive.
## Requirements
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
- **AND** 次の command メソッドが `&mut self` で提供されている: `mark_attach(&mut self)`, `mark_detach(&mut self)`, `schedule_shutdown_if_sensible(&mut self) -> ShutdownSchedule`, `shutdown(&mut self)`
- **AND** `mark_attach` / `mark_detach` は戻り値なしの純粋 command である（CQS 例外なし）
- **AND** `mark_detach` は inhabitants が負になる場合 `debug_assert!` でパニックし、release ビルドでは `i64::max(self.inhabitants, 0)` で clamp する（Pekko の `IllegalStateException("ACTOR SYSTEM CORRUPTED!!!")` 相当の防御）
- **AND** release で clamp する場合でも `tracing::error!` 等で状態破損を必ず記録する
- **AND** `schedule_shutdown_if_sensible` は遷移後の `ShutdownSchedule` 値を返す。これは CQS 許容例外として扱われ、`MessageDispatcherShared::detach` が lock 解放前に値を copy して delayed shutdown 登録判定に使うためである（lock 解放後の再観測による race window を避ける）
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

### Requirement: `register_for_execution` は `MessageDispatcherShared` の純粋な CAS + executor submit 経路として提供される

`MessageDispatcherShared::register_for_execution` は Pekko `Dispatcher.registerForExecution` と同じ契約で動作しなければならない (MUST)。mailbox の `canBeScheduledForExecution` / `setAsScheduled` 相当の CAS に成功した場合のみ executor へ submit する。**`MessageDispatcher` trait 側に `register_for_execution` hook は存在しない** — dispatcher policy 固有の判定（例: BalancingDispatcher の teamWork 候補展開）は `dispatch` hook の戻り値配列で表現する。

#### Scenario: スケジュール可能かつ CAS 成功時のみ executor へ submit する
- **WHEN** `MessageDispatcherShared::register_for_execution(&self, mbox: &ArcShared<Mailbox>, has_message_hint: bool, has_system_hint: bool)` を呼ぶ
- **THEN** mailbox の `can_be_scheduled_for_execution(hints)` が `false` を返す場合、戻り値 `false` で即 return する
- **AND** `set_as_scheduled` の CAS に失敗した場合、戻り値 `false` で即 return する
- **AND** CAS 成功後に `with_read` で throughput / throughput_deadline / executor_shared を 1 度に取得する
- **AND** ロック解放後に closure を組み立てて `ExecutorShared::execute` に submit する
- **AND** submit に成功したら `true` を返す
- **AND** submit が `Err(ExecuteError)` を返した場合は `mbox.set_as_idle()` に rollback して `false` を返す
- **AND** rollback 時に失敗はログ / メトリクスへ記録される

#### Scenario: ロック区間は最小化される
- **WHEN** `MessageDispatcherShared::register_for_execution` の実装を確認する
- **THEN** mailbox の CAS 評価はロック外（`with_read` / `with_write` 外）で実行される
- **AND** throughput / throughput_deadline / executor_shared の取得は 1 度の `with_read` で完結する
- **AND** closure 構築と `ExecutorShared::execute` 呼び出しはロック解放後に実行される
- **AND** 既存の AShared 系と同じく、ロックを保持したまま副作用 closure を実行しない（再入デッドロック防止）
- **AND** trait 側に `register_for_execution` hook は存在しないため、dispatcher policy 判定は `dispatch` hook の戻り値配列経由で行う

#### Scenario: submit された task は mailbox.run を駆動する
- **WHEN** `register_for_execution` が `ExecutorShared::execute` に渡す task を確認する
- **THEN** task は `mbox.run(throughput, throughput_deadline)` を呼ぶ
- **AND** 終端で `set_as_idle` を呼び、さらに captured `MessageDispatcherShared` の clone 経由で `self_clone.register_for_execution(&mbox, false, false)` を呼ぶ（Pekko の `Mailbox.run` の finally に相当）
- **AND** panic が発生しても `set_as_idle` と再スケジュール経路が実行されるよう guard される

### Requirement: `MessageDispatcherShared::dispatch` は trait hook の候補配列を順に register_for_execution する

`MessageDispatcherShared::dispatch` / `system_dispatch` は `MessageDispatcher::dispatch` / `system_dispatch` hook が返す候補 mailbox 配列を、ロック解放後に優先度順に走査して `register_for_execution` を試みなければならない (MUST)。

#### Scenario: dispatch hook の候補配列を順に register_for_execution する
- **WHEN** `MessageDispatcherShared::dispatch(&self, receiver, env)` を呼ぶ
- **THEN** `with_write` で trait hook `dispatch(&mut self, receiver, env)` が呼ばれ、`Vec<ArcShared<Mailbox>>` の候補配列を取得する
- **AND** trait hook 内では enqueue のみが行われ、schedule 副作用は起こさない
- **AND** ロック解放後、候補配列を優先度順に走査して各 mailbox に対し適切な hint で `self.register_for_execution(...)` を試みる
- **AND** `dispatch` は `has_message_hint=true, has_system_hint=false` を使う
- **AND** `system_dispatch` は `has_message_hint=false, has_system_hint=true` を使う
- **AND** 最初に `register_for_execution` が `true` を返した候補で完了する
- **AND** 全候補が `false` を返しても `Ok(())` を返す（envelope は queue 内に残り、次回 dispatch / drain サイクルで pick up される）
- **AND** `MessageDispatcherShared` 自身が追加の mailbox 候補を探索・合成しない。候補集合の決定は常に dispatcher hook 側の責務である

#### Scenario: BalancingDispatcher の dispatch override は shared queue に enqueue する
- **WHEN** `BalancingDispatcher` の `dispatch(&mut self, receiver, env)` が呼ばれる
- **THEN** `self.shared_queue.enqueue(env)?` で envelope を shared queue に入れる
- **AND** 戻り値は `receiver.mailbox()` を先頭にした team candidate mailbox 配列である
- **AND** shared wrapper は通常通り候補配列を順に register_for_execution する
- **AND** receiver mailbox が unschedulable でも、後続候補への fallback により shared queue の drain 機会が失われない

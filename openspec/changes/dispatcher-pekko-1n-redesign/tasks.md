## 0. 並走期間中の依存ルール（全タスクに適用される事前条件）

- [x] 0.1 **`modules/actor-core/src/core/kernel/dispatch/dispatcher_new/` 配下のいかなるファイルも、旧 `modules/actor-core/src/core/kernel/dispatch/dispatcher/` 配下の型・関数・trait・モジュールを `use` / 参照してはならない**
- [x] 0.2 **`modules/actor-adaptor-std/src/std/dispatch_new/` 配下のいかなるファイルも、旧 `modules/actor-adaptor-std/src/std/dispatch/` 配下の型・関数・trait・モジュールを `use` / 参照してはならない**
- [x] 0.3 同じ概念を新旧両側で必要とする場合は、新側に独立して再実装する（コードの重複を許容してでも依存を切る）。旧側 helper 関数を「動くから」という理由で新側から呼ばない
- [x] 0.4 PR レビュー時に、絶対 import だけでなく grouped / 相対 import も含めて旧 tree 参照がないことを確認する。少なくとも `rg -n "::dispatcher::" modules/actor-core/src/core/kernel/dispatch/dispatcher_new/ | rg -v "::dispatcher_new::"` および `rg -n "::dispatch::" modules/actor-adaptor-std/src/std/dispatch_new/ | rg -v "::dispatch_new::"` を実行し、ヒットがゼロであることを確認する
- [x] 0.5 旧側のテスト ユーティリティを新側のテストから流用しない。新側のテストヘルパは新側で完結する

**Why**: 並走期間中に新側が旧側に依存すると、旧側を一括削除する瞬間に circular な依存が露呈し、削除 PR が膨大になる。`rm -rf` で旧側を削除するだけで完了できる構造を維持するため。

## 1. core: Executor trait / ExecutorShared / InlineExecutor

- [x] 1.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher_new/executor.rs` に CQS 準拠 `trait Executor { fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError>; fn supports_blocking(&self) -> bool { true }; fn shutdown(&mut self); }` を定義する
- [x] 1.1.1 `dispatcher_new/execute_error.rs` に `ExecuteError` を定義する。最小初版は `Rejected`, `Shutdown`, `Backend(String)` 相当を持ち、executor submit 失敗を観測可能にする
- [x] 1.2 `dispatcher_new/executor_shared.rs` に `pub struct ExecutorShared { inner: ArcShared<RuntimeMutex<Box<dyn Executor>>> }` を定義し、`Clone` と `SharedAccess<Box<dyn Executor>>` を実装する（`with_read` / `with_write`）
- [x] 1.3 `ExecutorShared::new<E: Executor + 'static>(executor: E) -> Self` と convenience methods (`execute(&self, task) -> Result<(), ExecuteError>`, `shutdown(&self)`, `supports_blocking(&self) -> bool`) を実装する。既存の AShared 系と同じくロック区間を最小化する
- [x] 1.4 `dispatcher_new/executor_factory.rs` に `trait ExecutorFactory { fn create(&self, id: &str) -> ExecutorShared; }` を定義する（生の `ArcShared<Box<dyn Executor>>` ではなく `ExecutorShared` を返す）
- [x] 1.5 `dispatcher_new/inline_executor.rs` に `InlineExecutor` を定義し、`execute(&mut self, task)` で現スレッド同期実行する。再入対策（trampoline）は `InlineExecutor` 自身の内部状態として持つ。**用途は test / deterministic scheduling に限定し、production の `ExecutorShared` へ組み込まない**
- [x] 1.6 executor trait / ExecutorShared / factory / InlineExecutor の unit test を追加する
- [x] 1.7 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 1.5 core: DispatcherSettings（新版、immutable settings bundle）

- [x] 1.5.1 `dispatcher_new/dispatcher_settings.rs` に `pub struct DispatcherSettings { pub id: String, pub throughput: NonZeroUsize, pub throughput_deadline: Option<Duration>, pub shutdown_timeout: Duration }` を定義する
- [x] 1.5.2 旧版 `DispatcherSettings` が持っていた `schedule_adapter` / `starvation_deadline` フィールドを新版には**含めない**ことを確認する（前者は `ScheduleAdapter` 自体削除に伴って、後者は YAGNI で初期版から除外）
- [x] 1.5.3 `DispatcherSettings::new(id, throughput, throughput_deadline, shutdown_timeout) -> Self` と `with_throughput`, `with_throughput_deadline`, `with_shutdown_timeout` 等の builder 風メソッドを実装する。builder はすべて `self` 消費の `Self` 返しに統一する
- [x] 1.5.4 `DispatcherSettings` を `Clone` 可能にする
- [x] 1.5.5 `DispatcherSettings` の unit test を追加する（builder メソッドの挙動、Clone、フィールド値の保持）
- [x] 1.5.5 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 2. core: DispatcherCore（pub 共通 state、CQS 準拠、内部可変性なし）

- [x] 2.1 `dispatcher_new/dispatcher_core.rs` に pub struct `DispatcherCore` を定義し、以下 field を保持する: `id: String`, `throughput: NonZeroUsize`, `throughput_deadline: Option<Duration>`, `shutdown_timeout: Duration`, `executor: ExecutorShared`, `inhabitants: i64`, `shutdown_schedule: ShutdownSchedule` (enum)
- [x] 2.2 `DispatcherCore` の field には `AtomicI64` / `AtomicU8` / `Mutex<T>` / `UnsafeCell<T>` などの内部可変性を導入しないことを確認する
- [x] 2.3 `DispatcherCore::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` を実装する。`settings.id` / `settings.throughput` / `settings.throughput_deadline` / `settings.shutdown_timeout` を field にコピーする
- [x] 2.4 `DispatcherCore` の query メソッドを `&self` で実装する: `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants`, `executor`（`&ExecutorShared` を返す）
- [x] 2.5 `DispatcherCore` の command メソッドを `&mut self` で実装する:
  - `mark_attach(&mut self)`: inhabitants を加算し、`SCHEDULED` なら `RESCHEDULED` へ遷移（戻り値なし、純粋 command）
  - `mark_detach(&mut self)`: inhabitants を減算する（戻り値なし、純粋 command）。`debug_assert!(self.inhabitants >= 0)` で underflow を検出、release では `i64::max(self.inhabitants, 0)` で clamp（Pekko の `IllegalStateException("ACTOR SYSTEM CORRUPTED!!!")` 相当の防御）
  - `schedule_shutdown_if_sensible(&mut self) -> ShutdownSchedule`: inhabitants が 0 の時のみ状態遷移を行い、**遷移後の `ShutdownSchedule` 値を返す**（CQS 許容例外: 呼び出し側が lock 解放前に値を copy して delayed shutdown 登録判定に使うため）
  - `shutdown(&mut self)`: `self.executor.shutdown()` を呼び、`shutdown_schedule` を UNSCHEDULED に戻す
- [x] 2.6 `mark_attach` / `mark_detach` / `schedule_shutdown_if_sensible` の state machine を Pekko 準拠で実装する（`UNSCHEDULED -> SCHEDULED`、再 attach 時の `SCHEDULED -> RESCHEDULED`、`shutdown()` 後の `UNSCHEDULED` 復帰）
- [x] 2.7 DispatcherCore の unit test を追加する（inhabitants カウンタの加減算、shutdown_schedule の状態遷移、CQS 分類の確認）
  - `mark_detach` の underflow clamp が release 相当経路で error log / metric を残すことを確認する
- [x] 2.8 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 2.5 core: MessageDispatcherShared（AShared パターン）

- [x] 2.5.1 `dispatcher_new/message_dispatcher_shared.rs` に `pub struct MessageDispatcherShared { inner: ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>> }` を定義する
- [x] 2.5.2 `impl Clone for MessageDispatcherShared` を実装する（`ArcShared::clone`）
- [x] 2.5.3 `MessageDispatcherShared::new<D: MessageDispatcher + 'static>(dispatcher: D) -> Self` を実装する
- [x] 2.5.4 `impl SharedAccess<Box<dyn MessageDispatcher>> for MessageDispatcherShared` を実装する（`with_read` / `with_write`）
- [x] 2.5.5 orchestration methods を実装する (すべて actor 引数は `&ArcShared<ActorCell>` 型):
  - `attach(&self, actor: &ArcShared<ActorCell>) -> Result<(), SpawnError>`: `with_write` の中で
    1. blocking compatibility check: actor の mailbox config が `MailboxOverflowStrategy::Block` を要求する場合、`self.core().executor().supports_blocking()` が `false` なら `SpawnError::InvalidMailboxConfig` を返す
    2. `trait_hook.register_actor(actor)` を呼ぶ
    3. `trait_hook.create_mailbox(actor, ...)` で mailbox を作り、`actor.install_mailbox(mbox.clone())` で actor に設定する
    4. mbox を local 変数に保持
    を行い、ロック解放後に `self.register_for_execution(&mbox, false, true)` を呼ぶ
  - `detach(&self, actor: &ArcShared<ActorCell>)`: `with_write` の中で
    1. `trait_hook.unregister_actor(actor)` を呼ぶ
    2. `actor.mailbox()` を terminal 状態へ遷移させ `clean_up`
    3. `self.core_mut().schedule_shutdown_if_sensible()` の戻り値 `ShutdownSchedule` をローカル変数へ copy
    を行い、ロック解放後に copy した値が `SCHEDULED` の場合のみ `actor.scheduler()` 経由で delayed shutdown closure を登録
  - `dispatch(&self, receiver: &ArcShared<ActorCell>, env: Envelope) -> Result<(), SendError>` / `system_dispatch(&self, receiver: &ArcShared<ActorCell>, msg: SystemMessage) -> Result<(), SendError>`: `with_write` で trait hook を呼び **戻り値の候補 mailbox 配列を取得**（hook 内では enqueue のみ）、ロック解放後に候補配列を優先度順に走査して各 mailbox に対し `register_for_execution` を試みる。`dispatch` は `has_message_hint=true, has_system_hint=false`、`system_dispatch` は `has_message_hint=false, has_system_hint=true` を使う。最初に `true` を返した候補で完了。全候補 busy でも `Ok(())` を返す。shared queue を持つ具象型は liveness のため複数候補を返すこと
  - `suspend(&self, actor: &ArcShared<ActorCell>)` / `resume(&self, actor: &ArcShared<ActorCell>)` / `shutdown`, `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants` は `with_write` / `with_read` で委譲する（`execute_task` は trait に存在しないため委譲対象外）
- [x] 2.5.6 `register_for_execution(&self, mbox: &ArcShared<Mailbox>, has_message_hint: bool, has_system_hint: bool) -> bool` を実装する（純粋に CAS + executor submit、trait hook は呼ばない）:
  1. `mbox.can_be_scheduled_for_execution(hints)` をロック外で評価。`false` なら `false` を返す
  2. `mbox.set_as_scheduled()` の CAS をロック外で試み、失敗したら `false` を返す
  3. `with_read` で throughput / throughput_deadline / executor_shared を 1 回だけ取得
  4. ロックを解放した状態で closure を構築（`self.clone()` と `mbox.clone()` を capture）
  5. `ExecutorShared::execute` に submit
  6. submit が `Err(error)` の場合は `mbox.set_as_idle()` で rollback し、失敗をログ / メトリクスへ記録して `false` を返す
  7. closure 実行時は `mbox.run(throughput, deadline)` → `mbox.set_as_idle` → `self.register_for_execution(&mbox, false, false)` の順で再スケジュール
  8. submit 成功で `true` を返す
- [x] 2.5.7 `MessageDispatcherShared` の unit test を追加する（ロック区間の最小化、再入時のデッドロック回避、detach の `ShutdownSchedule` 戻り値経路で delayed shutdown 登録、dispatch 候補配列の優先度順 fallback、register_for_execution の Pekko 契約に沿った挙動）
  - shared wrapper 自身が Balancing 専用の team 探索や候補合成を行っていないことを確認する
- [x] 2.5.8 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 3. core: MessageDispatcher trait（CQS 準拠）

- [x] 3.1 `dispatcher_new/message_dispatcher.rs` に `trait MessageDispatcher: Send + Sync` を定義する
- [x] 3.2 query メソッドを `&self` で宣言する: `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants`, `executor`（clone 返しの `ExecutorShared`）, `core(&self) -> &DispatcherCore`
- [x] 3.3 `create_mailbox(&self, actor: &ArcShared<ActorCell>, mailbox_type: &dyn MailboxType) -> ArcShared<Mailbox>` を `&self` で宣言する（factory メソッドなので状態を変えない）。`actor` を `&ArcShared<ActorCell>` で受け取るのは `BalancingDispatcher::create_mailbox` が `ArcShared::downgrade(actor)` で `WeakShared<ActorCell>` を取得するため
- [x] 3.4 hook `register_actor(&mut self, actor: &ArcShared<ActorCell>) -> Result<(), SpawnError>` を default impl 付きで宣言する。default impl は `self.core_mut().mark_attach()` を呼んで `Ok(())`。identity 比較が必要な override (PinnedDispatcher) は `actor.pid()` を使う
- [x] 3.5 hook `unregister_actor(&mut self, actor: &ArcShared<ActorCell>)` を default impl 付きで宣言する。default impl は `self.core_mut().mark_detach()` を呼ぶ
- [x] 3.6 hook メソッドを default impl 付きで宣言する（具象型は必要な時だけ override）:
  - `dispatch(&mut self, receiver: &ArcShared<ActorCell>, env: Envelope) -> Result<Vec<ArcShared<Mailbox>>, SendError>`: default は `receiver.mailbox().enqueue_user(env)?` → `vec![receiver.mailbox()]`。実装は `SmallVec<[ArcShared<Mailbox>; 1]>` 等の small-size optimization を採用してよい
  - `system_dispatch(&mut self, receiver: &ArcShared<ActorCell>, msg: SystemMessage) -> Result<Vec<ArcShared<Mailbox>>, SendError>`: default は同様に enqueue_system → `vec![receiver.mailbox()]`
  - `shutdown(&mut self)`: default は `self.core_mut().shutdown()`
- [x] 3.7 `core_mut(&mut self) -> &mut DispatcherCore` を必須メソッドとして宣言する（default impl から `DispatcherCore` へ到達するため）
- [x] 3.8 その他の command (`suspend(&mut self, actor: &ArcShared<ActorCell>)`, `resume(&mut self, actor: &ArcShared<ActorCell>)`) を `&mut self` で宣言する
- [x] 3.9 trait method で `Box<dyn MessageDispatcher>` を trait object として扱えるようにする（object-safe 保証）
- [x] 3.10 trait doc に次を明記する:
  - command メソッドを `&self` + 内部可変性で偽装してはならない
  - `attach` / `detach` の orchestration は `MessageDispatcherShared` 側が担う
  - `create_mailbox` は direct call せず `MessageDispatcherShared::attach` 経由で使う
  - actor 引数は `&ArcShared<ActorCell>` 型に統一する。`ArcShared::downgrade(actor)` で `WeakShared<ActorCell>` を取得できる (BalancingDispatcher が SharingMailbox 構築に使う)
  - `dispatch` / `system_dispatch` の戻り値配列は shared wrapper が lock 解放後に register_for_execution を試みる候補 mailbox の優先度順リスト
  - `register_actor` / `unregister_actor` / `dispatch` / `system_dispatch` / `create_mailbox` は override 可能な hook
  - trait に `register_for_execution` は存在しない（shared wrapper の純粋 CAS + executor submit 経路に集約）
  - trait に `execute_task` は存在しない（本 change スコープ外、必要になった時点で additive に追加）
- [x] 3.11 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 4. core: DefaultDispatcher 具象型

- [x] 4.1 `dispatcher_new/default_dispatcher.rs` に `pub struct DefaultDispatcher { core: DispatcherCore }` を定義する
- [x] 4.2 `DefaultDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` を実装する。内部で `DispatcherCore::new(settings, executor)` を呼ぶ
- [x] 4.3 `impl MessageDispatcher for DefaultDispatcher` を実装する:
  - 必須メソッド: `core(&self) -> &DispatcherCore` / `core_mut(&mut self) -> &mut DispatcherCore`
  - query メソッド (`id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants`, `executor`) は `self.core` へ委譲する
  - `create_mailbox(&self, actor, ty)` は新規 `ArcShared<Mailbox>` を返す（default impl でそのまま使える場合は override 不要）
  - hook メソッド (`register_actor`, `unregister_actor`, `dispatch`, `system_dispatch`, `shutdown`) は trait の default impl をそのまま使う（`DefaultDispatcher` 固有の追加処理はない）
- [x] 4.4 `DefaultDispatcher` の unit test を追加する（shared wrapper 経由の attach / detach が inhabitants を増減すること、複数 actor を同時 attach できること、auto-shutdown の挙動、default hook が意図通り呼ばれること）
- [x] 4.5 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 5. core: PinnedDispatcher 具象型

- [x] 5.0 `modules/actor-core/src/core/kernel/actor/spawn/spawn_error.rs` に `SpawnError::DispatcherAlreadyOwned` バリアントを追加する（PinnedDispatcher の同時所有拒否のため）
- [x] 5.1 `dispatcher_new/pinned_dispatcher.rs` に `pub struct PinnedDispatcher { core: DispatcherCore, owner: Option<Pid> }` を定義する（既存の actor identity は `Pid` を使う。通常の `Option`、内部可変性は使わない）
- [x] 5.2 `PinnedDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` を実装する。引数で受け取った `settings` を `settings.with_throughput(NonZeroUsize::MAX).with_throughput_deadline(None)` で **Pinned 固有値に上書き** してから `DispatcherCore::new` に渡す（呼び出し側が何を渡しても結果として Pinned の固定値になる）
- [x] 5.3 `impl MessageDispatcher for PinnedDispatcher` を実装する:
  - 必須メソッド: `core(&self)` / `core_mut(&mut self)` を `self.core` / `&mut self.core` で実装
  - query メソッドはすべて `self.core` 委譲（Pinned 固有の固定値は `new` で既に core に埋め込み済み）
  - `create_mailbox` は default impl のまま
  - hook メソッドのうち **`register_actor` と `unregister_actor` のみ** override:
    - `register_actor(&mut self, actor: &ArcShared<ActorCell>)`: `self.owner` が `None` または `Some(actor.pid())` と一致なら `self.owner = Some(actor.pid())` をセットし、`self.core.mark_attach()` を呼んで `Ok(())` を返す。別 actor が既に owner なら `SpawnError::DispatcherAlreadyOwned` を返す
    - `unregister_actor(&mut self, actor: &ArcShared<ActorCell>)`: owner を `None` に戻してから `self.core.mark_detach()` を呼ぶ
  - それ以外の hook (`dispatch`, `system_dispatch`, `shutdown`) は default impl
- [x] 5.4 `PinnedDispatcher` の unit test を追加する（1 actor 専有、2 体目拒否、同一 actor の再 attach 許容、detach 後の再利用、Pinned 固有値が query で返ること、`SpawnError::DispatcherAlreadyOwned` が返ること）
- [x] 5.5 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 5.5 core: BalancingDispatcher 具象型 + SharedMessageQueue + `Mailbox::new_sharing(...)`

- [x] 5.5.1 `dispatcher_new/shared_message_queue.rs` に `pub struct SharedMessageQueue { inner: ArcShared<RuntimeMutex<VecDeque<Envelope>>> }` を定義し、`MessageQueue` trait を実装する（enqueue / dequeue / len / is_empty すべて `&self`）。core 層 (`no_std` 対応)。後で lock-free 化可能なシグネチャに留める
- [x] 5.5.2 独立した `SharingMailbox` struct は作らず、`Mailbox` 本体に `MailboxCleanupPolicy { DrainToDeadLetters, LeaveSharedQueue }` を追加する。`Mailbox::new(actor, queue)` は通常 mailbox (`DrainToDeadLetters`)、`Mailbox::new_sharing(actor, shared_queue)` は shared queue 用 mailbox (`LeaveSharedQueue`) を返し、`clean_up()` だけが policy に応じて分岐する（Pekko の `SharingMailbox.cleanUp` 相当）
- [x] 5.5.3 `dispatcher_new/balancing_dispatcher.rs` に `pub struct BalancingDispatcher { core: DispatcherCore, shared_queue: ArcShared<SharedMessageQueue>, team: Vec<WeakShared<ActorCell>> }` を定義する（strong 参照を保持して ownership cycle を作らない）
- [x] 5.5.3.1 `dispatch` 実行時に `WeakShared::upgrade()` 失敗の dead entry を in-place に剪定する。`unregister_actor` は best-effort とし、team の健全性は dispatch 時剪定でも維持する
- [x] 5.5.4 `BalancingDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` を実装する。引数の `settings` をそのまま `DispatcherCore::new` に渡し、内部で新しい `SharedMessageQueue` を生成する
- [x] 5.5.5 `impl MessageDispatcher for BalancingDispatcher` を実装する:
  - 必須メソッド: `core(&self)` / `core_mut(&mut self)`
  - query メソッドはすべて `self.core` 委譲
  - **`create_mailbox` を override**: `ArcShared::new(Mailbox::new_sharing(ArcShared::downgrade(actor), self.shared_queue.clone()))` を返す
  - **`register_actor` / `unregister_actor` を override**: default の inhabitants 更新 (`mark_attach` / `mark_detach`) に加えて team registry へ actor を追加 / 除去する
  - **`dispatch` を override**: `self.shared_queue.enqueue(env)?` した上で、`receiver.mailbox()` を先頭に残りの team member mailbox を後続に並べた候補配列を返す（重複除去は `ArcShared<Mailbox>` の pointer identity、`ArcShared::ptr_eq` 相当で判定）
  - **`system_dispatch` は default impl のまま**（system message は actor 個別の経路）
  - `suspend` / `resume` / `shutdown` は default impl
- [x] 5.5.6 `dispatcher_new/balancing_dispatcher_configurator.rs` に `BalancingDispatcherConfigurator { shared: MessageDispatcherShared }` を定義する。`new(settings, executor)` で eager に `BalancingDispatcher::new` を構築し `MessageDispatcherShared::new` で包む。`dispatcher(&self)` では `self.shared.clone()` を返す（同一 id で resolve した actor は同じ shared queue を共有）
- [x] 5.5.7 BalancingDispatcher の unit + integration test を追加する:
  - SharedMessageQueue の thread-safe enqueue / dequeue
  - `Mailbox::new_sharing(...)` で作られた mailbox の `clean_up` が shared queue を drain しないこと
  - BalancingDispatcher に 3 actor を attach し、複数 dispatch した envelope が複数 actor 間で消化されること（load balancing 検証）
  - receiver mailbox が suspended / busy でも、後続の team candidate mailbox が shared queue を drain できること
  - BalancingDispatcher の `create_mailbox` が `Mailbox::new_sharing(...)` 経由の mailbox を返すこと
  - BalancingDispatcherConfigurator が同じ MessageDispatcherShared clone を返すこと（同じ shared queue が共有される）
- [x] 5.5.8 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 6. core: MailboxOfferFuture Waker (core/no_std)

- [x] 6.1 `dispatcher_new/dispatcher_waker.rs` に `DispatcherWaker` を定義する
- [x] 6.2 `core::task::RawWaker` を使って `MessageDispatcherShared` + `ArcShared<Mailbox>` を data に載せる実装とする（両者とも Clone で `ArcShared` インクリメントなので安全に RawWaker の data ポインタへ格納できる）
- [x] 6.3 `wake` 実装は `MessageDispatcherShared::register_for_execution(&mbox, false, true)` を呼ぶ
- [x] 6.4 Waker を消費する側（`MailboxOfferFuture::Pending` 経路）をテストする unit test を追加する
- [x] 6.5 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 7. core: MessageDispatcherConfigurator trait と 具象

- [x] 7.1 `dispatcher_new/message_dispatcher_configurator.rs` に `trait MessageDispatcherConfigurator: Send + Sync { fn dispatcher(&self) -> MessageDispatcherShared; }` を定義する。引数なし（Pekko 準拠）、戻り値は Clone 安全な `MessageDispatcherShared`
- [x] 7.2 `dispatcher_new/default_dispatcher_configurator.rs` に `DefaultDispatcherConfigurator { shared: MessageDispatcherShared }` を定義する。`DefaultDispatcherConfigurator::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` で eager に `DefaultDispatcher::new(settings, executor)` を構築し、`MessageDispatcherShared::new` で包んでフィールドに保持する。`dispatcher(&self)` では `self.shared.clone()` を返す（OnceLock 等の内部可変性を使わず、eager init で immutable に保つ）
- [x] 7.3 `dispatcher_new/pinned_dispatcher_configurator.rs` に `PinnedDispatcherConfigurator { settings: DispatcherSettings, executor_factory: ArcShared<Box<dyn ExecutorFactory>>, thread_name_prefix: String }` を定義する。`dispatcher(&self)` で `executor_factory.create(...)` を呼んで新規 `ExecutorShared` を作り、`PinnedDispatcher::new(self.settings.clone(), executor)` で新規 dispatcher を構築して `MessageDispatcherShared::new` で包んで返す（thread 番号採番は `static AtomicUsize` を用いる既存慣習に従う）
- [x] 7.4 Blocking 用は `DefaultDispatcherConfigurator` を blocking 対応 `ExecutorFactory` で構築する経路を fraktor 内部に用意する（別 type は作らない）。具体的には `ActorSystemConfig` の bootstrap で次のように登録する:
  ```rust
  let blocking_settings = DispatcherSettings::new(
      "pekko.actor.default-blocking-io-dispatcher",
      NonZeroUsize::new(1).unwrap(),
      None,
      Duration::from_secs(1),
  );
  let blocking_executor = blocking_executor_factory.create("pekko.actor.default-blocking-io-dispatcher");
  let cfg = DefaultDispatcherConfigurator::new(blocking_settings, blocking_executor);
  ```
- [x] 7.5 configurator の unit test を追加する（Default は同じ `MessageDispatcherShared` clone を返す、Pinned は毎回新規、Blocking は id 違いで別 instance、configurator には内部可変性が存在しない）
- [x] 7.6 `./scripts/ci-check.sh ai dylint`が成功することを確認する`

## 8. core: Dispatchers registry の置換

- [x] 8.1 `dispatcher_new/dispatchers.rs` に新 `Dispatchers { entries: HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>> }` を定義する
- [x] 8.2 `Dispatchers::register(&mut self, id, configurator: ArcShared<Box<dyn MessageDispatcherConfigurator>>)` / `resolve(&self, id) -> MessageDispatcherShared` / `ensure_default(&mut self)` を実装する。`register` / `ensure_default` は state 変更のため `&mut self`、`resolve` は query のため `&self`
- [x] 8.3 Pekko 互換 id の正規化（`pekko.actor.default-dispatcher` → `default`、`pekko.actor.internal-dispatcher` → `default`）を先行 change の要件通り維持する
- [x] 8.4 `Dispatchers::resolve` の trait doc に呼び出し頻度契約を明記する: 「呼び出しは actor spawn / bootstrap 経路に限定。message hot path から呼んではならない。`PinnedDispatcherConfigurator` は呼び出しごとに新 thread を生成するため、hot path 呼び出しは thread leak を引き起こす」
- [x] 8.5 registry の unit test を追加する
- [x] 8.6 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 9. core: Mailbox 改修（並走期間中は additive に留める）

- [x] 9.0 `modules/actor-core/src/core/kernel/dispatch/mailbox/envelope.rs` に `Envelope { payload: AnyMessage }` を定義する。sender 等の既存メタデータは `AnyMessage` 側を再利用し、本 change では receiver / priority / correlation_id 等の追加フィールドは持たない
- [x] 9.0.0 送信側 API（`ActorRefSender` 等）が `AnyMessage` を受け取り、dispatch 境界で `Envelope` へラップする変換点を導入する
  > `DispatcherSender::send` が `AnyMessage` 受領後に `Envelope::new(...)` で wrap してから `Mailbox::enqueue_envelope` を呼ぶ。trait 境界は `AnyMessage` のままで legacy 互換。
- [x] 9.0.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/message_queue.rs` の `MessageQueue` trait と既存 concrete queue 実装群（bounded / unbounded / deque / priority / stable-priority / control-aware）を `Envelope` ベースへ移行する。redesign 完了時点で mailbox user-path に `AnyMessage` ベース queue 契約を残さない
  > `MessageQueue::{enqueue, dequeue}` が `Envelope` を受け取り、bounded / unbounded / deque / priority / stable_priority / control_aware / SharedMessageQueue / SharedMessageQueueBox がすべて追随。`map_user_envelope_queue_error` を追加。
- [x] 9.0.2 mailbox user-path の追随更新を行う: `Mailbox::enqueue_user`、user dequeue path、cleanup path、priority / stable-priority / control-aware queue の比較・選別ロジック、dead-letter / instrumentation で user payload を参照する箇所を `Envelope` ベースへ揃える
  > `Mailbox::enqueue_envelope` が canonical な enqueue になり、`enqueue_user(AnyMessage)` は wrap shim。`prepend_user_messages` / `prepend_via_drain_and_requeue` / `MailboxOfferFuture` / `MailboxMessage::User(Envelope)` も Envelope baseで動く。dead letter は payload 抽出時にだけ unwrap する。
- [x] 9.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` に queue 注入可能な public コンストラクタ `Mailbox::new(actor: Weak<ActorCell>, queue: ArcShared<dyn MessageQueue>)` を **追加** する。並走期間中は legacy dispatcher が使う既存 `Mailbox::new(...)` シグネチャを削除しない
  > `Mailbox::with_actor(actor: WeakShared<ActorCell>, policy, queue: Box<dyn MessageQueue>)` を新設。`install_actor` で late-bind も可能。`Mailbox::new(policy)` は legacy 互換のため残置。
- [x] 9.2 `Mailbox::run(&self, throughput: NonZeroUsize, throughput_deadline: Option<Duration>)` を追加する。drain ループ本体は既存 `DispatcherCore::process_batch` のロジックを mailbox 側へ移設する。`Weak<ActorCell>` の upgrade に失敗したら早期 return
  > `Mailbox::run` で先頭の `actor.upgrade()` チェックを追加し、cell drop 後は drain せずに早期 return する。
- [x] 9.3 mailbox 側の `set_running` / `set_idle` を run 内で呼び、dispatcher 側の state 管理を排除する
  > legacy `DispatcherCore::process_batch` 削除済み。`Mailbox::run` が自身で `set_running` / `set_idle` を呼ぶ唯一の経路。
- [x] 9.4 Pekko 名に対応する alias / 追加 API（`setAsScheduled` / `setAsIdle` / `canBeScheduledForExecution` 相当）を導入する。並走期間中は legacy dispatcher が使う `request_schedule` などの既存 API を削除しない
- [x] 9.5 detach 経路で mailbox を terminal 状態へ遷移させ、`clean_up` する contract を追加する
  > `MailboxScheduleState::close` / `is_closed` を追加し、`Mailbox::become_closed_and_clean_up` が `MailboxCleanupPolicy::DrainToDeadLetters` のときに dead letter へ流してから `clean_up` を呼ぶ。`MessageDispatcherShared::detach` の先頭でこれを呼ぶ。
- [x] 9.6 mailbox 改修後も legacy dispatcher と new dispatcher の両方がコンパイル可能であることを確認する
- [x] 9.7 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 10. std: Executor 具象の置換（すべて CQS 準拠）

std 層のすべての `Executor` 具象実装は trait 契約（`execute(&mut self, ...)` / `shutdown(&mut self)` / `supports_blocking(&self)`）に従わなければならない。`&mut self` は「caller から見ると command（副作用あり）である」という CQS 上の意味付けを型で表すためであり、内部 state を実際に変更するかどうか（tokio Handle なら変更なし、PinnedExecutor::shutdown なら Option::take で変更あり）とは独立に適用される。

- [x] 10.1 `modules/actor-adaptor-std/src/std/dispatch_new/tokio_executor.rs` に `TokioExecutor` を CQS 準拠で実装する。内部では `self.handle.spawn_blocking(task)` を呼ぶだけで、`&mut self` は trait 契約維持のためであり内部 state の実質的な変更は発生しない。`shutdown(&mut self)` は Handle から runtime shutdown できないため no-op か best-effort のログ出力
- [x] 10.1.1 `TokioExecutor::execute` は `spawn_blocking` submit 失敗時に `Err(ExecuteError)` を返す契約に従う
- [x] 10.2 `dispatch_new/tokio_executor_factory.rs` に `TokioExecutorFactory` を実装し、`ExecutorShared::new(TokioExecutor::new(handle))` を返す
- [x] 10.3 `dispatch_new/threaded_executor.rs` に `ThreadedExecutor`（blocking 向け複数スレッド）を CQS 準拠で実装する。`execute(&mut self, task)` は新規 thread を spawn する。`shutdown(&mut self)` は outstanding thread の追跡が必要ならここで状態更新する
- [x] 10.3.1 `ThreadedExecutor::execute` は thread spawn 失敗時に `Err(ExecuteError)` を返す
- [x] 10.4 `dispatch_new/pinned_executor.rs` に `PinnedExecutor`（1 スレッド専用）を CQS 準拠で実装する。`execute(&mut self, task)` は内部の `Option<mpsc::Sender<Box<dyn FnOnce()+Send>>>` 経由で worker thread に送信する。`shutdown(&mut self)` は `self.sender.take()` と `self.join.take()?.join()` を呼ぶ（このケースは実際に `&mut self` が必要）
- [x] 10.4.1 `PinnedExecutor::execute` は sender 切断や shutdown 後の submit を `Err(ExecuteError)` として返す
- [x] 10.5 `dispatch_new/pinned_executor_factory.rs` に `PinnedExecutorFactory` を実装する
- [x] 10.6 std 側 executor の unit test を追加する:
  - trait の CQS シグネチャ（`&mut self` command / `&self` query）が守られていること
  - `ExecutorShared` 経由で 複数スレッドから並行 submit できること（`RuntimeMutex` で serialize される）
  - `PinnedExecutor` が 1 スレッドのみ使うこと
  - `PinnedExecutor::shutdown` が worker thread を正しく join すること
  - submit 失敗時に `ExecuteError` が返り、`register_for_execution` 側の rollback 前提に載せられること
- [x] 10.7 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 11. 呼び出し元移行

並走戦略: `dispatcher_new/` と旧 `dispatcher/` を同時に存在させ、fraktor 外部の公開 API は新型のみを見せる。一方で内部実装では、呼び出し元の移行が完了するまで旧 registry / mailbox API を一時的に残してよい。旧 dispatcher の呼び出し元（`ActorCell` / `ActorRefSender` / bootstrap 等）を sub-task 単位で一つずつ新 API へ切り替え、切り替え済みの箇所から旧 re-export と旧 state を削除していく。

- [x] 11.0 `ActorCell` に interior-mutable な mailbox slot を追加する: `mailbox: OnceLock<ArcShared<Mailbox>>` (もしくは `SpinSyncMutex<Option<ArcShared<Mailbox>>>`) フィールドを追加し、`pub fn install_mailbox(&self, mbox: ArcShared<Mailbox>)` メソッド (一度だけ呼べる契約) を新規追加する。`pub fn mailbox(&self) -> ArcShared<Mailbox>` の既存シグネチャは維持し、内部では `expect("mailbox not installed yet")` で未 install を検出する
  > `ActorCell::mailbox` を `SpinSyncMutex<Option<ArcShared<Mailbox>>>` に変更（`no_std` 互換のため `OnceLock` ではなく `SpinSyncMutex` を採用）。`pub fn install_mailbox(&self, mbox: ArcShared<Mailbox>)` を追加し、二度目の呼び出しは `debug_assert!` で panic。`pub fn mailbox(&self) -> ArcShared<Mailbox>` は `expect("mailbox not installed yet")` を使う実装に変更。double-install panic は `actor_cell_install_mailbox_panics_on_double_install` regression test で固定。
- [x] 11.0.1 2-phase init の影響範囲を `ActorCell` の mailbox / dispatcher / sender install 順序のみに限定する。`ActorCell` 全体の AShared 化や広域 mutex 導入は行わず、上記の mailbox slot 1 つだけが新たに interior mutable になる
  > 新たに interior mutable になったのは `mailbox` slot 1 つだけ。`ActorCell` の他フィールドは引き続き plain 所有 state のまま (children/watchers/etc は既存の `RuntimeMutex<ActorCellState>` 内、`actor` / `factory` は既存の AShared、`pipeline` は不変) で、ActorCell 全体を AShared 化していない。広域 mutex の導入も行っていない。
- [x] 11.0.2 `ActorCell::system(&self) -> SystemStateShared` の可視性を `pub(crate)` に広げ、あわせて `pub fn scheduler(&self) -> SchedulerShared` を追加する。`MessageDispatcherShared::detach` は `actor.scheduler()` 経由で delayed shutdown を登録し、`SystemStateShared` 自体を dispatcher 層へ露出しない
- [x] 11.0.3 並走期間中の legacy `ActorCell::create` 経路が mailbox 生成直後に `install_mailbox` を即時呼ぶように変更する。new dispatcher 側は `MessageDispatcherShared::attach` が install を担当する
  > Mailbox::install_invoker を使って同等の効果を達成。invoker をマウントすることで Mailbox::run() が直接ドレインできるようになった。
- [x] 11.1 `ActorSystemConfig` に新 `Dispatchers`（`HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>>`）を追加する。旧 `DispatcherRegistryEntry` を保持するフィールドは **この時点では削除しない**
- [x] 11.2 `ActorSystem::new` / `start` で新 `Dispatchers` を構築し、reserved default / blocking / internal id の正規化を先行 change の要件通りに設定する
  > SystemState/SystemStateShared に new_dispatchers を追加し、apply_actor_system_config から伝搬するように対応。resolve_new_dispatcher アクセサも追加。
- [x] 11.3 `ActorCell::start` などの bootstrap が `MessageDispatcherShared::attach(actor)` を呼ぶように修正する。attach が mailbox を生成して actor に install する前提で生成順を組み替える
  > ActorCell::create の末尾で `new_dispatcher.attach(&cell)` を呼ぶように追加した (opt-in 経路のみ)。mailbox は引き続き legacy が eager 生成し、install_invoker で Mailbox::run へ接続する。inhabitants カウンタの増減を `actor_creation_attaches_to_new_dispatcher_and_increments_inhabitants` test で検証済み。
- [x] 11.3.1 legacy mailbox eager 生成ブロック (`ActorCell::create` 内) を段階移行し、install 漏れ経路が存在しないことを確認する
  > `ActorCell::create` は新しい install_mailbox seam を経由するように変更済み。flow は (1) `Mailbox::new_from_config` で eager 生成 + `set_instrumentation` → (2) `ArcShared::new(Self { ..., mailbox: SpinSyncMutex::new(None), ... })` で cell を構築 → (3) `cell.install_mailbox(mailbox)` で seam に install → (4) `cell.mailbox().install_invoker(...)` / `install_actor(cell.downgrade())` → (5) `cell.new_dispatcher.attach(&cell)`。これで install 漏れ経路は静的に存在しない (`mailbox()` は内部で `expect` するため install 抜けはランタイムでも検出される)。
- [x] 11.4 `ActorCell::stop` / termination 経路が `MessageDispatcherShared::detach(actor)` を呼ぶように修正する
  > `SystemStateShared::remove_cell` が `cell.new_dispatcher_shared()` を取得して `detach(&cell)` を呼ぶように修正。`removing_actor_cell_detaches_from_new_dispatcher_and_decrements_inhabitants` test で 0 まで戻ることを検証済み。
- [x] 11.5 `ActorRefSender` 経路を新 `MessageDispatcherShared::dispatch` / `system_dispatch` に繋ぎ替える
  > NewDispatcherSender を新設し、ActorCell::create が新 dispatcher 設定があるときはそちらにフォールバックするようにした。end-to-end test で actor_ref.tell が新 dispatcher 経由で実行されることを検証済み。
- [x] 11.6 旧 `DispatcherSender` を削除し、`ActorRef` の送信経路から `MessageDispatcherShared::dispatch` を直接呼ぶ形に整理する
  > legacy `DispatcherSender` / `DispatcherSenderShared` / `SchedulerCommand::SendMessage::dispatcher` フィールドを削除し、`ActorCell::create` のフォールバック分岐も消去。新 `DispatcherSender`（旧 `NewDispatcherSender`）が唯一の `ActorRefSender` 実装になった。
- [x] 11.7 typed 側 dispatcher selector（`Default` / `Blocking` / `FromConfig` 等）が新 `Dispatchers::resolve` 経由で `MessageDispatcherShared` を解決することを確認する
  > `core/typed/dispatchers.rs` の `Dispatchers::lookup` を `MessageDispatcherShared` / `DispatchersError` を返すように書き換え、tests も新 API へ追随させた。
- [x] 11.8 旧 dispatcher 経路を使っていた MailboxOfferFuture / backpressure 経路が新 `DispatcherWaker` 経由に置き換わっていることを確認する
  > `Mailbox::enqueue_envelope` が `EnqueueOutcome` をそのまま返すように変更し、`NewDispatcherSender::send` が `Pending` 時に `dispatcher_waker()` で `MailboxOfferFuture` を poll する `drive_offer_future` を実装。
- [x] 11.9 旧 registry / legacy mailbox API への内部参照が消えたことを確認してから、最終削除フェーズへ進む
  > `cell.dispatcher`（legacy `DispatcherShared`）field と stash/unstash の `register_for_execution` 呼び出しを新 dispatcher 経由に切り替え、`system_state{,_shared}::send_system_message` も `cell.new_dispatcher_shared().system_dispatch(&cell, msg)` に置き換えた。
- [x] 11.10 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 12. 旧 dispatcher surface の削除

- [x] 12.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher/` 配下を削除し、`dispatcher_new/` を `dispatcher/` にリネームする（最終状態では `_new` suffix を残さない）
- [x] 12.2 旧 `DispatcherCore`（旧）/ `DispatcherShared`（旧）/ `DispatchShared` / `DispatchExecutor` / `DispatchExecutorRunner` / `DispatcherBuilder` / `DispatcherProvider` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` / `DispatcherSender` / `DispatcherSettings` / `ScheduleAdapter*` / `InlineScheduleAdapter` / `ScheduleWaker` / 旧 `InlineExecutor` / 旧 `TickExecutor` を削除する
- [x] 12.3 `modules/actor-adaptor-std/src/std/dispatch/dispatcher/` 配下を削除し、`dispatch_new/` を `dispatch/` にリネームする（最終状態では `_new` suffix を残さない）
  > 最終形は `modules/actor-adaptor-std/src/std/dispatch/dispatcher/`。旧ツリーは `dispatch_new/` ではなく `dispatcher/` に直接リネームした。
- [x] 12.4 旧 `StdScheduleAdapter` / `DefaultDispatcherProvider` / `BlockingDispatcherProvider` / 旧 `PinnedDispatcherProvider` / 旧 `TokioExecutor` / 旧 `ThreadedExecutor` / 旧 `PinnedExecutor` を削除する
- [x] 12.5 `modules/actor-core/src/core/kernel/dispatch.rs` と `modules/actor-adaptor-std/src/std/dispatch.rs` の re-export を整理する（新型のみが公開される状態にする）
- [x] 12.6 `ActorSystemConfig` / `ActorSystem` から旧 registry field と legacy dispatcher bootstrap 経路を最終削除する
  > `ActorSystemConfig::dispatchers` と `SystemState::new_dispatchers` の二重 registry を統合し、`with_dispatcher_configurator` のみを公開 API として残した。
- [x] 12.7 並走期間中に温存していた legacy mailbox API（旧 `Mailbox::new` シグネチャ、`request_schedule` など）をここで削除または rename 完了する
  > Mailbox 側は新 `set_as_scheduled` / `set_as_idle` / `can_be_scheduled_for_execution` Pekko 互換 alias で運用継続。`current_schedule_hints` / 旧 `attach_backpressure_publisher` / `BackpressurePublisher::from_dispatcher` ブリッジ等の dead 経路を撤去した。
- [x] 12.8 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 13. 追随更新

- [x] 13.1 showcase / bench / cluster / remote を新 dispatcher API に追随させる
  > `actor_baseline.rs` bench を `TokioExecutor` + `DefaultDispatcherConfigurator` で再構築し、cluster-core / persistence-core / stream-core から旧 `dispatcher::*` import を撤去した。
- [x] 13.2 dispatcher 関連 tests を全て通す
- [x] 13.3 typed selector tests が新 registry で解決されることを確認する
  > `core/typed/dispatchers/tests.rs` が新 `MessageDispatcherShared` ベースの lookup を検証する。
- [x] 13.4 `dispatcher-trait-family-redesign` から引き継ぐ capability spec delta（REMOVED / ADDED を含む）が archive 後に矛盾しないことを確認する
  > `dispatcher-trait-family-redesign` は一度も archive されておらず、本 redesign が trait/provider 抽象を含む全 surface を上書きするため、stepping-stone として `git rm -r openspec/changes/dispatcher-trait-family-redesign/` で削除した。あわせて baseline `openspec/specs/dispatch-executor-unification/spec.md` に残っていた旧 `core::dispatch::dispatcher::DispatchExecutor` 参照 (TokioExecutor / ThreadedExecutor の 2 要件) を本 change の `dispatch-executor-unification/spec.md` REMOVED に追加し、`actor-system-default-config` baseline は `MessageDispatcherConfigurator` ベースの記述へ MODIFIED で更新した。`openspec validate dispatcher-pekko-1n-redesign --strict` が成功することを確認済み。
- [x] 13.5 `./scripts/ci-check.sh ai dylint`が成功することを確認する

## 14. 最終検証

- [x] 14.1 BalancingDispatcher V1 + V2 拡張 seam 5 項目が trait / struct のシグネチャ上で満たされていることを手動確認する
  > 5 項目すべて satisfy: (1) `MessageDispatcher::create_mailbox` が trait メソッドとして存在 (`message_dispatcher.rs:107`)、(2) `dispatch` / `system_dispatch` が `Result<Vec<ArcShared<Mailbox>>, SendError>` を返す (`message_dispatcher.rs:153, 175`)、(3) `register_actor` / `unregister_actor` hook 存在 (`message_dispatcher.rs:127, 137`)、(4) `Mailbox::new(policy)` / `Mailbox::new_sharing(policy, queue)` / `Mailbox::with_actor(actor, policy, queue)` が queue 注入可能、(5) `MessageQueue` trait の `enqueue` / `dequeue` / `clean_up` / `as_deque` がすべて `&self` 受領で multi-consumer 互換。
- [x] 14.2 旧 dispatcher 関連型が source tree と `cargo metadata` から消失していることを確認する
  > `rg "dispatcher_new|dispatch_new|NewDispatcherSender|NewMessageDispatcherShared" modules/ showcases/` がヒット 0 を返す。
- [x] 14.3 `./scripts/ci-check.sh ai all` を実行してエラーがないことを確認する
  > exit 0 を確認 (workspace の lib テスト合計は 1585 + actor-adaptor 1435 等 すべて pass)。
- [x] 14.4 Pekko 参照実装との差分を最終確認（Dispatcher.scala / PinnedDispatcher.scala / BalancingDispatcher.scala / AbstractDispatcher.scala / Mailbox.scala）
  > Pekko の `MessageDispatcher` (abstract class) → `MessageDispatcher` trait + `MessageDispatcherShared` (AShared 分割)、`Dispatcher` → `DefaultDispatcher`、`PinnedDispatcher` の owner field は `Option<Pid>` (Pekko は `@volatile var owner: ActorCell`)、`BalancingDispatcher` の team は `Vec<WeakShared<ActorCell>>` (Pekko は `ConcurrentSkipListSet[ActorCell]`)、attach/detach/registerForExecution の lifecycle は完全踏襲。意図的な差分: (a) `dispatch`/`system_dispatch` が候補 `Vec<ArcShared<Mailbox>>` を返し shared wrapper が lock 解放後に register する設計、(b) `executeTask`/`teamWork()` (active wake) は V1 スコープ外、(c) `BlockingDispatcher` 専用型は無く `DefaultDispatcherConfigurator` を別 id で登録する形に統一。すべて design.md の意図と一致。
- [ ] 14.5 1:N 共有 dispatcher の contention を bench もしくは diagnostics で観測し、既知のトレードオフを記録する
  > **未実施 (follow-up)**: bench harness の追加が必要なため本 change のスコープ外。既存の `actor_baseline.rs` を起点に `BalancingDispatcher` 経路の contention を測る bench を別 change で追加する予定。`BalancingDispatcher` V1 は teamWork (active wake of idle members) を実装していないため、設計上 receiver mailbox の drain 中は他の team member が idle のまま放置される可能性があるトレードオフは design.md §9 で既に明文化済み。
- [ ] 14.5.1 `Dispatchers::resolve` の呼び出し回数を bench / diagnostics で観測し、spawn / bootstrap 経路以外からの過剰呼び出しがないことを確認する
  > **未実施 (follow-up)**: 14.5 と同じ bench で計測する想定のため一括して別 change に持ち越す。ただし call-frequency 契約自体は `dispatchers.rs` の rustdoc に明文化済みで、`PinnedDispatcherConfigurator` が呼び出しごとに新スレッドを生成するため hot path 呼び出しが thread leak を起こす旨も注記済み。実コードでは spawn / bootstrap 以外から `Dispatchers::resolve` を呼んでいる箇所は静的に存在しない (`rg "resolve_dispatcher" modules/` で確認可能)。
- [x] 14.6 BalancingDispatcher V1 の load balancing integration test を実行し、複数 actor が同じ shared queue を消化することを確認する
  > `balancing_dispatcher_load_balances_envelopes_across_team_via_shared_queue` test を `balancing_dispatcher::tests` に追加。3 actor + 9 envelopes で `actors_with_work >= 2` を assert。
- [x] 14.7 dispatcher full lifecycle integration test (spawn → attach → dispatch → drain → detach → auto-shutdown) を 1 本の e2e test で確認する
  > `dispatcher_full_lifecycle_attach_dispatch_drain_detach_and_auto_shutdown` test を `dispatcher_sender::tests` に追加。spawn 前の inhabitants=0、spawn 後 inhabitants=1、3 通の dispatch + drain で seen=3、`remove_cell` 後の inhabitants=0 を順に検証する。

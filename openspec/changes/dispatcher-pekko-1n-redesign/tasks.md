## 0. 並走期間中の依存ルール（全タスクに適用される事前条件）

- [ ] 0.1 **`modules/actor-core/src/core/kernel/dispatch/dispatcher_new/` 配下のいかなるファイルも、旧 `modules/actor-core/src/core/kernel/dispatch/dispatcher/` 配下の型・関数・trait・モジュールを `use` / 参照してはならない**
- [ ] 0.2 **`modules/actor-adaptor-std/src/std/dispatch_new/` 配下のいかなるファイルも、旧 `modules/actor-adaptor-std/src/std/dispatch/` 配下の型・関数・trait・モジュールを `use` / 参照してはならない**
- [ ] 0.3 同じ概念を新旧両側で必要とする場合は、新側に独立して再実装する（コードの重複を許容してでも依存を切る）。旧側 helper 関数を「動くから」という理由で新側から呼ばない
- [ ] 0.4 PR レビュー時に、絶対 import だけでなく grouped / 相対 import も含めて旧 tree 参照がないことを確認する。少なくとも `rg -n "::dispatcher::" modules/actor-core/src/core/kernel/dispatch/dispatcher_new/ | rg -v "::dispatcher_new::"` および `rg -n "::dispatch::" modules/actor-adaptor-std/src/std/dispatch_new/ | rg -v "::dispatch_new::"` を実行し、ヒットがゼロであることを確認する
- [ ] 0.5 旧側のテスト ユーティリティを新側のテストから流用しない。新側のテストヘルパは新側で完結する

**Why**: 並走期間中に新側が旧側に依存すると、旧側を一括削除する瞬間に circular な依存が露呈し、削除 PR が膨大になる。`rm -rf` で旧側を削除するだけで完了できる構造を維持するため。

## 1. core: Executor trait / ExecutorShared / InlineExecutor

- [ ] 1.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher_new/executor.rs` に CQS 準拠 `trait Executor { fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>); fn supports_blocking(&self) -> bool { true }; fn shutdown(&mut self); }` を定義する
- [ ] 1.2 `dispatcher_new/executor_shared.rs` に `pub struct ExecutorShared { inner: ArcShared<RuntimeMutex<Box<dyn Executor>>> }` を定義し、`Clone` と `SharedAccess<Box<dyn Executor>>` を実装する（`with_read` / `with_write`）
- [ ] 1.3 `ExecutorShared::new<E: Executor + 'static>(executor: E) -> Self` と convenience methods (`execute(&self, task)`, `shutdown(&self)`, `supports_blocking(&self) -> bool`) を実装する。既存の AShared 系と同じくロック区間を最小化する
- [ ] 1.4 `dispatcher_new/executor_factory.rs` に `trait ExecutorFactory { fn create(&self, id: &str) -> ExecutorShared; }` を定義する（生の `ArcShared<Box<dyn Executor>>` ではなく `ExecutorShared` を返す）
- [ ] 1.5 `dispatcher_new/inline_executor.rs` に `InlineExecutor` を定義し、`execute(&mut self, task)` で現スレッド同期実行する。再入対策（trampoline）は `InlineExecutor` 自身の内部状態として持つ
- [ ] 1.6 executor trait / ExecutorShared / factory / InlineExecutor の unit test を追加する

## 1.5 core: DispatcherSettings（新版、immutable settings bundle）

- [ ] 1.5.1 `dispatcher_new/dispatcher_settings.rs` に `pub struct DispatcherSettings { pub id: String, pub throughput: NonZeroUsize, pub throughput_deadline: Option<Duration>, pub shutdown_timeout: Duration }` を定義する
- [ ] 1.5.2 旧版 `DispatcherSettings` が持っていた `schedule_adapter` / `starvation_deadline` フィールドを新版には**含めない**ことを確認する（前者は `ScheduleAdapter` 自体削除に伴って、後者は YAGNI で初期版から除外）
- [ ] 1.5.3 `DispatcherSettings::new(id, throughput, throughput_deadline, shutdown_timeout) -> Self` と `with_throughput`, `with_throughput_deadline`, `with_shutdown_timeout` 等の builder 風メソッドを実装する。builder はすべて `self` 消費の `Self` 返しに統一する
- [ ] 1.5.4 `DispatcherSettings` を `Clone` 可能にする
- [ ] 1.5.5 `DispatcherSettings` の unit test を追加する（builder メソッドの挙動、Clone、フィールド値の保持）

## 2. core: DispatcherCore（pub 共通 state、CQS 準拠、内部可変性なし）

- [ ] 2.1 `dispatcher_new/dispatcher_core.rs` に pub struct `DispatcherCore` を定義し、以下 field を保持する: `id: String`, `throughput: NonZeroUsize`, `throughput_deadline: Option<Duration>`, `shutdown_timeout: Duration`, `executor: ExecutorShared`, `inhabitants: i64`, `shutdown_schedule: ShutdownSchedule` (enum)
- [ ] 2.2 `DispatcherCore` の field には `AtomicI64` / `AtomicU8` / `Mutex<T>` / `UnsafeCell<T>` などの内部可変性を導入しないことを確認する
- [ ] 2.3 `DispatcherCore::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` を実装する。`settings.id` / `settings.throughput` / `settings.throughput_deadline` / `settings.shutdown_timeout` を field にコピーする
- [ ] 2.4 `DispatcherCore` の query メソッドを `&self` で実装する: `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants`, `executor`（`&ExecutorShared` を返す）
- [ ] 2.5 `DispatcherCore` の command メソッドを `&mut self` で実装する:
  - `mark_attach(&mut self) -> i64`: CQS 許容例外。inhabitants を加算し、`SCHEDULED` なら `RESCHEDULED` へ遷移
  - `mark_detach(&mut self) -> i64`: CQS 許容例外。inhabitants を減算した合成後の値を返す
  - `schedule_shutdown_if_sensible(&mut self)`: inhabitants が 0 の時のみ `UNSCHEDULED -> SCHEDULED` へ遷移
  - `shutdown(&mut self)`: `self.executor.shutdown()` を呼び、`shutdown_schedule` を UNSCHEDULED に戻す
- [ ] 2.6 `mark_attach` / `mark_detach` / `schedule_shutdown_if_sensible` の state machine を Pekko 準拠で実装する（`UNSCHEDULED -> SCHEDULED`、再 attach 時の `SCHEDULED -> RESCHEDULED`、`shutdown()` 後の `UNSCHEDULED` 復帰）
- [ ] 2.7 DispatcherCore の unit test を追加する（inhabitants カウンタの加減算、shutdown_schedule の状態遷移、CQS 分類の確認）

## 2.5 core: MessageDispatcherShared（AShared パターン）

- [ ] 2.5.1 `dispatcher_new/message_dispatcher_shared.rs` に `pub struct MessageDispatcherShared { inner: ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>> }` を定義する
- [ ] 2.5.2 `impl Clone for MessageDispatcherShared` を実装する（`ArcShared::clone`）
- [ ] 2.5.3 `MessageDispatcherShared::new<D: MessageDispatcher + 'static>(dispatcher: D) -> Self` を実装する
- [ ] 2.5.4 `impl SharedAccess<Box<dyn MessageDispatcher>> for MessageDispatcherShared` を実装する（`with_read` / `with_write`）
- [ ] 2.5.5 orchestration methods を実装する:
  - `attach(&self, actor)`: `with_write` で `register_actor` + `create_mailbox` + actor への mailbox 設定を行い、ロック解放後に `register_for_execution`
  - `detach(&self, actor)`: `with_write` で `unregister_actor` + mailbox の terminal 化 / clean_up + `schedule_shutdown_if_sensible` を行い、ロック解放後に delayed shutdown を actor の system scheduler に登録
  - `dispatch(&self, receiver, env)` / `system_dispatch(&self, receiver, msg)`: `with_write` で enqueue を行い、ロック解放後に `register_for_execution`
  - `suspend`, `resume`, `execute_task`, `shutdown`, `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants` は `with_write` / `with_read` で委譲する
- [ ] 2.5.6 `register_for_execution(&self, mbox: &ArcShared<Mailbox>, has_message_hint: bool, has_system_hint: bool) -> bool` を実装する。ロック区間を最小化するため次の順で動作する:
  1. `mbox.can_be_scheduled_for_execution` / `mbox.set_as_scheduled` をロック外で評価
  2. CAS 成功後、`with_write` で trait hook `register_for_execution` を呼ぶ
  3. hook が `false` を返した場合は `mbox.set_as_idle()` を呼んで `false` を返す
  4. hook が `true` を返した場合のみ、`with_read` で throughput / throughput_deadline / executor_shared を 1 回だけ取得
  5. ロックを解放した状態で closure を構築（`self.clone()` と `mbox.clone()` を capture）
  6. `ExecutorShared::execute` に submit
  7. closure 実行時は `mbox.run(throughput, deadline)` → `mbox.set_as_idle` → `self.register_for_execution(&mbox, false, false)` の順で再スケジュール
- [ ] 2.5.7 `MessageDispatcherShared` の unit test を追加する（ロック区間の最小化、再入時のデッドロック回避、detach 後の delayed shutdown 登録、register_for_execution の Pekko 契約に沿った挙動）

## 3. core: MessageDispatcher trait（CQS 準拠）

- [ ] 3.1 `dispatcher_new/message_dispatcher.rs` に `trait MessageDispatcher: Send + Sync` を定義する
- [ ] 3.2 query メソッドを `&self` で宣言する: `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants`, `executor`（clone 返しの `ExecutorShared`）, `core(&self) -> &DispatcherCore`
- [ ] 3.3 `create_mailbox(&self, actor, mailbox_type: &dyn MailboxType) -> ArcShared<Mailbox>` を `&self` で宣言する（factory メソッドなので状態を変えない）
- [ ] 3.4 hook `register_actor(&mut self, actor) -> Result<(), SpawnError>` を default impl 付きで宣言する。default impl は `self.core_mut().mark_attach()` を呼んで `Ok(())`
- [ ] 3.5 hook `unregister_actor(&mut self, actor)` を default impl 付きで宣言する。default impl は `self.core_mut().mark_detach()` を呼ぶ
- [ ] 3.6 hook メソッドを default impl 付きで宣言する（具象型は必要な時だけ override）:
  - `dispatch(&mut self, receiver, env) -> Result<(), SendError>`: default は `receiver.mailbox().enqueue_user(env)?`
  - `system_dispatch(&mut self, receiver, msg) -> Result<(), SendError>`: default は system 経路で同様
  - `register_for_execution(&mut self, mbox, h1, h2) -> bool`: default は mailbox CAS 判定のみ
  - `execute_task(&mut self, task)`: default は `mark_attach` → task 実行 → cleanup で `mark_detach` / 必要時 `schedule_shutdown_if_sensible`
  - `shutdown(&mut self)`: default は `self.core_mut().shutdown()`
- [ ] 3.7 `core_mut(&mut self) -> &mut DispatcherCore` を必須メソッドとして宣言する（default impl から `DispatcherCore` へ到達するため）
- [ ] 3.8 その他の command (`suspend`, `resume`) を `&mut self` で宣言する
- [ ] 3.9 trait method で `Box<dyn MessageDispatcher>` を trait object として扱えるようにする（object-safe 保証）
- [ ] 3.10 trait doc に「command メソッドを `&self` + 内部可変性で偽装してはならない」「`attach` / `detach` の orchestration は `MessageDispatcherShared` 側が担う」「`create_mailbox` は direct call せず `MessageDispatcherShared::attach` 経由で使う」「`register_actor` / `unregister_actor` / `dispatch` / `register_for_execution` / `create_mailbox` は override 可能な hook」旨を明記する

## 4. core: DefaultDispatcher 具象型

- [ ] 4.1 `dispatcher_new/default_dispatcher.rs` に `pub struct DefaultDispatcher { core: DispatcherCore }` を定義する
- [ ] 4.2 `DefaultDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` を実装する。内部で `DispatcherCore::new(settings, executor)` を呼ぶ
- [ ] 4.3 `impl MessageDispatcher for DefaultDispatcher` を実装する:
  - 必須メソッド: `core(&self) -> &DispatcherCore` / `core_mut(&mut self) -> &mut DispatcherCore`
  - query メソッド (`id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants`, `executor`) は `self.core` へ委譲する
  - `create_mailbox(&self, actor, ty)` は新規 `ArcShared<Mailbox>` を返す（default impl でそのまま使える場合は override 不要）
  - hook メソッド (`register_actor`, `unregister_actor`, `dispatch`, `system_dispatch`, `register_for_execution`, `shutdown`) は trait の default impl をそのまま使う（`DefaultDispatcher` 固有の追加処理はない）
- [ ] 4.4 `DefaultDispatcher` の unit test を追加する（shared wrapper 経由の attach / detach が inhabitants を増減すること、複数 actor を同時 attach できること、auto-shutdown の挙動、default hook が意図通り呼ばれること）

## 5. core: PinnedDispatcher 具象型

- [ ] 5.1 `dispatcher_new/pinned_dispatcher.rs` に `pub struct PinnedDispatcher { core: DispatcherCore, owner: Option<Pid> }` を定義する（既存の actor identity は `Pid` を使う。通常の `Option`、内部可変性は使わない）
- [ ] 5.2 `PinnedDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` を実装する。引数で受け取った `settings` を `settings.with_throughput(NonZeroUsize::MAX).with_throughput_deadline(None)` で **Pinned 固有値に上書き** してから `DispatcherCore::new` に渡す（呼び出し側が何を渡しても結果として Pinned の固定値になる）
- [ ] 5.3 `impl MessageDispatcher for PinnedDispatcher` を実装する:
  - 必須メソッド: `core(&self)` / `core_mut(&mut self)` を `self.core` / `&mut self.core` で実装
  - query メソッドはすべて `self.core` 委譲（Pinned 固有の固定値は `new` で既に core に埋め込み済み）
  - `create_mailbox` は default impl のまま
  - hook メソッドのうち **`register_actor` と `unregister_actor` のみ** override:
    - `register_actor(&mut self, actor)`: `self.owner` が `None` か同一 actor なら owner をセットし、`self.core.mark_attach()` を呼んで `Ok(())` を返す。別 actor が既に owner なら `SpawnError::DispatcherAlreadyOwned` を返す
    - `unregister_actor(&mut self, actor)`: owner を `None` に戻してから `self.core.mark_detach()` を呼ぶ
  - それ以外の hook (`dispatch`, `system_dispatch`, `register_for_execution`, `shutdown`) は default impl
- [ ] 5.4 `PinnedDispatcher` の unit test を追加する（1 actor 専有、2 体目拒否、同一 actor の再 attach 許容、detach 後の再利用、Pinned 固有値が query で返ること）

## 6. core: MailboxOfferFuture Waker (core/no_std)

- [ ] 6.1 `dispatcher_new/dispatcher_waker.rs` に `DispatcherWaker` を定義する
- [ ] 6.2 `core::task::RawWaker` を使って `MessageDispatcherShared` + `ArcShared<Mailbox>` を data に載せる実装とする（両者とも Clone で `ArcShared` インクリメントなので安全に RawWaker の data ポインタへ格納できる）
- [ ] 6.3 `wake` 実装は `MessageDispatcherShared::register_for_execution(&mbox, false, true)` を呼ぶ
- [ ] 6.4 Waker を消費する側（`MailboxOfferFuture::Pending` 経路）をテストする unit test を追加する

## 7. core: MessageDispatcherConfigurator trait と 具象

- [ ] 7.1 `dispatcher_new/message_dispatcher_configurator.rs` に `trait MessageDispatcherConfigurator: Send + Sync { fn dispatcher(&self) -> MessageDispatcherShared; }` を定義する。引数なし（Pekko 準拠）、戻り値は Clone 安全な `MessageDispatcherShared`
- [ ] 7.2 `dispatcher_new/default_dispatcher_configurator.rs` に `DefaultDispatcherConfigurator { shared: MessageDispatcherShared }` を定義する。`DefaultDispatcherConfigurator::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` で eager に `DefaultDispatcher::new(settings, executor)` を構築し、`MessageDispatcherShared::new` で包んでフィールドに保持する。`dispatcher(&self)` では `self.shared.clone()` を返す（OnceLock 等の内部可変性を使わず、eager init で immutable に保つ）
- [ ] 7.3 `dispatcher_new/pinned_dispatcher_configurator.rs` に `PinnedDispatcherConfigurator { settings: DispatcherSettings, executor_factory: ArcShared<Box<dyn ExecutorFactory>>, thread_name_prefix: String }` を定義する。`dispatcher(&self)` で `executor_factory.create(...)` を呼んで新規 `ExecutorShared` を作り、`PinnedDispatcher::new(self.settings.clone(), executor)` で新規 dispatcher を構築して `MessageDispatcherShared::new` で包んで返す（thread 番号採番は `static AtomicUsize` を用いる既存慣習に従う）
- [ ] 7.4 Blocking 用は `DefaultDispatcherConfigurator` を blocking 対応 `ExecutorFactory` で構築する経路を fraktor 内部に用意する（別 type は作らない）。具体的には `ActorSystemConfig` の bootstrap で次のように登録する:
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
- [ ] 7.5 configurator の unit test を追加する（Default は同じ `MessageDispatcherShared` clone を返す、Pinned は毎回新規、Blocking は id 違いで別 instance、configurator には内部可変性が存在しない）

## 8. core: Dispatchers registry の置換

- [ ] 8.1 `dispatcher_new/dispatchers.rs` に新 `Dispatchers { entries: HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>> }` を定義する
- [ ] 8.2 `Dispatchers::register(&mut self, id, configurator: ArcShared<Box<dyn MessageDispatcherConfigurator>>)` / `resolve(&self, id) -> MessageDispatcherShared` / `ensure_default(&mut self)` を実装する。`register` / `ensure_default` は state 変更のため `&mut self`、`resolve` は query のため `&self`
- [ ] 8.3 Pekko 互換 id の正規化（`pekko.actor.default-dispatcher` → `default`、`pekko.actor.internal-dispatcher` → `default`）を先行 change の要件通り維持する
- [ ] 8.4 registry の unit test を追加する

## 9. core: Mailbox 改修（並走期間中は additive に留める）

- [ ] 9.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` に queue 注入可能な public コンストラクタを **追加** する。並走期間中は legacy dispatcher が使う既存 `Mailbox::new(...)` シグネチャを削除しない
- [ ] 9.2 `Mailbox::run(&self, throughput: NonZeroUsize, throughput_deadline: Option<Duration>)` を追加する。drain ループ本体は既存 `DispatcherCore::process_batch` のロジックを mailbox 側へ移設する
- [ ] 9.3 mailbox 側の `set_running` / `set_idle` を run 内で呼び、dispatcher 側の state 管理を排除する
- [ ] 9.4 Pekko 名に対応する alias / 追加 API（`setAsScheduled` / `setAsIdle` / `canBeScheduledForExecution` 相当）を導入する。並走期間中は legacy dispatcher が使う `request_schedule` などの既存 API を削除しない
- [ ] 9.5 detach 経路で mailbox を terminal 状態へ遷移させ、`clean_up` する contract を追加する
- [ ] 9.6 mailbox 改修後も legacy dispatcher と new dispatcher の両方がコンパイル可能であることを確認する

## 10. std: Executor 具象の置換（すべて CQS 準拠）

std 層のすべての `Executor` 具象実装は trait 契約（`execute(&mut self, ...)` / `shutdown(&mut self)` / `supports_blocking(&self)`）に従わなければならない。`&mut self` は「caller から見ると command（副作用あり）である」という CQS 上の意味付けを型で表すためであり、内部 state を実際に変更するかどうか（tokio Handle なら変更なし、PinnedExecutor::shutdown なら Option::take で変更あり）とは独立に適用される。

- [ ] 10.1 `modules/actor-adaptor-std/src/std/dispatch_new/tokio_executor.rs` に `TokioExecutor` を CQS 準拠で実装する。内部では `self.handle.spawn_blocking(task)` を呼ぶだけで、`&mut self` は trait 契約維持のためであり内部 state の実質的な変更は発生しない。`shutdown(&mut self)` は Handle から runtime shutdown できないため no-op か best-effort のログ出力
- [ ] 10.2 `dispatch_new/tokio_executor_factory.rs` に `TokioExecutorFactory` を実装し、`ExecutorShared::new(TokioExecutor::new(handle))` を返す
- [ ] 10.3 `dispatch_new/threaded_executor.rs` に `ThreadedExecutor`（blocking 向け複数スレッド）を CQS 準拠で実装する。`execute(&mut self, task)` は新規 thread を spawn する。`shutdown(&mut self)` は outstanding thread の追跡が必要ならここで状態更新する
- [ ] 10.4 `dispatch_new/pinned_executor.rs` に `PinnedExecutor`（1 スレッド専用）を CQS 準拠で実装する。`execute(&mut self, task)` は内部の `Option<mpsc::Sender<Box<dyn FnOnce()+Send>>>` 経由で worker thread に送信する。`shutdown(&mut self)` は `self.sender.take()` と `self.join.take()?.join()` を呼ぶ（このケースは実際に `&mut self` が必要）
- [ ] 10.5 `dispatch_new/pinned_executor_factory.rs` に `PinnedExecutorFactory` を実装する
- [ ] 10.6 std 側 executor の unit test を追加する:
  - trait の CQS シグネチャ（`&mut self` command / `&self` query）が守られていること
  - `ExecutorShared` 経由で 複数スレッドから並行 submit できること（`RuntimeMutex` で serialize される）
  - `PinnedExecutor` が 1 スレッドのみ使うこと
  - `PinnedExecutor::shutdown` が worker thread を正しく join すること

## 11. 呼び出し元移行

並走戦略: `dispatcher_new/` と旧 `dispatcher/` を同時に存在させ、fraktor 外部の公開 API は新型のみを見せる。一方で内部実装では、呼び出し元の移行が完了するまで旧 registry / mailbox API を一時的に残してよい。旧 dispatcher の呼び出し元（`ActorCell` / `ActorRefSender` / bootstrap 等）を sub-task 単位で一つずつ新 API へ切り替え、切り替え済みの箇所から旧 re-export と旧 state を削除していく。

- [ ] 11.0 `ActorCell` の生成順を 2-phase init に変更し、dispatcher attach 前に cell を確保できるようにする。必要なら mailbox / dispatcher / sender の install メソッドまたは builder 分割を追加する
- [ ] 11.0.1 2-phase init の影響範囲を `ActorCell` の mailbox / dispatcher / sender install 順序に限定する。`ActorCell` 全体の AShared 化や広域 mutex 導入は行わない
- [ ] 11.1 `ActorSystemConfig` に新 `Dispatchers`（`HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>>`）を追加する。旧 `DispatcherRegistryEntry` を保持するフィールドは **この時点では削除しない**
- [ ] 11.2 `ActorSystem::new` / `start` で新 `Dispatchers` を構築し、reserved default / blocking / internal id の正規化を先行 change の要件通りに設定する
- [ ] 11.3 `ActorCell::start` などの bootstrap が `MessageDispatcherShared::attach(actor)` を呼ぶように修正する。attach が mailbox を生成して actor に install する前提で生成順を組み替える
- [ ] 11.4 `ActorCell::stop` / termination 経路が `MessageDispatcherShared::detach(actor)` を呼ぶように修正する
- [ ] 11.5 `ActorRefSender` 経路を新 `MessageDispatcherShared::dispatch` / `system_dispatch` に繋ぎ替える
- [ ] 11.6 旧 `DispatcherSender` を削除し、`ActorRef` の送信経路から `MessageDispatcherShared::dispatch` を直接呼ぶ形に整理する
- [ ] 11.7 typed 側 dispatcher selector（`Default` / `Blocking` / `FromConfig` 等）が新 `Dispatchers::resolve` 経由で `MessageDispatcherShared` を解決することを確認する
- [ ] 11.8 旧 dispatcher 経路を使っていた MailboxOfferFuture / backpressure 経路が新 `DispatcherWaker` 経由に置き換わっていることを確認する
- [ ] 11.9 旧 registry / legacy mailbox API への内部参照が消えたことを確認してから、最終削除フェーズへ進む

## 12. 旧 dispatcher surface の削除

- [ ] 12.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher/` 配下を削除し、`dispatcher_new/` を `dispatcher/` にリネームする（最終状態では `_new` suffix を残さない）
- [ ] 12.2 旧 `DispatcherCore`（旧）/ `DispatcherShared`（旧）/ `DispatchShared` / `DispatchExecutor` / `DispatchExecutorRunner` / `DispatcherBuilder` / `DispatcherProvider` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` / `DispatcherSender` / `DispatcherSettings` / `ScheduleAdapter*` / `InlineScheduleAdapter` / `ScheduleWaker` / 旧 `InlineExecutor` / 旧 `TickExecutor` を削除する
- [ ] 12.3 `modules/actor-adaptor-std/src/std/dispatch/dispatcher/` 配下を削除し、`dispatch_new/` を `dispatch/` にリネームする（最終状態では `_new` suffix を残さない）
- [ ] 12.4 旧 `StdScheduleAdapter` / `DefaultDispatcherProvider` / `BlockingDispatcherProvider` / 旧 `PinnedDispatcherProvider` / 旧 `TokioExecutor` / 旧 `ThreadedExecutor` / 旧 `PinnedExecutor` を削除する
- [ ] 12.5 `modules/actor-core/src/core/kernel/dispatch.rs` と `modules/actor-adaptor-std/src/std/dispatch.rs` の re-export を整理する（新型のみが公開される状態にする）
- [ ] 12.6 `ActorSystemConfig` / `ActorSystem` から旧 registry field と legacy dispatcher bootstrap 経路を最終削除する
- [ ] 12.7 並走期間中に温存していた legacy mailbox API（旧 `Mailbox::new` シグネチャ、`request_schedule` など）をここで削除または rename 完了する

## 13. 追随更新

- [ ] 13.1 showcase / bench / cluster / remote を新 dispatcher API に追随させる
- [ ] 13.2 dispatcher 関連 tests を全て通す
- [ ] 13.3 typed selector tests が新 registry で解決されることを確認する
- [ ] 13.4 `dispatcher-trait-family-redesign` から引き継ぐ capability spec delta（REMOVED / ADDED を含む）が archive 後に矛盾しないことを確認する

## 14. 最終検証

- [ ] 14.1 Balancing 拡張 seam 6 項目が trait / struct のシグネチャ上で満たされていることを手動確認する
- [ ] 14.2 旧 dispatcher 関連型が source tree と `cargo metadata` から消失していることを確認する
- [ ] 14.3 `./scripts/ci-check.sh ai all` を実行してエラーがないことを確認する
- [ ] 14.4 Pekko 参照実装との差分を最終確認（Dispatcher.scala / PinnedDispatcher.scala / AbstractDispatcher.scala / Mailbox.scala）
- [ ] 14.5 1:N 共有 dispatcher の contention を bench もしくは diagnostics で観測し、既知のトレードオフを記録する

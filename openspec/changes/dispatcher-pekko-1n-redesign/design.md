## Context

先行 change `dispatcher-trait-family-redesign` は `Dispatcher` trait と `DispatcherProvider` trait を公開抽象として導入することを決めたが、以下の核心項目を固定していなかったため、実装が Pekko 互換を満たさない状態で完了扱いになっている。

- dispatcher と actor の所有関係（1 : 1 or 1 : N）
- dispatcher lifecycle（attach / detach / inhabitants / 自動 shutdown）
- mailbox がスケジューリング主体（runnable として executor に submit される）である契約
- Executor trait のシグネチャ（`&self` or `&mut self`）
- async backpressure と no_std の両立方針

結果として現行実装は 1 dispatcher = 1 mailbox の結合で完了しており、Pekko の `MessageDispatcher` 抽象階層が意図していた「executor を複数 actor で共有する」モデルが成立していない。この change は Pekko 実装から直接抽出した契約を使って、当該抽象階層を正しく完了させる。

参照実装:

- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/AbstractDispatcher.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatcher.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/PinnedDispatcher.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatchers.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailbox.scala`
- `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/BalancingDispatcher.scala`（将来拡張 seam の検証用）

## Design Constraints

### 1. Dispatcher と Actor の所有関係

#### 満たすべき条件

- dispatcher は executor と設定を保持する実体であり、複数の actor を同時に収容できなければならない（1 : N）
- actor の mailbox は executor に submit される対象であり、dispatcher は submit の経路を提供する
- `DefaultDispatcherConfigurator::dispatcher()` は同一 instance を毎回返す（1 dispatcher を多 actor で共有する）
- `PinnedDispatcherConfigurator::dispatcher()` は呼び出しのたびに新しい instance を返す（結果として 1 actor : 1 dispatcher : 1 thread が成立する）

#### 満たしてはいけない条件

- dispatcher が特定の mailbox への参照を field として保持してはならない
- actor 起動時に dispatcher を毎回新規構築する経路を唯一の経路として残してはならない
- `PinnedDispatcher` 以外の configurator が呼び出しごとに dispatcher を新規生成してはならない

### 2. `MessageDispatcher` trait（CQS 準拠の query / hook 面）

#### 満たすべき条件

- `MessageDispatcher` trait は dispatcher 公開抽象の中心となる hook/query 面でなければならない
- trait メソッドは CQS 原則に従って `&self`（query）と `&mut self`（command / hook）を厳密に分ける
- trait は Pekko の `MessageDispatcher` 抽象クラスの契約を Rust へ写すが、**lock 解放後に副作用を起こす orchestration は `MessageDispatcherShared` に置く**。trait 自身は次の query / hook 群を要求する（`Send + Sync` を含む）:

  **Queries (`&self`, 戻り値あり)**
  - `id(&self) -> &str`
  - `throughput(&self) -> NonZeroUsize`
  - `throughput_deadline(&self) -> Option<Duration>`
  - `shutdown_timeout(&self) -> Duration`
  - `inhabitants(&self) -> i64`
  - `executor(&self) -> ExecutorShared`（clone を返す。`MessageDispatcherShared::register_for_execution` の再スケジュール経路が短時間の read lock 越しにこれを取り出す）

  **Overridable hooks (protected 相当、具象型が override 可能)**
  - `register_actor(&mut self, actor: &ActorCell) -> Result<(), SpawnError>`
    - default impl: `self.core_mut().mark_attach()` を呼んで `Ok(())`
    - identity check が必要な場合は `actor.pid()` を使う（`PinnedDispatcher` は owner として `Pid` を保持）
    - `PinnedDispatcher` は owner check + owner 設定を追加する
    - Pekko の `register(actorCell)` に対応
  - `unregister_actor(&mut self, actor: &ActorCell)`
    - default impl: `self.core_mut().mark_detach()` を呼ぶ
    - `PinnedDispatcher` は owner を `None` に戻す
    - Pekko の `unregister(actor)` に対応
  - `dispatch(&mut self, receiver: &ActorCell, envelope: Envelope) -> Result<Vec<ArcShared<Mailbox>>, SendError>`
    - default impl: `receiver.mailbox().enqueue_user(envelope)?` → `vec![receiver.mailbox()]` を返す
    - 戻り値は shared wrapper が lock 解放後に `register_for_execution` を試みる候補 mailbox 配列。優先度順に並んでおり、最初に CAS が通った候補で schedule 完了とみなす
    - `BalancingDispatcher` は `self.shared_queue.enqueue(envelope)?` → `vec![receiver.mailbox()]` で override する（V1 は単一要素、V2 teamWork で複数要素を返せる構造）
    - hot path の allocation を避けるため、実装は `SmallVec<[ArcShared<Mailbox>; 1]>` 等の small-size optimization を採用してよい（spec レベルでは `Vec` 表記）
  - `system_dispatch(&mut self, receiver: &ActorCell, msg: SystemMessage) -> Result<Vec<ArcShared<Mailbox>>, SendError>`
    - default impl: `receiver.mailbox().enqueue_system(msg)?` → `vec![receiver.mailbox()]` を返す
    - 戻り値の意味は `dispatch` と同じ
  - `suspend(&mut self, actor: &ActorCell)`
  - `resume(&mut self, actor: &ActorCell)`
  - `shutdown(&mut self)`
    - default impl: `self.core_mut().shutdown()` を呼ぶ
  - `create_mailbox(&self, actor: &ActorCell, mailbox_type: &dyn MailboxType) -> ArcShared<Mailbox>`
    - **`&self`**（factory メソッド、状態を変えない）
    - default impl: 通常の per-actor mailbox を返す
    - `BalancingDispatcher` は `SharingMailbox`（self.shared_queue を注入した mailbox）を返す形で override する
    - 運用規律: 外部 caller は直接呼ばず、常に `MessageDispatcherShared::attach` 経由で mailbox を生成する（inhabitants 管理をスキップしないため）

  **trait に存在しない（あえて trait method にしないもの）**
  - `register_for_execution`: trait hook ではなく `MessageDispatcherShared::register_for_execution` に純粋に集約する。理由は section 2.5 step `register_for_execution` を参照（CAS と executor submit 以外の policy 判定は `dispatch` hook の戻り値配列で表現するため、trait hook を別に持たない）
  - `execute_task`: 本 change のスコープ外（YAGNI）。Pekko の `executeTask` 相当（dispatcher の executor へ任意 closure を submit する経路）は具体的な caller が現れた時点で additive に追加する

  **Core accessor (trait object から `DispatcherCore` へ到達するため)**
  - `core(&self) -> &DispatcherCore`
  - `core_mut(&mut self) -> &mut DispatcherCore`

- trait object `Box<dyn MessageDispatcher>` として `MessageDispatcherShared` から取り回せなければならない
- 具象型は CQS と内部可変性ポリシーに従い、`&mut self` を内部可変性で偽装してはならない
- `create_mailbox` は public trait method として露出するが、運用規律として外部 caller は直接呼ばず、常に `MessageDispatcherShared::attach` 経由で mailbox を生成する

#### 満たしてはいけない条件

- command メソッドに `&self` を要求してはならない（内部可変性ポリシー違反。ただし `create_mailbox` は状態を変えない factory なので `&self` が正しい）
- `Dispatcher` という単純名を trait 名として残してはならない（Pekko の抽象クラス名に合わせる）
- trait method を通じてガードやロックを外部へ返してはならない（ロック区間は `MessageDispatcherShared` 内部に閉じる）

### 2.5 `MessageDispatcherShared`（AShared パターン）

#### 満たすべき条件

- 複数スレッド・複数所有者で `MessageDispatcher` を共有する唯一の経路は `MessageDispatcherShared` でなければならない
- `MessageDispatcherShared` は既存の AShared 系 (`ActorFactoryShared` など) と同じパターンを踏襲する:
  ```rust
  pub struct MessageDispatcherShared {
      inner: ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>,
  }

  impl Clone for MessageDispatcherShared { /* ArcShared::clone */ }

  impl MessageDispatcherShared {
      pub fn new<D: MessageDispatcher + 'static>(dispatcher: D) -> Self;
      // 便利メソッドは with_write / with_read を通じて trait へ委譲
  }

  impl SharedAccess<Box<dyn MessageDispatcher>> for MessageDispatcherShared {
      fn with_read<R>(&self, f: impl FnOnce(&Box<dyn MessageDispatcher>) -> R) -> R;
      fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn MessageDispatcher>) -> R) -> R;
  }
  ```
- `MessageDispatcherShared` は公開 lifecycle/orchestration API を持つ:
  - `attach(&self, actor: &ActorCell) -> Result<(), SpawnError>`
  - `detach(&self, actor: &ActorCell)`
  - `dispatch(&self, receiver: &ActorCell, envelope: Envelope) -> Result<(), SendError>`
  - `system_dispatch(&self, receiver: &ActorCell, msg: SystemMessage) -> Result<(), SendError>`
  - `register_for_execution(&self, mbox: &ArcShared<Mailbox>, has_message_hint: bool, has_system_hint: bool) -> bool`
- `MessageDispatcherShared::attach` は次の順で動作する:
  1. `with_write` で `register_actor(actor)` を呼ぶ
  2. 同じ `with_write` の中で `create_mailbox(actor, ...)` を呼び、生成した mailbox を actor へ設定する
  3. ロック解放後に `self.register_for_execution(&mbox, false, true)` を呼ぶ
- `MessageDispatcherShared::dispatch` / `system_dispatch` は次の順で動作する:
  1. `with_write` で trait hook の `dispatch` / `system_dispatch` を呼び、戻り値の **候補 mailbox 配列** を取得する（trait hook 内では enqueue のみ、schedule 副作用は起こさない）
  2. ロック解放後、候補配列を優先度順に走査し、各 mailbox に対して `self.register_for_execution(&mbox, has_message_hint=true, has_system_hint=false)` を試みる
  3. 最初に `register_for_execution` が `true` を返した候補で完了する（その候補のみが executor へ submit される）
  4. 全候補が CAS 失敗（busy）でも `Ok(())` を返す（envelope は queue 内に残り、次回 dispatch / drain サイクルで pick up される）
- `MessageDispatcherShared::detach` は次の順で動作する:
  1. `with_write` で `unregister_actor(actor)` を呼ぶ
  2. 同じ `with_write` の中で actor から detached mailbox を terminal 状態へ遷移させ、`clean_up` する
  3. 同じ `with_write` の中で `self.core_mut().schedule_shutdown_if_sensible()` を呼び、**戻り値の `ShutdownSchedule` をローカル変数へ copy する**
  4. ロック解放後、copy した値が `SCHEDULED` の場合のみ `actor.system().scheduler()` から取得した handle に delayed shutdown closure を登録する
  5. delayed closure 発火時に `with_write` で `shutdown_schedule` と `inhabitants` を再確認し、`SCHEDULED && inhabitants == 0` の場合のみ `shutdown()` を呼ぶ
  6. 発火時に `RESCHEDULED` または `inhabitants > 0` なら `shutdown_schedule` を `UNSCHEDULED` に戻して何もしない
  - **lock 境界での state 観測**: step 3 の戻り値経由で copy するため、lock 解放後に再度 mutex を取って状態確認する race window を作らない
- `MessageDispatcherShared::register_for_execution` は次の順で動作する（純粋に CAS + executor submit、trait hook は呼ばない）:
  1. mailbox の `can_be_scheduled_for_execution(hints)` を（ロック外で）評価する。`false` なら `false` を返す
  2. mailbox の `set_as_scheduled()` の CAS を試みる。失敗したら `false` を返す
  3. `with_read` で throughput / throughput_deadline / executor_shared を一度に取り出す（短時間の read lock）
  4. ロックを解放した状態で closure を組み立てる（`self.clone()` と `mbox.clone()` を capture）
  5. closure 内では `mbox.run(throughput, deadline)` → `mbox.set_as_idle()` → `self.register_for_execution(&mbox, false, false)` の順で実行
  6. 組み立てた closure を `ExecutorShared::execute` 経由で submit し、`true` を返す
  - dispatcher policy 固有の判定（例: `BalancingDispatcher` の teamWork 候補展開）は trait hook ではなく `dispatch` の戻り値配列で表現される。`register_for_execution` は CAS と executor submit のみを担う
- `MessageDispatcherShared` は query 系の便利メソッドとして trait の主要 query を委譲する（`id` / `throughput` 等）
- 再入デッドロック防止のため、AShared 系の共通原則に従ってロック区間を最小化する: executor submit、delayed shutdown の登録、closure の実行は `with_write` のブロックを抜けた後で行う

#### 満たしてはいけない条件

- `MessageDispatcherShared` 経由以外で `Box<dyn MessageDispatcher>` を多所有状態にしてはならない
- `with_read` / `with_write` が返すガードを外部ユーザに露出してはならない
- `MessageDispatcherShared` から `MutexGuard` を直接返す API を公開してはならない
- ロックを保持したまま任意の closure を executor に submit してはならない（他アクターから register_for_execution 呼び出しによる再入デッドロックを引き起こす）

### 3. `DispatcherCore` 共通 state（CQS 準拠、内部可変性なし）

#### 満たすべき条件

- dispatcher 共通 state は pub struct `DispatcherCore` に集約されなければならない
- `DispatcherCore` は次を保持する:
  - `id: String`
  - `throughput: NonZeroUsize`
  - `throughput_deadline: Option<Duration>`
  - `shutdown_timeout: Duration`
  - `executor: ExecutorShared`         ← AShared wrapper（生の `ArcShared<Box<dyn Executor>>` ではない）
  - `inhabitants: i64`                 ← 通常の i64（atomic ではない、`&mut self` 経由で更新）
  - `shutdown_schedule: ShutdownSchedule` ← 通常 enum（atomic ではない、`&mut self` 経由で遷移）
- `DispatcherCore` は次の method を提供する:

  **Queries (`&self`)**
  - `id(&self) -> &str`
  - `throughput(&self) -> NonZeroUsize`
  - `throughput_deadline(&self) -> Option<Duration>`
  - `shutdown_timeout(&self) -> Duration`
  - `inhabitants(&self) -> i64`
  - `executor(&self) -> &ExecutorShared`（参照返し。trait 側の同名メソッドは clone 返しなので、trait impl 内で `self.core.executor().clone()` を呼ぶ）

  **Commands (`&mut self`)**
  - `mark_attach(&mut self)`: `inhabitants += 1` し、`shutdown_schedule == SCHEDULED` なら `RESCHEDULED` に遷移させる（純粋 command、戻り値なし）
  - `mark_detach(&mut self)`: `inhabitants -= 1` する（純粋 command、戻り値なし）
    - `inhabitants` が負になることは設計上ありえない。`debug_assert!(self.inhabitants >= 0)` でデバッグ時に検出し、release ビルドでは `i64::max(self.inhabitants, 0)` で clamp する（Pekko の `IllegalStateException("ACTOR SYSTEM CORRUPTED!!!")` 相当の防御）
  - `schedule_shutdown_if_sensible(&mut self) -> ShutdownSchedule`: `inhabitants == 0` の時のみ `UNSCHEDULED → SCHEDULED` または `SCHEDULED → RESCHEDULED` の状態遷移を行い、**遷移後の `ShutdownSchedule` 値を返す**
    - CQS 許容例外: `MessageDispatcherShared::detach` step 3 でこの戻り値を lock 解放前に copy し、step 4 の delayed shutdown 登録判定に使うため。Pekko の `updateShutdownSchedule` 相当。代替手段（lock 解放後に query で再観測）は race window を作るため不採用
  - `shutdown(&mut self)`（内部で `self.executor.shutdown()` を呼び、`shutdown_schedule` を UNSCHEDULED に戻す）

- `DispatcherCore` は atomic field を field として公開しない（内部可変性を封じる）。inhabitants とスケジュール状態は `&mut self` ロック越しに更新する
- delayed shutdown の実行予約そのものは `DispatcherCore` ではなく `MessageDispatcherShared::detach` が actor の system scheduler を使って行う
- delayed shutdown の scheduler handle は `detach(actor)` の引数から `actor.system().scheduler()` を辿って取得し、`DispatcherCore` / `MessageDispatcherShared` の field には保持しない
- inhabitants カウンタのロックフリー性が要求されるのは `MessageDispatcherShared` のロック越しの経路のみであり、ロックを取得している文脈内では `&mut self` で十分である
- `DispatcherCore` は pub 公開され、fraktor 外部の独自 `MessageDispatcher` 実装から共通 state として利用可能でなければならない
- 各具象型（`DefaultDispatcher` / `PinnedDispatcher`）は `core: DispatcherCore` として `DispatcherCore` を field に保持し、自身が実装する `MessageDispatcher` trait の query / command メソッドを `self.core` へ委譲する

#### 満たしてはいけない条件

- `DispatcherCore` のフィールドに `AtomicI64` / `AtomicU8` / `Mutex<T>` / `UnsafeCell<T>` などの内部可変性を持たせてはならない
- `DispatcherCore` を wrap する別名 struct（旧 `DispatcherShared` / `DispatchShared` 等）を同時に残してはならない
- `DispatcherCore::register_for_execution` を `&self` メソッドとして提供してはならない（内部可変性なしでは成立しない）
- `DispatcherCore` を `pub(crate)` に閉じて外部拡張を塞いではならない

### 4. 具象 `DefaultDispatcher` / `PinnedDispatcher`

#### 満たすべき条件

- **両具象型はいずれも core 層（`modules/actor-core/src/core/kernel/dispatch/dispatcher_new/`）に配置する**。tokio や std::thread への型レベル依存は持たず、`no_std` 対応とする。具体的なスレッドプール実装は `ExecutorShared` 経由で構築時に注入される（Pekko の `Dispatcher` が `ExecutorServiceFactoryProvider` を注入されるのと同じ構図）
- `DefaultDispatcher` は次を満たす:
  - `core: DispatcherCore` を保持する
  - `MessageDispatcher` trait の default impl をほぼそのまま使う（`core()` / `core_mut()` / `create_mailbox` のみ実装）
  - hook メソッド (`register_actor`, `unregister_actor`, `dispatch`, `system_dispatch`, `register_for_execution`, `shutdown`) も trait default impl を使う
- `PinnedDispatcher` は次を満たす:
  - `core: DispatcherCore` と `owner: Option<Pid>` を保持する（内部可変性は使わない）
  - `DispatcherCore::new` 呼び出し時に `throughput = NonZeroUsize::MAX`, `throughput_deadline = None` を固定で渡す（Pekko: `Int.MaxValue` / `Duration.Zero`）
  - `register_actor` hook を override: owner が `None` または同一 actor (`Some(actor.pid())` と一致) のときのみ owner を `Some(actor.pid())` にセットして `self.core.mark_attach()` を呼んで `Ok(())` を返す。別 actor が既に owner なら `SpawnError::DispatcherAlreadyOwned` を返す
  - `unregister_actor` hook を override: owner を `None` に戻してから `self.core.mark_detach()` を呼ぶ
  - 同一 actor の再 attach を許容する（Pekko の `register` override と同じ挙動）
  - **`SpawnError::DispatcherAlreadyOwned`** バリアントは本 change で `SpawnError` enum に新規追加する（Task 5.0 を参照）
- `BalancingDispatcher` は次を満たす:
  - `core: DispatcherCore` と `shared_queue: ArcShared<SharedMessageQueue>` を保持する
  - `DispatcherCore::new` には通常の `DispatcherSettings` の `throughput` / `throughput_deadline` をそのまま渡す（Pinned のような上書きはしない）
  - `create_mailbox` を override: `SharingMailbox::new(actor, self.shared_queue.clone())` を返す。SharingMailbox は Mailbox の薄いラッパで `shared_queue` を内部 message queue として使う点が差分
  - `dispatch` hook を override: `self.shared_queue.enqueue(envelope)?` した上で `vec![receiver.mailbox()]` を返す（V1: 単一候補。V2 teamWork で team member を追加する形に拡張可能）
  - `system_dispatch` hook は default impl のまま（system message は actor 個別の経路）
  - `register_actor` / `unregister_actor` / `suspend` / `resume` / `shutdown` は default impl のまま使う
  - **load balancing のメカニズム**: 全 attach 済み actor の SharingMailbox は同じ `shared_queue` を参照する。actor のいずれかが `mailbox.run()` で dequeue するとき、shared queue から FIFO で envelope が取り出される。これにより複数 actor が同じ queue を消化する形の load balancing が成立する。teamWork による即時 wake up fallback (Pekko の `BalancingDispatcher.teamWork()`) は V1 では実装しない (queue 内 envelope は次の dispatch / actor 再 schedule で消化されるため、機能的には正しい)
- 3 具象型 (`DefaultDispatcher` / `PinnedDispatcher` / `BalancingDispatcher`) はすべて `MessageDispatcherShared::new(concrete)` 経由で `MessageDispatcherShared` へ格納可能でなければならない（直接 `ArcShared<dyn MessageDispatcher>` のような形で多所有化しない）
- **blocking workload 向け dispatcher は `DefaultDispatcher` の別 id + 別 `ExecutorShared` 構成で表現する**。`BlockingDispatcher` という具象型は作らない（Pekko にも存在しない。Pekko では `reference.conf` で `pekko.actor.default-blocking-io-dispatcher` を `type = "Dispatcher"` として別 pool 設定で登録するだけ）

#### 満たしてはいけない条件

- `DefaultDispatcher` / `PinnedDispatcher` / `BalancingDispatcher` を std 層（`actor-adaptor-std`）に配置してはならない
- 3 具象型に tokio / std::thread / std 固有型への型レベル依存を導入してはならない
- 3 具象型の差分を enum 変種やフラグで潰してはならない（Pekko の型階層を忠実に写す）
- `PinnedDispatcher` の 1 : 1 制約を runtime assert のみで表現し、register 時に拒否しない設計にしてはならない
- `DefaultDispatcher` に blocking workload 用 executor を同居させ、別 dispatcher として扱わないようにしてはならない（blocking は別 id + 別 configurator で表現する）
- **`BlockingDispatcher` という独立した具象型を作ってはならない**（Pekko に存在しない、`DefaultDispatcher` + blocking executor factory で表現する）
- `BalancingDispatcher` の V1 で teamWork load balancing fallback を実装してはならない（YAGNI、phase 2 で additive に追加可能）
- `BalancingDispatcher` 以外の dispatcher で `SharedMessageQueue` / `SharingMailbox` を使ってはならない

### 5. `Executor` trait（CQS 準拠）と `ExecutorShared`

#### 満たすべき条件

- `Executor` trait は CQS 原則に従う:
  - **Command (`&mut self`)**: `fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>)`
  - **Command (`&mut self`)**: `fn shutdown(&mut self)`
  - **Query (`&self`)**: `fn supports_blocking(&self) -> bool { true }`
- `Executor` trait は core 層に置かれなければならない（`no_std` 対応）
- 複数所有者で executor を共有する唯一の経路は `ExecutorShared` でなければならない:
  ```rust
  pub struct ExecutorShared {
      inner: ArcShared<RuntimeMutex<Box<dyn Executor>>>,
  }

  impl Clone for ExecutorShared { /* ArcShared::clone */ }

  impl ExecutorShared {
      pub fn new<E: Executor + 'static>(executor: E) -> Self;
      pub fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>);
      pub fn shutdown(&self);
      pub fn supports_blocking(&self) -> bool;
  }

  impl SharedAccess<Box<dyn Executor>> for ExecutorShared {
      fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Executor>) -> R) -> R;
      fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Executor>) -> R) -> R;
  }
  ```
- `ExecutorShared::execute` は `with_write` でロックを取り、trait の `execute` に委譲する
- `ExecutorShared::execute` は submit 完了後速やかにロックを解放し、task の実行自体は executor の責任範囲（別スレッドや別 loop）で行われる
- `InlineExecutor`（現スレッド同期実行）は core 層に配置する。再入対策（inline で task 実行中に executor.execute が再帰呼び出しされる場合）は `InlineExecutor` 自身の実装内部で trampoline する（trait 層には持ち込まない）
- `TokioExecutor`、`ThreadedExecutor`、`PinnedExecutor`（1 スレッド dedicated）は std 層に配置する

#### 満たしてはいけない条件

- `Executor::execute` に `&self` を要求してはならない（内部可変性ポリシー違反）
- `DispatchExecutorRunner` 相当の「executor 共有のためだけの queue + mutex + running atomic」を再発明してはならない（共有は `ExecutorShared` の `RuntimeMutex` のみで済ませる）
- `ExecutorShared` を経由せずに `Box<dyn Executor>` を多所有状態にしてはならない
- `ExecutorShared` から `MutexGuard` を返す API を公開してはならない
- `InlineExecutor` を std 層に置いてはならない
- core 層から std / tokio 型へ直接依存してはならない
- Inline 実行時の再入対策を `ExecutorShared` 側で行ってはならない（`InlineExecutor` の実装詳細）

### 6. Mailbox を Runnable として扱う

#### 満たすべき条件

- mailbox は自らの `run(&self, throughput: NonZeroUsize, throughput_deadline: Option<Duration>)` を持ち、drain ループ本体を所有する
- `run` は次の順で動作する:
  - closed なら即 return
  - system message を全件処理
  - user message を throughput まで処理（throughput_deadline が定義されていればその時間内）
- mailbox の二重スケジュール防止は mailbox 自身の atomic state（`set_as_scheduled` / `set_as_idle`）の CAS で完結する
- mailbox は `Mailbox::new(actor: Weak<ActorCell>, queue: ArcShared<dyn MessageQueue>)` の形でコンストラクタが message queue を外部から注入できる
  - `actor` は `Weak<ActorCell>` で、circular reference 回避とライフサイクル分離のため。`run()` 実行時に `Weak::upgrade()` で actor を取得し、None なら early return
  - `queue` は `ArcShared<Box<dyn MessageQueue>>` 等、同一 instance を複数 mailbox で共有可能な形（BalancingDispatcher の SharingMailbox がこれを利用する）
- mailbox は dispatcher への参照を持たない。`register_for_execution` への終端コールバックは executor submit 時に closure がキャプチャする `MessageDispatcherShared`（= `ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>`）から呼ぶ

#### 満たしてはいけない条件

- drain ループを dispatcher 側に残してはならない
- mailbox に dispatcher への Weak reference を fields として持たせてはならない
- mailbox と dispatcher が `set_running` / `set_idle` を二重管理してはならない
- `MailboxOfferFuture` の async 経路のために mailbox 側で ScheduleAdapter を保持してはならない

### 6.5. `SharedMessageQueue` と `SharingMailbox`（BalancingDispatcher 用）

#### 満たすべき条件

- `SharedMessageQueue` は複数 actor が同時に enqueue / dequeue できる thread-safe な message queue として core 層に置く（no_std 対応）
  - 初版実装: `ArcShared<RuntimeMutex<VecDeque<Envelope>>>` ベースの単純な FIFO
  - 後で lock-free 化（crossbeam, mpmc 等）も可能だが本 change では scope 外
  - `MessageQueue` trait を実装し、enqueue / dequeue / len / is_empty が `&self` シグネチャを持つ（multi-consumer 許容）
- `SharingMailbox` は通常の Mailbox の薄いラッパとして core 層に置く
  - `Mailbox::new(actor: Weak<ActorCell>, queue: ArcShared<dyn MessageQueue>)` の seam を使い、`queue` として `SharedMessageQueue` を渡して構築する
  - `run()` の挙動は通常 Mailbox と同じ（system 全件 → user throughput まで dequeue）
  - **`clean_up()` の挙動だけ通常 Mailbox と異なる**: 通常 Mailbox は close 時に残り envelope を dead letter に流すが、SharingMailbox は **shared queue を drain しない**（queue は他の team member が引き続き使用するため、勝手に drain すると他 actor のメッセージが失われる）
- `BalancingDispatcher` を 2 つ別々に構築すると `shared_queue` も別物になる（instance 毎に独立した queue）

#### 満たしてはいけない条件

- `SharingMailbox` を `BalancingDispatcher` 以外の dispatcher で使用してはならない
- `SharingMailbox::clean_up()` が shared queue を drain してはならない（shared semantic を破壊する）
- `SharedMessageQueue` を直接外部 API として公開してはならない（`BalancingDispatcher` の internal detail）

### 7. async backpressure と `DispatcherWaker`

#### 満たすべき条件

- `MailboxOfferFuture::Pending` が返るとき、dispatcher は最終的に再スケジュール可能な Waker を提供しなければならない
- Waker 実装は core 層（`no_std` 対応）に `DispatcherWaker`（もしくは同等の最小実装）1 つだけ置く
- `DispatcherWaker` は `core::task::RawWaker` を用いて実装され、`wake` で `MessageDispatcherShared::register_for_execution(&mbox, false, true)` を呼ぶ
- std / tokio / 将来 embedded のいずれでも同じ Waker 実装が動く（Executor だけ差し替える）
- `ScheduleAdapter` trait / `ScheduleAdapterShared` / `InlineScheduleAdapter` / `ScheduleWaker` / `StdScheduleAdapter` は redesign 後に存在してはならない

#### 満たしてはいけない条件

- Waker 層を executor 実装ごとに別型で用意してはならない
- Waker の作成経路に trait 切替を挟んではならない（オーバーヘッド最小化）
- `MailboxOfferFuture` を同期 API で置換する方向へ後退してはならない

### 8. `MessageDispatcherConfigurator` と Registry

#### 満たすべき条件

- `MessageDispatcherConfigurator` trait は次のシグネチャを要求する:
  - `fn dispatcher(&self) -> MessageDispatcherShared`
  - trait method は `&self` でなければならない（内部可変性を用いずに成立させる）
  - 引数は不要（Pekko 準拠。スレッド名等は configurator 構築時に受け取った値か、`static AtomicUsize` の連番で採番する）
- 具象として次の 3 つを提供する:
  - `DefaultDispatcherConfigurator`: eager に `DefaultDispatcher` と `MessageDispatcherShared` を構築してフィールドとして保持し、`dispatcher(&self)` では `self.shared.clone()` を返す（`OnceLock` などの内部可変性を用いない）
  - `PinnedDispatcherConfigurator`: キャッシュせず、`dispatcher(&self)` の呼び出しのたびに新しい `PinnedDispatcher` を 1 スレッド専用 executor と共に構築し、`MessageDispatcherShared::new` で包んで返す
  - `BalancingDispatcherConfigurator`: eager に `SharedMessageQueue` と `BalancingDispatcher` と `MessageDispatcherShared` を構築してフィールドとして保持し、`dispatcher(&self)` では `self.shared.clone()` を返す（同一 id で resolve した actor は同じ shared queue を共有することになる）
- blocking workload 用 dispatcher は `DefaultDispatcherConfigurator` の変種として、blocking 対応 executor factory を差し込むことで表現する（新しい configurator 型を作らない）
- `Dispatchers` registry は `HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>>` を保持する（configurator は `&self` で query-only のため内部 mutex は不要、`ArcShared<Box<dyn _>>` で十分）
- `Dispatchers::resolve(&self, id)` は `MessageDispatcherShared` を返す
- **`Dispatchers::resolve` の呼び出し頻度契約**: 呼び出しは actor spawn / bootstrap の経路のみに限定する。message dispatch の hot path から `resolve` を呼んではならない。`PinnedDispatcherConfigurator` は呼び出しごとに新しい OS thread を生成するため、hot path 呼び出しは thread leak を引き起こす
- Pekko 互換 id（`pekko.actor.default-dispatcher` / `pekko.actor.internal-dispatcher` / `pekko.actor.default-blocking-io-dispatcher`）の正規化は先行 change の要件をそのまま維持する

#### 満たしてはいけない条件

- `DispatcherProvider` / `DispatcherBuilder` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` を 1 : 1 モデル前提の型として残してはならない
- `DispatcherSettings` を dispatcher の公開主語として残してはならない（configurator の内部へ移す）
- registry entry が provider と settings を別個に保持する設計を残してはならない（`ArcShared<Box<dyn MessageDispatcherConfigurator>>` 1 本に束ねる）

### 9. BalancingDispatcher V1 の構成と将来拡張 seam

#### 満たすべき条件

- 本 change で `BalancingDispatcher` を **V1** として実装する。V1 の機能スコープ:
  1. `SharedMessageQueue` を 1 つ持ち、attach した全 actor の `SharingMailbox` がそれを参照する
  2. `dispatch` / `system_dispatch` override で envelope を shared queue に enqueue する
  3. shared queue の dequeue は `SharingMailbox::run()` 経由で行われ、actor のいずれかが run した時に取り出される（自然な load balancing）
  4. **team tracking や teamWork fallback は V1 では実装しない**（YAGNI）
- 将来 V2 (teamWork load balancing fallback、active wake-up of idle members) を **既存ファイル無変更で additive に追加** できるよう、次の seam を保持する:
  1. `MessageDispatcher::create_mailbox` が trait メソッドとして存在する（V1 で `BalancingDispatcher` が override し `SharingMailbox` を返す）
  2. `MessageDispatcher::dispatch` / `system_dispatch` の戻り値が `Vec<ArcShared<Mailbox>>` 構造（V1 は `vec![receiver.mailbox()]` 単一要素、V2 では team member を複数要素として返せる）
  3. `MessageDispatcher::register_actor` / `unregister_actor` hook が存在し、V2 で team Vec の add/remove を override で挿入できる
  4. `Mailbox::new(actor, queue)` のように message queue を外部から注入可能なコンストラクタを持つ（V1 で `SharingMailbox` が利用）
  5. `MessageQueue` trait は multi-consumer を許容するシグネチャ（`&self` のみ、`&mut` を要求しない）— 既存の mailbox 実装でこのポリシーは既に成立しており、project 全体の例外扱いとして維持される
- 上記 seam の存在を tests ではなく trait / struct のシグネチャ上で担保する
- `attach` / `detach` 自体は `MessageDispatcherShared` 側の orchestration として提供され、`BalancingDispatcher` を含め具象型差分は trait の overridable hook 側だけで表現する

#### 満たしてはいけない条件

- 上記 seam のどれか 1 つでも塞ぐ設計を選んではならない
- V1 で teamWork fallback (active wake-up of idle team members) を実装してはならない（V2 で additive に追加可能にする）
- `SharedMessageQueue` を `BalancingDispatcher` 以外の dispatcher で使ってはならない

### 9.5. `DispatcherSettings`（新版、immutable settings snapshot）

#### 満たすべき条件

- `DispatcherSettings` は `DefaultDispatcher` / `PinnedDispatcher` / `DispatcherCore` / `DefaultDispatcherConfigurator` / `PinnedDispatcherConfigurator` に渡すための **immutable な settings bundle** として再定義する
- 旧版 `DispatcherSettings`（`throughput_deadline` / `starvation_deadline` / `schedule_adapter` を持っていたもの）とは **別物** であり、新版は次のフィールドのみを持つ:
  ```rust
  #[derive(Clone)]
  pub struct DispatcherSettings {
      pub id: String,
      pub throughput: NonZeroUsize,
      pub throughput_deadline: Option<Duration>,
      pub shutdown_timeout: Duration,
  }
  ```
- `DispatcherSettings` は core 層に配置し（`modules/actor-core/.../dispatcher_new/dispatcher_settings.rs`）、`no_std` 対応とする
- builder 風の更新メソッド（`with_throughput`, `with_throughput_deadline`, `with_shutdown_timeout` 等）は `self` を消費する `Self` 返しに統一する
- `DispatcherSettings` は `Clone` 可能で、複数の configurator から同じ settings を共有できる
- `DispatcherCore::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` のシグネチャで使用する
- `DefaultDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` も同様
- `PinnedDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` の場合、`settings.throughput` と `settings.throughput_deadline` は **構築時に Pinned 固有値（`NonZeroUsize::MAX` / `None`）に上書き** される（呼び出し側は何を渡しても良いが、結果として Pinned の固定値になる）

#### 満たしてはいけない条件

- 旧版 `DispatcherSettings` の `schedule_adapter` フィールドを残してはならない
- 旧版 `DispatcherSettings` の `starvation_deadline` フィールドを残してはならない（Pekko になく、YAGNI で初期版から除外。必要になったら orthogonal に追加）
- `DispatcherSettings` を mutable な共有 state として扱ってはならない（immutable snapshot として渡す）
- `DispatcherSettings` を dispatcher 公開抽象の **主語** として残してはならない（主語は `MessageDispatcher` trait と `MessageDispatcherShared`）。`DispatcherSettings` はあくまで構築時のパラメータ bundle

### 9.6. `dispatcher_new/` は旧 `dispatcher/` に依存してはならない

#### 満たすべき条件

- `modules/actor-core/src/core/kernel/dispatch/dispatcher_new/` 配下の実装は、旧 `modules/actor-core/src/core/kernel/dispatch/dispatcher/` 配下のいかなる型・関数・trait・モジュールも `use` / 参照してはならない (MUST NOT)
- 同様に `modules/actor-adaptor-std/src/std/dispatch_new/` 配下は旧 `modules/actor-adaptor-std/src/std/dispatch/` 配下を `use` / 参照してはならない
- 並走期間中、両者は **完全に独立した tree** として共存し、共通の helper や型を共有しない
- 同じ概念で同じロジックが必要な場合は、新側に独立して再実装する（コードの重複を許容してでも依存を切る）
- レビュー時には `grep -rn "use crate::core::kernel::dispatch::dispatcher::" modules/actor-core/src/core/kernel/dispatch/dispatcher_new/` で旧モジュールへの参照がないことを確認する

#### 満たしてはいけない条件

- 並走期間中に新側が旧側の型を再利用して短絡してはならない（例: 旧 `DispatchError` を新側が import する等）
- 旧側 helper 関数を「動くから」という理由で新側から呼んではならない
- 共通基盤を作って両側から参照させる「中間層」を新設してはならない（その中間層が削除タイミングのブロッカーになる）
- 旧側のテスト ユーティリティを新側のテストから流用してはならない

#### Why

並走期間中に新側が旧側に依存すると:
1. 旧側を一括削除する瞬間に circular な dependency が露呈する
2. 削除 PR が膨大になり、レビューと CI 通過が困難になる
3. 「気づかぬうちに新側が旧側のセマンティクスを引き継いでいる」という設計上の汚染が起きる

そのため、**並走期間中は両者が無関係**である状態を厳守し、最終的に旧側を `rm -rf` するだけで完了できる構造を維持する。

### 10. Feature Gating と並走

#### 満たすべき条件

- `tokio-executor` feature 無効時は `TokioExecutor` 系 dispatcher が存在しない扱いになる（先行 change の要件を維持）
- redesign 期間中、旧 `dispatcher/` モジュールと新 `dispatcher_new/` モジュールは並走する
- 並走期間中、fraktor 外部に公開される dispatcher 主語は新 `MessageDispatcher` trait と `MessageDispatcherShared` に統一される（旧 `DispatcherBuilder` 等は `#[doc(hidden)]` 扱いとして段階的に削除する）
- 並走終了条件: fraktor 内部の呼び出し元が全て新 API へ移行した時点で旧 `dispatcher/` を一括削除する

#### 満たしてはいけない条件

- 旧 dispatcher と新 dispatcher の同時使用を前提とした互換ブリッジ API を作成してはならない
- 移行期間が redesign 完了後も残存してはならない

### 11. ActorCell 影響

#### 満たすべき条件

- `MessageDispatcherShared::attach(actor)` が mailbox を生成して actor に install する設計に合わせ、`ActorCell` の生成順は 2-phase init を許容する形へ再設計しなければならない
- この redesign では `ActorCell` 自体を AShared 化することまでは要求しない
- 影響範囲は mailbox / dispatcher / sender の install 順序に限定し、`ActorCell` 全体へ広い内部可変性を導入しない

#### 満たしてはいけない条件

- `ActorCell` の大半の field を `Option` / `OnceLock` 化して初期化順問題を雑に回避してはならない
- `ActorCell` へ新たな広域 mutex を導入して attach/install を強引に通してはならない

## Acceptance Checklist

- `MessageDispatcher` trait と `MessageDispatcherShared` が dispatcher 公開抽象の中心として存在する
- `MessageDispatcher` trait の command / hook メソッドはすべて `&mut self`、query メソッドはすべて `&self` である（CQS 準拠）
- `MessageDispatcher` trait に `register_for_execution` / `execute_task` は存在しない（前者は shared wrapper、後者は本 change スコープ外）
- `MessageDispatcher::dispatch` / `system_dispatch` の戻り値が `Vec<ArcShared<Mailbox>>` (または equivalent な small-vec optimization) であり、shared wrapper 側で候補配列を順に register_for_execution する
- `MessageDispatcherShared` が既存の AShared 系 (`ActorFactoryShared` など) と同じパターンで定義され、`ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>` を内包する
- `MessageDispatcherShared` が `attach` / `detach` / `dispatch` / `system_dispatch` / `register_for_execution` の orchestration を担当する
- `DefaultDispatcher` / `PinnedDispatcher` / `BalancingDispatcher` の **3 具象型** が独立した struct として存在する
- `DispatcherCore` が pub で公開され、3 具象型の共通 state を保持する
- `DispatcherCore` の command メソッドはすべて `&mut self`、query メソッドはすべて `&self` である
- `DispatcherCore::mark_attach` / `mark_detach` は戻り値なしの純粋 command である（CQS 例外なし）
- `DispatcherCore::schedule_shutdown_if_sensible` の戻り値が `ShutdownSchedule` であり、CQS 許容例外として `MessageDispatcherShared::detach` が lock 解放前に値を copy するために使われる
- `DispatcherCore` の field に `AtomicI64` / `AtomicU8` / `Mutex<T>` などの内部可変性が存在しない
- 1 dispatcher が複数 actor を同時に収容できる
- `MessageDispatcherShared::attach` / `detach` が `inhabitants` を増減し、0 到達後 `shutdown_timeout` で auto-shutdown が発火する
- `PinnedDispatcher` が actor 単位の専用 lane を持ち、owner check で共有を拒否し、`SpawnError::DispatcherAlreadyOwned` を返す
- `BalancingDispatcher` が `SharedMessageQueue` + `SharingMailbox` の組み合わせで複数 actor 間の load balancing を実現する
- `SharedMessageQueue` / `SharingMailbox` が core 層に置かれ、`BalancingDispatcher` 以外からは使われない
- `Executor` trait が `execute(&mut self, ...)` / `shutdown(&mut self)` / `supports_blocking(&self)` の CQS 分類で定義されている
- `ExecutorShared` が `ArcShared<RuntimeMutex<Box<dyn Executor>>>` を内包する AShared wrapper として定義されている
- `DispatcherCore::executor` が `ExecutorShared` を保持し、raw `ArcShared<Box<dyn Executor>>` を保持しない
- `DispatchExecutorRunner` 相当の「executor 共有のためだけの queue + mutex + running atomic」が存在しない
- mailbox が `run()` を持ち、drain ループが mailbox 側に存在する
- `DispatcherWaker` 1 実装だけで async backpressure が維持される
- `MessageDispatcherConfigurator` trait と `DefaultDispatcherConfigurator` / `PinnedDispatcherConfigurator` / `BalancingDispatcherConfigurator` の **3 具象** が存在する
- registry が `ArcShared<Box<dyn MessageDispatcherConfigurator>>` を id 引きで解決し、`MessageDispatcherShared` を返す
- `Dispatchers::resolve` の呼び出しは spawn / bootstrap の経路に限定され、message hot path から呼ばれない（trait doc に明記）
- 新版 `DispatcherSettings` が `id` / `throughput` / `throughput_deadline` / `shutdown_timeout` のみを持つ pub struct として core に存在する
- 旧版 `DispatcherSettings` の `schedule_adapter` / `starvation_deadline` フィールドが新版に引き継がれていない
- `DispatcherCore::new` / `DefaultDispatcher::new` / `PinnedDispatcher::new` / `BalancingDispatcher::new` / `DefaultDispatcherConfigurator::new` がすべて `DispatcherSettings` を受け取る形で統一されている
- `SpawnError::DispatcherAlreadyOwned` バリアントが追加されている
- mailbox / overflow strategy が要求する blocking compatibility が `executor.supports_blocking()` と照合され、不適合な組み合わせは `SpawnError::InvalidMailboxConfig` で attach 時に拒否される
- 旧 `DispatcherShared` / `DispatchShared` / `DispatcherCore`（旧）/ 旧 `DispatcherSettings` / `DispatchExecutorRunner` / `ScheduleAdapter*` / `DispatcherBuilder` / `DispatcherProvider` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` が削除されている
- 並走期間中 `dispatcher_new/` 配下が旧 `dispatcher/` 配下のいかなる型・関数も `use` していない（絶対 import だけでなく grouped / 相対 import を含めてゼロヒット）
- 並走期間中 `std/dispatch_new/` 配下が旧 `std/dispatch/` 配下のいかなる型・関数も `use` していない（同様の検査でゼロヒット）
- BalancingDispatcher V1 + V2 拡張 seam 5 項目が trait / struct のシグネチャ上で確認できる
- 1:N 共有 dispatcher の contention が bench もしくは diagnostics で観測され、既知トレードオフとして記録される
- BalancingDispatcher V1 の load balancing が integration test で確認される（複数 actor が同じ shared queue から消化することを確認）
- `./scripts/ci-check.sh ai all` が成功する

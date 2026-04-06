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

### 2. `MessageDispatcher` trait（CQS 準拠）

#### 満たすべき条件

- `MessageDispatcher` trait は dispatcher 公開抽象の単一の主語でなければならない
- trait メソッドは CQS 原則に従って `&self`（query）と `&mut self`（command）を厳密に分ける
- trait は Pekko の `MessageDispatcher` 抽象クラスの契約を Rust へ写した次のメソッド群を要求する（`Send + Sync` を含む）:

  **Queries (`&self`, 戻り値あり)**
  - `id(&self) -> &str`
  - `throughput(&self) -> NonZeroUsize`
  - `throughput_deadline(&self) -> Option<Duration>`
  - `shutdown_timeout(&self) -> Duration`
  - `inhabitants(&self) -> i64`
  - `executor(&self) -> ExecutorShared`（clone を返す。`MessageDispatcherShared::register_for_execution` の再スケジュール経路が短時間の read lock 越しにこれを取り出す）

  **Template methods (trait default impl、具象は override しない)**
  - `attach(&mut self, actor: &ActorCell) -> Result<(), SpawnError>`
    - default impl: `self.register_actor(actor)?` → `self.create_mailbox(actor, ...)` を呼んで mailbox を作り actor に設定 → `self.register_for_execution(&mbox, false, true)` を呼ぶ
    - Pekko の `MessageDispatcher.attach`（`final`）に対応
  - `detach(&mut self, actor: &ActorCell)`
    - default impl: `self.unregister_actor(actor)`（この中で `DispatcherCore::add_inhabitants(-1)` と `schedule_shutdown_if_sensible` を呼ぶ）
    - Pekko の `MessageDispatcher.detach`（`final`）に対応

  **Overridable hooks (protected 相当、具象型が override 可能)**
  - `register_actor(&mut self, actor: &ActorCell) -> Result<(), SpawnError>`
    - default impl: `self.core_mut().add_inhabitants(1)` を呼ぶ
    - `PinnedDispatcher` は owner check + owner 設定を追加する
    - 将来の `BalancingDispatcher` は `team.add(actor)` を追加する
    - Pekko の `register(actorCell)` に対応
  - `unregister_actor(&mut self, actor: &ActorCell)`
    - default impl: `self.core_mut().add_inhabitants(-1)` → `self.core_mut().schedule_shutdown_if_sensible()` を呼ぶ
    - `PinnedDispatcher` は owner を `None` に戻す
    - 将来の `BalancingDispatcher` は `team.remove(actor)` + `team_work()` を追加する
    - Pekko の `unregister(actor)` に対応
  - `dispatch(&mut self, receiver: &ActorCell, envelope: Envelope) -> Result<(), SendError>`
    - default impl: `receiver.mailbox().enqueue_user(envelope)?` → `self.register_for_execution(&mbox, true, false)`
    - 将来の `BalancingDispatcher` は shared queue へ直接 enqueue する形で override する
  - `system_dispatch(&mut self, receiver: &ActorCell, msg: SystemMessage) -> Result<(), SendError>`
    - default impl: `receiver.mailbox().enqueue_system(msg)?` → `self.register_for_execution(&mbox, false, true)`
  - `register_for_execution(&mut self, mbox: &ArcShared<Mailbox>, has_message_hint: bool, has_system_hint: bool) -> bool`
    - default impl: mailbox の `can_be_scheduled_for_execution` / `set_as_scheduled` の CAS 判定のみを行う。実際の closure 組み立てと executor submit は `MessageDispatcherShared::register_for_execution` 側で行う
    - Pekko 契約に合わせ、実行可否の戻り値あり（CQS 許容例外として人間許可済みと扱う）
  - `suspend(&mut self, actor: &ActorCell)`
  - `resume(&mut self, actor: &ActorCell)`
  - `execute_task(&mut self, task: Box<dyn FnOnce() + Send + 'static>)`
  - `shutdown(&mut self)`
    - default impl: `self.core_mut().shutdown()` を呼ぶ
  - `create_mailbox(&self, actor: &ActorCell, mailbox_type: &dyn MailboxType) -> ArcShared<Mailbox>`
    - **`&self`**（factory メソッド、状態を変えない）
    - default impl: 通常の per-actor mailbox を返す
    - 将来の `BalancingDispatcher` は `SharingMailbox`（shared queue を注入した mailbox）を返す形で override する

  **Core accessor (trait object から `DispatcherCore` へ到達するため)**
  - `core(&self) -> &DispatcherCore`
  - `core_mut(&mut self) -> &mut DispatcherCore`

- trait object `Box<dyn MessageDispatcher>` として `MessageDispatcherShared` から取り回せなければならない
- 具象型は CQS と内部可変性ポリシーに従い、`&mut self` を内部可変性で偽装してはならない

#### 満たしてはいけない条件

- command メソッドに `&self` を要求してはならない（内部可変性ポリシー違反。ただし `create_mailbox` は状態を変えない factory なので `&self` が正しい）
- `attach` / `detach` を具象型が override してはならない（Pekko の `final` に相当する規律。Rust は `final` を表現できないため、trait doc と review での運用規約として扱う）
- `Dispatcher` という単純名を trait 名として残してはならない（Pekko の抽象クラス名に合わせる）
- trait method を通じてガードやロックを外部へ返してはならない（ロック区間は `MessageDispatcherShared` 内部に閉じる）

### 2.5 `MessageDispatcherShared`（AShared パターン）

#### 満たすべき条件

- 複数スレッド・複数所有者で `MessageDispatcher` を共有する唯一の経路は `MessageDispatcherShared` でなければならない
- `MessageDispatcherShared` は既存の `ActorRefSenderShared` と同じ AShared パターンを踏襲する:
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
- `MessageDispatcherShared::register_for_execution` は次の順で動作する:
  1. mailbox の `can_be_scheduled_for_execution` / `set_as_scheduled` を（ロック外で）評価する
  2. CAS 成功時、`with_read` で throughput / throughput_deadline / executor_shared を一度に取り出す（短時間の read lock）
  3. ロックを解放した状態で closure を組み立てる（`self.clone()` と `mbox.clone()` を capture）
  4. closure 内では `mbox.run(throughput, deadline)` → `mbox.set_as_idle()` → `self.register_for_execution(&mbox, false, false)` の順で実行
  5. 組み立てた closure を `ExecutorShared::execute` 経由で submit する
- `MessageDispatcherShared` は便利メソッドとして trait の主要 command / query を委譲する（`attach` / `detach` / `dispatch` / `system_dispatch` / `id` / `throughput` 等）
- 再入デッドロック防止のため、`ActorRefSenderShared::send` と同様にロック区間を最小化する: 副作用を伴う closure の実行は `with_write` のブロックを抜けた後で行う

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
  - `add_inhabitants(&mut self, delta: i64) -> i64`（CQS 許容例外: Pekko の `addInhabitants: Long` と合わせて合成後カウントを返す。`ifSensibleToDoSoThenScheduleShutdown` の判定に必要）
  - `schedule_shutdown_if_sensible(&mut self)`
  - `shutdown(&mut self)`（内部で `self.executor.shutdown()` を呼び、`shutdown_schedule` を UNSCHEDULED に戻す）

- `DispatcherCore` は atomic field を field として公開しない（内部可変性を封じる）。inhabitants とスケジュール状態は `&mut self` ロック越しに更新する
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
  - `attach` / `detach` は trait default impl を使う
  - hook メソッド (`register_actor`, `unregister_actor`, `dispatch`, `system_dispatch`, `register_for_execution`, `shutdown`) も trait default impl を使う
- `PinnedDispatcher` は次を満たす:
  - `core: DispatcherCore` と `owner: Option<ActorCellId>` を保持する（内部可変性は使わない）
  - `DispatcherCore::new` 呼び出し時に `throughput = NonZeroUsize::MAX`, `throughput_deadline = None` を固定で渡す（Pekko: `Int.MaxValue` / `Duration.Zero`）
  - `register_actor` hook を override: owner が `None` か同一 actor のときのみ owner をセットして default 処理を実行、別 actor が既に owner なら `SpawnError::DispatcherAlreadyOwned` を返す
  - `unregister_actor` hook を override: owner を `None` に戻してから default 処理を実行
  - 同一 actor の再 attach を許容する（Pekko の `register` override と同じ挙動）
  - `attach` / `detach` 本体は trait default impl をそのまま使う（override しない）
- 両具象型は `MessageDispatcherShared::new(concrete)` 経由で `MessageDispatcherShared` へ格納可能でなければならない（直接 `ArcShared<dyn MessageDispatcher>` のような形で多所有化しない）
- **blocking workload 向け dispatcher は `DefaultDispatcher` の別 id + 別 `ExecutorShared` 構成で表現する**。`BlockingDispatcher` という具象型は作らない（Pekko にも存在しない。Pekko では `reference.conf` で `pekko.actor.default-blocking-io-dispatcher` を `type = "Dispatcher"` として別 pool 設定で登録するだけ）

#### 満たしてはいけない条件

- `DefaultDispatcher` / `PinnedDispatcher` を std 層（`actor-adaptor-std`）に配置してはならない
- `DefaultDispatcher` / `PinnedDispatcher` に tokio / std::thread / std 固有型への型レベル依存を導入してはならない
- `DefaultDispatcher` と `PinnedDispatcher` の差分を enum 変種やフラグで潰してはならない（Pekko の型階層を忠実に写す）
- `PinnedDispatcher` の 1 : 1 制約を runtime assert のみで表現し、register 時に拒否しない設計にしてはならない
- `DefaultDispatcher` に blocking workload 用 executor を同居させ、別 dispatcher として扱わないようにしてはならない（blocking は別 id + 別 configurator で表現する）
- **`BlockingDispatcher` という独立した具象型を作ってはならない**（Pekko に存在しない、`DefaultDispatcher` + blocking executor factory で表現する）

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
- mailbox は `Mailbox::new(actor, queue)` の形でコンストラクタが message queue を外部から注入できなければならない（Balancing の shared queue 注入に備える）
- mailbox は dispatcher への参照を持たない。`register_for_execution` への終端コールバックは executor submit 時に closure がキャプチャする `MessageDispatcherShared`（= `ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>`）から呼ぶ

#### 満たしてはいけない条件

- drain ループを dispatcher 側に残してはならない
- mailbox に dispatcher への Weak reference を fields として持たせてはならない
- mailbox と dispatcher が `set_running` / `set_idle` を二重管理してはならない
- `MailboxOfferFuture` の async 経路のために mailbox 側で ScheduleAdapter を保持してはならない

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
- 具象として次の 2 つを提供する:
  - `DefaultDispatcherConfigurator`: eager に `DefaultDispatcher` と `MessageDispatcherShared` を構築してフィールドとして保持し、`dispatcher(&self)` では `self.shared.clone()` を返す（`OnceLock` などの内部可変性を用いない）
  - `PinnedDispatcherConfigurator`: キャッシュせず、`dispatcher(&self)` の呼び出しのたびに新しい `PinnedDispatcher` を 1 スレッド専用 executor と共に構築し、`MessageDispatcherShared::new` で包んで返す
- blocking workload 用 dispatcher は `DefaultDispatcherConfigurator` の変種として、blocking 対応 executor factory を差し込むことで表現する（新しい configurator 型を作らない）
- `Dispatchers` registry は `HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>>` を保持する（configurator は `&self` で query-only のため内部 mutex は不要、`ArcShared<Box<dyn _>>` で十分）
- `Dispatchers::resolve(&self, id)` は `MessageDispatcherShared` を返す
- Pekko 互換 id（`pekko.actor.default-dispatcher` / `pekko.actor.internal-dispatcher` / `pekko.actor.default-blocking-io-dispatcher`）の正規化は先行 change の要件をそのまま維持する

#### 満たしてはいけない条件

- `DispatcherProvider` / `DispatcherBuilder` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` を 1 : 1 モデル前提の型として残してはならない
- `DispatcherSettings` を dispatcher の公開主語として残してはならない（configurator の内部へ移す）
- registry entry が provider と settings を別個に保持する設計を残してはならない（`ArcShared<Box<dyn MessageDispatcherConfigurator>>` 1 本に束ねる）

### 9. Balancing 拡張のための seam

#### 満たすべき条件

- 新設計は将来 `BalancingDispatcher` を追加する際、既存ファイル無変更で追加のみで済むように次の seam を固守する:
  1. `MessageDispatcher::create_mailbox` が trait メソッドとして存在する（`BalancingDispatcher` は shared queue を注入した `SharingMailbox` 相当を返す）
  2. `MessageDispatcher::register_for_execution` は戻り値 `bool` を返し、override 可能（`BalancingDispatcher` の team work 発動判定に必要）
  3. `MessageDispatcher::dispatch` は trait メソッドとして override 可能（`BalancingDispatcher` は shared queue へ直接 enqueue する）
  4. `MessageDispatcher` に `register_actor(&mut self, actor) -> Result<(), SpawnError>` / `unregister_actor(&mut self, actor)` の protected 相当 hook が存在する（`BalancingDispatcher` は `team.add` / `team.remove` + `teamWork()` を行う）
  5. `Mailbox::new(actor, queue)` のように message queue を外部から注入可能なコンストラクタを持つ
  6. `MessageQueue` trait は multi-consumer を許容するシグネチャ（`&self` のみ、`&mut` を要求しない）— 既存の mailbox 実装でこのポリシーは既に成立しており、project 全体の例外扱いとして維持される
- 上記 seam の存在を tests ではなく trait / struct のシグネチャ上で担保する
- `attach` / `detach` 自体は Pekko の `final` と同じく trait の default impl として提供され、`BalancingDispatcher` を含めいかなる具象型も override しない（override するのは上記 hooks 側）

#### 満たしてはいけない条件

- Balancing を初期リリースに含めてはならない（YAGNI）
- 上記 seam のどれか 1 つでも塞ぐ設計を選んではならない

### 10. Feature Gating と並走

#### 満たすべき条件

- `tokio-executor` feature 無効時は `TokioExecutor` 系 dispatcher が存在しない扱いになる（先行 change の要件を維持）
- redesign 期間中、旧 `dispatcher/` モジュールと新 `dispatcher_new/` モジュールは並走する
- 並走期間中、fraktor 外部に公開される dispatcher 主語は新 `MessageDispatcher` trait のみに統一される（旧 `DispatcherBuilder` 等は `#[doc(hidden)]` 扱いとして段階的に削除する）
- 並走終了条件: fraktor 内部の呼び出し元が全て新 API へ移行した時点で旧 `dispatcher/` を一括削除する

#### 満たしてはいけない条件

- 旧 dispatcher と新 dispatcher の同時使用を前提とした互換ブリッジ API を作成してはならない
- 移行期間が redesign 完了後も残存してはならない

## Acceptance Checklist

- `MessageDispatcher` trait が dispatcher 公開抽象の単一の主語として存在する
- `MessageDispatcher` trait の command メソッドはすべて `&mut self`、query メソッドはすべて `&self` である（CQS 準拠）
- `MessageDispatcherShared` が既存 `ActorRefSenderShared` と同じ AShared パターンで定義され、`ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>` を内包する
- `DefaultDispatcher` と `PinnedDispatcher` が独立した具象型として存在する
- `DispatcherCore` が pub で公開され、両具象型の共通 state を保持する
- `DispatcherCore` の command メソッドはすべて `&mut self`、query メソッドはすべて `&self` である
- `DispatcherCore` の field に `AtomicI64` / `AtomicU8` / `Mutex<T>` などの内部可変性が存在しない
- 1 dispatcher が複数 actor を同時に収容できる
- `attach` / `detach` が `inhabitants` を増減し、0 到達後 `shutdown_timeout` で auto-shutdown が発火する
- `PinnedDispatcher` が actor 単位の専用 lane を持ち、owner check で共有を拒否する
- `Executor` trait が `execute(&mut self, ...)` / `shutdown(&mut self)` / `supports_blocking(&self)` の CQS 分類で定義されている
- `ExecutorShared` が `ArcShared<RuntimeMutex<Box<dyn Executor>>>` を内包する AShared wrapper として定義されている
- `DispatcherCore::executor` が `ExecutorShared` を保持し、raw `ArcShared<Box<dyn Executor>>` を保持しない
- `DispatchExecutorRunner` 相当の「executor 共有のためだけの queue + mutex + running atomic」が存在しない
- mailbox が `run()` を持ち、drain ループが mailbox 側に存在する
- `DispatcherWaker` 1 実装だけで async backpressure が維持される
- `MessageDispatcherConfigurator` trait と `DefaultDispatcherConfigurator` / `PinnedDispatcherConfigurator` の 2 具象が存在する
- registry が `ArcShared<Box<dyn MessageDispatcherConfigurator>>` を id 引きで解決し、`MessageDispatcherShared` を返す
- 旧 `DispatcherShared` / `DispatchShared` / `DispatcherCore`（旧）/ `DispatchExecutorRunner` / `ScheduleAdapter*` / `DispatcherBuilder` / `DispatcherProvider` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` が削除されている
- Balancing 拡張 seam 5 項目が trait / struct のシグネチャ上で確認できる
- `./scripts/ci-check.sh ai all` が成功する

## MODIFIED Requirements

### Requirement: dispatcher public abstraction は `MessageDispatcher` trait と `MessageDispatcherShared` を中心としなければならない

dispatcher の public abstraction は Pekko の `MessageDispatcher` 抽象クラスに対応する `MessageDispatcher` trait と、その共有経路を提供する `MessageDispatcherShared`（AShared パターン）を中心に定義されなければならない (MUST)。`Dispatcher` / `DispatcherProvider` のような汎称名や、`DispatcherConfig` / 旧 `DispatcherShared` / `DispatchExecutor` / `DispatchExecutorRunner` のような runtime primitive を public concept の主語にしてはならない (MUST NOT)。

#### Scenario: public surface に MessageDispatcher trait が現れる
- **WHEN** `core::kernel::dispatch` の公開面を確認する
- **THEN** `MessageDispatcher` trait が公開 API として存在する
- **AND** `MessageDispatcher` trait は `Send + Sync` を要求する
- **AND** `MessageDispatcher` trait は `Box<dyn MessageDispatcher>` として trait object 化できる

#### Scenario: MessageDispatcher trait は CQS 準拠の query / hook 群を要求する
- **WHEN** `MessageDispatcher` trait のメソッドを確認する
- **THEN** query メソッドは `&self` をレシーバとする: `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants`, `executor`, `core`
- **AND** factory メソッド `create_mailbox(&self, actor, mailbox_type: &dyn MailboxType) -> ArcShared<Mailbox>` も `&self` である（状態を変えない）
- **AND** hook メソッドは `&mut self` で override 可能: `register_actor`, `unregister_actor`, `dispatch`, `system_dispatch`, `suspend`, `resume`, `shutdown`
- **AND** actor を受け取る hook / factory メソッドの actor 引数型はすべて `&ArcShared<ActorCell>` に統一される
- **AND** `core_mut(&mut self) -> &mut DispatcherCore` が必須メソッドとして存在する
- **AND** `dispatch` / `system_dispatch` の戻り値型は `Result<Vec<ArcShared<Mailbox>>, SendError>` であり、shared wrapper が lock 解放後に register_for_execution を試みる候補 mailbox 配列を返す
- **AND** command メソッドを `&self` + 内部可変性で偽装する実装は存在しない
- **AND** trait に `register_for_execution` メソッドは存在しない（shared wrapper の純粋 CAS + executor submit 経路に集約）
- **AND** trait に `execute_task` メソッドは存在しない（本 change スコープ外）

#### Scenario: MessageDispatcherShared は lifecycle orchestration を提供する
- **WHEN** `MessageDispatcherShared` の public API を確認する
- **THEN** `attach`, `detach`, `dispatch`, `system_dispatch`, `register_for_execution` が `MessageDispatcherShared` に存在する
- **AND** これらは `with_write` / `with_read` を使って trait hook と query を組み合わせる
- **AND** executor submit や delayed shutdown 登録のような lock 解放後副作用は `MessageDispatcherShared` が担当する

#### Scenario: create_mailbox は外部から直接呼ばれない運用規律
- **WHEN** `MessageDispatcher::create_mailbox` の呼び出し方を確認する
- **THEN** 外部 caller は `create_mailbox` を直接呼ばない（trait doc に明記される）
- **AND** mailbox の作成は常に `MessageDispatcherShared::attach` 経由で行う
- **AND** inhabitants 管理を経由しない mailbox 生成経路は存在しない

#### Scenario: MessageDispatcherShared は AShared パターンに従う
- **WHEN** `MessageDispatcherShared` の定義を確認する
- **THEN** `MessageDispatcherShared` は `pub struct` として公開されている
- **AND** 内部に `ActorLockProvider` が materialize した opaque lock backend を保持する
- **AND** opaque lock backend の concrete 型・公開 trait 名は public API として露出しない
- **AND** `Clone` を実装する（内部 backend の clone ベース）
- **AND** `SharedAccess<Box<dyn MessageDispatcher>>` を実装し、`with_read` / `with_write` を提供する
- **AND** 既存の AShared 系 (`ActorFactoryShared` など) と同じパターンに従っている

#### Scenario: public surface は runtime primitive を主語にしない
- **WHEN** dispatcher 関連の public API を確認する
- **THEN** `DispatcherCore` と `MessageDispatcherShared` を除き、旧 `DispatcherShared`、`DispatchShared`、`DispatchExecutor`、`DispatchExecutorRunner`、`DispatcherBuilder`、`DispatcherProvider`、`DispatcherProvisionRequest`、`DispatcherRegistryEntry`、`ConfiguredDispatcherBuilder` は存在しない
- **AND** `DispatcherSettings` は configurator の internal detail へ移され、公開抽象の主語として残らない

#### Scenario: DispatcherCore は pub struct として公開される
- **WHEN** `core::kernel::dispatch::dispatcher` の公開面を確認する
- **THEN** `DispatcherCore` は `pub struct` として公開される
- **AND** `DispatcherCore` は `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `executor: ExecutorShared`, `inhabitants: i64`, `shutdown_schedule: ShutdownSchedule` を状態として保持する
- **AND** `DispatcherCore` のフィールドには `AtomicI64` / `AtomicU8` / `Mutex<T>` / `UnsafeCell<T>` などの内部可変性が存在しない
- **AND** `DispatcherCore` は query メソッド (`&self`): `id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants`, `executor` を pub で提供する
- **AND** `DispatcherCore` は command メソッド (`&mut self`): `mark_attach`, `mark_detach`, `schedule_shutdown_if_sensible`, `shutdown` を pub で提供する

### Requirement: `DispatcherSettings` は新版として再定義され immutable bundle として扱われる

新版 `DispatcherSettings` は dispatcher 構築時に渡す immutable な settings bundle として再定義されなければならない (MUST)。旧版 `DispatcherSettings` の `schedule_adapter` / `starvation_deadline` フィールドは新版に引き継がれてはならない (MUST NOT)。

#### Scenario: 新版 DispatcherSettings は dispatcher 構築パラメータとして提供される
- **WHEN** `DispatcherSettings` の field 構造を確認する
- **THEN** 次の field のみが存在する: `id: String`, `throughput: NonZeroUsize`, `throughput_deadline: Option<Duration>`, `shutdown_timeout: Duration`
- **AND** `schedule_adapter` field は存在しない
- **AND** `starvation_deadline` field は存在しない
- **AND** `Clone` が実装されている

#### Scenario: DispatcherSettings は dispatcher / configurator のコンストラクタに渡される
- **WHEN** dispatcher と configurator の `new` のシグネチャを確認する
- **THEN** `DispatcherCore::new(settings: &DispatcherSettings, executor: ExecutorShared) -> Self` が存在する
- **AND** `DefaultDispatcher::new(settings: &DispatcherSettings, executor: ExecutorShared, provider: ArcShared<dyn ActorLockProvider>) -> Self` が存在する
- **AND** `PinnedDispatcher::new(settings: &DispatcherSettings, executor: ExecutorShared, provider: ArcShared<dyn ActorLockProvider>) -> Self` が存在する
- **AND** `BalancingDispatcher::new(settings: &DispatcherSettings, executor: ExecutorShared, provider: ArcShared<dyn ActorLockProvider>) -> Self` が存在する
- **AND** `PinnedDispatcher::new` は受け取った `settings` を `throughput = NonZeroUsize::MAX`, `throughput_deadline = None` に上書きしてから core に渡す
- **AND** `BalancingDispatcher::new` は受け取った `settings` をそのまま core に渡し、内部で新しい `SharedMessageQueue` を生成する
- **AND** `DefaultDispatcherConfigurator::new(settings: &DispatcherSettings, executor: ExecutorShared, provider: ArcShared<dyn ActorLockProvider>)` が存在する
- **AND** `BalancingDispatcherConfigurator::new(settings: &DispatcherSettings, executor: ExecutorShared, provider: ArcShared<dyn ActorLockProvider>)` が存在する
- **AND** `PinnedDispatcherConfigurator::new(settings: DispatcherSettings, executor_factory: ArcShared<Box<dyn ExecutorFactory>>, provider: ArcShared<dyn ActorLockProvider>, thread_name_prefix: impl Into<String>)` が存在する
- **AND** `ActorLockProvider` は `DispatcherSettings` 自体には埋め込まれず、constructor 引数として別に渡される

#### Scenario: DispatcherSettings は public abstraction の主語ではない
- **WHEN** dispatcher 公開抽象を確認する
- **THEN** dispatcher 公開抽象の主語は `MessageDispatcher` trait と `MessageDispatcherShared` のみである
- **AND** `DispatcherSettings` は dispatcher の構築時に渡すパラメータ bundle として位置づけられている
- **AND** `DispatcherSettings` を持って dispatcher の挙動を切り替える type-level dispatch は存在しない

### Requirement: 3 種の Configurator が異なるインスタンス戦略を取る

configurator の `dispatcher()` は Pekko の `DispatcherConfigurator` / `PinnedDispatcherConfigurator` / `BalancingDispatcherConfigurator` と同じインスタンス戦略を取らなければならない (MUST)。

#### Scenario: DefaultDispatcherConfigurator は同一 MessageDispatcherShared を clone して返す
- **WHEN** `DefaultDispatcherConfigurator::dispatcher(&self)` を 2 回呼ぶ
- **THEN** 返される `MessageDispatcherShared` は同じ shared instance を clone して指す
- **AND** configurator の `new` で eager に 1 回だけ `DefaultDispatcher` を構築する
- **AND** `OnceLock` などの内部可変性を configurator 内部に持たない

#### Scenario: PinnedDispatcherConfigurator は毎回新規 MessageDispatcherShared を返す
- **WHEN** `PinnedDispatcherConfigurator::dispatcher(&self)` を 2 回呼ぶ
- **THEN** 返される `MessageDispatcherShared` は毎回新規の shared instance を指す
- **AND** 各 instance は 1 スレッド専用 executor を別々に保持する
- **AND** 各 instance の `throughput` は `NonZeroUsize::MAX` を返す
- **AND** configurator 自身は `&self` で呼び出し可能（内部可変性なし、引数なし）

#### Scenario: BalancingDispatcherConfigurator は同一 MessageDispatcherShared を clone して返す
- **WHEN** `BalancingDispatcherConfigurator::dispatcher(&self)` を 2 回呼ぶ
- **THEN** 返される `MessageDispatcherShared` は同じ shared instance を clone して指す
- **AND** これにより同じ id で resolve した複数 actor は同じ `SharedMessageQueue` を共有する (load balancing が成立する)
- **AND** configurator の `new` で eager に 1 回だけ `BalancingDispatcher` (および `SharedMessageQueue`) を構築する
- **AND** `OnceLock` などの内部可変性を configurator 内部に持たない

#### Scenario: Blocking dispatcher は DefaultDispatcherConfigurator の別 id 登録で表現される
- **WHEN** blocking workload 用の dispatcher を登録する
- **THEN** それは `DefaultDispatcherConfigurator` を blocking 対応 `ExecutorFactory` で構築し、別 id で registry に登録する形で表現される
- **AND** `BlockingDispatcher` という専用 type は存在しない
- **AND** 予約 id `pekko.actor.default-blocking-io-dispatcher` の解決は先行 change の要件を維持する

#### Scenario: Dispatchers::resolve は spawn / bootstrap 経路にのみ呼ばれる
- **WHEN** `Dispatchers::resolve(id)` の trait doc を確認する
- **THEN** 「呼び出しは actor spawn / bootstrap 経路に限定する。message hot path から呼んではならない」と明記されている
- **AND** 理由として「`PinnedDispatcherConfigurator` は呼び出しごとに新 thread を生成するため、hot path 呼び出しは thread leak を引き起こす」が記載されている

## ADDED Requirements

### Requirement: dispatcher configurator は `ActorLockProvider` を束縛して dispatcher shared を生成する

この Requirement は、既存 Requirement「3 種の Configurator が異なるインスタンス戦略を取る」に対して、instance 戦略とは直交する「provider family をどこで束縛するか」という観点を追加しなければならない（MUST）。

`MessageDispatcherConfigurator` 実装は、`ActorLockProvider` を構築時に束縛し、その provider を使って `MessageDispatcherShared` を生成しなければならない（MUST）。`dispatcher()` 呼び出し時に global state から provider を解決したり、public generic parameter を通して driver family を露出したりしてはならない（MUST NOT）。

#### Scenario: DefaultDispatcherConfigurator は provider を束縛した shared instance を返す
- **WHEN** `DefaultDispatcherConfigurator` を同じ `ActorLockProvider` で構築して `dispatcher()` を 2 回呼ぶ
- **THEN** 返される `MessageDispatcherShared` は同じ provider family で構築された shared instance を指す
- **AND** public abstraction は引き続き `MessageDispatcherShared` のままである

#### Scenario: BalancingDispatcherConfigurator は provider を束縛した shared queue path を返す
- **WHEN** `BalancingDispatcherConfigurator` を特定の `ActorLockProvider` で構築する
- **THEN** `dispatcher()` が返す `MessageDispatcherShared`、その executor、mailbox hot path は同じ provider family で構築される
- **AND** load balancing の公開契約は維持される

#### Scenario: PinnedDispatcherConfigurator は毎回新規 instance でも provider family は固定される
- **WHEN** `PinnedDispatcherConfigurator` を 1 つの `ActorLockProvider` で構築して `dispatcher()` を複数回呼ぶ
- **THEN** 返される `MessageDispatcherShared` は毎回新しい instance である
- **AND** すべて同じ `ActorLockProvider` family で構築される
- **AND** public API に driver generic parameter は現れない

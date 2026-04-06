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
- **AND** hook メソッドは `&mut self` で override 可能: `register_actor`, `unregister_actor`, `dispatch`, `system_dispatch`, `register_for_execution`, `suspend`, `resume`, `execute_task`, `shutdown`
- **AND** `core_mut(&mut self) -> &mut DispatcherCore` が必須メソッドとして存在する
- **AND** `register_for_execution` は戻り値 `bool` を返す（Pekko 契約に合わせた CQS 許容例外）
- **AND** command メソッドを `&self` + 内部可変性で偽装する実装は存在しない

#### Scenario: MessageDispatcherShared は lifecycle orchestration を提供する
- **WHEN** `MessageDispatcherShared` の public API を確認する
- **THEN** `attach`, `detach`, `dispatch`, `system_dispatch`, `register_for_execution` が `MessageDispatcherShared` に存在する
- **AND** これらは `with_write` / `with_read` を使って trait hook と query を組み合わせる
- **AND** executor submit や delayed shutdown 登録のような lock 解放後副作用は `MessageDispatcherShared` が担当する

#### Scenario: MessageDispatcherShared は AShared パターンに従う
- **WHEN** `MessageDispatcherShared` の定義を確認する
- **THEN** `MessageDispatcherShared` は `pub struct` として公開されている
- **AND** 内部に `ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>` を保持する
- **AND** `Clone` を実装する（`ArcShared::clone` ベース）
- **AND** `SharedAccess<Box<dyn MessageDispatcher>>` を実装し、`with_read` / `with_write` を提供する
- **AND** 既存 `ActorRefSenderShared` と同じパターンに従っている

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

### Requirement: dispatcher selection は registry entry と `MessageDispatcherConfigurator` で行われる

actor / system の dispatcher selection は、registry の `MessageDispatcherConfigurator` 経由で行われなければならない (MUST)。`Props` が `MessageDispatcher` や configurator を直接保持してはならない (MUST NOT)。

#### Scenario: Props は dispatcher 選択情報だけを保持する
- **WHEN** `Props` の dispatcher selection API を確認する
- **THEN** `Props` は dispatcher id を指定できる
- **AND** same-as-parent の選択を表現できる
- **AND** `MessageDispatcher` 実体や configurator を direct に保持する API は存在しない

#### Scenario: Dispatchers registry は MessageDispatcherConfigurator を保持する
- **WHEN** `Dispatchers` registry の内部構造を確認する
- **THEN** registry は id → `ArcShared<Box<dyn MessageDispatcherConfigurator>>` の写像を保持する
- **AND** `Dispatchers::resolve(&self, id)` は `MessageDispatcherShared` を返す
- **AND** `DispatcherRegistryEntry` は存在しない

#### Scenario: ActorSystemConfig は MessageDispatcherConfigurator を登録する
- **WHEN** `ActorSystemConfig` の dispatcher registration API を確認する
- **THEN** dispatcher id に対して `ArcShared<Box<dyn MessageDispatcherConfigurator>>` を登録できる
- **AND** bootstrap は registry から `MessageDispatcherShared` を解決して actor を attach する

### Requirement: `DefaultDispatcher` と `PinnedDispatcher` は独立した具象型として存在する

dispatcher policy family は `MessageDispatcher` を実装する独立した具象 struct として提供されなければならない (MUST)。Pekko の `Dispatcher` / `PinnedDispatcher` 継承階層を enum 変種やフラグで潰してはならない (MUST NOT)。

#### Scenario: DefaultDispatcher は concrete struct として存在する
- **WHEN** `core::kernel::dispatch` 配下を確認する
- **THEN** `DefaultDispatcher` は `pub struct` として存在する
- **AND** `impl MessageDispatcher for DefaultDispatcher` が存在する
- **AND** `DefaultDispatcher` は `DispatcherCore` を field として保持する

#### Scenario: PinnedDispatcher は concrete struct として存在する
- **WHEN** `core::kernel::dispatch` 配下を確認する
- **THEN** `PinnedDispatcher` は `pub struct` として存在する
- **AND** `impl MessageDispatcher for PinnedDispatcher` が存在する
- **AND** `PinnedDispatcher` は `DispatcherCore` と `Option<ActorCellId>` の owner field を保持する
- **AND** owner field に `AtomicPtr` / `AtomicU64` / `Mutex<T>` などの内部可変性は用いない（`&mut self` 経由で更新される）

#### Scenario: DefaultDispatcher は多数の actor を同時収容できる
- **WHEN** `DefaultDispatcher` で 2 体以上の actor を `attach` する
- **THEN** すべての `attach` が成功する
- **AND** `inhabitants` は attach した actor 数と一致する
- **AND** 複数 actor の mailbox が同じ `ExecutorShared` 経由で submit される

#### Scenario: PinnedDispatcher は 1 actor 専有で 2 体目を拒否する
- **WHEN** `PinnedDispatcher` で 1 体目の actor を `attach` した後に 2 体目の別 actor を `attach` する
- **THEN** 2 体目の `attach` は `SpawnError::DispatcherAlreadyOwned` で失敗する
- **AND** `PinnedDispatcher::throughput` は `NonZeroUsize::MAX` を返す（`&self` query）
- **AND** `PinnedDispatcher::throughput_deadline` は `None` を返す（`&self` query）

#### Scenario: PinnedDispatcher は同一 actor の再 attach を許容する
- **WHEN** `PinnedDispatcher` で同じ actor を 2 回 `attach` する
- **THEN** 2 回目の `attach` は成功する
- **AND** `inhabitants` は重複登録されない

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
- **THEN** `DispatcherCore::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` が存在する
- **AND** `DefaultDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` が存在する
- **AND** `PinnedDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` が存在する
- **AND** `PinnedDispatcher::new` は受け取った `settings` を `throughput = NonZeroUsize::MAX`, `throughput_deadline = None` に上書きしてから core に渡す
- **AND** `DefaultDispatcherConfigurator::new(settings: DispatcherSettings, executor: ExecutorShared)` が存在する
- **AND** `PinnedDispatcherConfigurator` は `settings: DispatcherSettings` をフィールドとして保持する

#### Scenario: DispatcherSettings は public abstraction の主語ではない
- **WHEN** dispatcher 公開抽象を確認する
- **THEN** dispatcher 公開抽象の主語は `MessageDispatcher` trait と `MessageDispatcherShared` のみである
- **AND** `DispatcherSettings` は dispatcher の構築時に渡すパラメータ bundle として位置づけられている
- **AND** `DispatcherSettings` を持って dispatcher の挙動を切り替える type-level dispatch は存在しない

### Requirement: `DefaultDispatcherConfigurator` はキャッシュし、`PinnedDispatcherConfigurator` は毎回新規生成する

configurator の `dispatcher()` は Pekko の `DispatcherConfigurator` / `PinnedDispatcherConfigurator` と同じインスタンス戦略を取らなければならない (MUST)。

#### Scenario: DefaultDispatcherConfigurator は同一 MessageDispatcherShared を clone して返す
- **WHEN** `DefaultDispatcherConfigurator::dispatcher(&self)` を 2 回呼ぶ
- **THEN** 返される `MessageDispatcherShared` は同じ `ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>` を指す
- **AND** configurator の `new` で eager に 1 回だけ `DefaultDispatcher` を構築する
- **AND** `OnceLock` などの内部可変性を configurator 内部に持たない

#### Scenario: PinnedDispatcherConfigurator は毎回新規 MessageDispatcherShared を返す
- **WHEN** `PinnedDispatcherConfigurator::dispatcher(&self)` を 2 回呼ぶ
- **THEN** 返される `MessageDispatcherShared` は異なる `ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>` を指す
- **AND** 各 instance は 1 スレッド専用 executor を別々に保持する
- **AND** 各 instance の `throughput` は `NonZeroUsize::MAX` を返す
- **AND** configurator 自身は `&self` で呼び出し可能（内部可変性なし、引数なし）

#### Scenario: Blocking dispatcher は DefaultDispatcherConfigurator の別 id 登録で表現される
- **WHEN** blocking workload 用の dispatcher を登録する
- **THEN** それは `DefaultDispatcherConfigurator` を blocking 対応 `ExecutorFactory` で構築し、別 id で registry に登録する形で表現される
- **AND** `BlockingDispatcher` という専用 type は存在しない
- **AND** 予約 id `pekko.actor.default-blocking-io-dispatcher` の解決は先行 change の要件を維持する

## REMOVED Requirements

### Requirement: dispatcher public abstraction は trait/provider 中心でなければならない

先行 change `dispatcher-trait-family-redesign` が導入した `Dispatcher` / `DispatcherProvider` 中心の公開抽象は、この redesign では維持しない。

### Requirement: dispatcher selection は registry entry と selector 意味論で行われる

provider + settings を束ねた registry entry を主語にした selection 契約は、この redesign では維持しない。

### Requirement: default / blocking / typed selector の意味論は固定される

selector 意味論を旧 registry entry 契約へ結びつける requirement は、この redesign では維持しない。

### Requirement: same-as-parent は独立した選択意味論として扱われる

この requirement 名は後続の dispatcher selection requirement へ統合し、独立 requirement としては維持しない。

### Requirement: PinnedDispatcher は dedicated lane policy を提供する

`PinnedDispatcher` を provider policy として表現する requirement は、この redesign では維持しない。

## ADDED Requirements

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
- **AND** 内部に `ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>` を保持する
- **AND** `Clone` を実装する（`ArcShared::clone` ベース）
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

### Requirement: `DefaultDispatcher` / `PinnedDispatcher` / `BalancingDispatcher` は独立した具象型として存在する

dispatcher policy family は `MessageDispatcher` を実装する 3 つの独立した具象 struct として提供されなければならない (MUST)。Pekko の `Dispatcher` / `PinnedDispatcher` / `BalancingDispatcher` 継承階層を enum 変種やフラグで潰してはならない (MUST NOT)。

#### Scenario: DefaultDispatcher は concrete struct として存在する
- **WHEN** `core::kernel::dispatch` 配下を確認する
- **THEN** `DefaultDispatcher` は `pub struct` として存在する
- **AND** `impl MessageDispatcher for DefaultDispatcher` が存在する
- **AND** `DefaultDispatcher` は `DispatcherCore` を field として保持する

#### Scenario: PinnedDispatcher は concrete struct として存在する
- **WHEN** `core::kernel::dispatch` 配下を確認する
- **THEN** `PinnedDispatcher` は `pub struct` として存在する
- **AND** `impl MessageDispatcher for PinnedDispatcher` が存在する
- **AND** `PinnedDispatcher` は `DispatcherCore` と `Option<Pid>` の owner field を保持する
- **AND** owner field に `AtomicPtr` / `AtomicU64` / `Mutex<T>` などの内部可変性は用いない（`&mut self` 経由で更新される）

#### Scenario: BalancingDispatcher は concrete struct として存在する
- **WHEN** `core::kernel::dispatch` 配下を確認する
- **THEN** `BalancingDispatcher` は `pub struct` として存在する
- **AND** `impl MessageDispatcher for BalancingDispatcher` が存在する
- **AND** `BalancingDispatcher` は `DispatcherCore` と `ArcShared<SharedMessageQueue>` を field として保持する
- **AND** `BalancingDispatcher::create_mailbox` は `SharingMailbox` (shared queue を参照する Mailbox) を返す
- **AND** `BalancingDispatcher::dispatch` は `self.shared_queue.enqueue(env)?` した上で `vec![receiver.mailbox()]` を返す（V1: 単一候補）

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
- **AND** owner identity の比較は `actor.pid()` を使う

#### Scenario: PinnedDispatcher は同一 actor の再 attach を許容する
- **WHEN** `PinnedDispatcher` で同じ actor を 2 回 `attach` する
- **THEN** 2 回目の `attach` は成功する
- **AND** `inhabitants` は重複登録されない

#### Scenario: BalancingDispatcher は複数 actor が同じ shared queue を消化する
- **WHEN** `BalancingDispatcher` に 3 actor を `attach` し、3 体に対する envelope を計 9 つ dispatch する
- **THEN** すべての envelope は同じ `SharedMessageQueue` に enqueue される
- **AND** いずれかの actor が `mailbox.run()` で dequeue を始めると、shared queue から FIFO 順で envelope を取り出す
- **AND** 結果として 1 体の actor だけでなく複数 actor が処理に参加する（load balancing が成立する）
- **AND** `BalancingDispatcher` の V1 では teamWork による active wake-up は実装されない（受信側 actor が naturally に run() するときに dequeue するのみ）

#### Scenario: SpawnError::DispatcherAlreadyOwned バリアントが存在する
- **WHEN** `modules/actor-core/src/core/kernel/actor/spawn/spawn_error.rs` の `SpawnError` enum を確認する
- **THEN** `DispatcherAlreadyOwned` バリアントが存在する
- **AND** `PinnedDispatcher::register_actor` の owner check で別 actor 検出時にこのバリアントが返される

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
- **AND** `BalancingDispatcher::new(settings: DispatcherSettings, executor: ExecutorShared) -> Self` が存在する
- **AND** `PinnedDispatcher::new` は受け取った `settings` を `throughput = NonZeroUsize::MAX`, `throughput_deadline = None` に上書きしてから core に渡す
- **AND** `BalancingDispatcher::new` は受け取った `settings` をそのまま core に渡し、内部で新しい `SharedMessageQueue` を生成する
- **AND** `DefaultDispatcherConfigurator::new(settings: DispatcherSettings, executor: ExecutorShared)` が存在する
- **AND** `BalancingDispatcherConfigurator::new(settings: DispatcherSettings, executor: ExecutorShared)` が存在する
- **AND** `PinnedDispatcherConfigurator` は `settings: DispatcherSettings` をフィールドとして保持する

#### Scenario: DispatcherSettings は public abstraction の主語ではない
- **WHEN** dispatcher 公開抽象を確認する
- **THEN** dispatcher 公開抽象の主語は `MessageDispatcher` trait と `MessageDispatcherShared` のみである
- **AND** `DispatcherSettings` は dispatcher の構築時に渡すパラメータ bundle として位置づけられている
- **AND** `DispatcherSettings` を持って dispatcher の挙動を切り替える type-level dispatch は存在しない

### Requirement: 3 種の Configurator が異なるインスタンス戦略を取る

configurator の `dispatcher()` は Pekko の `DispatcherConfigurator` / `PinnedDispatcherConfigurator` / `BalancingDispatcherConfigurator` と同じインスタンス戦略を取らなければならない (MUST)。

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

#### Scenario: BalancingDispatcherConfigurator は同一 MessageDispatcherShared を clone して返す
- **WHEN** `BalancingDispatcherConfigurator::dispatcher(&self)` を 2 回呼ぶ
- **THEN** 返される `MessageDispatcherShared` は同じ `ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>` を指す
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

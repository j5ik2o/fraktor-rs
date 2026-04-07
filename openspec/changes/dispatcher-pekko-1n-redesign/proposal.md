## Why

先行 change `dispatcher-trait-family-redesign` は `Dispatcher` trait / `DispatcherProvider` / `DispatcherSettings` 主体の公開抽象を導入したが、以下の核心を固定していなかったため実装が Pekko 互換を満たさない状態で完了扱いになっている。

- dispatcher と actor の所有関係（1:1 vs 1:N）が未固定
- dispatcher lifecycle（`attach` / `detach` / `inhabitants` / 自動 shutdown）の契約欠如
- mailbox がスケジューリングの主体（runnable として executor に submit される存在）であることの明文化欠如
- executor trait のシグネチャ（`&self` か `&mut self` か）の固定欠如
- async backpressure (`MailboxOfferFuture`) と no_std の両立方針が未定義

結果として現行実装は:

- `DispatcherCore` / `DispatcherShared` / `DispatchShared` の三重ラッパが同じ `ArcShared<DispatcherCore>` の別名として存在
- `DispatchExecutor::execute(&mut self, dispatcher: DispatchShared)` が closure ではなく dispatcher 全体を受け取る設計のため、`DispatchExecutorRunner`（queue + mutex + running AtomicBool）を再発明
- 1 dispatcher = 1 mailbox の結合により `DispatcherBuilder` / `DispatcherProvider` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` の 5 型が actor 毎に dispatcher を組み立てるためだけに存在
- `ScheduleAdapter` / `ScheduleAdapterShared` / `InlineScheduleAdapter` / `ScheduleWaker` / std 側 `StdScheduleAdapter` の 5 層で Waker 1 個を表現
- drain ループが `DispatcherCore::drive` / `process_batch` に存在し、Mailbox 側の `schedule_state` と二重管理
- inhabitants カウンタと auto-shutdown 契約が不在

これらを Pekko 基準で是正する。

## What Must Hold

- dispatcher の公開抽象は `MessageDispatcher` trait と `MessageDispatcherShared` を中心とし、trait は query / hook、shared wrapper は lock 解放後の副作用を伴う orchestration（`attach` / `detach` / `dispatch` / `system_dispatch` / `register_for_execution`）を担当する
- `MessageDispatcher` trait のメソッドは CQS 原則に従う:
  - **Query**（状態を読むのみ）は `&self` + 戻り値あり（`id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants` 等）
  - **Command / Hook**（状態を変える）は `&mut self`（`register_actor`, `unregister_actor`, `dispatch`, `system_dispatch`, `suspend`, `resume`, `shutdown` 等）
  - `create_mailbox` は状態を変えない factory として `&self`
- `MessageDispatcher::dispatch` / `system_dispatch` の戻り値は `Vec<ArcShared<Mailbox>>` (または small-vec optimization 同等) で、shared wrapper が lock 解放後に候補配列を順に `register_for_execution` する。この設計により BalancingDispatcher の teamWork load balancing も同じ seam で表現できる
- `register_for_execution` は trait method ではなく `MessageDispatcherShared` の純粋な CAS + executor submit 経路として提供する（trait hook の dual CAS を排除するため）
- `execute_task` は本 change のスコープ外（YAGNI）。Pekko の `executeTask` 相当（dispatcher の executor へ任意 closure を submit する経路）は具体的な caller が現れた時点で additive に追加する
- 内部可変性は使用しない。`MessageDispatcher` を複数スレッド・複数所有者で共有する経路は `MessageDispatcherShared` を通じてのみ提供する
- `MessageDispatcherShared` は既存の AShared 系 (`ActorFactoryShared` など) と同じパターンに従い、`ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>` を内包し、`SharedAccess<Box<dyn MessageDispatcher>>` を実装する（`with_read` / `with_write`）
- 1 つの dispatcher は同時に複数の actor を収容できる（1 : N）
- `MessageDispatcherShared::attach(actor)` は `inhabitants` を加算し、`MessageDispatcherShared::detach(actor)` は減算する
- `detach` は mailbox を terminal 状態へ遷移させて clean up してから shutdown 判定へ進む
- 全 actor が detach された後、`shutdown_timeout` 経過で executor を自動停止する
- delayed shutdown の実行予約は `MessageDispatcherShared::detach` が actor の system scheduler を使って行い、`DispatcherCore` は state machine だけを保持する
- delayed shutdown 用の scheduler handle は `detach(actor)` の引数から `actor.system().scheduler()` を辿って取得する。`DispatcherCore` / `MessageDispatcherShared` は scheduler を field として保持しない
- `MessageDispatcher` の具象型として `DefaultDispatcher` / `PinnedDispatcher` / `BalancingDispatcher` の **3 つ** を独立した型として提供する
- `PinnedDispatcher` は 1 actor 専有の dedicated lane を提供し、owner check を register 時に行う。重複所有時は `SpawnError::DispatcherAlreadyOwned` を返す（本 change で `SpawnError` enum に新規バリアント追加）
- `BalancingDispatcher` は本 change で **V1 として実装する**: `SharedMessageQueue` を 1 つ持ち、attach した全 actor の `SharingMailbox` がそれを参照することで自然に load balancing を実現する。teamWork による active wake-up fallback は V2 (additive) として将来追加する
- `PinnedDispatcherConfigurator::dispatcher()` は呼び出しのたびに新しい `PinnedDispatcher` を返す（Pekko と同じ）
- `DefaultDispatcherConfigurator::dispatcher()` / `BalancingDispatcherConfigurator::dispatcher()` は同一インスタンスをキャッシュして返す（Pekko と同じ）
- `Dispatchers::resolve` の呼び出しは spawn / bootstrap 経路に限定する。message hot path から呼んではならない（PinnedDispatcherConfigurator は呼び出しごとに新 thread 生成のため、hot path 呼び出しは thread leak を引き起こす）
- dispatcher の共通 state と private helper は `DispatcherCore` （pub struct）に集約し、各具象型が `core: DispatcherCore` として保持する
- `DispatcherCore` 自身も CQS 原則に従う（commands は `&mut self`、queries は `&self`）
- `DispatcherCore::mark_attach` / `mark_detach` は戻り値なしの純粋 command として定義する（CQS 例外なし）
- `DispatcherCore::schedule_shutdown_if_sensible` は `ShutdownSchedule` を返す。これは CQS 許容例外であり、`MessageDispatcherShared::detach` が lock 解放前に値を copy して delayed shutdown 登録判定に使うため
- `MessageDispatcherShared::detach` の delayed shutdown 登録は、状態遷移後の `ShutdownSchedule` を lock 内で copy out し、ロック解放後にその値で判定する（lock 解放後の再観測による race window を作らない）
- mailbox / overflow strategy が要求する blocking compatibility は `executor.supports_blocking()` と照合し、不適合な組み合わせは `SpawnError::InvalidMailboxConfig` で attach 時に拒否する
- dispatcher 単位の mutex による 1:N 共有時の contention は既知のトレードオフとして受け入れ、bench / diagnostics で観測する
- `Executor` trait も CQS 原則に従う:
  - `fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>)` は command
  - `fn shutdown(&mut self)` は command
  - `fn supports_blocking(&self) -> bool` は query
- `Executor` を複数所有者で共有する経路は `ExecutorShared`（`ArcShared<RuntimeMutex<Box<dyn Executor>>>` の AShared 薄ラッパ）を通じてのみ提供する
- `DispatcherCore::executor` は `ExecutorShared` を保持する（生の `ArcShared<Box<dyn Executor>>` ではない）
- mailbox は drain ループを自らの `run()` に持ち、dispatcher から throughput 設定を注入される
- mailbox の二重スケジュール防止は mailbox 自身の atomic state（`set_as_scheduled` / `set_as_idle`）の CAS で完結する
- `MailboxOfferFuture` による async backpressure は維持し、`DispatcherWaker`（no_std 対応）1 実装で表現する
- `Executor` trait は core 層に置き、`InlineExecutor` は core 層に置く
- `TokioExecutor` / `ThreadedExecutor` / `PinnedExecutor` などの具象は std 層に置く
- `DispatcherCore` は `pub` として公開し、fraktor 外部でも `MessageDispatcher` 独自実装を構築する際の共通 state として利用可能にする
- 新版 `DispatcherSettings` を immutable な settings bundle として **新規** に定義する（旧版 `DispatcherSettings` とは別物）。フィールドは `id: String`, `throughput: NonZeroUsize`, `throughput_deadline: Option<Duration>`, `shutdown_timeout: Duration` のみ。`DispatcherCore::new` / `DefaultDispatcher::new` / `PinnedDispatcher::new` / configurator に渡すパラメータ bundle として使う
- 新設計は将来 `BalancingDispatcher` を **既存コード無変更で** 追加できる拡張 seam を保持しなければならない
- 並走期間中、`modules/actor-core/src/core/kernel/dispatch/dispatcher_new/` 配下の実装は **旧 `modules/actor-core/src/core/kernel/dispatch/dispatcher/` 配下のいかなる型・関数・モジュールも `use` / 参照してはならない**（同様に `modules/actor-adaptor-std/src/std/dispatch_new/` は旧 `std/dispatch/` を参照してはならない）。両者は完全に独立した tree として共存し、最終的に旧側を `rm -rf` するだけで完了できる構造を維持する

## What Must Not Hold

- 1 dispatcher = 1 mailbox の結合を残してはならない
- 旧 `DispatcherShared` / `DispatchShared` のように「同じ `DispatcherCore` を別名で包むだけ」の重複ラッパ層を残してはならない（`DispatcherCore` は直接公開、`MessageDispatcherShared` は trait object 共有のための AShared 薄ラッパ、という 2 層だけに限定する）
- `DispatchExecutor::execute` のような旧 trait を残してはならない
- `DispatchExecutorRunner` のような「executor を serialize するためだけの queue + mutex + running atomic」を残してはならない
- drain ループを dispatcher 側に残し、mailbox を受け身のデータ構造にしてはならない
- `ScheduleAdapter` trait / `ScheduleAdapterShared` / `InlineScheduleAdapter` / `ScheduleWaker` の多層構造を残してはならない
- `DispatcherBuilder` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` を 1:1 前提の「 actor 毎 dispatcher 組み立て」のために残してはならない
- `MessageDispatcher` / `Executor` trait の command メソッドを内部可変性で `&self` に偽装してはならない（CQS 違反、内部可変性ポリシー違反）
- `MessageDispatcher` trait に `register_for_execution` / `execute_task` を残してはならない（前者は shared wrapper に集約、後者は本 change スコープ外）
- inhabitants と自動 shutdown 契約を省略して「手動 shutdown のみ」の運用に戻してはならない
- 後方互換ブリッジとして旧 `DispatcherCore` / `DispatcherShared` と新 `MessageDispatcher` を同時に公開し続けてはならない
- 並走期間中に `dispatcher_new/` から旧 `dispatcher/` の型・関数・モジュールを `use` / 参照してはならない（短絡的な再利用を含めて禁止。同じ概念が必要なら新側に独立して再実装する）
- 旧版 `DispatcherSettings` の `schedule_adapter` / `starvation_deadline` フィールドを新版に持ち込んではならない（前者は `ScheduleAdapter` 自体の削除に伴って、後者は YAGNI で初期版から除外）
- `BalancingDispatcher` の V1 で teamWork load balancing fallback を実装してはならない（V2 として additive に追加する）
- `SharingMailbox` を `BalancingDispatcher` 以外の dispatcher で使ってはならない

## Capabilities

### Modified Capabilities

- `dispatcher-trait-provider-abstraction`: trait の名前を `MessageDispatcher` に固定し、`MessageDispatcherShared` が `attach` / `detach` / `dispatch` / `systemDispatch` / `registerForExecution` の orchestration を担う契約へ更新する。`DefaultDispatcher` / `PinnedDispatcher` を具象型として要求する。registry entry を `ArcShared<Box<dyn MessageDispatcherConfigurator>>` で保持する要件に置き換える
- `dispatch-executor-unification`: `Executor` trait のシグネチャを `&mut self`（command）/ `&self`（query）の CQS 準拠に固定し、共有経路を `ExecutorShared`（AShared パターン）に統一、`DispatchExecutorRunner` を internal primitive からも除去する。core 層 / std 層の配置境界を固定する

### New Capabilities

- `dispatcher-attach-detach-lifecycle`: 1 : N 収容モデルと inhabitants / auto-shutdown の lifecycle を独立 capability として定義する
- `mailbox-runnable-drain`: mailbox が自らの `run()` に drain ループを持つ契約を定義する
- `dispatcher-core-shared-state`: `DispatcherCore` の pub 公開と共通 state / helper の契約を定義する

## Impact

- 影響コード:
  - `modules/actor-core/src/core/kernel/dispatch/dispatcher/*`（ほぼ全面書き換え→削除）
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`（`new` コンストラクタ改修、`run()` 追加）
  - `modules/actor-core/src/core/kernel/actor/cell/*`（attach / detach 呼び出し経路）
  - `modules/actor-core/src/core/kernel/actor/spawn/*`
  - `modules/actor-core/src/core/kernel/system/*`（dispatcher 自動 shutdown のスケジューラ連携）
  - `modules/actor-adaptor-std/src/std/dispatch/*`（具象 executor の `&self` 移行、`StdScheduleAdapter` 削除）
  - `modules/actor-core` 内の showcase / bench / tests
- 影響設計:
  - `ActorCell` は 2-phase init が必要になる可能性が高い。cell 本体の確保と mailbox / dispatcher / sender の install を分離する設計判断が必要
  - `MessageDispatcher::create_mailbox` は public trait method だが、運用規律として `MessageDispatcherShared::attach` 経由以外から直接呼ばない前提で扱う
- 影響 API:
  - `MessageDispatcher` trait (new, CQS 準拠、`register_for_execution` / `execute_task` を持たない)
  - `MessageDispatcherShared` struct (new, AShared パターン)
  - `DispatcherCore` (pub, new)
  - `DispatcherSettings` (new, **旧版とは別物**: `id` / `throughput` / `throughput_deadline` / `shutdown_timeout` の immutable bundle。旧 `schedule_adapter` / `starvation_deadline` フィールドは持たない)
  - `DefaultDispatcher` / `PinnedDispatcher` / `BalancingDispatcher` concrete types (new, **3 つすべて** core 層配置)
  - `SharedMessageQueue` struct (new, core 層、`BalancingDispatcher` 専用)
  - `SharingMailbox` struct (new, core 層、Mailbox の薄いラッパ、shared queue 参照)
  - `MessageDispatcherConfigurator` trait (renamed from `DispatcherProvider` concept)
  - `DefaultDispatcherConfigurator` / `PinnedDispatcherConfigurator` / `BalancingDispatcherConfigurator` の 3 具象 configurator
  - `Executor` trait (reshaped, CQS 準拠 `&mut self`)
  - `ExecutorShared` struct (new, AShared パターン)
  - `DispatcherWaker` (new, no_std)
  - `Mailbox::new(actor: Weak<ActorCell>, queue: ArcShared<dyn MessageQueue>)` (breaking, queue injection)
  - `Mailbox::run` (new)
  - `SpawnError::DispatcherAlreadyOwned` バリアント (new, PinnedDispatcher の同時所有拒否)
  - removal: `DispatcherShared`(旧), `DispatchShared`, `DispatcherBuilder`, `DispatcherProvider`, `DispatchExecutorRunner`, `DispatcherProvisionRequest`, `DispatcherRegistryEntry`, `ConfiguredDispatcherBuilder`, `DispatchExecutor`(旧), 旧 `DispatcherSettings`(`schedule_adapter` / `starvation_deadline` を含む版), `ScheduleAdapter*`, `InlineScheduleAdapter`, `ScheduleWaker`, `StdScheduleAdapter`
- 互換性:
  - 後方互換は不要（プロジェクト方針に従う）
  - 旧 dispatcher と新 dispatcher の並走期間中は、fraktor-rs 内部の呼び出し元が順次移行される間のみ限定的に許容する
  - 並走終了条件: 呼び出し元を全て新 API に置換し、旧 `dispatcher/` モジュールを一括削除した時点

## Why

先行 change `dispatcher-trait-family-redesign` は `Dispatcher` trait / `DispatcherProvider` / `DispatcherSettings` 主体の公開抽象を導入したが、以下の核心を固定していなかったため実装が Pekko 互換を満たさない状態で完了扱いになっている。

- dispatcher と actor の所有関係（1:1 vs 1:N）が未固定
- dispatcher lifecycle（`attach` / `detach` / `inhabitants` / 自動 shutdown）の契約欠如
- mailbox がスケジューリングの主体（runnable として executor に submit される存在）であることの明文化欠如
- executor trait のシグネチャ（`&self` か `&mut self` か）の固定欠如
- async backpressure (`MailboxOfferFuture`) と no_std の両立方針が未定義

結果として現行実装は:

- `DispatcherCore` / `DispatcherShared` / `DispatchShared` の三重ラッパが同じ `ArcShared<DispatcherCore>` の別名として存在
- `DispatchExecutor::execute(&mut self, ...)` の制約から `DispatchExecutorRunner`（queue + mutex + running AtomicBool）を再発明
- 1 dispatcher = 1 mailbox の結合により `DispatcherBuilder` / `DispatcherProvider` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` の 5 型が actor 毎に dispatcher を組み立てるためだけに存在
- `ScheduleAdapter` / `ScheduleAdapterShared` / `InlineScheduleAdapter` / `ScheduleWaker` / std 側 `StdScheduleAdapter` の 5 層で Waker 1 個を表現
- drain ループが `DispatcherCore::drive` / `process_batch` に存在し、Mailbox 側の `schedule_state` と二重管理
- inhabitants カウンタと auto-shutdown 契約が不在

これらを Pekko 基準で是正する。

## What Must Hold

- dispatcher の公開抽象は `MessageDispatcher` trait を中心とし、Pekko の `MessageDispatcher` 抽象クラスの契約（attach / detach / dispatch / systemDispatch / registerForExecution / suspend / resume / executeTask / shutdown / createMailbox）をそのまま要求する
- `MessageDispatcher` trait のメソッドは CQS 原則に従う:
  - **Query**（状態を読むのみ）は `&self` + 戻り値あり（`id`, `throughput`, `throughput_deadline`, `shutdown_timeout`, `inhabitants` 等）
  - **Command**（状態を変える）は `&mut self`（`attach`, `detach`, `dispatch`, `system_dispatch`, `register_for_execution`, `suspend`, `resume`, `execute_task`, `shutdown`, `create_mailbox` 等）
- 内部可変性は使用しない。`MessageDispatcher` を複数スレッド・複数所有者で共有する経路は `MessageDispatcherShared` を通じてのみ提供する
- `MessageDispatcherShared` は `ActorRefSenderShared` と同じ AShared パターンに従い、`ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>` を内包し、`SharedAccess<Box<dyn MessageDispatcher>>` を実装する（`with_read` / `with_write`）
- 1 つの dispatcher は同時に複数の actor を収容できる（1 : N）
- `attach(actor)` は `inhabitants` を加算し、`detach(actor)` は減算する
- 全 actor が detach された後、`shutdown_timeout` 経過で executor を自動停止する
- `MessageDispatcher` の具象型として `DefaultDispatcher` と `PinnedDispatcher` を独立した型として提供する
- `PinnedDispatcher` は 1 actor 専有の dedicated lane を提供し、owner check を register 時に行う
- `PinnedDispatcherConfigurator::dispatcher()` は呼び出しのたびに新しい `PinnedDispatcher` を返す（Pekko と同じ）
- `DefaultDispatcherConfigurator::dispatcher()` は同一インスタンスをキャッシュして返す（Pekko と同じ）
- dispatcher の共通 state と private helper は `DispatcherCore` （pub struct）に集約し、各具象型が `core: DispatcherCore` として保持する
- `DispatcherCore` 自身も CQS 原則に従う（commands は `&mut self`、queries は `&self`）
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
- 新設計は将来 `BalancingDispatcher` を **既存コード無変更で** 追加できる拡張 seam を保持しなければならない

## What Must Not Hold

- 1 dispatcher = 1 mailbox の結合を残してはならない
- 旧 `DispatcherShared` / `DispatchShared` のように「同じ `DispatcherCore` を別名で包むだけ」の重複ラッパ層を残してはならない（`DispatcherCore` は直接公開、`MessageDispatcherShared` は trait object 共有のための AShared 薄ラッパ、という 2 層だけに限定する）
- `DispatchExecutor::execute` のような旧 trait を残してはならない
- `DispatchExecutorRunner` のような「executor を serialize するためだけの queue + mutex + running atomic」を残してはならない
- drain ループを dispatcher 側に残し、mailbox を受け身のデータ構造にしてはならない
- `ScheduleAdapter` trait / `ScheduleAdapterShared` / `InlineScheduleAdapter` / `ScheduleWaker` の多層構造を残してはならない
- `DispatcherBuilder` / `DispatcherProvisionRequest` / `DispatcherRegistryEntry` / `ConfiguredDispatcherBuilder` を 1:1 前提の「 actor 毎 dispatcher 組み立て」のために残してはならない
- `MessageDispatcher` / `Executor` trait の command メソッドを内部可変性で `&self` に偽装してはならない（CQS 違反、内部可変性ポリシー違反）
- `MessageDispatcher::attach` / `detach` を具象型が override する運用を許容してはならない（Pekko の `final` に相当する規律）
- inhabitants と自動 shutdown 契約を省略して「手動 shutdown のみ」の運用に戻してはならない
- 後方互換ブリッジとして旧 `DispatcherCore` / `DispatcherShared` と新 `MessageDispatcher` を同時に公開し続けてはならない

## Capabilities

### Modified Capabilities

- `dispatcher-trait-provider-abstraction`: trait の名前を `MessageDispatcher` に固定し、attach / detach / dispatch / systemDispatch / registerForExecution / inhabitants / auto-shutdown 契約を追加する。`DefaultDispatcher` / `PinnedDispatcher` を具象型として要求する。registry entry を `ArcShared<Box<dyn MessageDispatcherConfigurator>>` で保持する要件に置き換える
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
- 影響 API:
  - `MessageDispatcher` trait (new, CQS 準拠)
  - `MessageDispatcherShared` struct (new, AShared パターン)
  - `DispatcherCore` (pub, new)
  - `DefaultDispatcher` / `PinnedDispatcher` concrete types (new)
  - `MessageDispatcherConfigurator` trait (renamed from `DispatcherProvider` concept)
  - `Executor` trait (reshaped, CQS 準拠 `&mut self`)
  - `ExecutorShared` struct (new, AShared パターン)
  - `DispatcherWaker` (new, no_std)
  - `Mailbox::new` (breaking, queue injection)
  - `Mailbox::run` (new)
  - removal: `DispatcherShared`(旧), `DispatchShared`, `DispatcherBuilder`, `DispatcherProvider`, `DispatchExecutorRunner`, `DispatcherProvisionRequest`, `DispatcherRegistryEntry`, `ConfiguredDispatcherBuilder`, `DispatchExecutor`(旧), `ScheduleAdapter*`, `InlineScheduleAdapter`, `ScheduleWaker`, `StdScheduleAdapter`
- 互換性:
  - 後方互換は不要（プロジェクト方針に従う）
  - 旧 dispatcher と新 dispatcher の並走期間中は、fraktor-rs 内部の呼び出し元が順次移行される間のみ限定的に許容する
  - 並走終了条件: 呼び出し元を全て新 API に置換し、旧 `dispatcher/` モジュールを一括削除した時点

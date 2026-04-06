## REMOVED Requirements

### Requirement: executor 系は internal backend primitive としてのみ扱われる

先行 change が定義した `DispatchExecutor` / `DispatchExecutorRunner` 中心の backend primitive 契約は、この redesign では維持しない。

## ADDED Requirements

### Requirement: `Executor` trait は CQS 準拠の internal primitive として再定義される

dispatcher の内部で使われる executor 抽象は、CQS 準拠の単一 trait として再定義されなければならない (MUST)。command メソッドは `&mut self`、query メソッドは `&self` を要求する。executor を共有する経路は `ExecutorShared`（AShared パターン）を通じてのみ提供され、「queue + mutex + running atomic」のような共有のための再発明は存在してはならない (MUST NOT)。

#### Scenario: Executor trait は CQS 準拠のシグネチャを持つ
- **WHEN** `Executor` trait のシグネチャを確認する
- **THEN** command: `fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>)` が定義されている
- **AND** command: `fn shutdown(&mut self)` が定義されている
- **AND** query: `fn supports_blocking(&self) -> bool { true }` が default 付きで定義されている
- **AND** command を `&self` + 内部可変性で偽装する実装は存在しない

#### Scenario: ExecutorShared は AShared パターンに従う
- **WHEN** `ExecutorShared` の定義を確認する
- **THEN** `ExecutorShared` は `pub struct` として公開されている
- **AND** 内部に `ArcShared<RuntimeMutex<Box<dyn Executor>>>` を保持する
- **AND** `Clone` を実装する（`ArcShared::clone` ベース）
- **AND** `SharedAccess<Box<dyn Executor>>` を実装し、`with_read` / `with_write` を提供する
- **AND** convenience メソッド `execute(&self, task)` / `shutdown(&self)` / `supports_blocking(&self) -> bool` を提供する
- **AND** 既存の AShared 系 (`ActorFactoryShared` など) と同じパターンに従っている

#### Scenario: DispatchExecutorRunner は存在しない
- **WHEN** `core::kernel::dispatch` 配下を確認する
- **THEN** `DispatchExecutorRunner` および同等の serializing runner は存在しない
- **AND** executor を共有するために `Mutex<Box<dyn ...>>` + internal task queue + `AtomicBool running` を独自に組んだ型は存在しない
- **AND** 複数所有者間の共有は `ExecutorShared` の `ArcShared<RuntimeMutex<Box<dyn Executor>>>` のみで達成される

#### Scenario: Executor trait は core 層に置かれる
- **WHEN** `Executor` trait の定義ファイルを確認する
- **THEN** `Executor` trait は `modules/actor-core` 配下にある
- **AND** trait 定義は `no_std` 対応である
- **AND** core 層から std / tokio 型への直接依存は存在しない

#### Scenario: InlineExecutor は core 層に置かれる
- **WHEN** `InlineExecutor` の定義ファイルを確認する
- **THEN** `InlineExecutor` は `modules/actor-core` 配下にある
- **AND** `InlineExecutor::execute` は現スレッドで同期に task を実行する
- **AND** `InlineExecutor::supports_blocking` は `false` を返す

#### Scenario: TokioExecutor / ThreadedExecutor / PinnedExecutor は std 層に置かれる
- **WHEN** 各 std 側 executor 具象の定義ファイルを確認する
- **THEN** これらは `modules/actor-adaptor-std` 配下にある
- **AND** すべて `Executor` trait を `&mut self` command / `&self` query の契約で実装する
- **AND** `TokioExecutor` は `tokio-executor` feature 下でのみ提供される

### Requirement: `DispatcherWaker` は core 層に 1 実装で提供される

`MailboxOfferFuture` の backpressure pending 用 Waker は、executor 実装ごとに型を増やさず、core 層の 1 実装で提供されなければならない (MUST)。

#### Scenario: DispatcherWaker は core 層にある
- **WHEN** `DispatcherWaker` の定義ファイルを確認する
- **THEN** `DispatcherWaker` は `modules/actor-core` 配下にある
- **AND** `core::task::RawWaker` を使って実装されている
- **AND** `no_std` 対応である

#### Scenario: DispatcherWaker::wake は MessageDispatcherShared::register_for_execution を呼ぶ
- **WHEN** `DispatcherWaker` の `wake` 実装を確認する
- **THEN** wake 実装は capture した `MessageDispatcherShared` に対して `register_for_execution(&mbox, has_message_hint=false, has_system_hint=true)` を呼ぶ
- **AND** Waker 1 実装のみで std / tokio / inline / 将来 embedded すべてに対応できる

#### Scenario: ScheduleAdapter 系は存在しない
- **WHEN** `core::kernel::dispatch` 配下を確認する
- **THEN** `ScheduleAdapter` trait、`ScheduleAdapterShared`、`InlineScheduleAdapter`、`ScheduleWaker`、`StdScheduleAdapter` は存在しない
- **AND** Waker 生成経路は `DispatcherWaker` に一本化されている

### Requirement: 並走期間中 `dispatcher_new/` は旧 `dispatcher/` に依存してはならない

新旧の dispatcher 実装が並走している期間中、新側の実装は旧側のいかなる型・関数・trait・モジュールも `use` / 参照してはならない (MUST NOT)。両者は完全に独立した tree として共存し、最終的に旧側を `rm -rf` するだけで完了できる構造を維持しなければならない (MUST)。

#### Scenario: dispatcher_new は旧 dispatcher を import しない
- **WHEN** `modules/actor-core/src/core/kernel/dispatch/dispatcher_new/` 配下のすべての `.rs` ファイルを確認する
- **THEN** `use crate::core::kernel::dispatch::dispatcher::` で始まる import 文は存在しない
- **AND** `super::super::dispatcher::` などの相対パスでの旧モジュール参照も存在しない
- **AND** 旧 `DispatcherCore` / 旧 `DispatcherShared` / 旧 `DispatchShared` / 旧 `DispatchExecutor` / 旧 `DispatcherSettings` / 旧 `DispatcherProvider` などの型を新側から参照していない

#### Scenario: std/dispatch_new は旧 std/dispatch を import しない
- **WHEN** `modules/actor-adaptor-std/src/std/dispatch_new/` 配下のすべての `.rs` ファイルを確認する
- **THEN** `use crate::std::dispatch::` で始まる import 文は存在しない
- **AND** 旧 `TokioExecutor` / 旧 `ThreadedExecutor` / 旧 `PinnedExecutor` / 旧 `StdScheduleAdapter` などの型を新側から参照していない

#### Scenario: 同じ概念は新側に独立して再実装される
- **WHEN** 新旧両方で同じ概念（例: `DispatchError` 相当）が必要となる
- **THEN** 新側は旧側を import せず、独立して新側に同等の型を定義する
- **AND** 共通基盤を作って両側から参照させる「中間層」は新設しない（中間層は最終削除のブロッカーになる）

#### Scenario: 旧側のテストヘルパは新側のテストから流用されない
- **WHEN** `dispatcher_new/` 配下の test モジュールを確認する
- **THEN** 旧 `dispatcher/` 配下の `tests` モジュールや helper 関数を `use` していない
- **AND** 新側のテストヘルパは新側で完結している

### Requirement: dispatcher の drain ループは mailbox 側に配置される

dispatcher の drain ループ本体は Pekko の `Mailbox.run()` と同じく mailbox 側に配置されなければならない (MUST)。dispatcher は throughput 設定を注入する経路のみを持つ。

#### Scenario: Mailbox は run() を持ち、throughput を外部から受け取る
- **WHEN** `Mailbox` の API を確認する
- **THEN** `Mailbox::run(&self, throughput: NonZeroUsize, throughput_deadline: Option<Duration>)` が定義されている
- **AND** `run` は system message を全件処理した後、user message を throughput まで処理する
- **AND** `run` は closed mailbox を即 return する

#### Scenario: Mailbox の二重スケジュール防止は mailbox 自身の atomic state の CAS で完結する
- **WHEN** mailbox がスケジュールされる経路を確認する
- **THEN** `set_as_scheduled` の CAS に成功した submitter のみが executor へ task を submit する
- **AND** dispatcher 側に別の排他制御キュー / mutex は存在しない
- **AND** mailbox と dispatcher が schedule 状態を二重管理しない

#### Scenario: Mailbox コンストラクタは message queue を外部注入できる
- **WHEN** `Mailbox::new` の呼び出し側を確認する
- **THEN** `Mailbox::new(actor, queue)` の形で message queue を外部から渡せる
- **AND** これは将来 `BalancingDispatcher` が shared queue を注入するための seam となる

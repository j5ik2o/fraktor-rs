## Context

現在の `actor-core-kernel` は Pekko 型の mailbox-driven scheduler をかなり忠実に持っている。`MessageDispatcherShared::register_for_execution` は mailbox を scheduled にし、`Executor` へ `Box<dyn FnOnce()>` を submit し、その closure が `Mailbox::run(throughput, throughput_deadline)` を実行する。

この構造の強みは、`Mailbox::run` が system message priority、suspend / resume、throughput、throughput deadline、cleanup ownership を 1 つの non-awaiting 境界で扱える点である。ここに `.await` を入れると、lock discipline、reentrancy、supervision、stash、cleanup が一気に async state machine 化される。

一方で、std 環境を Tokio 固定とするなら、現状の `TokioExecutor` が default mailbox drain を `spawn_blocking` に送っている点は重い。Embassy 前提では thread blocking の逃げ道自体がないため、Pekko の blocking bounded mailbox (`pushTimeOut`) 互換を厚くする方向はさらに合わない。

したがって本 change では、core の mailbox drain は維持しつつ、外側の executor / tick driver / clock / wakeup を async 実行環境に接続しやすくする。既存 `pipe_to_self` / `pipe_to` はユーザー向け API を変えず、future-to-message adapter 境界として明文化・補強する。

## Goals / Non-Goals

### Goals

- opt-in std Tokio helper の default dispatcher を non-blocking task executor として扱えるようにする。
- 既存 `TokioExecutor` / `TokioExecutorFactory` を source / behavior compatible に維持したまま、opt-in の Tokio task executor を追加する。
- opt-in std Tokio helper で default dispatcher と blocking dispatcher を別登録できるようにする。
- Embassy adapter crate を新設し、`actor-core-kernel` を Embassy task / signal / timer に接続する。
- 既存 `ActorContext::pipe_to_self` / `pipe_to` と `ContextPipeTask` を signature 互換の future-to-message kernel contract として先に固定し、その上に `TypedActorContext::pipe_to_self` / `pipe_to` を signature 互換の薄い wrapper として維持する。
- `showcases/std` 配下に opt-in std Tokio helper、blocking dispatcher、typed `pipe_to_self` を組み合わせた実行可能サンプルを追加する。
- mailbox drain / actor invocation の core contract は sync / non-awaiting のまま守る。

### Non-Goals

- `Actor::receive` / `MessageInvoker::invoke` / `Mailbox::run` の full async 化。
- `pushTimeOut` 系 blocking bounded mailbox の実装。
- Embassy remote transport、Embassy stream materializer、Embassy persistence adapter。
- Tokio current-thread flavor の初期対応。
- actor / behavior handler が `Future` を返す API、`.await` を跨いで actor state を mutable borrow する API。
- 既存 public API の `async fn` 化、rename、削除、または caller に `.await` を要求する変更。

## Decisions

### Decision 1: `TokioTaskExecutor` を opt-in の default task executor として追加する

既存 `TokioExecutor` は `spawn_blocking` 系の互換 executor として維持し、既存 caller の source / behavior compatibility を守る。default task executor としては追加 API の `TokioTaskExecutor` を導入する。

`TokioTaskExecutor` は `Handle::spawn` で async task を起動し、その task 内で mailbox drain closure を短時間実行する。`Executor` trait は `Box<dyn FnOnce()>` を受け取る sync submit primitive のままにするため、実装は概念上次の形になる。

```rust
fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
  drop(self.handle.spawn(async move {
    task();
  }));
  Ok(())
}
```

この design は `Executor` trait を future submit に変えない。Embassy では dynamic future spawn より static task + queue のほうが合うため、core trait を Tokio 形に寄せすぎない。

### Decision 2: blocking executor は既存 `TokioExecutor` 互換を維持する

blocking actor 用には既存 `TokioExecutor` / `TokioExecutorFactory` をそのまま使える。`TokioExecutor` を task executor に再定義しない。

明示的な naming が必要な場合は `TokioBlockingExecutor` / `TokioBlockingExecutorFactory` を additive に追加してよい。ただしそれは既存 `TokioExecutor` の置き換えではなく、同じ `spawn_blocking` 意味論を持つ alias / wrapper に留める。

### Decision 3: default と blocking dispatcher の既定登録を分離する

`Dispatchers::ensure_default` は現在、default と blocking に同じ configurator を入れ得る。core の no_std default は inline executor でよいが、opt-in std Tokio helper では default と blocking を別 factory で登録する必要がある。既存の config builder の呼び出し方は変更せず、dispatcher id は `actor-core-kernel` の `DEFAULT_DISPATCHER_ID` / `DEFAULT_BLOCKING_DISPATCHER_ID` 定数を使う。

候補 API は次のどちらかにする。

```rust
pub fn with_default_dispatcher_factory(mut self, configurator: ArcShared<Box<dyn MessageDispatcherFactory>>) -> Self
pub fn with_blocking_dispatcher_factory(mut self, configurator: ArcShared<Box<dyn MessageDispatcherFactory>>) -> Self
```

または、既存 `with_dispatcher_factory(DEFAULT_DISPATCHER_ID, ...)` と `with_dispatcher_factory(DEFAULT_BLOCKING_DISPATCHER_ID, ...)` を使う std helper を `actor-adaptor-std` に置く。実装時は既存 API で十分なら新 API を足さない。新 helper は opt-in とし、既存 `ActorSystemConfig::new(...)` の挙動を変えない。

### Decision 4: Embassy adapter は新 crate `actor-adaptor-embassy` として分離する

Embassy 依存は `actor-core-kernel` に入れない。`modules/actor-adaptor-embassy` を追加し、`actor-core-kernel` の `Executor` / `TickDriver` / mailbox clock injection へ接続する adapter だけを持つ。

初期構成は次の責務に分ける。

- `EmbassyExecutor`: mailbox drain request を bounded ready queue へ積む。
- `EmbassyExecutorDriver`: Embassy task 内で signal を待ち、ready queue から closure を drain する。
- `EmbassyTickDriver`: `embassy-time::Ticker` または `Timer` で tick を供給する。
- `EmbassyMailboxClock`: `embassy-time::Instant` を `MailboxClock` に変換して注入する helper。

`Executor::execute` は sync method なので、Embassy 側では closure を queue に入れて signal を通知するだけにする。実行は Embassy task が所有する。

`EmbassyTickDriver::kind()` は `TickDriverKind::Embassy` を返す。`AutoProfileKind::Embassy` は既に存在するため、metrics / snapshot 上の driver kind だけを追加する。

### Decision 5: Embassy の ready queue は bounded とし、submit 失敗を `ExecuteError` として返す

Embassy では unbounded allocation を前提にしない。ready queue は `embassy_sync::channel::Channel` などの bounded primitive を使う。満杯時は block せず `ExecuteError` を返し、dispatcher は既存どおり mailbox scheduling CAS を rollback する。

これは backpressure を thread block で扱わない方針と一致する。

### Decision 6: actor API は同期のまま、Future 連携は `pipe_to_self` に集約する

Pekko typed は `Behavior` / `AbstractBehavior` の message handler を同期的に保ち、`ActorContext.pipeToSelf` で `Future` / `CompletionStage` の完了結果を actor message に戻す。fraktor-rs も同じ境界を既に持っている。

現在の実装では、untyped 側に次がある。

- `ActorContext::pipe_to_self(future, map) -> Result<(), PipeSpawnError>`
- `ActorContext::pipe_to(future, target, map) -> Result<(), PipeSpawnError>`
- `ContextPipeTask` が `Future<Output = Option<AnyMessage>>` を保持し、waker 経由で actor cell に再 poll を要求する
- delivery 失敗は `record_send_error` と warn log に乗る

この untyped 側が kernel contract であり、実装はここを先に固定する。typed 側は kernel contract の利用者であり、独立した async 実行モデルを持たせない。

typed 側にも次がある。

- `TypedActorContext::pipe_to_self(future, map_ok, map_err) -> Result<(), PipeSpawnError>`
- `TypedActorContext::pipe_to(future, recipient, map_ok, map_err) -> Result<(), PipeSpawnError>`
- `pipe_to_self` は `AdaptMessage` を使い、adapter 実行を actor thread 側へ戻す
- `ask` / `ask_with_status` はこの typed `pipe_to_self` に乗っている

したがって本 change では handler が `Future` を返す新 contract を足さない。既存 untyped `pipe_to_self` / `pipe_to` を source-compatible な kernel contract として守り、typed context はその薄い wrapper として `AnyMessage` を直接扱わせずに `Future` 完了結果を message 化できることをテストと docs で固定する。これらのメソッドは `async fn` 化せず、caller に `.await` を要求しない。

想定する利用形は次である。

```rust
ctx.pipe_to_self(
  future,
  |value| Ok(Msg::Completed(value)),
  |error| Ok(Msg::Failed(error)),
)?;
Ok(Behaviors::same())
```

handler は future を起動するまでに必要な値を clone / move し、future 完了後は message として戻す。actor state の更新は completion message を処理する通常の同期 handler で行う。

### Decision 7: async future の lifecycle は actor mailbox delivery に従う

future 完了結果は actor mailbox へ user message として戻る。actor が停止済みなら delivery failure は既存の send error / dead letter 観測経路に乗せる。

restart 時に in-flight future を強制 cancel する機構は初期スコープに入れない。これは actor state を future が borrow しない設計なら安全側であり、完了結果は restart 後の新 actor instance に通常 message として届く。cancel semantics が必要な場合は generation token を上位 API で追加する。

### Decision 8: `pushTimeOut` 互換は optional compatibility として後回しにする

Tokio / Embassy 前提では blocking bounded mailbox の優先度を下げる。bounded mailbox の満杯は現行の overflow strategy、dead letter、mailbox pressure event で観測する。待ちたいユースケースは typed delivery、ask timeout、pull protocol のほうで表現する。

## Implementation Shape

### Phase 1: Tokio task executor addition

`TokioTaskExecutor` / `TokioTaskExecutorFactory` を追加する。既存 `TokioExecutor` / `TokioExecutorFactory` は `spawn_blocking` 互換 executor として維持する。public surface tests を更新し、`DispatcherSelector::Blocking` 経路で blocking executor が使える構成を用意する。

### Phase 2: std Tokio system helper

`actor-adaptor-std` に Tokio 前提の opt-in system config helper を追加する。helper は default dispatcher に `TokioTaskExecutorFactory`、blocking dispatcher に既存 `TokioExecutorFactory` または additive な `TokioBlockingExecutorFactory`、tick driver に `TokioTickDriver` を入れる。

### Phase 3: untyped kernel future-to-message contract の固定

既存 `ActorContext::pipe_to_self` / `pipe_to`、`ContextPipeTask`、waker、actor cell の poll / delivery 経路を確認し、Pekko `pipeToSelf` と同じ future-to-message kernel contract として tests と rustdoc に固定する。actor cell 不在、停止済み、delivery failure、`pipe_to` の `None` delivery suppression を先に明文化する。

### Phase 4: typed thin wrapper の固定

`TypedActorContext::pipe_to_self` / `pipe_to` は untyped kernel adapter の薄い wrapper として維持する。typed E2E と context tests を追加し、Ok / Err future の両方を typed self message に変換すること、`AnyMessage` を caller に露出しないこと、`AdaptMessage` / adapter failure が actor の failure 経路で観測されることを検証する。

### Phase 5: Embassy adapter crate

`modules/actor-adaptor-embassy` を追加し、Embassy executor adapter、tick driver、clock injection helper を実装する。CI 対象 target / feature は別途確認し、初期は compile check と unit contract test を優先する。

### Phase 6: std showcase

`showcases/std/typed/async-first-actor-adapters/main.rs` を追加し、`showcases/std/Cargo.toml` に `typed_async_first_actor_adapters` example として登録する。Tokio 依存を使うため、既存 showcase の `advanced` feature に載せる。

サンプルは次を 1 つの実行フローで示す。

- std Tokio 用 opt-in helper で default dispatcher と blocking dispatcher を分離した `ActorSystemConfig` を作る。
- non-blocking actor は default dispatcher 上で `pipe_to_self` を使い、async completion を typed self message として処理する。
- blocking workload は `DispatcherSelector::Blocking` を指定した actor に分け、default dispatcher を占有しない。
- サンプルは `cargo run -p fraktor-showcases-std --features advanced --example typed_async_first_actor_adapters` で実行できる。

## Risks / Trade-offs

### Risk 1: Tokio worker を actor handler が占有する

default dispatcher が Tokio task executorになると、blocking actor が default dispatcher 上で同期 I/O を実行した場合、Tokio worker を占有する。

緩和策として、opt-in helper の docs と tests で「default dispatcher は non-blocking actor 用」「blocking actor は `DispatcherSelector::Blocking`」を明記し、blocking executor 分離を同じ change で提供する。既存 `TokioExecutor` 利用者の挙動は変えない。

### Risk 2: `tokio::spawn(async move { task(); })` は closure 実行自体を中断しない

mailbox drain closure は `.await` しないため、task 内で一度動き始めると throughput / deadline まで同期実行される。これは現在の mailbox-driven scheduler と同じ性質であり、throughput / throughput deadline を適切に設定することで公平性を保つ。

### Risk 3: Embassy の closure queue が `Box<dyn FnOnce()>` を扱えるか

`actor-core-kernel` の `Executor` trait は `alloc` 前提の boxed closure を受け取る。Embassy target でも alloc を使う前提にするか、Embassy adapter だけ別 executor trait を必要とするかは実装時に確認する。

初期案では `actor-core-kernel` が alloc crate であるため、Embassy adapter も alloc 有効 target を前提にする。

### Risk 4: 既存 `pipe_to_self` の意味論を崩す

`pipe_to_self` は Pekko 互換の重要な async adapter 境界であり、`ask` / `ask_with_status` もこの経路に依存している。ここを別の future-returning handler surface で置き換えると、同期 actor 記述と future 連携の分離が崩れる。

緩和策として、actor API は同期維持を明文化し、既存 `pipe_to_self` / `pipe_to` の signature、呼び出し方、delivery semantics を壊さない。改善は typed docs / tests / error observability の強化に限定する。

### Risk 5: in-flight future の restart semantics

future completion が restart 後の actor に届くことは、場合によっては望ましくない。初期 API では caller が generation token を message に含められるようにし、actor system が暗黙 cancel しないことを明文化する。

## Validation

- `actor-adaptor-std` の Tokio executor tests で default task executor と blocking executor の実行経路を分けて検証する。
- typed integration test で `pipe_to_self` に渡した Ok / Err future の完了が typed message として self に戻ることを検証する。
- `ask` / `ask_with_status` が既存 typed `pipe_to_self` 経路を維持していることを regression test または既存 test で確認する。
- dispatcher tests で `DispatcherSelector::Blocking` が blocking dispatcher id に解決され、std Tokio helper で別 executor factory が登録されることを検証する。
- `showcases/std` の `typed_async_first_actor_adapters` example を `advanced` feature 付きで実行し、std Tokio helper と typed `pipe_to_self` の利用例が動くことを検証する。
- Embassy adapter は初期段階では compile check と bounded queue / signal の unit contract を優先する。
- 最終的にソースコード編集後は `./scripts/ci-check.sh ai all` を実行する。

## Open Questions

- Embassy adapter crate 名は `actor-adaptor-embassy` で確定するか、将来他の embedded executor も想定して `actor-adaptor-embedded` にするか。
- blocking dispatcher helper に既存 `TokioExecutorFactory` を使うか、additive な `TokioBlockingExecutorFactory` を追加して naming を明確化するか。
- `TypedActorContext::pipe_to_self` の rustdoc と cookbook をどこに置き、Pekko `pipeToSelf` との差分をどこまで明文化するか。
- in-flight future の cancel API をこの change に含めるか、generation token cookbook に留めるか。

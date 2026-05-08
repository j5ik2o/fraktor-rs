## Why

`std=Tokio`、`embedded=Embassy` を固定前提にするなら、現在の actor 実行戦略はまだ async runtime の強みを十分に使えていない。

現状の `TokioExecutor` は mailbox drain closure を `spawn_blocking` に流す。これは actor handler が blocking し得る前提では安全側だが、Tokio を std 環境の標準実行基盤にするなら既定 executor としては重い。逆に Embassy では thread blocking という逃げ道がないため、Pekko の `pushTimeOut` 型 blocking bounded mailbox 互換を厚くするより、task / timer / waker / signal を actor runtime adapter に接続するほうが自然である。

この change は、`Mailbox::run` と `Actor::receive` の core 契約を full async 化せず、外側の executor / tick driver / timer と、既存 `pipe_to_self` / `pipe_to` による future-to-message adapter を async-first に整える。Pekko 互換の mailbox drain 意味論と `pipeToSelf` 型の利用者体験を保ちつつ、Tokio / Embassy を「ただの実行先」ではなく actor runtime の主実行基盤として使える状態にする。

## Current Code Baseline

2026-05-08 時点の最新コードでは、次は既に存在している。

- `TokioExecutor` / `TokioExecutorFactory` は `actor-adaptor-std` の `tokio-executor` feature 下に存在するが、`TokioExecutor` はまだ `spawn_blocking` を使う。
- `TokioBlockingExecutor` / `TokioBlockingExecutorFactory` は未実装である。
- `std_actor_system_config` は std monotonic mailbox clock を入れる helper であり、default / blocking dispatcher の Tokio executor factory 分離はまだ行わない。
- `Dispatchers` は `DEFAULT_DISPATCHER_ID` と `DEFAULT_BLOCKING_DISPATCHER_ID` を持ち、`with_dispatcher_factory` で個別登録できる。core 側に Tokio 専用 builder を追加する必要はない。
- `ActorContext::pipe_to_self` / `pipe_to`、`ContextPipeTask`、context pipe waker は untyped kernel 側に実装済みである。
- `TypedActorContext::pipe_to_self` / `pipe_to` は untyped kernel adapter に委譲する typed wrapper として実装済みで、`ask` / `ask_with_status` もこの経路に乗っている。
- `TickDriverKind` は `Auto`、`Manual`、`Std`、`Tokio` までを持つ。`AutoProfileKind::Embassy` は存在するが、`TickDriverKind::Embassy` と `actor-adaptor-embassy` crate は未実装である。

したがって本 change の残作業は、既存 future-to-message surface の再実装ではなく、Tokio executor family の分離、std Tokio helper、Embassy adapter crate、`TickDriverKind::Embassy`、および不足している docs / regression test の補強である。

## What Changes

### 1. Tokio executor を default task と blocking に分ける

`actor-adaptor-std` の Tokio executor family を次の責務に分ける。

- `TokioExecutor`: default dispatcher 用。`tokio::spawn` 相当で短時間の mailbox drain closure を async task として実行する。
- `TokioBlockingExecutor`: blocking dispatcher 用。`tokio::task::spawn_blocking` で同期 I/O / CPU heavy / legacy sync API 呼び出しを隔離する。
- `TokioExecutorFactory`: default dispatcher 用 factory として `TokioExecutor` を生成する。
- `TokioBlockingExecutorFactory`: `DEFAULT_BLOCKING_DISPATCHER_ID` 用 factory として `TokioBlockingExecutor` を生成する。

`DispatcherSelector::Blocking` は `pekko.actor.default-blocking-io-dispatcher` に解決済みなので、その解決先に blocking executor を登録できる構成を整える。

### 2. default dispatcher と blocking dispatcher の既定登録を分離する

現在の `Dispatchers::ensure_default` は default と blocking に同じ configurator を入れ得る。この change では、std / Tokio 構成で default dispatcher と blocking dispatcher に別 configurator を登録できる API または helper を追加する。

`ActorSystemConfig::default()` の no_std / core 単体既定は引き続き inline executor でよい。一方、std Tokio 用 helper は default を Tokio task executor、blocking を Tokio blocking executor にする。

### 3. Embassy adapter の設計を追加する

新しい `actor-adaptor-embassy` workspace member を追加する。初期スコープは actor-core への Embassy 接続に限定し、remote / stream / persistence の Embassy 対応は含めない。

Embassy adapter は次を提供する。

- Embassy task で mailbox drain request を受ける dispatcher executor adapter
- `embassy-time` による `TickDriver` 実装
- `embassy-time::Instant` 相当を mailbox throughput deadline clock として注入する helper
- thread blocking を前提にしない bounded ready queue / signal ベースの wakeup

### 4. `pipe_to_self` / `pipe_to` を future-to-message adapter として維持する

Pekko typed は actor 記述を同期的に保ちつつ、`ActorContext.pipeToSelf` で `Future` / `CompletionStage` の完了結果を self message に変換する。fraktor-rs も既に `ActorContext::pipe_to_self`、`ActorContext::pipe_to`、`TypedActorContext::pipe_to_self`、`TypedActorContext::pipe_to` を持つため、この change ではそれらを壊さず、Pekko 互換の async adapter 境界として明文化する。

actor API 面では `Actor::receive`、`TypedActor::receive`、`Behaviors::receive_message`、`MessageInvoker::invoke` は同期 contract のまま維持する。async I/O の完了結果は `pipe_to_self` / `pipe_to` 経由で mailbox message に戻し、actor state の更新は completion message handler 内で同期的に行う。

実装順序は untyped kernel first とする。最新コードでは `ActorContext::pipe_to_self` / `pipe_to`、`ContextPipeTask`、waker は既に存在するため、追加実装は不足している failure / stopped actor 観測の regression test と rustdoc 補強を中心にする。その上にある `TypedActorContext::pipe_to_self` / `pipe_to` は薄い typed wrapper として維持し、追加作業は Err future、adapter failure、`ask` / `ask_with_status` regression の明文化に絞る。

必要な改善は handler が `Future` を返す新 contract の追加ではなく、既存 typed pipe helper のテスト、rustdoc、adapter failure 観測、Pekko `pipeToSelf` との差分整理である。

### 5. blocking bounded mailbox 互換を低優先度へ明文化する

Pekko `pushTimeOut` 系 bounded mailbox は、この change では実装しない。Tokio / Embassy 前提では、bounded mailbox の満杯を thread block で待つより、overflow policy、mailbox pressure notification、typed delivery / ask / pull protocol で backpressure を表現する。

## Capabilities

### Modified Capabilities

- **`dispatch-executor-unification`**
  - Tokio executor family を default task executor と blocking executor に分ける。
  - default dispatcher と blocking dispatcher の既定登録を分離する。
  - mailbox drain は sync / non-awaiting のまま維持する。

- **`std-tick-driver`**
  - `TickDriverKind` に Embassy variant を追加し、Embassy driver を metrics / snapshot 上で識別できるようにする。

### New Capabilities

- **`actor-embassy-adapter`**
  - `actor-core` を Embassy task / timer / signal に接続する adapter を定義する。

- **`actor-future-to-message-surface`**
  - 既存 `pipe_to_self` / `pipe_to` を Pekko `pipeToSelf` 型の async adapter surface として維持・強化する。

## Impact

**影響を受けるコード**:

- `modules/actor-adaptor-std/src/std/dispatch/dispatcher/`
  - `TokioExecutor` の実行方式変更
  - `TokioBlockingExecutor` / `TokioBlockingExecutorFactory` 追加
  - public re-export と public surface test 更新
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers.rs`
  - default / blocking の同時登録または個別登録 helper 追加
- `modules/actor-core/src/core/kernel/actor/setup/actor_system_config.rs`
  - std adapter が default / blocking dispatcher を分けて登録しやすい config 経路追加
- `modules/actor-core/src/core/kernel/actor/`
  - `ActorContext::pipe_to_self` / `pipe_to`、`ContextPipeTask`、waker、delivery failure 観測を future-to-message kernel contract として固定
- `modules/actor-core/src/core/typed/actor/`
  - kernel contract の上に乗る薄い typed wrapper として、既存 `TypedActorContext::pipe_to_self` / `pipe_to` の互換性テスト、rustdoc、adapter failure 観測を強化
- `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tick_driver_kind.rs`
  - `TickDriverKind::Embassy` 追加
- `modules/actor-adaptor-embassy/` (新規)
  - Embassy executor adapter / tick driver / clock injection helper 追加
- `docs/plan/2026-04-26-mailbox-dispatcher-async-judgement.md`
  - 実装後に採用判断と未解決論点を更新

**影響を受ける公開 API 契約**:

- `TokioExecutor` は default / non-blocking dispatcher 用 executor として定義し直される。
- blocking 用に `TokioBlockingExecutor` と `TokioBlockingExecutorFactory` が追加される。
- std Tokio 用 actor system helper が default / blocking dispatcher の別登録を提供する。
- 既存 untyped `pipe_to_self` / `pipe_to` は future-to-message kernel adapter として維持される。
- typed `pipe_to_self` / `pipe_to` は kernel adapter の薄い wrapper として維持され、typed actor 利用者が `AnyMessage` を直接扱わずに `Future` 完了結果を self / target message へ戻せることを公開契約として明文化する。
- Embassy adapter crate が新規公開される。

**破壊的変更**:

- `TokioExecutor` の実行方式は `spawn_blocking` から Tokio task 実行へ変わる。
- blocking 前提の actor は default dispatcher ではなく `DispatcherSelector::Blocking` を明示的に使う必要がある。
- `TokioExecutorFactory` を blocking executor として使っていた内部テストや例は `TokioBlockingExecutorFactory` に移行する。

## Non-goals

- **full async core 化**: `Actor::receive`、`MessageInvoker::invoke`、`Mailbox::run` を `async fn` 化しない。
- **Pekko `pushTimeOut` 互換 mailbox**: blocking bounded mailbox はこの change では実装しない。
- **remote / stream / persistence の Embassy 対応**: 初期 Embassy adapter は actor runtime の executor / tick / clock 接続に限定する。
- **Tokio current-thread runtime 対応**: 初期スコープでは multi-thread Tokio runtime を前提にし、current-thread runtime は明示的に非対応または future work とする。
- **future-returning actor API**: actor / behavior handler が `Future` を返す新 contract はこの change では導入しない。
- **`pipe_to_self` の置き換え**: 既存 `pipe_to_self` / `pipe_to` を別 API へ置き換えず、Pekko 互換の future-to-message adapter として維持する。

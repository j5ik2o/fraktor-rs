## Why

`std=Tokio`、`embedded=Embassy` を固定前提にするなら、現在の actor 実行戦略はまだ async 実行環境の強みを十分に使えていない。

現状の `TokioExecutor` は mailbox drain closure を `spawn_blocking` に流す。これは actor handler が blocking し得る前提では安全側だが、Tokio を std 環境の標準実行基盤にするなら既定 executor としては重い。逆に Embassy では thread blocking という逃げ道がないため、Pekko の `pushTimeOut` 型 blocking bounded mailbox 互換を厚くするより、task / timer / waker / signal を actor adapter に接続するほうが自然である。

この change は、`Mailbox::run` と `Actor::receive` の core 契約を full async 化せず、外側の executor / tick driver / timer を async 実行環境に接続しやすくする。既存 `pipe_to_self` / `pipe_to` はユーザー向け API を変えず、Pekko 互換の future-to-message adapter 境界として明文化・補強する。ここでの async-first は、既存 API を `async fn` 化したり caller に `.await` を要求したりする意味ではない。

## What Changes

### 1. Tokio task executor を opt-in で追加する

`actor-adaptor-std` の既存 `TokioExecutor` / `TokioExecutorFactory` は source / behavior compatible に維持する。既存型を default task executor に再定義しない。

async-first な default dispatcher 用には、追加 API として次を提供する。

- `TokioTaskExecutor`: default dispatcher 用。`tokio::spawn` 相当で短時間の mailbox drain closure を async task として実行する。
- `TokioTaskExecutorFactory`: default dispatcher 用 factory として `TokioTaskExecutor` を生成する。

既存 `TokioExecutor` は `spawn_blocking` 系の互換 executor として残し、blocking dispatcher 用 helper では既存 `TokioExecutorFactory` を使える。明示的な命名が必要なら `TokioBlockingExecutor` / `TokioBlockingExecutorFactory` を追加してよいが、それは既存 `TokioExecutor` の置き換えではなく additive alias / wrapper に留める。

`DispatcherSelector::Blocking` は `DEFAULT_BLOCKING_DISPATCHER_ID` に解決済みなので、その解決先に blocking executor を登録できる構成を整える。文字列リテラルではなく `actor-core-kernel` の dispatcher id 定数を使う。

### 2. default dispatcher と blocking dispatcher の既定登録を分離する

現在の `Dispatchers::ensure_default` は default と blocking に同じ configurator を入れ得る。この change では、std / Tokio 構成で default dispatcher と blocking dispatcher に別 configurator を登録できる opt-in helper を追加する。既存の `ActorSystemConfig::default()`、`ActorSystemConfig::new(...)`、既存 dispatcher factory の呼び出し方は変更しない。

`ActorSystemConfig::default()` の no_std / core 単体既定は引き続き inline executor でよい。一方、std Tokio 用 opt-in helper は default を Tokio task executor、blocking を spawn_blocking 互換 executor にする。

### 3. Embassy adapter の設計を追加する

新しい `actor-adaptor-embassy` workspace member を追加する。初期スコープは `actor-core-kernel` への Embassy 接続に限定し、remote / stream / persistence の Embassy 対応は含めない。

Embassy adapter は次を提供する。

- Embassy task で mailbox drain request を受ける dispatcher executor adapter
- `embassy-time` による `TickDriver` 実装
- `embassy-time::Instant` 相当を mailbox throughput deadline clock として注入する helper
- thread blocking を前提にしない bounded ready queue / signal ベースの wakeup

### 4. `pipe_to_self` / `pipe_to` を future-to-message adapter として維持する

Pekko typed は actor 記述を同期的に保ちつつ、`ActorContext.pipeToSelf` で `Future` / `CompletionStage` の完了結果を self message に変換する。fraktor-rs も既に `ActorContext::pipe_to_self`、`ActorContext::pipe_to`、`TypedActorContext::pipe_to_self`、`TypedActorContext::pipe_to` を持つため、この change ではそれらを壊さず、Pekko 互換の async adapter 境界として明文化する。

この項目は API 変更ではない。既存メソッド名、同期メソッドとしての呼び出し方、戻り値、mapper closure contract を source-compatible に維持する。`pipe_to_self` / `pipe_to` を `async fn` 化しない。caller に `ctx.pipe_to_self(...).await` のような新しい呼び出し方を要求しない。

actor API 面では `Actor::receive`、`TypedActor::receive`、`Behaviors::receive_message`、`MessageInvoker::invoke` は同期 contract のまま維持する。async I/O の完了結果は `pipe_to_self` / `pipe_to` 経由で mailbox message に戻し、actor state の更新は completion message handler 内で同期的に行う。

実装順序は untyped kernel first とする。まず `ActorContext::pipe_to_self` / `pipe_to`、`ContextPipeTask`、waker、delivery failure 観測を kernel contract として固定し、その上に `TypedActorContext::pipe_to_self` / `pipe_to` を薄い typed wrapper として整える。

必要な改善は handler が `Future` を返す新 contract の追加ではなく、既存 typed pipe helper のテスト、rustdoc、adapter failure 観測、Pekko `pipeToSelf` との差分整理である。

### 5. blocking bounded mailbox 互換を低優先度へ明文化する

Pekko `pushTimeOut` 系 bounded mailbox は、この change では実装しない。Tokio / Embassy 前提では、bounded mailbox の満杯を thread block で待つより、overflow policy、mailbox pressure notification、typed delivery / ask / pull protocol で backpressure を表現する。

### 6. std showcase を追加する

`showcases/std` 配下に、この change の std/Tokio 側 API を利用する実行可能サンプルを追加する。サンプルは `showcases/std/typed/async-first-actor-adapters/main.rs` に置き、`showcases/std/Cargo.toml` の `[[example]]` として `typed_async_first_actor_adapters` を登録する。

サンプルでは、std Tokio 用 opt-in helper で default dispatcher に `TokioTaskExecutorFactory`、blocking dispatcher に spawn_blocking 互換 executor を設定し、typed actor が `pipe_to_self` で async completion を self message として処理する流れを示す。blocking workload は `DispatcherSelector::Blocking` を使う actor に分け、default dispatcher で同期 I/O を直接実行しない構成を示す。

## Capabilities

### Modified Capabilities

- **`dispatch-executor-unification`**
  - 既存 `TokioExecutor` を維持したまま、default task executor 用の opt-in Tokio task executor を追加する。
  - opt-in std Tokio helper で default dispatcher と blocking dispatcher の登録を分離する。
  - `showcases/std` に opt-in std Tokio helper と blocking dispatcher の使い分けを示す実行可能サンプルを追加する。
  - mailbox drain は sync / non-awaiting のまま維持する。

- **`std-tick-driver`**
  - `TickDriverKind` に Embassy variant を追加し、Embassy driver を metrics / snapshot 上で識別できるようにする。

### New Capabilities

- **`actor-embassy-adapter`**
  - `actor-core-kernel` を Embassy task / timer / signal に接続する adapter を定義する。

- **`actor-future-to-message-surface`**
  - 既存 `pipe_to_self` / `pipe_to` を Pekko `pipeToSelf` 型の async adapter surface として維持・強化する。

## Impact

**影響を受けるコード**:

- `modules/actor-adaptor-std/src/dispatch/dispatcher/`
  - `TokioTaskExecutor` / `TokioTaskExecutorFactory` 追加
  - 必要なら additive な `TokioBlockingExecutor` / `TokioBlockingExecutorFactory` 追加
  - public re-export と public surface test 更新
- `modules/actor-core-kernel/src/dispatch/dispatcher/dispatchers.rs`
  - default / blocking の同時登録または個別登録 helper 追加
- `modules/actor-core-kernel/src/actor/setup/actor_system_config.rs`
  - std adapter が default / blocking dispatcher を分けて登録しやすい config 経路追加
- `modules/actor-core-kernel/src/actor/`
  - `ActorContext::pipe_to_self` / `pipe_to`、`ContextPipeTask`、waker、delivery failure 観測を future-to-message kernel contract として固定
- `modules/actor-core-typed/src/actor/`
  - kernel contract の上に乗る薄い typed wrapper として、既存 `TypedActorContext::pipe_to_self` / `pipe_to` の互換性テスト、rustdoc、adapter failure 観測を強化
- `modules/actor-core-kernel/src/actor/scheduler/tick_driver/tick_driver_kind.rs`
  - `TickDriverKind::Embassy` 追加
- `modules/actor-adaptor-embassy/` (新規)
  - Embassy executor adapter / tick driver / clock injection helper 追加
- `docs/plan/2026-04-26-mailbox-dispatcher-async-judgement.md`
  - 実装後に採用判断と未解決論点を更新
- `showcases/std/`
  - `typed_async_first_actor_adapters` example を追加し、std Tokio helper、blocking dispatcher、typed `pipe_to_self` の利用例を示す

**影響を受ける公開 API 契約**:

- 既存 `TokioExecutor` / `TokioExecutorFactory` は source / behavior compatible に維持される。
- default task executor 用に `TokioTaskExecutor` / `TokioTaskExecutorFactory` が additive に追加される。
- std Tokio 用 actor system helper が opt-in で default / blocking dispatcher の別登録を提供する。
- 既存 untyped `pipe_to_self` / `pipe_to` は signature と呼び出し方を変えず、future-to-message kernel adapter として維持される。
- typed `pipe_to_self` / `pipe_to` は signature と呼び出し方を変えず、kernel adapter の薄い wrapper として維持され、typed actor 利用者が `AnyMessage` を直接扱わずに `Future` 完了結果を self / target message へ戻せることを公開契約として明文化する。
- Embassy adapter crate が新規公開される。

**破壊的変更**:

- 既存ユーザー向け API の破壊的変更は含めない。
- 新しい async-first std Tokio helper を opt-in で使う場合、blocking workload は `DispatcherSelector::Blocking` へ分ける必要がある。既存 `TokioExecutor` / `TokioExecutorFactory` を使う既存構成の呼び出し方は変更しない。

## Non-goals

- **full async core 化**: `Actor::receive`、`MessageInvoker::invoke`、`Mailbox::run` を `async fn` 化しない。
- **Pekko `pushTimeOut` 互換 mailbox**: blocking bounded mailbox はこの change では実装しない。
- **remote / stream / persistence の Embassy 対応**: 初期 Embassy adapter は actor system の executor / tick / clock 接続に限定する。
- **Tokio current-thread flavor 対応**: 初期スコープでは multi-thread Tokio flavor を前提にし、current-thread flavor は明示的に非対応または future work とする。
- **future-returning actor API**: actor / behavior handler が `Future` を返す新 contract はこの change では導入しない。
- **`pipe_to_self` の置き換え**: 既存 `pipe_to_self` / `pipe_to` を別 API へ置き換えず、Pekko 互換の future-to-message adapter として維持する。
- **既存 public API の async fn 化 / rename**: 既存の actor context API、executor API、system config API を `async fn` 化、rename、削除しない。必要な async-first surface は additive API として追加する。
- **module-local examples**: `modules/**/examples` にはサンプルを追加しない。std 環境向けの実行可能サンプルは `showcases/std` 配下に置く。

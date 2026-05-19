## Context

`modules/actor/src/std` には、`core` が定義した port を std/tokio/tracing に接続する adapter 実装と、`core` 型を std 向けに包み直した façade / wrapper が同居している。現在の `std` 公開面には `ActorContext`、`Props`、`TypedActorContext`、`TypedActorRef`、`TypedProps`、`TypedActorSystem` などの mirror API があり、examples / tests / `std` 内部実装がそれらに依存している。

一方で、モジュール構造ルールでは `std` は「core の port を実装する adapter 層」であることが求められている。後方互換は不要なので、今の段階で façade を縮退させ、`std` を adapter と std 固有ユーティリティに寄せる価値が高い。

## Goals / Non-Goals

**Goals:**
- `modules/actor/src/std` の公開 API から、`core` mirror の純粋 wrapper / façade を除外する
- examples / tests / `std` 内部実装の依存を `std` façade から `core` に付け替える
- façade 依存で成立していた shim (`ActorAdapter`, `TypedActorAdapter`) を不要化して削除する
- `dispatch` / `scheduler` / `event` の adapter subsystem は維持する

**Non-Goals:**
- `std::dispatch`, `std::scheduler::tick`, `std::event`, `std::system::coordinated_shutdown`, `std::pattern::circuit_breaker` を adapter / std 固有実装として再設計し直すこと
- `core` 側の port や意味論を変更すること
- backward compatibility の維持

## Decisions

### 1. 純粋 wrapper を先に `pub(crate)` 化し、削除は最後に行う

いきなり削除すると影響範囲が追いにくい。まず `std.rs` の再エクスポートを止め、対象型を `pub(crate)` へ縮退させる。これにより examples / tests / 他モジュールで壊れる箇所が明確になる。依存を `core` に付け替えた後で、実体ファイルを削除する。

### 2. 第1波は `std::typed::actor::*` の mirror wrapper から始める

最初に対象にするのは次の純粋 wrapper。

- `TypedActorContext`
- `TypedActorContextRef`
- `TypedActorRef`
- `TypedChildRef`

これらは `inner` に core typed 型を保持し、`from_core` / `as_core` / `into_core` / `Deref` が主責務であり、std/tokio/tracing 接続責務を持たない。まずここを縮退させることで、typed examples / tests の依存を `core::typed` へ移す。

### 3. `std::typed::Behaviors` は今回残す

`std::typed::Behaviors` は façade 的な側面もあるが、`tracing` 連携 (`log_messages*`, `with_mdc*`) を抱える std 固有 helper でもある。今回の変更では「純粋 wrapper だけ」を対象にし、`Behaviors` 自体は残す。その代わり、内部で必要な wrapper は `pub(crate)` でのみ維持し、外部からは見せない。

### 4. `ActorAdapter` / `TypedActorAdapter` は façade 群の依存を消した後に削除する

これらは Port&Adapter の adapter ではなく、`std::Props` / `std::TypedProps` と `std::actor::Actor` / `std::typed::actor::TypedActor` を成立させる shim である。したがって単独で判断せず、以下の façade と同じ波で消す。

- `std::actor::Actor`
- `std::actor::ActorContext`
- `std::props::Props`
- `std::typed::TypedProps`
- `std::typed::TypedActorSystem`

### 5. 残す adapter subsystem を明確化する

次は削除対象外とする。

- `std::dispatch::dispatcher::{DispatchExecutor, ThreadedExecutor, TokioExecutor, DispatchExecutorAdapter, StdScheduleAdapter, PinnedDispatcher}`
- `std::scheduler::tick::{TickDriverConfig, tokio_impl}`
- `std::event::logging::TracingLoggerSubscriber`
- `std::event::stream::{EventStreamSubscriber, subscriber_handle, EventStreamSubscriberAdapter, DeadLetterLogSubscriber}`
- `std::system::coordinated_shutdown*`
- `std::pattern::circuit_breaker*`

これらは core port 実装、または std 固有の runtime 接続責務を持つため、今回の削除対象ではない。

### 6. 例示コードは `core` を直接使う

`modules/actor/examples/*_std` は、名前に `std` を含んでいても、typed/untyped actor API 自体は `core` を直接使う形へ寄せる。`std` から使うのは runtime adapter と std 固有 utility のみとする。

## Risks / Trade-offs

- **`std::typed::Behaviors` の内部依存が広い** → 第1波では wrapper の公開面だけを閉じ、内部使用は許容する
- **examples の import 変更範囲が広い** → まず `std::typed::actor::*` を閉じて、壊れた箇所を順に `core::typed` へ付け替える
- **`std::Props` / `std::TypedProps` を落とすと shim 削除が連鎖する** → façade 削除を wave 単位で進め、typed 側を先に、untyped 側を後に回す
- **`std::system::base.rs` は façade 色が強いが quickstart convenience でもある** → 今回は残し、純粋 wrapper に限定する

## Migration Plan

1. `std.rs` で `std::typed::actor::*` の公開 re-export を止める
2. examples / `std/tests.rs` / `std` 内部の依存を `core::typed` へ付け替える
3. `std::typed::actor::*` の実体ファイルを削除する
4. 同じ手順で `std::typed::{TypedProps, TypedActorSystem}` を縮退させる
5. 次に `std::actor::{Actor, ActorContext}` と `std::props::Props` を縮退させ、`ActorAdapter` を削除する
6. `std::system::ActorSystemConfig` を最後に縮退させる
7. `std/tests.rs` を「残すべき std API のみ」に更新して固定する

## Open Questions

- `std::system::base.rs` を将来的に convenience API として残すか、別 change で core へ寄せるか
- `std::typed::Behaviors` のうち、`core::typed::Behaviors` に寄せられるメソッドを別 change でさらに縮退させるか

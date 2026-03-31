## Why

`modules/actor/src/core` は前回の変更（`2026-03-28`）で `kernel` / `typed` の最上位分離と `typed/receptionist` / `pubsub` / `routing` の責務 package 化を行ったが、未対応領域が2つある。(1) `kernel/` は Pekko の `routing.*`・`io.*`・`util.*`・`actor/setup` に対応する package が存在せず、16 package がフラットに並んだまま責務境界が曖昧。(2) `typed/` の root 公開面は DSL ビルダー群（`Behaviors`・`FsmBuilder`・`StashBuffer`・routing builders 等）と内部実装型（`BehaviorRunner` 等）が混在したまま Pekko の `scaladsl/`・`internal/`・`eventstream/` に対応する package が存在しない。正式リリース前の今この段階で構造を整える。

## What Changes

- `kernel/actor/setup/` を新設する（Pekko `actor/setup` 相当）
- `kernel/io/` を新設する（Pekko `io` 相当、今回は package 境界確立）
- `kernel/routing/` を新設する（Pekko `routing` 相当、untyped routing、今回は package 境界確立）
- `kernel/util/` を新設し `messaging/byte_string` を移設する（Pekko `util` 相当）
- `typed/dsl/` を新設し、DSL ビルダー群（`Behaviors`・`FsmBuilder`・`StashBuffer`・`TimerScheduler`・`TypedAsk*`・`StatusReply*`・`Supervise`・`FailureHandler`）と `routing/`・`scheduler/` の公開 API を集約する（Pekko `scaladsl/` 相当）
- `typed/internal/` を新設し、内部実装型（`BehaviorRunner`・`TypedActorAdapter`・scheduler 実装・`ReceiveTimeoutConfig`・`BehaviorSignalInterceptor`）を隔離する（Pekko `typed/internal/` 相当）
- `typed/eventstream/` を新設する（Pekko `typed/eventstream/` 相当）
- `TypedActorRef` を `typed/actor/actor_ref.rs` から `typed/actor_ref.rs`（root level）へ昇格させる（Pekko `ActorRef.scala` が root 公開面に存在することに対応）
- `typed/routing/` と `typed/scheduler/` を上記 package へ吸収し削除する
- `core/typed.rs` の root 公開面を Pekko root 相当の基盤型のみに絞る
- **BREAKING** `crate::core::typed::Behaviors` 等 → `crate::core::typed::dsl::*`
- **BREAKING** `crate::core::typed::BehaviorSignalInterceptor` 等 → `crate::core::typed::internal::*`
- **BREAKING** `crate::core::typed::routing::*` → `crate::core::typed::dsl::*`
- **BREAKING** `crate::core::typed::actor::TypedActorRef` → `crate::core::typed::TypedActorRef`
- **BREAKING** `crate::core::kernel::messaging::ByteString` → `crate::core::kernel::util::ByteString`
- 実装時は file move / mod wiring ごとに `./scripts/ci-check.sh ai dylint` を実行し、最後に `./scripts/ci-check.sh ai all` で全体確認する

## Capabilities

### New Capabilities

- `actor-kernel-new-packages`: `kernel/io/`・`kernel/routing/`・`kernel/util/`・`kernel/actor/setup/` の package 境界を確立する
- `actor-typed-dsl-package`: `typed/dsl/` package を新設し、Pekko `scaladsl/` 相当の DSL ビルダー群を集約する
- `actor-typed-internal-package`: `typed/internal/` package を新設し、Pekko `internal/` 相当の内部実装型を集約する
- `actor-typed-eventstream-package`: `typed/eventstream/` package を新設する
- `actor-typed-root-refinement`: `typed` root 公開面を Pekko root 相当の基盤型に限定し、`TypedActorRef` を root へ昇格する

### Modified Capabilities

## Impact

- 影響対象コード: `modules/actor/src/core/kernel/**`、`modules/actor/src/core/typed/**`、`modules/actor/src/std/**`、関連 tests/examples
- 影響対象 API: `crate::core::typed` 配下の多数の import path（`dsl/`・`internal/` 経由に変更）、`crate::core::kernel::messaging::ByteString` → `util::ByteString`、`TypedActorRef` の path
- 依存関係への影響: 依存 crate の追加は不要。`mod` 配線、`use` 文、tests/examples import の更新が中心
- 検証への影響: 構造変更のたびに `./scripts/ci-check.sh ai dylint` を実行し、最終的に `./scripts/ci-check.sh ai all` が必要

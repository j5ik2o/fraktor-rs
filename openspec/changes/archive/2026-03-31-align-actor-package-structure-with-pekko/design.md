## Context

`modules/actor/src/core` は前回の変更（`2026-03-28`）で `kernel` / `typed` の最上位分離と `typed/receptionist` / `pubsub` / `routing` の責務 package 化を行ったが、以下が未対応のまま残っている。

**kernel 側**：`dead_letter`、`error`、`extension`、`futures`、`lifecycle`、`messaging`、`props`、`scheduler`、`spawn`、`supervision`、`system` が独立した package としてフラットに並んでおり、Pekko の `actor.*`（root まとめ）/ `routing.*`（ルーティング）/ `io.*`（IO）/ `util.*`（ユーティリティ）の責務境界に対応付けられていない。

**typed 側**：root 公開面に DSL ビルダー群（`Behaviors`、`FsmBuilder`、`StashBuffer` 等）と内部実装型（`BehaviorRunner`、`TypedActorAdapter` 等）が混在している。Pekko の `typed/scaladsl/`（DSL surface）/ `typed/internal/`（内部実装）/ `typed/eventstream/`（EventStream）に対応する package が存在しない。さらに `typed/routing/` と `typed/scheduler/` の公開 API が `typed/dsl/` に統合されていない。

## As-Is

```
modules/actor/src/
├── core.rs                # pub mod kernel; pub mod typed;
├── core/
│   ├── kernel.rs          # 16 package をフラットに列挙（多すぎる）
│   │                      # actor, dead_letter, dispatch, error, event, extension,
│   │                      # futures, lifecycle, messaging, pattern, props,
│   │                      # scheduler, serialization, spawn, supervision, system
│   ├── kernel/
│   │   ├── actor/         # actor, actor_path, actor_ref, actor_cell, ...
│   │   ├── dead_letter/   # ← actor 責務に近い
│   │   ├── dispatch/      # dispatcher + mailbox
│   │   ├── error/         # actor_error, send_error
│   │   ├── event/         # event stream, logging
│   │   ├── extension/     # extension registry
│   │   ├── futures/       # actor_future
│   │   ├── lifecycle/     # lifecycle signals
│   │   ├── messaging/     # byte_string, system_message, message_buffer...
│   │   ├── pattern/       # circuit_breaker
│   │   ├── props/         # untyped Props
│   │   ├── scheduler/     # tick_driver, cancellable...
│   │   ├── serialization/ # serializer, registry...
│   │   ├── spawn/         # name_registry, spawn_error
│   │   ├── supervision/   # supervisor strategy
│   │   └── system/        # ActorSystem, guardian, provider...
│   ├── typed.rs           # ← root に 25+ 型を pub use（多すぎる）
│   └── typed/
│       ├── actor/         # TypedActor, TypedActorContext, TypedActorRef (actor/actor_ref.rs)
│       ├── actor_ref_resolver{,_id}.rs
│       ├── behavior{,_interceptor,_runner,_signal,_signal_interceptor}.rs
│       ├── behaviors.rs   # ← dsl に移すべき
│       ├── death_pact_exception.rs
│       ├── delivery/
│       ├── dispatcher_selector.rs
│       ├── extension_setup.rs
│       ├── failure_handler.rs  # ← dsl
│       ├── fsm_builder.rs      # ← dsl
│       ├── mailbox_selector.rs
│       ├── message_adapter/
│       ├── props.rs
│       ├── pubsub/
│       ├── receive_timeout_config.rs  # ← internal
│       ├── receptionist/
│       ├── recipient_ref.rs
│       ├── routing/       # routers, resizer, *_router_builder ← dsl に移すべき
│       ├── scheduler/     # typed_scheduler_* ← dsl/internal に分離すべき
│       ├── spawn_protocol.rs
│       ├── stash_buffer.rs     # ← dsl
│       ├── status_reply{,_error}.rs  # ← dsl
│       ├── supervise.rs        # ← dsl
│       ├── system.rs
│       ├── timer_{key,scheduler}.rs  # ← dsl
│       ├── typed_actor_adapter.rs    # ← internal
│       └── typed_ask_{error,future,response}.rs  # ← dsl
```

## To-Be

```
modules/actor/src/
├── core.rs                # （変更なし）
├── core/
│   ├── kernel.rs          # 整理後の package 宣言
│   ├── kernel/
│   │   ├── actor/         # 変更なし（actor_path, actor_ref, actor_cell, ...)
│   │   │   └── setup/     # NEW: ActorSystemSetup 相当（system/ から移設）
│   │   ├── dead_letter/   # 変更なし
│   │   ├── dispatch/      # 変更なし
│   │   ├── error/         # 変更なし
│   │   ├── event/         # 変更なし
│   │   ├── extension/     # 変更なし
│   │   ├── futures/       # 変更なし
│   │   ├── io/            # NEW: IO サブシステム（Pekko io.* 相当）
│   │   ├── lifecycle/     # 変更なし
│   │   ├── messaging/     # 変更なし（byte_string は util/へ）
│   │   ├── pattern/       # 変更なし
│   │   ├── props/         # 変更なし
│   │   ├── routing/       # NEW: untyped routing（Pekko routing.* 相当）
│   │   ├── scheduler/     # 変更なし
│   │   ├── serialization/ # 変更なし
│   │   ├── spawn/         # 変更なし
│   │   ├── supervision/   # 変更なし
│   │   ├── system/        # 変更なし（setup/ 移設後）
│   │   └── util/          # NEW: ByteString 等（messaging/ から分離）
│   ├── typed.rs           # root 公開面を基盤型のみに絞る
│   └── typed/
│       ├── actor_ref.rs   # NEW: TypedActorRef を typed root へ昇格
│       │                  # （actor/actor_ref.rs から移動、Pekko ActorRef.scala 相当）
│       ├── actor/         # 変更なし（TypedActor, TypedActorContext のみ残す）
│       ├── actor_ref_resolver{,_id}.rs  # 変更なし（root 維持）
│       ├── behavior{,_interceptor,_signal}.rs  # 変更なし（root 維持）
│       ├── death_pact_exception.rs  # 変更なし
│       ├── delivery/      # 変更なし
│       ├── dsl/           # NEW: Pekko scaladsl/ 相当
│       │   ├── behaviors.rs         (← typed/behaviors.rs)
│       │   ├── failure_handler.rs   (← typed/failure_handler.rs)
│       │   ├── fsm_builder.rs       (← typed/fsm_builder.rs)
│       │   ├── routers.rs           (← typed/routing/routers.rs)
│       │   ├── pool_router_builder.rs (← typed/routing/)
│       │   ├── group_router_builder.rs (← typed/routing/)
│       │   ├── balancing_pool_router_builder.rs (← typed/routing/)
│       │   ├── scatter_gather_*.rs  (← typed/routing/)
│       │   ├── tail_chopping_*.rs   (← typed/routing/)
│       │   ├── resizer.rs           (← typed/routing/resizer.rs)
│       │   ├── default_resizer.rs   (← typed/routing/default_resizer.rs)
│       │   ├── stash_buffer.rs      (← typed/stash_buffer.rs)
│       │   ├── status_reply{,_error}.rs (← typed/)
│       │   ├── supervise.rs         (← typed/supervise.rs)
│       │   ├── timer_key.rs         (← typed/timer_key.rs)
│       │   ├── timer_scheduler.rs   (← typed/timer_scheduler.rs)
│       │   ├── typed_ask_error.rs   (← typed/)
│       │   ├── typed_ask_future.rs  (← typed/)
│       │   └── typed_ask_response.rs (← typed/)
│       ├── eventstream/   # NEW: Pekko typed/eventstream/ 相当
│       │   └── event_stream.rs
│       ├── internal/      # NEW: Pekko typed/internal/ 相当
│       │   ├── behavior_runner.rs         (← typed/)
│       │   ├── behavior_signal_interceptor.rs (← typed/, pub use 削除)
│       │   ├── receive_timeout_config.rs  (← typed/)
│       │   ├── typed_actor_adapter.rs     (← typed/)
│       │   ├── scheduler_context.rs       (← typed/scheduler/)
│       │   ├── typed_scheduler_guard.rs   (← typed/scheduler/)
│       │   └── typed_scheduler_shared.rs  (← typed/scheduler/)
│       ├── message_adapter/ # 変更なし
│       ├── pubsub/         # 変更なし
│       ├── receptionist/   # 変更なし
│       ├── recipient_ref.rs # 変更なし
│       ├── props.rs        # 変更なし
│       ├── spawn_protocol.rs # 変更なし
│       └── system.rs       # 変更なし
```

## 差分サマリ

| 対象 | As-Is | To-Be |
|------|-------|-------|
| `kernel/` 最上位 package 数 | 16 | 16 + 3 new (io, routing, util) + setup under actor |
| `typed/` root pub use 数 | 25+ | 13（基盤型のみ） |
| `typed/routing/` | 独立 package | `typed/dsl/` に吸収 |
| `typed/scheduler/` | 独立 package | 公開 API は `dsl/`、実装は `internal/` へ |
| `typed/eventstream/` | 存在しない | 新設 |
| `typed/dsl/` | 存在しない | 新設（DSL ビルダー群 + routing + scheduler API）|
| `typed/internal/` | 存在しない | 新設（内部実装型）|
| `TypedActorRef` の所在 | `typed/actor/actor_ref.rs` | `typed/actor_ref.rs`（root 昇格）|

## Goals / Non-Goals

**Goals:**
- `kernel/` に `io/`、`routing/`、`util/` を新設し Pekko の責務境界に対応する
- `kernel/actor/setup/` を新設する
- `typed/dsl/` を新設し DSL ビルダー群と routing / scheduler 公開 API を集約する
- `typed/internal/` を新設し内部実装型を隔離する
- `typed/eventstream/` を新設する
- `typed.rs` root 公開面を Pekko root 相当の基盤型に限定する
- 構造変更ごとに `./scripts/ci-check.sh ai dylint` を実行する

**Non-Goals:**
- `kernel/` の既存 16 package を今回の変更で全て統合すること（将来の変更で対応）
- actor runtime の振る舞いを新機能として拡張すること
- `kernel/io/`・`kernel/routing/`・`kernel/util/` を完全実装すること（今回は package 境界の確立と stub 新設が目標）

## Decisions

### 1. `typed/routing/` は `typed/dsl/` に吸収する

- 採用: `typed/routing/` 配下の `Routers`、`*RouterBuilder`、`Resizer` 等を `typed/dsl/` へ移設
- 理由: Pekko では `scaladsl/Routers.scala` がルーティング DSL の公開 API であり、router builder 群は scaladsl の一部
- 代替案: `typed/routing/` を独立 package として維持
- 不採用理由: 今後 Pekko scaladsl を参照する際に対応先が不明確になる

### 2. `typed/scheduler/` の公開 API は `dsl/` へ、実装は `internal/` へ移す

- 採用: `TimerScheduler`（facade）は `dsl/timer_scheduler.rs` へ、`TypedSchedulerGuard`・`TypedSchedulerShared`・`SchedulerContext` は `internal/` へ
- 理由: Pekko では `scaladsl/TimerScheduler.scala` が公開 trait、実装は `typed/internal/` に存在する
- 代替案: scheduler 全体を `dsl/` に入れる
- 不採用理由: 内部実装型（Guard、Shared）がクレート外に漏れるべきでない

### 3. `TypedActorRef` を `typed/actor_ref.rs` へ昇格する

- 採用: `typed/actor/actor_ref.rs` → `typed/actor_ref.rs`（root レベルへ移動）
- 理由: Pekko では `ActorRef[T]` が `typed/ActorRef.scala` として root 公開面に存在する。fraktor でも最も頻繁に参照される型であり root に置くべき
- 代替案: `typed/actor/actor_ref.rs` のまま維持
- 不採用理由: import が `crate::core::typed::actor::TypedActorRef` と深く、root 公開面から外れている

### 4. `kernel/io/`・`kernel/routing/`・`kernel/util/` は今回は package 境界の確立に留める

- 採用: 各 package を新設し、`util/` には `messaging/byte_string` を移設。`io/` と `routing/` は stub package として新設
- 理由: 完全実装は機能追加変更として別 change で対応すべき。今回は構造整合が目的
- 代替案: 完全実装まで含めて今回行う
- 不採用理由: scope が大きくなりすぎ、dylint 検証サイクルが回せなくなる

### 5. 実装順: kernel → typed dsl/internal → typed root → std 追随

- 採用: kernel 新 package 確立 → typed dsl 構築（routing/scheduler 含む）→ typed internal 構築 → typed root 公開面更新 → std/tests 追随
- 理由: kernel 変更は typed と独立しており先に完了できる。typed 内では dsl を先に構築することで internal の参照関係が明確になる

## Risks / Trade-offs

- [Risk] `typed/routing/` 吸収で routing 関連 import が広範囲に変わり tests/examples が壊れる → Mitigation: file move 前に `grep` で全参照を列挙し、`dsl::` 経由参照へ先に更新
- [Risk] `TypedActorRef` の昇格で `actor/actor_ref.rs` への参照が全て壊れる → Mitigation: `typed/actor/actor_ref.rs` に `pub use crate::core::typed::actor_ref::TypedActorRef;` を一時的に残し、追随完了後に削除
- [Risk] `typed/scheduler/` の分割で `TimerScheduler` の依存が `dsl/` と `internal/` をまたぐ → Mitigation: `dsl/timer_scheduler.rs` が `pub(crate) mod internal::typed_scheduler_shared` を再 export する形で分離を明確化
- [Risk] `kernel/io/` と `kernel/routing/` が stub のまま lint エラーになる → Mitigation: 各 stub に `//! Placeholder for future IO/routing implementation` を追加し、最低限の型 or empty module として lint を通す

## 実装手順への組み込み

1. 対象タスクで作る package と `mod` 宣言だけを先に追加する
2. 1 責務ずつ file move する
3. file move 直後に `./scripts/ci-check.sh ai dylint` を実行する
4. `pub use` / `use` / `mod` / import path などの mod wiring を行う
5. mod wiring 直後に `./scripts/ci-check.sh ai dylint` を実行する
6. tests / examples 追随が必要な場合は、その更新直後にも `./scripts/ci-check.sh ai dylint` を実行する
7. 1 タスク内で複数責務をまとめて動かさず、次の責務へ進む前に直近の `./scripts/ci-check.sh ai dylint` 成功を確認する
8. `./scripts/ci-check.sh ai all` は final タスクでのみ実行する

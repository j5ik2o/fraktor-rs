## GitHub Issue #734: refactor(actor): modules/actor/src/std/ の残骸クリーンアップ（破壊的変更）

## 概要

`modules/actor/src/std/` に、過去の移行時の残骸として「`core/` の型エイリアスを並べているだけ」の不要な中間層が多数存在する。これらを削除し、`std/` には正当なアダプタ実装のみを残す。

破壊的変更となるが、正式リリース前のため問題なし。

## 背景・ルール

`.agents/rules/rust/module-structure.md` で定義した通り：

- `core/` = no_std コアロジック・ポート（trait）定義
- `std/` = `core/` のポートを std/tokio を使って実装したアダプタ群

`std/` に残るべきは「アダプタの実装実体」のみ。`core/` の型に名前をつけ直しているだけのファイルは不要な残骸。

## 問題1: `mod types; pub use types::*;` パターン

以下のファイルが、単に `mod types; pub use types::*;` で委譲しているだけ：

| 親ファイル | `types.rs` の内容 |
|-----------|-----------------|
| `std/dead_letter.rs` | `DeadLetter`, `DeadLetterEntry` の型エイリアス |
| `std/error.rs` | `SendError` の型エイリアス |
| `std/futures.rs` | `ActorFuture`, `ActorFutureShared`, `ActorFutureListener` の型エイリアス |
| `std/messaging.rs` | `AnyMessage` 等 7 型の型エイリアス |
| `std/dispatch/mailbox.rs` | `Mailbox`, `MailboxOfferFuture`, `MailboxPollFuture` の型エイリアス |
| `std/dispatch/dispatcher/types.rs` | `DispatchShared`, `DispatcherShared` の型エイリアス |
| `std/event/stream/types.rs` | `EventStream`, `EventStreamEvent`, `EventStreamSubscription` の型エイリアス |

各 `types.rs` の内容は全て `pub type X = crate::core::...;` という単純な型エイリアス。親ファイルに直接インライン化し、`types.rs` とサブディレクトリを削除する。

## 問題2: `std/typed/` 内の不要な型エイリアスファイル群

以下は単純な型エイリアスのみで、独立ファイルにする意味がない：

| ファイル | 内容 |
|---------|------|
| `std/typed/behavior.rs` | `Behavior<M>`, `Supervise<M>` のエイリアスのみ |
| `std/typed/spawn_protocol.rs` | `SpawnProtocol` のエイリアスのみ |
| `std/typed/stash_buffer.rs` | `StashBuffer<M>` のエイリアスのみ |
| `std/typed/typed_ask_future.rs` | `TypedAskFutureStd<M>`（冗長）, `TypedAskFuture<M>` |
| `std/typed/typed_ask_response.rs` | `TypedAskResponseStd<M>`（冗長）, `TypedAskResponse<M>` |

これらを `std/typed.rs` に直接記述し、各ファイルを削除する。

## 問題3: `*Std` エイリアスの冗長性（破壊的変更）

```rust
// typed_ask_future.rs
pub type TypedAskFutureStd<M> = crate::core::typed::TypedAskFuture<M>;
pub type TypedAskFuture<M> = TypedAskFutureStd<M>;  // エイリアスのエイリアス
```

`TypedAskFutureStd<M>` / `TypedAskResponseStd<M>` は過去の移行時の残骸。削除する。

## 修正方針

### Step 1: `mod types; pub use types::*;` パターンの解体

各 `types.rs` の内容を親ファイルに直接インライン化し、`types.rs` ファイルと（他に中身がない場合は）サブディレクトリを削除。

### Step 2: `std/typed/` 型エイリアスファイルの統合

`behavior.rs`, `spawn_protocol.rs`, `stash_buffer.rs`, `typed_ask_future.rs`, `typed_ask_response.rs` の内容を `std/typed.rs` に直接記述し各ファイルを削除。`TypedAskFutureStd<M>` / `TypedAskResponseStd<M>` は削除。

### 変更後の `std/typed.rs` のイメージ

```rust
pub mod actor;
mod behaviors;

pub use behaviors::Behaviors;
pub use props::TypedProps;
pub use system::TypedActorSystem;

// 型エイリアスは直接定義
pub type Behavior<M> = crate::core::typed::Behavior<M>;
pub type Supervise<M> = crate::core::typed::Supervise<M>;
pub type BehaviorSignal = crate::core::typed::BehaviorSignal;
pub type SpawnProtocol = crate::core::typed::SpawnProtocol;
pub type StashBuffer<M> = crate::core::typed::StashBuffer<M>;
pub type TypedAskFuture<M> = crate::core::typed::TypedAskFuture<M>;
pub type TypedAskResponse<M> = crate::core::typed::TypedAskResponse<M>;
```

## `std/` に残る正当なアダプタ実装

| ファイル | 理由 |
|---------|------|
| `std/typed/behaviors.rs` | `tracing` を使った std 固有ロジック |
| `std/typed/props.rs` | `TypedActorAdapter` を介した std 固有ラッピング |
| `std/typed/system.rs` | `EventStreamSubscriberAdapter` 等の std 固有変換 |
| `std/typed/actor/` 配下 | `TypedActorContext`, `TypedActorAdapter` 等の実装 |
| `std/dispatch/dispatcher/` 実装群 | `TokioExecutor`, `ThreadExecutor`, `StdScheduleAdapter` 等 |
| `std/dispatch/dispatcher/base.rs` | `DispatchExecutor` trait の std 実装 |
| `std/event/stream/subscriber.rs` | `EventStreamSubscriber` trait の std 固有定義 |
| `std/event/stream/subscriber_adapter.rs` | core trait へのアダプタ実装 |
| `std/event/logging/` | `TracingLoggerSubscriber` — tracing 依存 |
| `std/actor/` | std 固有のアクターアダプタ |
| `std/system/` | `ActorSystem` の std 固有ビルダー |
| `std/scheduler/tick/tokio_impl.rs` | tokio の tick 実装 |

## 使用ピース

`refactoring`（plan → implement → ai_review → reviewers → supervise → COMPLETE）

### Labels
refactoring
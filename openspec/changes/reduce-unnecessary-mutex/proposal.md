## Why

`mailbox-once-cell` change (#1570) で Mailbox の write-once 3 フィールドを `spin::Once<T>` に置換し、mailbox enqueue が 21% 高速化された。同様のパターンが actor-core の他の箇所にも存在する。

`docs/plan/lock-strategy-analysis.md` の調査 B で分類した 50 箇所の Mutex 使用のうち、以下の 2 カテゴリが不必要な Mutex を持っている:

1. **write-once パターン**: 初期化時に 1 回セット、以後は読み取りのみ → `spin::Once<T>` で代替
2. **single-thread-access パターン**: dispatcher thread からのみアクセス → Mutex は理論上不要だが、`Send + Sync` 制約のため即座に `RefCell` 化はできない。将来の検討対象として記録

本 change は **カテゴリ 1 (write-once)** のみを対象とし、実測ベースで改善効果を確認しながら進める。

## What Changes

### 対象候補 (write-once パターン)

| 型 | フィールド | 現状 | セットタイミング | 以後 |
|---|---|---|---|---|
| `MiddlewareShared` | `inner` | `SharedRwLock<Box<dyn MessageInvokerMiddleware>>` | actor 生成時にチェーン構築 | invoke 時に読むだけ |
| `ActorRefProviderHandleShared` | `inner` | `SharedLock<Option<...>>` | system 初期化時に 1 回セット | lookup 時に読むだけ |
| `ExecutorShared` | `inner` | `SharedLock<Box<dyn Executor>>` | dispatcher 構築時に 1 回 | execute 呼び出し時に読むだけ |
| `MessageDispatcherShared` | `inner` | `SharedLock<Box<dyn MessageDispatcher>>` | system 初期化時に 1 回 | dispatch 時に読むだけ |
| `DeadLetterShared` | `inner` | `SharedRwLock<DeadLetter>` | system 初期化時に 1 回 | dead letter 処理時に読むだけ |

### 対象外 (single-thread-access だが write-once ではない)

以下は記録のみ。`Send + Sync` 制約のため `RefCell` 化は設計変更を伴う:

| 型 | フィールド | 理由 |
|---|---|---|
| `ActorCellStateShared` | `inner` | dispatcher thread のみだが mutable state |
| `ReceiveTimeoutStateShared` | `inner` | dispatcher thread のみだが mutable state |
| `ActorShared` | `inner` | dispatcher thread のみだが `recreate_actor` で書き換え |

## Capabilities

### Modified Capabilities
- `actor-runtime-performance`: write-once Shared 型の read path が Mutex acquire → atomic load に軽量化

## Impact

- 対象コード: `modules/actor-core/src/core/kernel/` 配下の各 `*_shared.rs`
- 影響内容: read path の高速化。write path は `spin::Once::call_once` に変わるが生涯 1 回なので影響なし
- 非目標:
  - single-thread-access パターンの `RefCell` 化（設計変更が必要）
  - Mailbox の `user_queue_lock` 削減（Phase IV outer lock reduction で別途対応）
  - MessageQueue 内部の lock 削減（Phase VII queue lock-free 化で別途対応）

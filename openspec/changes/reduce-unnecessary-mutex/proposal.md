## Why

`mailbox-once-cell` change (#1570) で Mailbox の write-once 3 フィールドを `spin::Once<T>` に置換し、mailbox enqueue が 21% 高速化された。同様のパターンが actor-core の他の箇所にも存在する。

`docs/plan/lock-strategy-analysis.md` の調査 B で分類した 50 箇所の Mutex 使用のうち、以下の 2 カテゴリが不必要な Mutex を持っている:

1. **write-once パターン**: 初期化時に 1 回セット、以後は読み取りのみ → `spin::Once<T>` で代替
2. **single-thread-access パターン**: dispatcher thread からのみアクセス → Mutex は理論上不要だが、`Send + Sync` 制約のため即座に `RefCell` 化はできない。将来の検討対象として記録

本 change は **カテゴリ 1 (write-once)** のみを対象とし、実測ベースで改善効果を確認しながら進める。

## What Changes

### 対象候補 (write-once パターン — コード読解で検証済み)

| 型 | フィールド | 現状 | セットタイミング | 以後 |
|---|---|---|---|---|
| `CoordinatedShutdown` | `reason` | `SharedLock<Option<CoordinatedShutdownReason>>` | `run()` で 1 回セット（`run_started` AtomicBool で排他） | `shutdown_reason()` で読むだけ |
| `ContextPipeWakerHandleShared` | `inner` | `SharedLock<ContextPipeWakerHandle>` | コンストラクタで 1 回セット | `wake()` で clone/copy するだけ（`with_lock` を使うが読み取りのみ） |

### 検証の結果 write-once ではなかった候補（除外）

以下は当初候補としていたが、コード読解の結果 hot path で `&mut self` メソッドが継続的に呼ばれており、`spin::Once<T>` への置換は不可能と判明した:

| 型 | フィールド | 除外理由 |
|---|---|---|
| `MiddlewareShared` | `inner` | `with_write` で `before_user`/`after_user`（`&mut self`）が毎メッセージ呼ばれる |
| `ActorRefProviderHandleShared` | `inner` | `register_temp_actor`/`unregister_temp_actor` 等の変更操作が ongoing |
| `ExecutorShared` | `inner` | `Executor::execute(&mut self)` が毎タスクで呼ばれる |
| `MessageDispatcherShared` | `inner` | `attach`/`detach`/`dispatch` 等が hot path で実行 |
| `DeadLetterShared` | `inner` | `record_send_error`/`record_entry` で dead letter 発生の度に追記 |

除外の詳細: これらの型は `inner` フィールドの **値自体は置換されない** が、`SharedLock`/`SharedRwLock` を通じて内部オブジェクトの `&mut self` メソッドが呼ばれるため、`spin::Once<T>` が返す `&T`（不変参照）では対応できない。

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

- 対象コード:
  - `modules/actor-core/src/core/kernel/system/coordinated_shutdown.rs`（`reason` フィールド）
  - `modules/actor-core/src/core/kernel/actor/context_pipe/context_pipe_waker_handle_shared.rs`（`inner` フィールド）
- 影響内容: read path の高速化。write path は `spin::Once::call_once` に変わるが生涯 1 回なので影響なし
- 非目標:
  - `&mut self` メソッドを呼ぶ Shared 型の置換（`MiddlewareShared` 等 5 型は除外済み）
  - single-thread-access パターンの `RefCell` 化（設計変更が必要）
  - Mailbox の `user_queue_lock` 削減（Phase IV outer lock reduction で別途対応）
  - MessageQueue 内部の lock 削減（Phase VII queue lock-free 化で別途対応）

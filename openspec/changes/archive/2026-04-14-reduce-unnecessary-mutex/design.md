## Context

`mailbox-once-cell` (#1570) で Mailbox の write-once フィールドを `spin::Once<T>` に置換し、mailbox enqueue 21% 高速化を実証した。同じパターンを他の write-once フィールドに適用する。

actor-core 全体（約 35 型・50 箇所の lock field）をコード読解で精査した結果、write-once と判定できたのは **2 箇所のみ** だった。

## Goals / Non-Goals

**Goals:**
- write-once パターンの `SharedLock<T>` を `spin::Once<T>` に置換
- ベンチマークで効果を計測

**Non-Goals:**
- `&mut self` メソッドを呼ぶ Shared 型の置換（MiddlewareShared, ActorRefProviderHandleShared, ExecutorShared, MessageDispatcherShared, DeadLetterShared は検証の結果 write-once ではなく除外済み）
- single-thread-access パターンの `RefCell` 化
- Mailbox `user_queue_lock` の削減
- actor-core 以外のクレート（stream-core, cluster-core 等）の最適化

## Decisions

### 1. 各候補の置換判定基準

write-once と判定する条件:
- フィールドが初期化後に `with_lock(|s| *s = value)` や `call_once` で **1 回だけ**セットされる
- 以後のアクセスが全て `with_read(|s| ...)` または `get()` である
- セット後の値の変更（replace, take, swap）がない
- **`with_lock`/`with_write` を使っていても、内部オブジェクトの `&mut self` メソッドを呼んでいる場合は write-once ではない**

各候補はコード読解で上記を検証してから置換する。検証が不合格なら候補から除外する。

### 2. 対象候補と置換パターン

#### 2a. `CoordinatedShutdown.reason`

```rust
// Before
reason: SharedLock<Option<CoordinatedShutdownReason>>,

// run() で 1 回セット
self.reason.with_write(|stored_reason| *stored_reason = Some(reason));

// shutdown_reason() で読み取り
self.reason.with_read(Clone::clone)

// After
reason: spin::Once<CoordinatedShutdownReason>,

// run() で 1 回セット
self.reason.call_once(|| reason);

// shutdown_reason() で読み取り
self.reason.get().cloned()
```

注意: `CoordinatedShutdown.reason` は `run()` の CAS（`run_started.swap(true, AcqRel)`）で排他されているため、`call_once` の 2 回呼び出しは発生しない。

#### 2b. `ContextPipeWakerHandleShared.inner`

```rust
// Before
pub(crate) struct ContextPipeWakerHandleShared {
  inner: SharedLock<ContextPipeWakerHandle>,
}

impl ContextPipeWakerHandleShared {
  pub(crate) fn new(handle: ContextPipeWakerHandle) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(handle) }
  }
}

// wake() で読み取りのみ（with_lock だが変更なし）
self.inner.with_lock(|guard| (guard.system.clone(), guard.pid, guard.task))

// After
pub(crate) struct ContextPipeWakerHandleShared {
  inner: spin::Once<ContextPipeWakerHandle>,
}

impl ContextPipeWakerHandleShared {
  pub(crate) fn new(handle: ContextPipeWakerHandle) -> Self {
    Self { inner: spin::Once::initialized(handle) }
  }
}

// wake() で読み取り
let handle = self.inner.get().expect("ContextPipeWakerHandle not initialized");
(handle.system.clone(), handle.pid, handle.task)
```

### 3. `spin::Once::initialized()` による即時初期化

`spin::Once` は `const fn initialized(data: T) -> Self` を提供する。コンストラクタで値が確定している場合はこれを使い、`call_once` のオーバーヘッドも省略できる:

- `ContextPipeWakerHandleShared` → `spin::Once::initialized(handle)` を使用
- `CoordinatedShutdown.reason` → `spin::Once::new()` + 後から `call_once` を使用（値が `run()` 時に確定するため）

## Risks / Trade-offs

- [Risk] write-once だと判断した箇所が実は再セットされるケース → Mitigation: 各候補をコード読解で検証済み。`spin::Once::call_once` は 2 回目の呼び出しを無視するため、panic はしないが値が更新されない
- [Risk] `spin::Once::get()` が `None` を返すケース（初期化前アクセス） → Mitigation: `CoordinatedShutdown.reason` は `run()` 前に読まれた場合に `None` を返すが、これは元の `SharedLock<Option<...>>` でも `None` だったので振る舞いが変わらない。`ContextPipeWakerHandleShared` はコンストラクタで即時初期化するため `get()` が `None` になることはない
- [Risk] 2 候補のみのため、パフォーマンス改善効果は限定的 → Mitigation: `ContextPipeWakerHandleShared.wake()` は `Waker::wake()` の hot path にあるため、効果を測定する価値がある

## Open Questions

- なし（当初の `MiddlewareShared` に関する疑問はコード読解で解消済み — hot path で `&mut self` メソッドを呼んでおり write-once ではない）

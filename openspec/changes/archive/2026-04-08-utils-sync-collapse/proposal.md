## Why

`modules/utils` の同期プリミティブまわりは、投機的に立てた抽象が想定通り使われず、4 階層の重ね書きと dead code が積み上がっている。具体的には:

- **`SyncQueueShared` family の **dead sub-types** (7 シンボル / 5 ファイル)** が完全に未使用:
  - producer/consumer 単独型 4 つ (`SyncMpscProducerShared` / `SyncMpscConsumerShared` / `SyncSpscProducerShared` / `SyncSpscConsumerShared`) は workspace 内 caller ゼロ
  - type alias 3 つ (`SyncMpscQueueShared` / `SyncSpscQueueShared` / `SyncPriorityQueueShared`) も同じく caller ゼロ
  - これに付随する `SyncQueueShared` 上の `Mpsc/Spsc/Priority` 専用 impl ブロック (`new_mpsc`, `into_mpsc_pair`, `into_spsc_pair`, `new_spsc`, `peek_min` 等) も到達不能になる

- **`SyncQueueShared` 本体と `SyncFifoQueueShared` alias は production で使用中**:
  - `modules/actor-core/src/core/kernel/dispatch/mailbox.rs:115-116` の `pub(crate) type UserQueueShared<T> = SyncFifoQueueShared<T, VecDequeBackend<T>, RuntimeMutex<...>>;` は mailbox の user message queue 本体
  - `modules/stream-core/src/core/impl/fusing/stream_buffer.rs:13` の `StreamBuffer<T> { queue: SyncFifoQueueShared<T, VecDequeBackend<T>> }` は stream の backpressure buffer 本体
  - したがって `SyncQueueShared` および `SyncFifoQueueShared` は **削除不能**。これらは保持する

- **`SyncMutexLike` trait は `SyncQueueShared` の generic bound として生きている**:
  ```rust
  pub struct SyncQueueShared<T, K, B, M = SpinSyncMutex<SyncQueue<T, K, B>>>
  where
    M: SyncMutexLike<SyncQueue<T, K, B>>, { ... }
  ```
  - 単に trait を消すだけでは `SyncQueueShared` がコンパイルできない
  - trait を消すには `SyncQueueShared` の `M` 型パラメータを **monomorphize** (`SpinSyncMutex` に固定) する必要がある
  - actor-core mailbox.rs の caller は第 3 型パラメータ `RuntimeMutex<...>` を明示的に渡しているため、type alias 1 行の修正が必要

- **`SyncRwLockLike` trait は完全な 1-impl 幽霊抽象**:
  - 唯一の impl は `SpinSyncRwLock` (Std 側 impl は dead code 経路)
  - generic bound としての caller はゼロ
  - actor-core の AShared 系 7 ファイルが `.read()` / `.write()` を呼ぶための trait import として使っているが、`SpinSyncRwLock` に inherent method を生やせば代替可能

- **`StdSyncMutex` / `StdSyncRwLock` 系** (6 ファイル) は production caller ゼロ:
  - すべて `modules/utils/src/std/` 配下の定義 + 自前テスト
  - `std::sync::Mutex` 固有挙動 (poisoning / OS thread parking) に依存している実コードはなし
  - actor framework は短時間ロック前提であり、`spin::Mutex` (`portable-atomic` + `critical-section` 構成) が全 target で十分

- **5 名前 → 1 型**の重ね書き alias chain:
  ```
  RuntimeMutex<T> ─┐
  StdMutex<T>   ──┼──→ RuntimeMutexBackend<T> ──┬→ StdSyncMutex<T> (feature=std)
  NoStdMutex<T> ──┘                              └→ SpinSyncMutex<T> (no std)
  ```
  - feature switch の Std 側がそもそも production 利用ゼロなので、cfg 切り替え自体が無意味化
  - workspace feature unification で actor-core が想定外に Std 経路を引いている可能性あり (foot-gun)

- **規約と clippy ルールと型定義の三方向不整合**:
  - 規約 `.agents/rules/rust/immutability-policy.md`: AShared パターンに `ArcShared<SpinSyncMutex<A>>` を**指名**
  - `clippy.toml` の `disallowed-types`: `std::sync::Mutex` の replacement target に `SyncMutexLike` を指定
  - 型定義: `RuntimeMutex<T>` を主流の alias として export
  - 結果として `SpinSyncMutex` 直接参照 44 ファイル + `RuntimeMutex` 経由 106 ファイルの**漏洩**が常態化

これらは「投機的抽象 → 実需が出なかった → 各所が最短経路を選択 → 規約が迂回側に引っ張られる」という典型的な失敗パターンであり、`mailbox-block-overflow-removal` が解消した async backpressure scaffolding と同じ系統の design debt である。

`modules/utils` の `utils-dead-code-removal` capability spec が **既に存在**しており、`RcShared` / `StaticRefShared` / `AsyncMutexLike` 等を「公開 API から除外されていなければならない」と requirement にしている。本 change は同 capability の禁止リストを拡張する形で、上記の dead code と幽霊抽象を一掃する。

## What Changes

### 削除対象 (本体)

#### Dead sub-types of SyncQueueShared family (caller ゼロ)

- ファイル削除:
  - `modules/utils/src/core/collections/queue/sync_mpsc_producer_shared.rs`
  - `modules/utils/src/core/collections/queue/sync_mpsc_consumer_shared.rs`
  - `modules/utils/src/core/collections/queue/sync_spsc_producer_shared.rs`
  - `modules/utils/src/core/collections/queue/sync_spsc_consumer_shared.rs`
  - `modules/utils/src/core/collections/queue/sync_spsc_producer_shared/tests.rs` (存在する場合)
- `sync_queue_shared.rs` から impl ブロック削除:
  - `impl<T, B, M> SyncQueueShared<T, MpscKey, B, M>` (`new_mpsc`, `producer_clone`, `into_mpsc_pair`)
  - `impl<T, B, M> SyncQueueShared<T, SpscKey, B, M>` (`new_spsc`, `into_spsc_pair`)
  - `impl<T, B, M> SyncQueueShared<T, PriorityKey, B, M>` (`peek_min`)
- `sync_queue_shared.rs` から alias 削除:
  - `pub type SyncMpscQueueShared = ...`
  - `pub type SyncSpscQueueShared = ...`
  - `pub type SyncPriorityQueueShared = ...`
- `queue.rs` の `mod` / `pub use` から該当エントリ削除
- `queue/tests.rs` から `MpscKey` / `SpscKey` / `PriorityKey` を使う test を削除 (`offer_and_poll_fifo_queue`, `vec_ring_backend_provides_fifo_behavior` 等の `FifoKey` テストは保持)
- 公開シンボル: `SyncMpscQueueShared`, `SyncSpscQueueShared`, `SyncPriorityQueueShared`, `SyncMpscProducerShared`, `SyncMpscConsumerShared`, `SyncSpscProducerShared`, `SyncSpscConsumerShared`

#### Std 側 sync 系 (production caller ゼロ)

- `modules/utils/src/std/` 全削除:
  - `sync_mutex.rs` / `sync_mutex_guard.rs` (+ tests)
  - `sync_rwlock.rs` / `sync_rwlock_read_guard.rs` / `sync_rwlock_write_guard.rs` (+ tests)
- `modules/utils/src/std.rs` (mod 宣言ファイル) 削除
- `modules/utils/src/lib.rs` の `#[cfg(feature = "std")] pub mod std;` と `#[cfg(not(feature = "std"))] mod std { ... compat shim ... }` を削除
- `modules/utils/src/lib.rs` の `RuntimeMutexBackend` / `RuntimeRwLockBackend` 中間 alias を削除
- `modules/utils/Cargo.toml` の `feature = "std"` を削除
- 公開シンボル: `StdSyncMutex`, `StdSyncMutexGuard`, `StdSyncRwLock`, `StdSyncRwLockReadGuard`, `StdSyncRwLockWriteGuard`, `StdMutex` (caller ゼロ)
- 内部シンボル: `RuntimeMutexBackend`, `RuntimeRwLockBackend`

#### 1-impl trait

- `modules/utils/src/core/sync/sync_mutex_like.rs` (trait 定義) 削除
- `modules/utils/src/core/sync/sync_rwlock_like.rs` (trait 定義) 削除
- `modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex.rs` の `impl SyncMutexLike for SpinSyncMutex` ブロック削除
- `modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock.rs` の `impl SyncRwLockLike for SpinSyncRwLock` ブロック削除
- 公開シンボル: `SyncMutexLike`, `SyncRwLockLike`

trait を削除可能にするため、本 change で **`SyncQueueShared` の `M` 型パラメータを monomorphize する**:

```rust
// Before
pub struct SyncQueueShared<T, K, B, M = SpinSyncMutex<SyncQueue<T, K, B>>>
where
  K: TypeKey,
  B: SyncQueueBackend<T>,
  M: SyncMutexLike<SyncQueue<T, K, B>>,
{
  inner: ArcShared<M>,
  _pd:   PhantomData<(T, K, B)>,
}

// After
pub struct SyncQueueShared<T, K, B>
where
  K: TypeKey,
  B: SyncQueueBackend<T>,
{
  inner: ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>>,
  _pd:   PhantomData<(T, K, B)>,
}
```

`SyncFifoQueueShared` alias も対応して 2 パラメータ化:

```rust
// Before
pub type SyncFifoQueueShared<T, B, M = SpinSyncMutex<SyncQueue<T, FifoKey, B>>> = SyncQueueShared<T, FifoKey, B, M>;

// After
pub type SyncFifoQueueShared<T, B> = SyncQueueShared<T, FifoKey, B>;
```

### 追加対象

- `modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock.rs` (削除直前のファイル) に inherent な `read()` / `write()` メソッドを追加。実装は `self.0.read()` / `self.0.write()` への薄い委譲。`SpinSyncMutex` の `lock()` と同じ pattern。
- `modules/utils/src/core/sync.rs` の `mod` / `pub use` 配線を整理し、`sync_mutex_like.rs` / `sync_rwlock_like.rs` 削除後も `SpinSyncMutex` / `SpinSyncRwLock` を公開し続ける
- ディレクトリのフラット化 (任意・後続): `sync_mutex_like/spin_sync_mutex.rs` → `sync/spin_sync_mutex.rs` などへの git mv は本 change の Phase 3 末尾で実施する。`git mv` は任意だが、公開を維持するための `mod` / `pub use` 再配線自体は必須

### 修正対象

#### `SyncQueueShared` monomorphize 対応 (型 alias の caller 修正)

- `modules/actor-core/src/core/kernel/dispatch/mailbox.rs`
  - `pub(crate) type UserQueueShared<T> = SyncFifoQueueShared<T, VecDequeBackend<T>, RuntimeMutex<SyncQueue<T, FifoKey, VecDequeBackend<T>>>>;`
  - → `pub(crate) type UserQueueShared<T> = SyncFifoQueueShared<T, VecDequeBackend<T>>;`
  - 第 3 型パラメータ削除のみ。construction site (`mailbox_queue_handles.rs:48-49`) は `RuntimeMutex::new(...)` → `ArcShared::new(...)` の組み立てなので無修正で動く (`RuntimeMutex` は `SpinSyncMutex` の alias であり、`ArcShared<SpinSyncMutex<...>>` を構築する)
- `modules/stream-core/src/core/impl/fusing/stream_buffer.rs` は既に 2 パラメータ形式 (`SyncFifoQueueShared<T, VecDequeBackend<T>>`) を使っているため無修正

#### caller 側 (`use SyncRwLockLike;` の整理)

- `modules/actor-core/src/core/kernel/system/state/system_state_shared.rs`
- `modules/actor-core/src/core/kernel/serialization/serialization_registry/registry.rs`
- `modules/actor-core/src/core/kernel/actor/actor_ref/dead_letter/dead_letter_shared.rs`
- `modules/actor-core/src/core/kernel/actor/scheduler/scheduler_shared.rs`
- `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/invoker_shared.rs`
- `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/middleware_shared.rs`
- `modules/actor-core/src/core/kernel/event/stream/event_stream_shared.rs`

これらの 7 ファイルは `use ...sync_rwlock_like::SyncRwLockLike;` を import して `RuntimeRwLock<T>` (= `SpinSyncRwLock<T>` alias) の `.read()` / `.write()` を呼んでいる。inherent method 化により import 行を削除するだけで動く。`.read()` / `.write()` 呼び出し側の修正は不要。

加えて、以下のテスト/補助コードも追従が必要:

- `modules/actor-core/src/core/kernel/system/state/system_state_shared/tests.rs` (`use ...SyncRwLockLike;` 削除)
- `modules/utils/src/core/sync/runtime_lock_alias/tests.rs` (`use ...SyncRwLockLike;` 削除、`cfg(not(feature = "std"))` ガード削除)
- `modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex/tests.rs` (trait 経由 assertion を inherent method ベースに更新、または移動後の新 path に追従)
- `modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock/tests.rs` (同上)

`SyncMutexLike` を import している `actor-core/src/core/typed/dsl/timer_scheduler.rs` の rustdoc 参照と `modules/utils/src/core/sync/shared_access.rs` の rustdoc 参照は `SpinSyncMutex` への置き換えに更新する。

#### clippy 設定

- `modules/utils/clippy.toml`
- `modules/actor-core/clippy.toml`
- `modules/cluster-core/clippy.toml`

これら 3 ファイルの `disallowed-types` で `std::sync::Mutex` の `replacement` target が `SyncMutexLike` 経由になっているのを `SpinSyncMutex` 直接参照に更新する。

#### 規約

- `.agents/rules/rust/immutability-policy.md` は既に `ArcShared<SpinSyncMutex<A>>` を指名しているので変更不要。本 change によって規約と実装の方向性が一致する。

### 触らない範囲 (non-goals)

- **`SyncQueueShared` および `SyncFifoQueueShared` の削除**: production で使用中なので保持する (本 change では `M` 型パラメータの monomorphize のみ)
- **`RuntimeMutex<T>` / `RuntimeRwLock<T>` / `NoStdMutex<T>` alias の削除**: これらは合計 173 caller があり、`SpinSyncMutex` の alias として機能的に意味がある (機能ゼロの cfg switch を介さない直接 alias になる)。リネームは pure cosmetic なので別 change で扱う。
- **`fraktor-utils-rs` の `*-core` / `*-adaptor-std` 分離**: 削除後 utils の std-side はゼロになるので分離しても adaptor crate に入るものがない。アーキテクチャ命名統一は別 change で扱う。
- **`modules/utils/src/core/` ディレクトリの flat 化**: 同じく cosmetic、別 change。
- **DebugMutex (再入検出) の導入**: 本 change の当初の動機だったが、scope out。本 change がベースを整えた後に独立 change として propose する。
- **`SharedAccess` trait**: `with_read` / `with_write` を提供する別の trait で、AShared パターンの主流 API。本 change は触らない。
- **`spin::Mutex` の挙動変更や `parking_lot` 等への切り替え**: 別問題。
- **`SyncQueue` (基底 generic queue 型) の削除や K 型パラメータの除去**: `SyncQueueShared` の K パラメータ (`FifoKey`/`MpscKey`/`SpscKey`/`PriorityKey`) は理論上 `FifoKey` のみで十分だが、K の構造そのものを撤去するのはスコープ膨張。`M` パラメータ monomorphize に絞る。

## Capabilities

### Modified Capabilities

- `utils-dead-code-removal`:
  - MODIFIED: `Requirement: 未使用の共有・同期補助型は公開 API に存在しない` (禁止リストに dead Sync*Shared sub-types / `Std{Sync,}Mutex` / `Std{Sync,}RwLock` / 各 Guard / `RuntimeMutexBackend` / `RuntimeRwLockBackend` / `SyncMutexLike` / `SyncRwLockLike` を追加)
  - 注: `SyncQueueShared` および `SyncFifoQueueShared` は production 利用ありのため禁止リストには含めない

新規 capability の追加はなし (既存 capability の禁止リストを拡張するだけ)。

## Impact

### 影響コード (utils 内部)

- `modules/utils/src/core/collections/queue/sync_mpsc_producer_shared.rs` (削除)
- `modules/utils/src/core/collections/queue/sync_mpsc_consumer_shared.rs` (削除)
- `modules/utils/src/core/collections/queue/sync_spsc_producer_shared.rs` (削除)
- `modules/utils/src/core/collections/queue/sync_spsc_consumer_shared.rs` (削除)
- `modules/utils/src/core/collections/queue/sync_queue_shared.rs` (impl ブロック削除 / alias 削除 / `M` パラメータ monomorphize)
- `modules/utils/src/core/collections/queue.rs` (`mod` + `pub use` 整理)
- `modules/utils/src/core/collections/queue/tests.rs` (Mpsc/Spsc/Priority 関連テスト削除)
- `modules/utils/src/std/` ディレクトリ全削除 (5+ ファイル)
- `modules/utils/src/std.rs` (削除)
- `modules/utils/src/lib.rs` (cfg switch + RuntimeMutexBackend 削除)
- `modules/utils/src/core/sync/runtime_lock_alias.rs` (alias を直接 SpinSync 系参照に書き換え)
- `modules/utils/src/core/sync/sync_mutex_like.rs` (削除)
- `modules/utils/src/core/sync/sync_rwlock_like.rs` (削除)
- `modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex.rs` (impl ブロック削除、ディレクトリ整理)
- `modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock.rs` (impl ブロック削除 + inherent method 追加)
- `modules/utils/src/core/sync.rs` または親 mod (mod 宣言の整理)
- `modules/utils/src/core/sync/shared_access.rs` (rustdoc 更新)
- `modules/utils/Cargo.toml` (`feature = "std"` 削除)

### 影響コード (caller 側)

- `modules/actor-core/src/core/kernel/dispatch/mailbox.rs` (`UserQueueShared` 型 alias から第 3 型パラメータ削除)
- `modules/actor-core/src/core/kernel/system/state/system_state_shared.rs` (`use SyncRwLockLike` 削除)
- `modules/actor-core/src/core/kernel/serialization/serialization_registry/registry.rs` (同上)
- `modules/actor-core/src/core/kernel/actor/actor_ref/dead_letter/dead_letter_shared.rs` (同上)
- `modules/actor-core/src/core/kernel/actor/scheduler/scheduler_shared.rs` (同上)
- `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/invoker_shared.rs` (同上)
- `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/middleware_shared.rs` (同上)
- `modules/actor-core/src/core/kernel/event/stream/event_stream_shared.rs` (同上)
- `modules/actor-core/src/core/typed/dsl/timer_scheduler.rs` (rustdoc 参照を `SpinSyncMutex` に更新)

### 影響コード (clippy / Cargo.toml)

- `modules/utils/clippy.toml` (`disallowed-types` の replacement 更新)
- `modules/actor-core/clippy.toml` (同上)
- `modules/cluster-core/clippy.toml` (同上)
- `modules/actor-adaptor-std/Cargo.toml` (`fraktor-utils-rs` の `features = ["std"]` 削除)
- `modules/cluster-adaptor-std/Cargo.toml` (同上)
- `modules/cluster-core/Cargo.toml` (同上)
- `modules/persistence-core/Cargo.toml` (`std = ["fraktor-utils-rs/std"]` feature 定義の整理)
- `modules/remote-adaptor-std/Cargo.toml` (同上)

### 影響 API (BREAKING)

- `SyncMpscQueueShared` / `SyncSpscQueueShared` / `SyncPriorityQueueShared` 型 alias 削除 (caller ゼロ)
- `SyncMpscProducerShared` / `SyncMpscConsumerShared` / `SyncSpscProducerShared` / `SyncSpscConsumerShared` 型削除 (caller ゼロ)
- `SyncQueueShared::new_mpsc` / `producer_clone` / `into_mpsc_pair` / `new_spsc` / `into_spsc_pair` / `peek_min` メソッド削除 (caller ゼロ)
- `SyncQueueShared` の第 4 型パラメータ `M` 削除 (caller: actor-core mailbox.rs 1 行のみ。`SyncFifoQueueShared` の第 3 型パラメータも同様)
- `StdSyncMutex` / `StdSyncRwLock` 一族の公開シンボル削除 (caller ゼロ)
- `StdMutex` alias 削除 (caller ゼロ)
- `RuntimeMutexBackend` / `RuntimeRwLockBackend` 内部 alias 削除
- `SyncMutexLike` / `SyncRwLockLike` trait 削除
- `RuntimeMutex` / `RuntimeRwLock` / `NoStdMutex` alias は **維持** (合計 173 caller、`SpinSync*` の直接 alias になる)
- `SpinSyncRwLock` に inherent `read()` / `write()` メソッドが追加される (新規 API、後方互換)
- `fraktor-utils-rs` の `feature = "std"` 削除 (BREAKING for downstream depending on it; ただし workspace 内では adapter 系 crate のみ)

### 触らない API (non-goals 再掲)

- `SyncQueueShared` / `SyncFifoQueueShared` (production 利用ありのため保持。ただし `M` パラメータは monomorphize)
- `RuntimeMutex<T>` / `RuntimeRwLock<T>` / `NoStdMutex<T>` 名前空間
- `SpinSyncMutex` の inherent API (既に `lock()` を持つ)
- `SharedAccess` trait
- `ArcShared` 全般

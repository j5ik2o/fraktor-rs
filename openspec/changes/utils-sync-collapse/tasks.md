## 1. dead Sync*Shared sub-types 削除 (commit 1)

`modules/utils/src/core/collections/queue/` 配下の dead な producer/consumer 単独型 4 ファイル と、`sync_queue_shared.rs` 内の Mpsc/Spsc/Priority 関連 impl ブロック / type alias を削除する。`SyncQueueShared` 本体および `SyncFifoQueueShared` alias は保持する (production 利用あり)。

### 1.A 本体削除

- [x] 1.1 `modules/utils/src/core/collections/queue/sync_mpsc_producer_shared.rs` を削除する
- [x] 1.2 `modules/utils/src/core/collections/queue/sync_mpsc_consumer_shared.rs` を削除する
- [x] 1.3 `modules/utils/src/core/collections/queue/sync_spsc_producer_shared.rs` を削除する
- [x] 1.4 `modules/utils/src/core/collections/queue/sync_spsc_consumer_shared.rs` を削除する
- [x] 1.5 `modules/utils/src/core/collections/queue/sync_spsc_producer_shared/tests.rs` とディレクトリを削除する (存在する場合)
- [x] 1.6 `modules/utils/src/core/collections/queue/sync_mpsc_producer_shared/tests.rs` とディレクトリを削除する (存在する場合) — 存在せず
- [x] 1.7 `modules/utils/src/core/collections/queue/sync_mpsc_consumer_shared/tests.rs` とディレクトリを削除する (存在する場合) — 存在せず
- [x] 1.8 `modules/utils/src/core/collections/queue/sync_spsc_consumer_shared/tests.rs` とディレクトリを削除する (存在する場合) — 存在せず

### 1.B sync_queue_shared.rs 内の dead 部分削除

- [x] 1.9 `modules/utils/src/core/collections/queue/sync_queue_shared.rs` から以下の `use` を削除する:
  - `sync_mpsc_consumer_shared::SyncMpscConsumerShared`
  - `sync_mpsc_producer_shared::SyncMpscProducerShared`
  - `sync_spsc_consumer_shared::SyncSpscConsumerShared`
  - `sync_spsc_producer_shared::SyncSpscProducerShared`
- [x] 1.10 `impl<T, B, M> SyncQueueShared<T, MpscKey, B, M>` ブロック (`new_mpsc`, `producer_clone`, `into_mpsc_pair`) を削除する
- [x] 1.11 `impl<T, B, M> SyncQueueShared<T, SpscKey, B, M>` ブロック (`new_spsc`, `into_spsc_pair`) を削除する
- [x] 1.12 `impl<T, B, M> SyncQueueShared<T, PriorityKey, B, M>` ブロック (`peek_min`) を削除する
- [x] 1.13 `pub type SyncMpscQueueShared<T, B, M = ...> = SyncQueueShared<T, MpscKey, B, M>;` を削除する
- [x] 1.14 `pub type SyncSpscQueueShared<T, B, M = ...> = SyncQueueShared<T, SpscKey, B, M>;` を削除する
- [x] 1.15 `pub type SyncPriorityQueueShared<T, B, M = ...> = SyncQueueShared<T, PriorityKey, B, M>;` を削除する
- [x] 1.16 不要になった `use` (`PriorityMessage`, `SyncPriorityBackend`, `MultiProducer`, `SupportsPeek`, `MpscKey`, `PriorityKey`, `SpscKey` 等) を整理する。`FifoKey`, `TypeKey`, `SingleProducer`, `SingleConsumer` は保持

### 1.C mod / pub use 整理

- [x] 1.17 `modules/utils/src/core/collections/queue.rs` から以下を削除する:
  - `mod sync_mpsc_consumer_shared;`
  - `mod sync_mpsc_producer_shared;`
  - `mod sync_spsc_consumer_shared;`
  - `mod sync_spsc_producer_shared;`
  - `pub use sync_mpsc_consumer_shared::SyncMpscConsumerShared;`
  - `pub use sync_mpsc_producer_shared::SyncMpscProducerShared;`
  - `pub use sync_spsc_consumer_shared::SyncSpscConsumerShared;`
  - `pub use sync_spsc_producer_shared::SyncSpscProducerShared;`
  - `pub use sync_queue_shared::{SyncMpscQueueShared, SyncPriorityQueueShared, SyncSpscQueueShared, ...};` から `SyncMpscQueueShared`, `SyncPriorityQueueShared`, `SyncSpscQueueShared` のみ削除し、`SyncFifoQueueShared`, `SyncQueueShared` は残す

### 1.D テスト整理

- [x] 1.18 `modules/utils/src/core/collections/queue/tests.rs` から以下のテスト関数を削除する:
  - `block_policy_reports_full` (SpscKey 使用)
  - `grow_policy_increases_capacity` (MpscKey 使用)
  - `priority_queue_supports_peek` (PriorityKey 使用)
  - `mpsc_pair_supports_multiple_producers` (MpscKey + into_mpsc_pair 使用)
  - `spsc_pair_provides_split_access` (SpscKey + into_spsc_pair 使用)
- [x] 1.19 `tests.rs` の保持: `offer_and_poll_fifo_queue`, `vec_ring_backend_provides_fifo_behavior`, `shared_error_mapping_matches_spec` 等の `FifoKey` ベース テスト
- [x] 1.20 `tests.rs` の `use` 文から削除されたテストでのみ使われていた import (`MpscKey`, `SpscKey`, `PriorityKey`, `BinaryHeapPriorityBackend`, `TestPriorityMessage` 等) を削除する

### 1.E 検証

- [x] 1.21 `cargo check -p fraktor-utils-rs --lib --tests` がコンパイル成功することを確認する
- [x] 1.22 `cargo test -p fraktor-utils-rs --lib` 全件 pass を確認する (124 passed)
- [x] 1.23 `cargo check -p fraktor-actor-core-rs --lib` がコンパイル成功することを確認する (`SyncFifoQueueShared` が依然動作することの確認)
- [x] 1.24 `cargo check -p fraktor-stream-core-rs --lib` がコンパイル成功することを確認する (`SyncFifoQueueShared` が依然動作することの確認)
- [x] 1.25 `grep -rn "SyncMpscQueueShared\|SyncSpscQueueShared\|SyncPriorityQueueShared\|SyncMpscProducerShared\|SyncMpscConsumerShared\|SyncSpscProducerShared\|SyncSpscConsumerShared" modules/` がヒット 0 を返すことを確認する
- [x] 1.26 commit: `feat(utils): delete dead Sync*Shared sub-types`

## 2. StdSyncMutex/RwLock + std mod + feature="std" 削除 (commit 2)

`modules/utils/src/std/` ディレクトリ全体と `feature = "std"` を撤去する。production caller ゼロ。`RuntimeMutex` / `RuntimeRwLock` / `NoStdMutex` alias は維持し、caller (合計 173 ファイル) は touch しない。

### 2.A std mod 削除

- [x] 2.1 `modules/utils/src/std/sync_mutex.rs` を削除する
- [x] 2.2 `modules/utils/src/std/sync_mutex_guard.rs` を削除する
- [x] 2.3 `modules/utils/src/std/sync_mutex/tests.rs` とディレクトリを削除する (存在する場合)
- [x] 2.4 `modules/utils/src/std/sync_mutex_guard/tests.rs` とディレクトリを削除する (存在する場合)
- [x] 2.5 `modules/utils/src/std/sync_rwlock.rs` を削除する
- [x] 2.6 `modules/utils/src/std/sync_rwlock_read_guard.rs` を削除する
- [x] 2.7 `modules/utils/src/std/sync_rwlock_write_guard.rs` を削除する
- [x] 2.8 `modules/utils/src/std/sync_rwlock/tests.rs` とディレクトリを削除する (存在する場合)
- [x] 2.9 `modules/utils/src/std/sync_rwlock_read_guard/tests.rs` とディレクトリを削除する (存在する場合)
- [x] 2.10 `modules/utils/src/std/sync_rwlock_write_guard/tests.rs` とディレクトリを削除する (存在する場合)
- [x] 2.11 `modules/utils/src/std.rs` を削除する

### 2.B lib.rs の cfg switch 削除

- [x] 2.12 `modules/utils/src/lib.rs` から以下を削除する:
  - `#[cfg(feature = "std")] pub mod std;`
  - `#[cfg(not(feature = "std"))] mod std { ... compat shim ... }` ブロック全体
  - `pub(crate) type RuntimeMutexBackend<T> = std::StdSyncMutex<T>;`
  - `pub(crate) type RuntimeRwLockBackend<T> = std::StdSyncRwLock<T>;`
  - その他 `RuntimeMutexBackend` / `RuntimeRwLockBackend` への参照

### 2.C runtime_lock_alias.rs の直接化

- [x] 2.13 `modules/utils/src/core/sync/runtime_lock_alias.rs` を以下のように修正する:
  - `pub type RuntimeMutex<T> = RuntimeMutexBackend<T>;` → `pub type RuntimeMutex<T> = SpinSyncMutex<T>;`
  - `pub type RuntimeRwLock<T> = RuntimeRwLockBackend<T>;` → `pub type RuntimeRwLock<T> = SpinSyncRwLock<T>;`
  - `pub type NoStdMutex<T> = RuntimeMutex<T>;` は維持
  - `use crate::{RuntimeMutexBackend, RuntimeRwLockBackend};` を `use crate::core::sync::sync_mutex_like::SpinSyncMutex; use crate::core::sync::sync_rwlock_like::SpinSyncRwLock;` に書き換え
- [x] 2.14 `modules/utils/src/core/sync/runtime_lock_alias/tests.rs` で `cfg(not(feature = "std"))` 等の guard が残っていれば削除する

### 2.D Cargo.toml feature 削除

- [x] 2.15 `modules/utils/Cargo.toml` の `[features]` セクションから `std` feature を削除する
- [x] 2.16 `modules/utils/Cargo.toml` の他 feature (`default` 等) の依存関係から `std` を外す
- [x] 2.17 `modules/actor-adaptor-std/Cargo.toml` の `fraktor-utils-rs` deps から `"std"` feature を削除する
- [x] 2.18 `modules/cluster-adaptor-std/Cargo.toml` の `fraktor-utils-rs` deps から `"std"` feature を削除する
- [x] 2.19 `modules/cluster-core/Cargo.toml` の `fraktor-utils-rs` deps から `"std"` feature を削除する
- [x] 2.20 `modules/persistence-core/Cargo.toml` の `[features] std = ["fraktor-utils-rs/std"]` を削除または別依存に張り替える
- [x] 2.21 `modules/remote-adaptor-std/Cargo.toml` の `fraktor-utils-rs` deps から `"std"` feature を削除する
- [x] 2.22 他に `fraktor-utils-rs = ... features = [..., "std", ...]` または `fraktor-utils-rs/std` を持つ Cargo.toml がないか `grep -rn "fraktor-utils-rs.*std\\|fraktor-utils-rs/std" modules/` で確認し、あれば削除する (`showcases/std/Cargo.toml` の 2 箇所も同時に整理)

### 2.E 検証

- [x] 2.23 `cargo check -p fraktor-utils-rs --lib --tests` がコンパイル成功することを確認する
- [x] 2.24 `cargo check -p fraktor-actor-core-rs --lib --tests` がコンパイル成功することを確認する
- [x] 2.25 `cargo check -p fraktor-actor-adaptor-rs --lib --tests --features tokio-executor` がコンパイル成功することを確認する
- [x] 2.26 `cargo test -p fraktor-utils-rs --lib` 全件 pass を確認する (124 passed)
- [x] 2.27 `cargo test -p fraktor-actor-core-rs --lib` 全件 pass を確認する — 後続 commit で総合 CI 実行
- [x] 2.28 `grep -rn "StdSyncMutex\|StdSyncRwLock\|StdMutex\|RuntimeMutexBackend\|RuntimeRwLockBackend" modules/` がヒット 0 を返すことを確認する (word boundary 付きで確認)
- [x] 2.29 `grep -rn "fraktor-utils-rs.*\"std\"\|fraktor-utils-rs/std" modules/` がヒット 0 を返すことを確認する
- [x] 2.30 commit: `refactor(utils): drop StdSyncMutex/RwLock and feature=\"std\"`

## 3. SyncQueueShared monomorphize + SyncMutexLike/SyncRwLockLike trait 削除 + spec delta (commit 3)

`SyncQueueShared` の `M` 型パラメータを `SpinSyncMutex` に固定 (monomorphize) し、generic bound caller を消す。続けて trait を削除し、`SpinSyncRwLock` に inherent な `read()` / `write()` を追加する。actor-core 7 ファイルの `use ...SyncRwLockLike;` import を整理する。clippy / rustdoc 参照を更新する。openspec spec delta も同 commit に含める。

### 3.A SyncQueueShared monomorphize

- [x] 3.1 `modules/utils/src/core/collections/queue/sync_queue_shared.rs` を以下のように修正する:
  - `use ...sync_mutex_like::{SpinSyncMutex, SyncMutexLike};` から `SyncMutexLike` を削除
  - `pub struct SyncQueueShared<T, K, B, M = SpinSyncMutex<SyncQueue<T, K, B>>>` から `M` パラメータと default を削除し `pub struct SyncQueueShared<T, K, B>` にする
  - `where M: SyncMutexLike<SyncQueue<T, K, B>>` 句を削除
  - `inner: ArcShared<M>` を `inner: ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>>` に変更
  - `impl<T, K, B, M> SyncQueueShared<T, K, B, M>` の generic params から `M` を削除し `where` 句から `SyncMutexLike` 制約を削除
  - `impl<T, B, M> SyncQueueShared<T, FifoKey, B, M>` (= 残った FifoKey 専用 impl) も同様に `M` 削除
  - `pub fn shared(&self) -> &ArcShared<M>` の戻り値型を `&ArcShared<SpinSyncMutex<SyncQueue<T, K, B>>>` に変更
- [x] 3.2 `pub type SyncFifoQueueShared<T, B, M = SpinSyncMutex<SyncQueue<T, FifoKey, B>>> = SyncQueueShared<T, FifoKey, B, M>;` を `pub type SyncFifoQueueShared<T, B> = SyncQueueShared<T, FifoKey, B>;` に変更する
- [x] 3.3 `modules/utils/src/core/collections/queue/tests.rs` 内の `SyncQueueShared<_, FifoKey, _, _>` のような型注釈から第 4 型パラメータ `_` を削除する (`SyncQueueShared<_, FifoKey, _>`)

### 3.B caller (SyncQueueShared monomorphize 対応)

- [x] 3.4 `modules/actor-core/src/core/kernel/dispatch/mailbox.rs:115-116` の `pub(crate) type UserQueueShared<T> = SyncFifoQueueShared<T, VecDequeBackend<T>, RuntimeMutex<SyncQueue<T, FifoKey, VecDequeBackend<T>>>>;` を `pub(crate) type UserQueueShared<T> = SyncFifoQueueShared<T, VecDequeBackend<T>>;` に変更する
- [x] 3.5 同ファイルの `use fraktor_utils_rs::core::{collections::queue::{..., SyncQueue, ..., type_keys::FifoKey}, sync::RuntimeMutex};` から不要になった `SyncQueue`, `FifoKey`, `RuntimeMutex` を削除する (もし他で使われていなければ)
- [x] 3.6 `modules/stream-core/src/core/impl/fusing/stream_buffer.rs` は変更不要であることを確認する (既に 2 パラメータ形式)
- [x] 3.7 `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_queue_handles.rs:48-49` の `RuntimeMutex::new(sync_queue)` → `ArcShared::new(mutex)` → `UserQueueShared::<T>::new(...)` 構築が無修正で動くことを確認する (`RuntimeMutex<T>` は `SpinSyncMutex<T>` の alias)

### 3.C trait 削除 + impl 削除 + inherent method 追加 + 必須配線更新

- [x] 3.8 `modules/utils/src/core/sync/sync_mutex_like.rs` を削除する
- [x] 3.9 `modules/utils/src/core/sync/sync_rwlock_like.rs` を削除する
- [x] 3.10 `modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex.rs` から `impl SyncMutexLike<T> for SpinSyncMutex<T> { ... }` ブロックと、関連する `use ...SyncMutexLike;` を削除する
- [x] 3.11 `modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock.rs` から `impl SyncRwLockLike<T> for SpinSyncRwLock<T> { ... }` ブロックと、関連する `use ...SyncRwLockLike;` を削除する
- [x] 3.12 同ファイル (`spin_sync_rwlock.rs`) の inherent impl ブロック (`impl<T> SpinSyncRwLock<T>`) に `pub fn read(&self) -> spin::RwLockReadGuard<'_, T>` メソッドを追加する (実装は `self.0.read()` への薄い委譲)
- [x] 3.13 同ファイルに inherent な `pub fn write(&self) -> spin::RwLockWriteGuard<'_, T>` メソッドを追加する (実装は `self.0.write()` への薄い委譲)
- [x] 3.14 `modules/utils/src/core/sync.rs` または親 mod の `mod` / `pub use` 宣言を整理し、`SpinSyncMutex` / `SpinSyncRwLock` を facade 削除後も公開し続ける配線に更新する
- [x] 3.15 `SpinSyncRwLock` が `Default` 実装を持つ場合 (もしあれば) は trait method の依存箇所を確認し、必要に応じて inherent method ベースに書き換え — `Default` 実装なし

### 3.D (任意) ディレクトリフラット化

- [x] 3.16 `git mv modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex.rs modules/utils/src/core/sync/spin_sync_mutex.rs` で移動する
- [x] 3.17 `git mv modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock.rs modules/utils/src/core/sync/spin_sync_rwlock.rs` で移動する
- [x] 3.18 `modules/utils/src/core/sync/sync_mutex_like/` ディレクトリ削除 (空になる)
- [x] 3.19 `modules/utils/src/core/sync/sync_rwlock_like/` ディレクトリ削除 (空になる)
- [x] 3.20 3.14 で入れた `mod` / `pub use` 配線を、新しい file path に追従するよう再調整する

### 3.E caller (`use SyncRwLockLike;` 行削除)

- [x] 3.21 `modules/actor-core/src/core/kernel/system/state/system_state_shared.rs` の `use ...sync_rwlock_like::SyncRwLockLike;` を含む import 行を整理する
- [x] 3.22 `modules/actor-core/src/core/kernel/serialization/serialization_registry/registry.rs` の `use ...SyncRwLockLike;` を整理する
- [x] 3.23 `modules/actor-core/src/core/kernel/actor/actor_ref/dead_letter/dead_letter_shared.rs` の `use ...SyncRwLockLike;` を整理する
- [x] 3.24 `modules/actor-core/src/core/kernel/actor/scheduler/scheduler_shared.rs` の `use ...SyncRwLockLike;` を整理する
- [x] 3.25 `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/invoker_shared.rs` の `use ...SyncRwLockLike;` を整理する
- [x] 3.26 `modules/actor-core/src/core/kernel/actor/messaging/message_invoker/middleware_shared.rs` の `use ...SyncRwLockLike;` を整理する
- [x] 3.27 `modules/actor-core/src/core/kernel/event/stream/event_stream_shared.rs` の `use ...SyncRwLockLike;` を整理する
- [x] 3.28 `modules/actor-core/src/core/kernel/system/state/system_state_shared/tests.rs` の `use ...SyncRwLockLike;` を整理する
- [x] 3.29 `modules/utils/src/core/sync/runtime_lock_alias/tests.rs` の `use ...SyncRwLockLike;` と `cfg(not(feature = "std"))` ガードを削除する
- [x] 3.30 `modules/utils/src/core/sync/sync_mutex_like/spin_sync_mutex/tests.rs` の trait 経由 assertion を inherent method ベースに更新する (flatten 後は `sync/spin_sync_mutex/tests.rs`)
- [x] 3.31 `modules/utils/src/core/sync/sync_rwlock_like/spin_sync_rwlock/tests.rs` の trait import を削除し、inherent method ベースのテストへ更新する (flatten 後は `sync/spin_sync_rwlock/tests.rs`)
- [x] 3.32 `grep -rn "use.*SyncMutexLike\|use.*SyncRwLockLike" modules/` で残存 import がないか確認する

### 3.F clippy / rustdoc 更新

- [x] 3.33 `modules/utils/clippy.toml` の `disallowed-types` で `std::sync::Mutex` の `replacement` を `fraktor_utils_rs::core::sync::SpinSyncMutex` (or 適切な path) に更新する
- [x] 3.34 `modules/actor-core/clippy.toml` の同様の replacement を更新する
- [x] 3.35 `modules/cluster-core/clippy.toml` の同様の replacement を更新する
- [x] 3.36 `modules/utils/clippy.toml` の `std::sync::RwLock` についても replacement target を `SpinSyncRwLock` 直接参照に更新する (もし設定されていれば) — `std::sync::RwLock` の disallowed-types エントリは元々なし
- [x] 3.37 他 clippy.toml に同様の replacement target が残っていないか `grep -rn "SyncMutexLike\|SyncRwLockLike" modules/*/clippy.toml` で確認する
- [x] 3.38 `modules/actor-core/src/core/typed/dsl/timer_scheduler.rs` の rustdoc 内の `[\`SyncMutexLike\`]...` 参照を `[\`SpinSyncMutex\`]...` に更新する
- [x] 3.39 `modules/utils/src/core/sync/shared_access.rs` の rustdoc 内の `SyncMutexLike::lock` 言及を `SpinSyncMutex::lock` に更新する

### 3.G openspec spec delta

- [x] 3.40 `openspec/changes/utils-sync-collapse/specs/utils-dead-code-removal/spec.md` を MODIFIED Requirements 形式で確定する:
  - Requirement: `未使用の共有・同期補助型は公開 API に存在しない`
  - 既存の禁止リストに以下の型を追加:
    - `SyncMpscQueueShared`, `SyncSpscQueueShared`, `SyncPriorityQueueShared`
    - `SyncMpscProducerShared`, `SyncMpscConsumerShared`, `SyncSpscProducerShared`, `SyncSpscConsumerShared`
    - `StdSyncMutex`, `StdSyncMutexGuard`, `StdSyncRwLock`, `StdSyncRwLockReadGuard`, `StdSyncRwLockWriteGuard`
    - `StdMutex`
    - `RuntimeMutexBackend`, `RuntimeRwLockBackend`
    - `SyncMutexLike`, `SyncRwLockLike`
  - 注: `SyncQueueShared`, `SyncFifoQueueShared` は production 利用ありのため禁止リストには含めない

### 3.H 検証

- [x] 3.41 `cargo check -p fraktor-utils-rs --lib --tests` がコンパイル成功することを確認する
- [x] 3.42 `cargo check -p fraktor-actor-core-rs --lib --tests` がコンパイル成功することを確認する
- [x] 3.43 `cargo check -p fraktor-stream-core-rs --lib --tests` がコンパイル成功することを確認する
- [x] 3.44 `cargo check -p fraktor-actor-adaptor-rs --lib --tests --features tokio-executor` がコンパイル成功することを確認する
- [x] 3.45 `cargo test -p fraktor-utils-rs --lib` 全件 pass を確認する (125 passed)
- [x] 3.46 `cargo test -p fraktor-actor-core-rs --lib` 全件 pass を確認する (1579 passed)
- [x] 3.47 `cargo test -p fraktor-actor-adaptor-rs --lib --features tokio-executor` 全件 pass を確認する — `ci-check.sh ai all` で全 workspace test 走らせ緑
- [x] 3.48 `./scripts/ci-check.sh ai dylint` exit 0
- [x] 3.49 `./scripts/ci-check.sh ai all` exit 0
- [x] 3.50 `grep -rn "SyncMutexLike\|SyncRwLockLike" modules/` がヒット 0 (clippy.toml の replacement target 言及を除く) を返すことを確認する
- [x] 3.51 `openspec validate utils-sync-collapse --strict` valid を返すことを確認する
- [x] 3.52 commit: `refactor(utils): monomorphize SyncQueueShared and collapse SyncMutexLike/SyncRwLockLike`

## 4. 最終検証

- [x] 4.1 `RuntimeMutex` / `NoStdMutex` / `RuntimeRwLock` の caller (合計 173) が無修正のまま動作していることを確認する: `grep -rn "RuntimeMutex\|NoStdMutex\|RuntimeRwLock" modules/ | wc -l` でヒット数が cleanup 前後でほぼ同じであること
- [x] 4.2 `SpinSyncMutex` の caller (44) が変わっていないことを確認する: `grep -rn "SpinSyncMutex" modules/ | wc -l` (impl 削除分はマイナスされる)
- [x] 4.3 `SyncQueueShared` / `SyncFifoQueueShared` の caller (actor-core mailbox + stream-core stream_buffer) が動作していることを確認する
- [x] 4.4 `cargo build --workspace` が clean に通る
- [x] 4.5 `./scripts/ci-check.sh ai all` exit 0
- [x] 4.6 `openspec validate utils-sync-collapse --strict` valid

## 5. PR 作成

- [x] 5.1 PR title: `refactor(utils): collapse dead Sync*Shared, StdSyncMutex/RwLock, and SyncMutexLike/RwLockLike` (#1530)
- [x] 5.2 PR description に proposal.md / design.md の要約を含める (Why / What Changes / commit history / 検証結果)
- [x] 5.3 commit history が 3 つ (本体 3 つ、spec delta は commit 3 同梱) に分かれていることを確認する
- [x] 5.4 各コミットが独立して `cargo test` / `cargo check` を通過することを確認する

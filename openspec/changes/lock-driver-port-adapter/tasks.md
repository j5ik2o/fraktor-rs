## 1. Phase 1: utils-core port contract introduction

`fraktor-utils-core-rs` に `LockDriver` / `RwLockDriver` trait と factory を新設し、`RuntimeMutex` / `RuntimeRwLock` を type alias から struct に格上げする。**デフォルト型引数を持たない** 形で実装し、port 定義ファイルは adapter 実装 (`SpinSyncMutex` 等) を一切参照しない。utils-core 自身の内部 caller も explicit `RuntimeMutex<T, SpinSyncMutex<T>>` に更新する。

### 1.A LockDriver trait + struct 化 (port 定義 — adapter を参照しない)

- [ ] 1.1 `modules/utils-core/src/core/sync/lock_driver.rs` を新設し、`LockDriver<T>` trait を定義する (`type Guard<'a>`, `new`, `lock`, `into_inner`)。このファイルは `SpinSyncMutex` を一切 use / 参照しない
- [ ] 1.2 `modules/utils-core/src/core/sync/runtime_mutex.rs` を新設し、`RuntimeMutex<T, D>` を concrete struct として実装する (`new`, `lock`, `into_inner` 等)。**デフォルト型引数を持たない**。このファイルは `SpinSyncMutex` を一切 use / 参照しない。`where D: LockDriver<T>` のみを境界として持つ
- [ ] 1.3 `modules/utils-core/src/core/sync/runtime_lock_alias.rs` の `pub type RuntimeMutex<T> = ...` を削除する

### 1.B RwLockDriver trait + struct 化 (port 定義 — adapter を参照しない)

- [ ] 1.4 `modules/utils-core/src/core/sync/rwlock_driver.rs` を新設し、`RwLockDriver<T>` trait を定義する (`type ReadGuard<'a>`, `type WriteGuard<'a>`, `new`, `read`, `write`, `into_inner`)。`SpinSyncRwLock` を参照しない
- [ ] 1.5 `modules/utils-core/src/core/sync/runtime_rwlock.rs` を新設し、`RuntimeRwLock<T, D>` を concrete struct として実装する (**デフォルト型引数なし**)。`SpinSyncRwLock` を参照しない
- [ ] 1.6 `modules/utils-core/src/core/sync/runtime_lock_alias.rs` の `pub type RuntimeRwLock<T> = ...` を削除する

### 1.C LockDriverFactory / RwLockDriverFactory (port 定義)

- [ ] 1.7 `modules/utils-core/src/core/sync/lock_driver_factory.rs` を新設し、`LockDriverFactory` / `RwLockDriverFactory` trait を定義する (associated `type Driver<T>: LockDriver<T>` / `type Driver<T>: RwLockDriver<T>`)。具象 driver を参照しない

### 1.D Built-in driver 側に trait impl を置く (port とファイル分離)

- [ ] 1.8 `modules/utils-core/src/core/sync/spin_sync_mutex.rs` に `impl<T> LockDriver<T> for SpinSyncMutex<T>` を追加 (inherent method への薄い委譲)
- [ ] 1.9 `modules/utils-core/src/core/sync/spin_sync_rwlock.rs` に `impl<T> RwLockDriver<T> for SpinSyncRwLock<T>` を追加 (inherent method への薄い委譲)
- [ ] 1.10 `modules/utils-core/src/core/sync/spin_sync_factory.rs` を新設し、`SpinSyncFactory` (unit struct) と `impl LockDriverFactory { type Driver<T> = SpinSyncMutex<T>; }` を実装する
- [ ] 1.11 `modules/utils-core/src/core/sync/spin_sync_rwlock_factory.rs` を新設し、`SpinSyncRwLockFactory` (unit struct) と `impl RwLockDriverFactory { type Driver<T> = SpinSyncRwLock<T>; }` を実装する

### 1.E sync 配線更新

- [ ] 1.12 `modules/utils-core/src/core/sync.rs` の `mod` / `pub use` 配線を更新する (port trait / port struct / built-in driver / built-in factory を expose、旧 `runtime_lock_alias` の `RuntimeMutex`/`RuntimeRwLock` export を削除)
- [ ] 1.13 必要なら `NoStdMutex<T>` alias を `pub type NoStdMutex<T> = RuntimeMutex<T, SpinSyncMutex<T>>;` として utils-core 内に **built-in driver 側のファイル** (例: `spin_sync_factory.rs` と同レベル) に残す。port 定義ファイルには置かない

### 1.F utils-core 自身の caller 更新

- [ ] 1.14 `modules/utils-core/src/core/collections/wait/node_shared.rs` を含む utils-core 内部の `RuntimeMutex<T>` / `RuntimeRwLock<T>` 使用箇所を列挙する (`grep -rn "RuntimeMutex\|RuntimeRwLock" modules/utils-core/src/`)
- [ ] 1.15 列挙箇所を `RuntimeMutex<T, SpinSyncMutex<T>>` / `RuntimeRwLock<T, SpinSyncRwLock<T>>` に explicit に書き換える

### 1.G Phase 1 検証

- [ ] 1.16 `cargo check -p fraktor-utils-core-rs --lib --tests` clean
- [ ] 1.17 `cargo test -p fraktor-utils-core-rs --lib` 全件 pass
- [ ] 1.18 `grep -rn "SpinSyncMutex\|SpinSyncRwLock" modules/utils-core/src/core/sync/lock_driver.rs modules/utils-core/src/core/sync/rwlock_driver.rs modules/utils-core/src/core/sync/runtime_mutex.rs modules/utils-core/src/core/sync/runtime_rwlock.rs modules/utils-core/src/core/sync/lock_driver_factory.rs` が空であることを確認する (port 定義が adapter を参照しないことの機械的検証)
- [ ] 1.19 commit Phase 1: `feat(utils-core): introduce LockDriver port contract and struct-based RuntimeMutex/RuntimeRwLock (no default driver)`

## 2. Phase 2: utils-adaptor-std debug driver impls

`fraktor-utils-adaptor-std-rs` (PR #1538 で skeleton 化済み) に `DebugSpinSyncMutex` / `DebugSpinSyncRwLock` を再導入し、`LockDriver` / `RwLockDriver` impl を追加する。`feature = "test-support"` で gate する。

### 2.A debug mutex 復活 + LockDriver impl

- [ ] 2.1 `modules/utils-adaptor-std/Cargo.toml` の `[features]` に `test-support = ["dep:spin"]` を再追加
- [ ] 2.2 `modules/utils-adaptor-std/Cargo.toml` の `[dependencies]` に optional `spin` を再追加、`[dev-dependencies]` の `spin` も復活
- [ ] 2.3 `modules/utils-adaptor-std/src/lib.rs` の placeholder を更新し、`#[cfg(any(test, feature = "test-support"))] pub mod std;` を再追加
- [ ] 2.4 `modules/utils-adaptor-std/src/std.rs` を新設、`pub mod debug;` を宣言
- [ ] 2.5 `modules/utils-adaptor-std/src/std/debug.rs` を新設、`mod debug_spin_sync_mutex; mod debug_spin_sync_mutex_guard;` および `pub use` を宣言
- [ ] 2.6 `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_mutex.rs` を復活させる (PR #1538 で削除した実装をベースに、`AtomicU64` owner tracking を維持)
- [ ] 2.7 `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_mutex_guard.rs` を復活させる (`Drop` で owner クリア)
- [ ] 2.8 `DebugSpinSyncMutex` に `impl<T> LockDriver<T> for DebugSpinSyncMutex<T>` を追加 (inherent method への委譲)

### 2.B debug rwlock 新設

- [ ] 2.9 `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_rwlock.rs` を新設し、`DebugSpinSyncRwLock<T>` を実装する (read 同時は許容、write 中の re-entry を panic)
- [ ] 2.10 `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_rwlock_read_guard.rs` / `..._write_guard.rs` を新設
- [ ] 2.11 `DebugSpinSyncRwLock` に `impl<T> RwLockDriver<T> for DebugSpinSyncRwLock<T>` を追加

### 2.C debug factory

- [ ] 2.12 `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_factory.rs` を新設し、`DebugSpinSyncFactory` (unit struct) を実装する (`impl LockDriverFactory { type Driver<T> = DebugSpinSyncMutex<T>; }`)
- [ ] 2.13 `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_rwlock_factory.rs` を新設し、`DebugSpinSyncRwLockFactory` を実装する (`impl RwLockDriverFactory { type Driver<T> = DebugSpinSyncRwLock<T>; }`)
- [ ] 2.14 `modules/utils-adaptor-std/src/std/debug.rs` の `pub use` を更新

### 2.D Phase 2 tests

- [ ] 2.15 `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_mutex/tests.rs` を復活させる (basic / re-entry panic / contention / sequential 等)
- [ ] 2.16 `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_rwlock/tests.rs` を新設し、read 同時 / write 排他 / write 中再入 panic / 通常 contention 等を verify
- [ ] 2.17 `cargo test -p fraktor-utils-adaptor-std-rs --features test-support --lib` 全件 pass

### 2.E Phase 2 検証

- [ ] 2.18 `cargo check --workspace --all-targets` は **Phase 1 完了時点では caller 未更新のため通らない可能性がある**。`cargo check -p fraktor-utils-core-rs -p fraktor-utils-adaptor-std-rs --all-targets` clean を確認する
- [ ] 2.19 `./scripts/ci-check.sh ai dylint` exit 0 (`pub mod std;` の cfg gate 等の lint 対応含む)
- [ ] 2.20 commit Phase 2: `feat(utils-adaptor-std): re-introduce DebugSpinSyncMutex/RwLock with LockDriver impl`

## 3. Phase 3: actor-core/kernel factory ジェネリック化

actor-core/kernel の shared 型を `<F: LockDriverFactory>` (および `<R: RwLockDriverFactory>`) ジェネリックにする (**デフォルトなし**)。`ActorSystem` / `ActorContext` 等の caller boundary で production は `SpinSyncFactory` を型引数として固定する。

### 3.A 対象 inventory + 設計確認

- [ ] 3.1 `modules/actor-core/src/core/kernel/` 配下で `RuntimeMutex` / `RuntimeRwLock` を field 型として持つ shared 型を全列挙する (`grep -rn "RuntimeMutex\|RuntimeRwLock" modules/actor-core/src/core/kernel/`)
- [ ] 3.2 列挙結果から、ジェネリック化対象の primary types を確定する (Mailbox, ActorCell, MessageInvokerShared, MiddlewareShared, EventStreamShared, DeadLetterShared, SchedulerShared, SystemStateShared, SerializationRegistry, etc.)
- [ ] 3.3 各型がどの factory parameter (Mutex factory `F` / RwLock factory `R`) を取るかを設計し、design.md に追記する
- [ ] 3.4 caller boundary (`ActorSystem::new`, `Props::new`, `ActorContext::new` 等) をリストアップし、`SpinSyncFactory` を固定する箇所を特定する

### 3.B Mailbox + lock 系のジェネリック化 (代表)

- [ ] 3.5 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` の `Mailbox` に `<F: LockDriverFactory>` パラメータ追加 (default なし)
- [ ] 3.6 各 field を `ArcShared<RuntimeMutex<T, F::Driver<T>>>` に書き換え
- [ ] 3.7 `Mailbox::new()` などの factory function を generic に書き換え
- [ ] 3.8 `Mailbox` を field とする上位型 (UserQueueShared, etc.) も同様に generic 化

### 3.C ActorCell / MessageInvoker / Middleware / EventStream / DeadLetter / Scheduler / SystemState / SerializationRegistry のジェネリック化

- [ ] 3.9 `ActorCell` を `<F, R>` で generic 化
- [ ] 3.10 `MessageInvokerShared` を `<R>` で generic 化 (rwlock のみ)
- [ ] 3.11 `MiddlewareShared` 同様
- [ ] 3.12 `EventStreamShared` 同様
- [ ] 3.13 `DeadLetterShared` 同様
- [ ] 3.14 `SchedulerShared` 同様
- [ ] 3.15 `SystemStateShared` 同様
- [ ] 3.16 `SerializationRegistry` 同様
- [ ] 3.17 その他列挙された残り型を順次対応

### 3.D caller boundary で SpinSyncFactory を固定

- [ ] 3.18 `ActorSystem::new` 等の public API に `<F: LockDriverFactory, R: RwLockDriverFactory>` を **明示的に** 取るか、`SpinSyncFactory` を型引数として固定するかを決定する (design.md に書く)
- [ ] 3.19 production path の caller (actor-core test, actor-adaptor-std, cluster-adaptor-std, showcases/std など) が `SpinSyncFactory` / `SpinSyncRwLockFactory` を明示する形に更新する
- [ ] 3.20 test path (actor-core 内部 unit test) は `SpinSyncFactory` で動作することを確認する

### 3.E Phase 3 検証

- [ ] 3.21 `cargo check -p fraktor-actor-core-rs --lib --tests` clean
- [ ] 3.22 `cargo test -p fraktor-actor-core-rs --lib` 全件 pass (behavioral 変化なし)
- [ ] 3.23 commit Phase 3: `refactor(actor-core): generify kernel shared types over LockDriverFactory (no default)`

## 4. Phase 4: 他 crate (cluster / persistence / stream / actor-adaptor-std) の per-crate alias 導入

本 change では cluster-core / persistence-core / stream-core / actor-adaptor-std は factory ジェネリック化せず、**per-crate alias** で `RuntimeMutex<T, SpinSyncMutex<T>>` を導入して互換維持する。将来必要になったら別 change で factory 化する。

### 4.A per-crate alias 導入

- [ ] 4.1 `modules/cluster-core/src/core/sync.rs` (or 適切な既存ファイル) に `pub type KernelMutex<T> = fraktor_utils_core_rs::RuntimeMutex<T, fraktor_utils_core_rs::SpinSyncMutex<T>>;` と `pub type KernelRwLock<T> = fraktor_utils_core_rs::RuntimeRwLock<T, fraktor_utils_core_rs::SpinSyncRwLock<T>>;` を追加する
- [ ] 4.2 `modules/persistence-core/src/core/sync.rs` 同様
- [ ] 4.3 `modules/stream-core/src/core/sync.rs` 同様
- [ ] 4.4 `modules/actor-adaptor-std/src/std/sync.rs` (or 適切なファイル) 同様
- [ ] 4.5 各 crate の `core.rs` / `std.rs` 等に `pub mod sync;` を配線

### 4.B field 型の bulk 置換

- [ ] 4.6 cluster-core の `RuntimeMutex<T>` / `RuntimeRwLock<T>` field 型を `KernelMutex<T>` / `KernelRwLock<T>` に bulk 置換する (ripgrep + sed / ast-grep で mechanical に実施)
- [ ] 4.7 persistence-core 同様
- [ ] 4.8 stream-core 同様
- [ ] 4.9 actor-adaptor-std 同様
- [ ] 4.10 各 crate から actor-core の generic 化された型を使う箇所 (`Mailbox`, `ActorCell` 等) では `Mailbox<SpinSyncFactory>` 等を明示する

### 4.C Phase 4 検証

- [ ] 4.11 `cargo check --workspace --all-targets` clean
- [ ] 4.12 `cargo test --workspace --lib` 全件 pass (behavioral 変化なし)
- [ ] 4.13 commit Phase 4: `refactor(cluster/persistence/stream/actor-adaptor-std): introduce per-crate KernelMutex alias over RuntimeMutex<T, SpinSyncMutex<T>>`

## 5. Phase 5: actor-core test instrumentation example

actor-core test target で `DebugSpinSyncFactory` を差し込み、deadlock 検知が機能することを `#[should_panic]` で verify する。

### 5.A actor-core 側 dev-dep 復活

- [ ] 5.1 `modules/actor-core/Cargo.toml` の `[dev-dependencies]` に `fraktor-utils-adaptor-std-rs = { workspace = true, features = ["test-support"] }` を追加 (PR #1538 で削除したものを復活)

### 5.B 検証 test 追加

- [ ] 5.2 `modules/actor-core/tests/deadlock_detection_example.rs` を新設し、以下をカバー:
  - [ ] `ActorSystem<DebugSpinSyncFactory, DebugSpinSyncRwLockFactory>` (or 同等の test harness) を構築する helper を定義
  - [ ] 通常の lock/unlock パターンが正常動作することを verify
  - [ ] 意図的に同一スレッドから再入する actor を用意し、`DebugSpinSyncFactory` 経由で panic することを `#[should_panic]` で verify
  - [ ] 別スレッドからの contention は panic しないことを verify

### 5.C Phase 5 検証

- [ ] 5.3 `cargo test -p fraktor-actor-core-rs --test deadlock_detection_example` 全件 pass
- [ ] 5.4 commit Phase 5: `test(actor-core): add deadlock detection example using DebugSpinSyncFactory`

## 6. Phase 6: 検証 + spec delta + ドキュメント更新

- [ ] 6.1 `modules/utils-core/src/core/sync/spin_sync_mutex.rs` の rustdoc を更新する (`# Deadlock` セクションで `DebugSpinSyncFactory` 経由の検出方法を案内)
- [ ] 6.2 `modules/utils-core/src/core/sync/spin_sync_rwlock.rs` に同様の rustdoc 追加
- [ ] 6.3 `.agents/rules/rust/immutability-policy.md` の AShared パターン記述に LockDriver の言及を追加 (任意)
- [ ] 6.4 `openspec validate lock-driver-port-adapter --strict` valid を確認する
- [ ] 6.5 `cargo check --workspace --all-targets` clean
- [ ] 6.6 `./scripts/ci-check.sh ai all` exit 0
- [ ] 6.7 commit Phase 6: `docs(openspec): finalize lock-driver-port-adapter spec delta and rustdoc`

## 7. PR 作成

- [ ] 7.1 PR title: `refactor(utils-core, actor-core): introduce LockDriver port + factory pattern (no default), enable test-time deadlock detection`
- [ ] 7.2 PR description に proposal.md / design.md の要約を含める (Why / What Changes / commit history / 検証結果)
- [ ] 7.3 commit history が phase ごとに分かれていることを確認する (Phase 1 / 2 / 3 / 4 / 5 / 6)
- [ ] 7.4 各 commit が独立して `cargo check -p <対象 crate>` を通過することを確認する (Phase 3 以降は workspace 全体)
- [ ] 7.5 PR description で「**default type parameter を採用しない** hexagonal 純度の設計判断」と「115 ファイルの caller 書き換え」を明示する

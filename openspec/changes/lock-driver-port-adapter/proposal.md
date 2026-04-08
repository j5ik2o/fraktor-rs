## Why

`fraktor-utils-core-rs` の `RuntimeMutex<T>` / `RuntimeRwLock<T>` は現在 `SpinSyncMutex<T>` / `SpinSyncRwLock<T>` への **type alias** にすぎず、port 契約 (trait) ではない。`actor-core/kernel` を含む **115 ファイル (utils-core 自身を除く)** がこの alias を「型」として参照しており、`self.user_queue_lock.lock()` のような呼び出しは concrete `SpinSyncMutex::lock()` に直リンクする。**ロック実装を test 時に差し替える接合点が存在しない**。

この設計負債は PR #1535 / #1537 で `DebugSpinSyncMutex` (再入検知) を追加したときに顕在化した: せっかく debug helper を作っても、actor-core/kernel の concrete `SpinSyncMutex::lock()` 呼び出しを差し替えられず、deadlock 検証手段が実用にならない。PR #1538 で `DebugSpinSyncMutex` 実装は **port 契約整備後に再導入する** 方針として一旦 OFF した (`utils-adaptor-std` crate skeleton のみ残存)。

本 change は以下を実現する:

1. **`LockDriver<T>` trait** (port 契約) を `utils-core` に新設
2. **`RuntimeMutex<T, D>`** を type alias から **concrete struct** に格上げ (driver `D` を内部に保持、**デフォルト型引数なし**)
3. **`LockDriverFactory` trait** で多 T フィールド (Mailbox 等) のジェネリック化を可能にする
4. **`utils-adaptor-std` から adapter driver** (`DebugSpinSyncMutex` 等) を差し込める形にする
5. **actor-core/kernel の shared 型を `<F: LockDriverFactory>` ジェネリック化**し、test 時に `Mailbox<DebugSpinSyncFactory>` のような instrumentation を可能にする

`RwLock` 側も同型 (`RwLockDriver`, `RuntimeRwLock<T, D>`, `RwLockDriverFactory`) で扱う。

### Port / Adapter 純度の原則

**`utils-core` (port 側) は adapter 実装を知らない**。これは hexagonal architecture の基本原則であり、本 change の核となる制約である:

- `pub struct RuntimeMutex<T, D = SpinSyncMutex<T>>` のような **default type parameter で adapter 実装を名指しする書き方は禁止**
- `utils-core` の port 定義ファイル (`runtime_mutex.rs` 等) は、`LockDriver<T>` trait 以外の具象 driver 型を参照してはならない
- `SpinSyncMutex<T>` の配置は本 change では **`utils-core` 内に据え置く** (no_std default built-in driver として) が、**port struct `RuntimeMutex<T, D>` の定義は `SpinSyncMutex` を一切参照しない**

driver 型を supply する責務は caller 側 (actor-core, cluster-core 等) にある。caller は以下のいずれかで D を決定する:

- **(a) Explicit**: 各フィールドで `RuntimeMutex<T, SpinSyncMutex<T>>` と書き下す
- **(b) Per-crate alias**: 各 caller crate が `pub type KernelMutex<T> = RuntimeMutex<T, SpinSyncMutex<T>>;` のような alias を **caller crate 側** に置き、field 型は alias 経由で書く
- **(c) Factory genericization**: caller shared 型を `<F: LockDriverFactory>` ジェネリックにし、field 型を `RuntimeMutex<T, F::Driver<T>>` に書き換え、driver 選択を呼び出し側に委譲

本 change では actor-core/kernel に対して **(c) + (b) の併用** を採用する:

- actor-core/kernel の shared 型は `<F: LockDriverFactory>` ジェネリック (no default)
- actor-core の caller boundary (e.g., `ActorSystem::new`) で `SpinSyncFactory` を渡す production path を固定
- 他 crate (cluster-core, persistence-core, etc.) は **per-crate alias** で `RuntimeMutex<T, SpinSyncMutex<T>>` を導入して現状を維持する (将来の別 change で factory 化する可能性を残す)

### 過去判断との関係

PR #1530 (utils-sync-collapse) で `SyncMutexLike` / `SyncRwLockLike` trait を削除した。当時の根拠は「1-impl の幽霊抽象」「`SyncQueueShared` 以外に generic bound caller が無い」「YAGNI」で、当時の情報範囲では正しい判断だった。

本 change はその判断を **巻き戻す**ものではなく、**新しい使用事例 (DebugSpinSyncMutex / 将来の代替 driver)** が出てきたため再導入するものである。再導入の際は:

- 名前を `LockDriver` に変更 (役割が「port 契約」と明確)
- `RuntimeMutex` を struct 化することで **driver 選択肢が複数実在**する設計にする (PR #1530 以前と異なる、強化された設計)
- `LockDriverFactory` で多 T 対応 (PR #1530 以前にはなかった)
- **default type parameter で caller 互換を密輸入しない** (PR #1530 以前との差分、hexagonal 純度を優先)

## What Changes

### 削除対象

- `modules/utils-core/src/core/sync/runtime_lock_alias.rs` の `pub type RuntimeMutex<T> = SpinSyncMutex<T>` および `pub type RuntimeRwLock<T> = SpinSyncRwLock<T>` (type alias から struct へ格上げ)
- 同ファイルの `NoStdMutex<T>` alias の配置は struct 化後の `RuntimeMutex<T, D>` に合わせて再設計する (配置変更で alias 自体は残存)

### 追加対象

#### `utils-core` 側 (port 契約と struct)

- `modules/utils-core/src/core/sync/lock_driver.rs` 新規:
  ```rust
  pub trait LockDriver<T>: Sized {
    type Guard<'a>: Deref<Target = T> + DerefMut where Self: 'a, T: 'a;
    fn new(value: T) -> Self;
    fn lock(&self) -> Self::Guard<'_>;
    fn into_inner(self) -> T;
  }
  ```

- `modules/utils-core/src/core/sync/rwlock_driver.rs` 新規:
  ```rust
  pub trait RwLockDriver<T>: Sized {
    type ReadGuard<'a>: Deref<Target = T> where Self: 'a, T: 'a;
    type WriteGuard<'a>: Deref<Target = T> + DerefMut where Self: 'a, T: 'a;
    fn new(value: T) -> Self;
    fn read(&self) -> Self::ReadGuard<'_>;
    fn write(&self) -> Self::WriteGuard<'_>;
    fn into_inner(self) -> T;
  }
  ```

- `modules/utils-core/src/core/sync/runtime_mutex.rs` 新規:
  ```rust
  pub struct RuntimeMutex<T, D>
  where D: LockDriver<T>,
  {
    driver: D,
    _pd:    PhantomData<T>,
  }

  impl<T, D: LockDriver<T>> RuntimeMutex<T, D> {
    pub fn new(value: T) -> Self { Self { driver: D::new(value), _pd: PhantomData } }
    pub fn lock(&self) -> D::Guard<'_> { self.driver.lock() }
    pub fn into_inner(self) -> T { self.driver.into_inner() }
  }
  ```
  **デフォルト型引数を持たない**。`D` は caller が必ず明示する (もしくは caller crate 側の type alias / factory 経由)。

- `modules/utils-core/src/core/sync/runtime_rwlock.rs` 新規: 同様に `pub struct RuntimeRwLock<T, D> where D: RwLockDriver<T>` (デフォルトなし)

- `modules/utils-core/src/core/sync/lock_driver_factory.rs` 新規:
  ```rust
  pub trait LockDriverFactory {
    type Driver<T>: LockDriver<T>;
  }
  pub trait RwLockDriverFactory {
    type Driver<T>: RwLockDriver<T>;
  }
  ```

- **port 定義ファイル内で `SpinSyncMutex` / `SpinSyncRwLock` を参照しない**:
  - `runtime_mutex.rs`, `runtime_rwlock.rs`, `lock_driver.rs`, `rwlock_driver.rs`, `lock_driver_factory.rs` は `LockDriver` / `RwLockDriver` trait と generic parameter `D` / `F` 経由のみで動作する
  - `SpinSyncMutex` の `impl LockDriver` は **`spin_sync_mutex.rs` 側** に置く (trait 定義は port、impl は driver ファイル)

#### `utils-core` 側 (built-in driver の trait impl)

- `modules/utils-core/src/core/sync/spin_sync_mutex.rs` に `impl<T> LockDriver<T> for SpinSyncMutex<T>` を追加 (inherent method への薄い委譲)
- `modules/utils-core/src/core/sync/spin_sync_rwlock.rs` に `impl<T> RwLockDriver<T> for SpinSyncRwLock<T>` を追加 (同様)
- `modules/utils-core/src/core/sync/spin_sync_factory.rs` 新規: `pub struct SpinSyncFactory; impl LockDriverFactory for SpinSyncFactory { type Driver<T> = SpinSyncMutex<T>; }`
- `modules/utils-core/src/core/sync/spin_sync_rwlock_factory.rs` 新規: 同様に `SpinSyncRwLockFactory`

**注意**: これら built-in driver 関連ファイルは utils-core に配置されるが、**port 定義と明確にファイル分離する**。ファイル分離により「port は adapter を知らない」という原則を物理的に可視化する。

#### `utils-adaptor-std` 側 (PR #1538 で skeleton を残してある crate)

- `DebugSpinSyncMutex<T>` を **再導入** (PR #1538 で OFF した実装を復活させ、`LockDriver` impl を追加)
- `DebugSpinSyncRwLock<T>` を新規追加 (`RwLockDriver` impl)
- `DebugSpinSyncFactory` (struct, unit) に `impl LockDriverFactory { type Driver<T> = DebugSpinSyncMutex<T>; }`
- `DebugSpinSyncRwLockFactory` (struct, unit) に `impl RwLockDriverFactory { type Driver<T> = DebugSpinSyncRwLock<T>; }`
- `feature = "test-support"` で gate

### 変更対象

#### caller 側 (115 ファイル) の移行

本 change では **default type parameter による密輸入を行わない**ため、**すべての caller サイト (115 ファイル)** を明示的に更新する必要がある。戦略は crate 単位で以下に分ける:

##### actor-core: factory ジェネリック化 (Phase 3)

- `Mailbox`, `ActorCell`, `MessageInvoker`, `EventStream` 等の shared 型に `<F: LockDriverFactory>` パラメータを追加 (default なし)
- 内部 field の型を `RuntimeMutex<(), F::Driver<()>>` 等に書き換え
- `ActorSystem` 等の caller boundary で `SpinSyncFactory` を渡す production path を固定
- test code では `SpinSyncFactory` を `DebugSpinSyncFactory` に差し替えて deadlock 検証

##### cluster-core / persistence-core / remote-core / stream-core: per-crate alias (Phase 4)

- 各 caller crate に `pub type KernelMutex<T> = RuntimeMutex<T, SpinSyncMutex<T>>;` (および RwLock 版) を導入
- 既存 field 型 `RuntimeMutex<T>` → `KernelMutex<T>` への **caller-side 書き換え**
- alias 配置: 各 crate の `src/core/sync.rs` や類似の共通モジュール (既存慣用に合わせる)
- 本 change では factory ジェネリック化はしない (将来の別 change で段階的に対応)

##### utils-core 自身の caller (wait/node 等) の更新

- utils-core 内部で `RuntimeMutex<T>` を使っている箇所 (例: `core/collections/wait/node_shared.rs`) も `RuntimeMutex<T, SpinSyncMutex<T>>` に明示更新

### 触らない範囲 (non-goals)

- **`SpinSyncMutex<T>` / `SpinSyncRwLock<T>` を utils-core から外に出す**: 本 change では utils-core に据え置く。ファイル分離 (port 定義と driver impl) で純度を担保する。将来 `fraktor-utils-adaptor-spin-rs` 等に切り出す余地は残す
- **`SpinSyncMutex<T>` / `SpinSyncRwLock<T>` の inherent API 削除**: そのまま残す。`lock()` / `read()` / `write()` の inherent method は LockDriver impl から delegate される
- **`parking_lot::Mutex` / `std::sync::Mutex` driver の追加**: 別 change で扱う
- **workspace 全体の feature flag による global driver swap**: 採用しない (ジェネリック factory で per-call-site swap が可能)
- **`RuntimeMutex` の rename**: 現在の名前を維持
- **`SyncMutexLike` / `SyncRwLockLike` trait の復活**: 名前は使わず、新しい設計の `LockDriver` / `RwLockDriver` を採用
- **cluster-core / remote-core / stream-core / persistence-core kernel の factory ジェネリック化**: 本 change では per-crate alias で互換維持に留める。factory 化は必要が出たら別 change で対応
- **`AShared` 系 (`with_read` / `with_write` 形式) の差し替え**: SharedAccess trait は別の concern。本 change では触らない

## Capabilities

### Added Capabilities

- **`utils-lock-driver-port`**: `fraktor-utils-core-rs` に `LockDriver<T>` / `RwLockDriver<T>` port 契約と、driver を保持する `RuntimeMutex<T, D>` / `RuntimeRwLock<T, D>` struct (デフォルト型引数なし)、および `LockDriverFactory` / `RwLockDriverFactory` を新設する capability。port 定義は adapter 実装を参照しない。adapter driver 実装は `fraktor-utils-adaptor-std-rs` に置く。built-in `SpinSyncMutex` / `SpinSyncRwLock` は utils-core に据え置くが port 定義ファイルと分離する。

### Modified Capabilities

なし (新規 capability の追加のみ)。

## Impact

### 影響コード (utils-core 内部 — port 定義)

- `modules/utils-core/src/core/sync/lock_driver.rs` (新規, port trait)
- `modules/utils-core/src/core/sync/rwlock_driver.rs` (新規, port trait)
- `modules/utils-core/src/core/sync/lock_driver_factory.rs` (新規, port trait)
- `modules/utils-core/src/core/sync/runtime_mutex.rs` (新規, port struct - `SpinSyncMutex` を参照しない)
- `modules/utils-core/src/core/sync/runtime_rwlock.rs` (新規, port struct - `SpinSyncRwLock` を参照しない)
- `modules/utils-core/src/core/sync/runtime_lock_alias.rs` (type alias を削除、必要なら NoStdMutex のみ `RuntimeMutex<T, SpinSyncMutex<T>>` alias として残す)

### 影響コード (utils-core 内部 — built-in driver 側)

- `modules/utils-core/src/core/sync/spin_sync_mutex.rs` (`impl LockDriver` 追加)
- `modules/utils-core/src/core/sync/spin_sync_rwlock.rs` (`impl RwLockDriver` 追加)
- `modules/utils-core/src/core/sync/spin_sync_factory.rs` (新規, `SpinSyncFactory`)
- `modules/utils-core/src/core/sync/spin_sync_rwlock_factory.rs` (新規, `SpinSyncRwLockFactory`)
- `modules/utils-core/src/core/sync.rs` (mod / pub use 配線更新)

### 影響コード (utils-adaptor-std)

- `modules/utils-adaptor-std/src/lib.rs` (skeleton 状態から復活)
- `modules/utils-adaptor-std/src/std.rs` (新規 or 復活)
- `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_mutex.rs` (PR #1538 で削除した実装を復活 + `LockDriver` impl)
- `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_mutex_guard.rs` (同上)
- `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_rwlock.rs` (新規)
- `modules/utils-adaptor-std/src/std/debug/debug_spin_sync_factory.rs` (新規、`DebugSpinSyncFactory` 等)
- `modules/utils-adaptor-std/Cargo.toml` (`feature = "test-support"`, `dep:spin` 復活)

### 影響コード (actor-core/kernel) — Phase 3

- 約 30 ファイルの shared 型 (`Mailbox`, `ActorCell`, `MessageInvoker`, `EventStream` 等) が `<F: LockDriverFactory>` (および対応する `<R: RwLockDriverFactory>`) ジェネリックを取得 (**default なし**)
- 内部 field 型を `RuntimeMutex<T, F::Driver<T>>` に書き換え
- `ActorSystem` / `ActorContext` 等の boundary で `SpinSyncFactory` を production path に固定
- test code では `DebugSpinSyncFactory` を差し替え

### 影響コード (cluster-core / persistence-core / stream-core / actor-adaptor-std 等) — Phase 4

- 各 crate に `KernelMutex<T> = RuntimeMutex<T, SpinSyncMutex<T>>` 等の per-crate alias を導入
- 既存 field 型を alias に置換 (bulk 書き換え)
- 本 change では factory ジェネリック化はしない (必要になったら別 change で対応)

### 影響 caller (本 change 適用後のソース変化)

- **115 ファイルが書き換わる** (utils-core 内部の `RuntimeMutex` 利用箇所を除く)
  - actor-core/kernel 関連 (~30 ファイル): factory ジェネリック化
  - cluster-core / persistence-core / stream-core / actor-adaptor-std (~85 ファイル): per-crate alias 経由の書き換え
- **観測可能な挙動変化なし**: どちらの戦略でも production code は `SpinSyncMutex` driver を使うため、runtime の lock 動作は PR 適用前と同一

### 影響 caller (test)

- `actor-core` の test target に `[dev-dependencies] fraktor-utils-adaptor-std-rs = { workspace = true, features = ["test-support"] }` を追加
- test code で `ActorSystem<DebugSpinSyncFactory>` のように factory を差し替えて deadlock 検証

### 影響 API (BREAKING の有無)

- **内部 API は BREAKING**: 115 ファイルがソース上書き換わる。ただし runtime 挙動は同一
- **外部 API (公開 re-export) も BREAKING**: `RuntimeMutex<T>` の型引数が 1 → 2 に増える
  - caller は `RuntimeMutex<T, SpinSyncMutex<T>>` または per-crate alias 経由で参照
  - **後方互換性は必要ない** (fraktor-rs はプレリリース段階、破壊的変更歓迎のプロジェクト方針)
- **新規追加 (BREAKING でない)**: `LockDriver`, `RwLockDriver`, `LockDriverFactory`, `RwLockDriverFactory` trait, `SpinSyncFactory` / `SpinSyncRwLockFactory`
- **test API 追加**: `DebugSpinSyncMutex`, `DebugSpinSyncFactory` 等が `utils-adaptor-std` の `feature = "test-support"` 配下に新登場
- **actor-core/kernel のジェネリック化** (Phase 3) は `<F: LockDriverFactory>` (no default) により、すべての caller site で F の明示が必要

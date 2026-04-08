## ADDED Requirements

### Requirement: LockDriver Port Contract for Mutex Primitives

`fraktor-utils-core-rs` は `LockDriver<T>` trait を **port 契約** として提供しなければならない（MUST）。この trait は mutex プリミティブの最小契約を表現し、以下を備える:

- `type Guard<'a>: Deref<Target = T> + DerefMut where Self: 'a, T: 'a;` (Generic Associated Type)
- `fn new(value: T) -> Self where Self: Sized;`
- `fn lock(&self) -> Self::Guard<'_>;`
- `fn into_inner(self) -> T where Self: Sized;`

trait 自体は **`Sized` のみを境界**とし、`Send` / `Sync` を強制しない（impl 側で個別に保証する）。

trait を定義するファイル (`modules/utils-core/src/core/sync/lock_driver.rs`) は、**具象 driver 実装 (`SpinSyncMutex`, `DebugSpinSyncMutex` 等) を一切 use / 参照してはならない**（MUST NOT）。port 定義は adapter 実装を知らない。

#### Scenario: SpinSyncMutex implements LockDriver
- **WHEN** `SpinSyncMutex<T>` の impl を確認する
- **THEN** `impl<T> LockDriver<T> for SpinSyncMutex<T>` が `modules/utils-core/src/core/sync/spin_sync_mutex.rs` (built-in driver 側のファイル) に存在し、`Guard<'a> = spin::MutexGuard<'a, T>` として inherent method (`SpinSyncMutex::lock` 等) に薄く委譲する
- **AND** trait impl は port 定義ファイル (`lock_driver.rs`) には置かれない

#### Scenario: caller specifies the driver explicitly via type parameter
- **WHEN** `RuntimeMutex<T, D>` を caller が利用する
- **THEN** `D` に任意の `LockDriver<T>` 実装を渡せる
- **AND** **デフォルト型引数は存在しない**。caller は必ず `D` を明示する (もしくは caller crate 側の type alias / factory 経由で間接的に指定する)

### Requirement: RwLockDriver Port Contract for Read-Write Locks

`fraktor-utils-core-rs` は `RwLockDriver<T>` trait を **port 契約** として提供しなければならない（MUST）。

- `type ReadGuard<'a>: Deref<Target = T> where Self: 'a, T: 'a;`
- `type WriteGuard<'a>: Deref<Target = T> + DerefMut where Self: 'a, T: 'a;`
- `fn new(value: T) -> Self where Self: Sized;`
- `fn read(&self) -> Self::ReadGuard<'_>;`
- `fn write(&self) -> Self::WriteGuard<'_>;`
- `fn into_inner(self) -> T where Self: Sized;`

trait 定義ファイル (`modules/utils-core/src/core/sync/rwlock_driver.rs`) は具象 driver 実装を一切参照してはならない（MUST NOT）。

#### Scenario: SpinSyncRwLock implements RwLockDriver
- **WHEN** `SpinSyncRwLock<T>` の impl を確認する
- **THEN** `impl<T> RwLockDriver<T> for SpinSyncRwLock<T>` が `modules/utils-core/src/core/sync/spin_sync_rwlock.rs` (built-in driver 側) に存在し、`spin::RwLock` の guard 型に委譲する

### Requirement: RuntimeMutex is a concrete struct that delegates to a LockDriver (no default type parameter)

`RuntimeMutex` は **concrete struct** として定義され（type alias であってはならない、MUST NOT type alias）、型引数 `<T, D>` を取り、内部に `LockDriver<T>` 実装の driver を保持しなければならない（MUST）。**デフォルト型引数を持ってはならない**（MUST NOT default type parameter）。port struct 定義は adapter 実装を一切参照しない。

struct 定義ファイル (`modules/utils-core/src/core/sync/runtime_mutex.rs`) は、`LockDriver<T>` trait 以外の具象 driver 型 (`SpinSyncMutex`, `DebugSpinSyncMutex` 等) を一切 use / 参照してはならない（MUST NOT）。

#### Scenario: RuntimeMutex requires explicit D
- **WHEN** caller code が `RuntimeMutex<MyState>` と 1 引数のみで記述する
- **THEN** compile error になる (デフォルト型引数が存在しないため、`D` が未指定)
- **AND** caller は `RuntimeMutex<MyState, SpinSyncMutex<MyState>>` のように `D` を明示するか、caller crate 側の type alias (`type KernelMutex<T> = RuntimeMutex<T, SpinSyncMutex<T>>;`) 経由で間接的に指定する必要がある

#### Scenario: caller uses per-crate alias for convenience
- **WHEN** caller crate が `pub type KernelMutex<T> = RuntimeMutex<T, SpinSyncMutex<T>>;` を **caller crate 側** のファイルに定義する
- **THEN** field 型として `KernelMutex<T>` を記述でき、`D` を explicit に繰り返す必要がない
- **AND** alias ファイルは caller crate 側 (例: `modules/cluster-core/src/core/sync.rs`) に置かれ、**utils-core 側には置かれない**

#### Scenario: test code can swap the driver
- **WHEN** test code が `let m: RuntimeMutex<MyState, DebugSpinSyncMutex<MyState>> = RuntimeMutex::new(MyState::default());` と記述する
- **THEN** 内部 driver が `DebugSpinSyncMutex<MyState>` に切り替わり、再入時の panic 検出が有効になる

### Requirement: RuntimeRwLock is a concrete struct that delegates to a RwLockDriver (no default type parameter)

`RuntimeRwLock` も `RuntimeMutex` と同様に concrete struct として定義され、`<T, D>` を取り、`RwLockDriver<T>` 実装の driver を保持しなければならない（MUST）。**デフォルト型引数を持ってはならない**（MUST NOT default type parameter）。struct 定義ファイル (`runtime_rwlock.rs`) は `SpinSyncRwLock` 等の具象 driver 型を一切参照してはならない（MUST NOT）。

#### Scenario: RuntimeRwLock requires explicit D
- **WHEN** caller code が `RuntimeRwLock<MyState>` と 1 引数のみで記述する
- **THEN** compile error になる
- **AND** caller は `RuntimeRwLock<MyState, SpinSyncRwLock<MyState>>` または per-crate alias を使う

### Requirement: LockDriverFactory enables type-family parameterization

`fraktor-utils-core-rs` は higher-kinded type の代替として `LockDriverFactory` trait を提供しなければならない（MUST）。これにより、複数の異なる `T` を持つ shared 型 (`Mailbox` 等) が **単一の factory parameter で全 lock の driver を一括選択** できる。

```rust
pub trait LockDriverFactory {
  type Driver<T>: LockDriver<T>;
}
```

`RwLockDriverFactory` も対称に提供する（MUST）。

trait 定義ファイル (`modules/utils-core/src/core/sync/lock_driver_factory.rs`) は具象 factory 実装 (`SpinSyncFactory` 等) を一切参照してはならない（MUST NOT）。

#### Scenario: SpinSyncFactory is a built-in factory
- **WHEN** `SpinSyncFactory` の impl を確認する
- **THEN** `impl LockDriverFactory for SpinSyncFactory { type Driver<T> = SpinSyncMutex<T>; }` が `modules/utils-core/src/core/sync/spin_sync_factory.rs` (built-in driver 側のファイル) に存在する
- **AND** actor-core/kernel の production caller で `Mailbox<SpinSyncFactory>` のように **明示的に** 参照される

#### Scenario: shared types are generic over a factory (no default)
- **WHEN** actor-core の shared 型 (`Mailbox` 等) を確認する
- **THEN** `<F: LockDriverFactory>` (および対応する `<R: RwLockDriverFactory>`) のジェネリックパラメータを持ち、**デフォルト型引数を持たない**
- **AND** 内部 field の型が `RuntimeMutex<T, F::Driver<T>>` 形式で記述される
- **AND** caller boundary (`ActorSystem::new` 等) で `SpinSyncFactory` を production path に明示する必要がある

#### Scenario: test code can substitute the factory globally for one shared type
- **WHEN** test code が `ActorSystem<DebugSpinSyncFactory, DebugSpinSyncRwLockFactory>` のように型引数を差し替える
- **THEN** 全 lock field が `DebugSpinSyncMutex` / `DebugSpinSyncRwLock` driver で構築される
- **AND** 同一スレッドの再入が検出され panic する

### Requirement: port definition files must not reference adapter implementations

`fraktor-utils-core-rs` の port 定義ファイル群は、**具象 driver 実装 (built-in または adapter crate 由来) を use / 参照してはならない**（MUST NOT）。対象ファイル:

- `modules/utils-core/src/core/sync/lock_driver.rs`
- `modules/utils-core/src/core/sync/rwlock_driver.rs`
- `modules/utils-core/src/core/sync/lock_driver_factory.rs`
- `modules/utils-core/src/core/sync/runtime_mutex.rs`
- `modules/utils-core/src/core/sync/runtime_rwlock.rs`

これらのファイルは `LockDriver<T>` / `RwLockDriver<T>` trait と generic parameter のみを使う。具象 driver 型 (`SpinSyncMutex`, `SpinSyncRwLock`, `DebugSpinSyncMutex` 等) を一切名指ししない。

#### Scenario: mechanical verification via grep
- **WHEN** `grep -n "SpinSyncMutex\|SpinSyncRwLock\|DebugSpinSync" modules/utils-core/src/core/sync/lock_driver.rs modules/utils-core/src/core/sync/rwlock_driver.rs modules/utils-core/src/core/sync/lock_driver_factory.rs modules/utils-core/src/core/sync/runtime_mutex.rs modules/utils-core/src/core/sync/runtime_rwlock.rs` を実行する
- **THEN** 出力は空である必要がある (port 定義は adapter 実装を知らない)

### Requirement: Adapter drivers live in fraktor-utils-adaptor-std-rs

std 依存の driver 実装 (`DebugSpinSyncMutex`, `DebugSpinSyncRwLock`, 将来の `parking_lot` driver 等) は **`fraktor-utils-adaptor-std-rs` crate に置かなければならない**（MUST）。`fraktor-utils-core-rs` の no_std 純度を維持するため、`utils-core` 側に std 依存の driver を実装してはならない（MUST NOT）。

adapter driver は `feature = "test-support"` で gate されなければならない（MUST）。production code から誤って enable されることを防ぐ。

#### Scenario: utils-core remains no_std
- **WHEN** `fraktor-utils-core-rs` を `cargo check --no-default-features` でビルドする
- **THEN** std 依存なしでビルドが成功する
- **AND** `LockDriver` / `RwLockDriver` trait や built-in driver (`SpinSyncMutex` / `SpinSyncRwLock`) は no_std 環境で利用できる

#### Scenario: DebugSpinSyncMutex lives in utils-adaptor-std under test-support feature
- **WHEN** `fraktor-utils-adaptor-std-rs` を `--features test-support` でビルドする
- **THEN** `std::debug::DebugSpinSyncMutex<T>` および対応する `DebugSpinSyncFactory` が公開される
- **AND** `LockDriver<T>` impl を持ち、`RuntimeMutex<T, DebugSpinSyncMutex<T>>` として使える

#### Scenario: utils-adaptor-std without test-support is empty
- **WHEN** `fraktor-utils-adaptor-std-rs` を default features (= test-support 無効) でビルドする
- **THEN** crate は skeleton 状態 (production 配下に公開シンボルなし) で正常にビルドできる
- **AND** spin crate 依存も pull されない

### Requirement: Existing inherent API of SpinSyncMutex / SpinSyncRwLock is preserved

`SpinSyncMutex<T>` および `SpinSyncRwLock<T>` の inherent method (`new`, `lock`, `read`, `write`, `into_inner`, `as_inner`) は **削除されてはならない**（MUST NOT）。`LockDriver` / `RwLockDriver` trait impl はこれらの inherent method への薄い委譲として実装される。

#### Scenario: callers can still use SpinSyncMutex directly
- **WHEN** caller code が `let mutex = SpinSyncMutex::new(0_u32); let guard = mutex.lock();` と記述する
- **THEN** trait import なしで動作する
- **AND** 既存の inherent method 経由の呼び出しは PR 適用前と同一の挙動を示す

### Requirement: Per-crate alias strategy for non-factory-genericized callers

本 change では actor-core/kernel のみ factory ジェネリック化し、cluster-core / persistence-core / stream-core / actor-adaptor-std は **per-crate alias** で `RuntimeMutex<T, SpinSyncMutex<T>>` を使う。各 caller crate は **自身の crate 内** に `KernelMutex<T>` / `KernelRwLock<T>` (または同等の名前) の type alias を定義しなければならない（MUST）。

alias の配置:

- 各 caller crate の `modules/<crate>/src/core/sync.rs` または既存慣用に合わせた場所
- **`fraktor-utils-core-rs` 内には配置してはならない**（MUST NOT）。utils-core は `SpinSyncMutex` を `RuntimeMutex` と紐づけて alias 化しない

#### Scenario: cluster-core defines its own KernelMutex alias
- **WHEN** `modules/cluster-core/src/core/sync.rs` を確認する
- **THEN** `pub type KernelMutex<T> = fraktor_utils_core_rs::RuntimeMutex<T, fraktor_utils_core_rs::SpinSyncMutex<T>>;` が存在する
- **AND** cluster-core の field 型は `KernelMutex<T>` を経由して記述される

#### Scenario: utils-core does not provide a convenience alias that names SpinSyncMutex
- **WHEN** `grep -rn "RuntimeMutex<.*, SpinSyncMutex" modules/utils-core/src/core/sync/` を実行する
- **THEN** `runtime_mutex.rs` / `lock_driver.rs` 等の port 定義ファイルにはマッチしない
- **AND** utils-core 内部の caller (例: `wait/node_shared.rs`) または built-in driver 側のファイル (例: `spin_sync_factory.rs` 内の `NoStdMutex` alias) のみマッチする (これらは port 定義ファイルではないので許容)

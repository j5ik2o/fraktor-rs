## ADDED Requirements

### Requirement: std-locks feature が有効な場合 DefaultMutex / DefaultRwLock は std::sync ベースに解決されなければならない

`std-locks` feature が有効な場合、`DefaultMutex<T>` は `StdSyncMutex<T>` に、`DefaultRwLock<T>` は `StdSyncRwLock<T>` に解決されなければならない（MUST）。

#### Scenario: std-locks 有効時は StdSyncMutex が使われる
- **WHEN** `std-locks` feature が有効の状態でビルドする
- **AND** `debug-locks` feature が無効の状態
- **THEN** `DefaultMutex<T>` は `StdSyncMutex<T>` に解決される
- **AND** `DefaultRwLock<T>` は `StdSyncRwLock<T>` に解決される

#### Scenario: debug-locks が std-locks より優先される
- **WHEN** `debug-locks` と `std-locks` の両方が有効の状態でビルドする
- **THEN** `DefaultMutex<T>` は `CheckedSpinSyncMutex<T>` に解決される
- **AND** `DefaultRwLock<T>` は `CheckedSpinSyncRwLock<T>` に解決される

#### Scenario: actor-adaptor-std を含むビルドで feature unification が機能する
- **WHEN** `actor-adaptor-std` を依存に含むバイナリをビルドする
- **THEN** `actor-core` 内の `DefaultMutex` も `StdSyncMutex` に解決される
- **AND** actor-core のソースコードに `#[cfg(feature = "std")]` が存在しない

### Requirement: StdSyncMutex / StdSyncRwLock は LockDriver / RwLockDriver を実装しなければならない

utils-core 内の `StdSyncMutex` は `LockDriver<T>` を、`StdSyncRwLock` は `RwLockDriver<T>` を実装しなければならない（MUST）。

#### Scenario: SharedLock 経由で構築できる
- **WHEN** `std-locks` feature が有効の状態で `SharedLock::new_with_driver::<StdSyncMutex<_>>(value)` を呼ぶ
- **THEN** SharedLock が正常に構築される
- **AND** `with_read` / `with_lock` で値にアクセスできる

#### Scenario: std-locks なしではコンパイルされない
- **WHEN** `std-locks` feature が無効の状態でビルドする
- **THEN** `StdSyncMutex` / `StdSyncRwLock` のモジュールはコンパイルされない
- **AND** no_std ターゲットでビルドが通る

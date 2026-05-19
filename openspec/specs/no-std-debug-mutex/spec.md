# no-std-debug-mutex Specification

## Purpose
TBD - created by archiving change compile-time-lock-backend-selection. Update Purpose after archive.
## Requirements
### Requirement: no_std 互換の CheckedSpinSyncMutex は再入ロックを検知して panic しなければならない

`CheckedSpinSyncMutex` は `no_std` 環境で動作し、同一実行コンテキストからの再入ロック（`lock()` 中に再度 `lock()` を呼ぶ）を検知して panic しなければならない（MUST）。`std::thread` に依存してはならない（MUST NOT）。

#### Scenario: 再入ロックが panic を引き起こす
- **WHEN** `CheckedSpinSyncMutex::lock()` で取得した guard がまだ保持されている状態で、同じ mutex に対して再度 `lock()` が呼ばれる
- **THEN** `panic!` が発生する
- **AND** panic メッセージに "re-entrant lock" を含む

#### Scenario: 通常の lock/unlock サイクルは正常に動作する
- **WHEN** `CheckedSpinSyncMutex::lock()` で guard を取得し、guard が drop される
- **THEN** 次の `lock()` 呼び出しは正常に成功する
- **AND** 内部の値に正しくアクセスできる

#### Scenario: LockDriver trait を実装する
- **WHEN** `CheckedSpinSyncMutex` が `LockDriver<T>` trait を実装する
- **THEN** `SharedLock::new_with_driver::<CheckedSpinSyncMutex<_>>(value)` で SharedLock を構築できる

#### Scenario: no_std ターゲットでコンパイルできる
- **WHEN** `debug-locks` feature を有効にして `thumbv8m.main-none-eabi` ターゲットでビルドする
- **THEN** コンパイルが成功する
- **AND** `std` crate への依存が存在しない

### Requirement: no_std 互換の CheckedSpinSyncRwLock は write 再入と read→write 昇格を検知して panic しなければならない

`CheckedSpinSyncRwLock` は `no_std` 環境で動作し、write ロック保持中の再入 write および read ロック保持中の write 取得を検知して panic しなければならない（MUST）。read ロック同士の再入は検知しない（MAY NOT）。

#### Scenario: write 再入が panic を引き起こす
- **WHEN** `CheckedSpinSyncRwLock::write()` で取得した guard がまだ保持されている状態で、同じ rwlock に対して再度 `write()` が呼ばれる
- **THEN** `panic!` が発生する
- **AND** panic メッセージに "re-entrant write lock" を含む

#### Scenario: read 保持中の write が panic を引き起こす
- **WHEN** `CheckedSpinSyncRwLock::read()` で取得した guard がまだ保持されている状態で、同じ rwlock に対して `write()` が呼ばれる
- **THEN** `panic!` が発生する
- **AND** panic メッセージに "write lock while read lock held" を含む

#### Scenario: RwLockDriver trait を実装する
- **WHEN** `CheckedSpinSyncRwLock` が `RwLockDriver<T>` trait を実装する
- **THEN** `SharedRwLock::new_with_driver::<CheckedSpinSyncRwLock<_>>(value)` で SharedRwLock を構築できる


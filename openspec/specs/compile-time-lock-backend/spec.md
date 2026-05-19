# compile-time-lock-backend Specification

## Purpose
TBD - created by archiving change compile-time-lock-backend-selection. Update Purpose after archive.
## Requirements
### Requirement: ロックバックエンドは compile-time type alias で選択されなければならない

production code の `SharedLock` / `SharedRwLock` 構築は、`SpinSyncMutex` / `SpinSyncRwLock` を直接指定するのではなく `DefaultMutex` / `DefaultRwLock` type alias を経由しなければならない（MUST）。type alias は `debug-locks` feature flag で compile-time に切り替わる。

#### Scenario: debug-locks 無効時は SpinSyncMutex が使われる
- **WHEN** `debug-locks` feature が無効の状態でビルドする
- **THEN** `DefaultMutex<T>` は `SpinSyncMutex<T>` に解決される
- **AND** `DefaultRwLock<T>` は `SpinSyncRwLock<T>` に解決される

#### Scenario: debug-locks 有効時は CheckedSpinSyncMutex が使われる
- **WHEN** `debug-locks` feature が有効の状態でビルドする
- **THEN** `DefaultMutex<T>` は `CheckedSpinSyncMutex<T>` に解決される
- **AND** `DefaultRwLock<T>` は `CheckedSpinSyncRwLock<T>` に解決される

#### Scenario: production code に SpinSyncMutex / SpinSyncRwLock の直書きが残らない
- **WHEN** production code（テスト以外）で `SharedLock` / `SharedRwLock` を構築する
- **THEN** `SharedLock::new_with_driver::<DefaultMutex<_>>(...)` または `SharedRwLock::new_with_driver::<DefaultRwLock<_>>(...)` を使う
- **AND** `SpinSyncMutex` / `SpinSyncRwLock` を `new_with_driver` の type parameter に直接指定しない

#### Scenario: テストコード内では SpinSyncMutex 直書きが許容される
- **WHEN** テストコード（`#[cfg(test)]` module、`tests/` ディレクトリ、`benches/`）で `SharedLock` を構築する
- **THEN** `SpinSyncMutex` / `SpinSyncRwLock` の直書きが許容される（MAY）
- **AND** `DefaultMutex` / `DefaultRwLock` を使ってもよい（MAY）

### Requirement: cargo test 時に dev-dependencies 経由で debug-locks が有効になる構成でなければならない

各クレートの dev-dependencies で `fraktor-utils-core-rs` の `debug-locks` feature を有効化することで、`cargo test` 実行時に自動的に debug backend が使われなければならない（MUST）。

#### Scenario: cargo test で再入ロックが検知される
- **WHEN** `cargo test` でテストを実行する
- **AND** テスト中に SharedLock の再入ロックが発生する
- **THEN** panic が発生し、テストが失敗する

#### Scenario: cargo build --release では debug backend が含まれない
- **WHEN** `cargo build --release` で production binary をビルドする
- **THEN** `DefaultMutex` は `SpinSyncMutex` に解決される
- **AND** `CheckedSpinSyncMutex` のコードはバイナリに含まれない


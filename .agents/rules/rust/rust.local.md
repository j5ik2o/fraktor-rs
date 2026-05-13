---
paths:
  - "**/*.rs"
---
# Rust Rules (Project-specific)

## Project-specific patterns

- `fraktor_utils_core_rs::core::sync::ArcShared<T>` - `Arc` の置き換え（`alloc::sync::Arc` / `std::sync::Arc` / `std::rc::Rc` は `clippy.toml` で disallowed）
- `SharedLock<T>` / `SharedRwLock<T>` - 内部可変性の唯一の許容ラッパー、`SharedAccess` 経由で操作
- `SharedAccess<B>` trait - `with_read(|b| ...)` / `with_write(|b| ...)`（ガードを外部に返さない）
- `DefaultMutex<T>` / `DefaultRwLock<T>` - feature flag で `CheckedSpinSync*` / `StdSync*` / `SpinSync*` に解決される type alias、初期化に渡す標準型
- `SharedLock::new_with_driver::<DefaultMutex<_>>(value)` - 標準初期化形（`SharedRwLock` は `DefaultRwLock<_>`）、テストでも同じ driver を使う
- `SpinSyncMutex<T>` / `SpinSyncRwLock<T>` / `SyncOnce` - canonical 同期プリミティブ（`std::sync::Mutex` / `spin::Mutex` / `spin::Once` は disallowed）
- `*Shared` 命名 = 薄い同期ラッパー / `*Handle` 命名 = ライフサイクル管理 / サフィックスなし = 所有権一意・同期不要
- `references/protoactor-go/` (Go) と `references/pekko/` (Scala) - 設計逆輸入元、新機能設計開始時と命名検討時に参照
- `./scripts/ci-check.sh ai all` - AI 向け最終フルチェック、`./scripts/ci-check.sh <subcommand>` で部分実行（`lint` / `dylint` / `clippy` / `no-std` / `unit-test` / `integration-test` / `e2e-test` 等）

## Examples

When in doubt: ./rust.examples.md

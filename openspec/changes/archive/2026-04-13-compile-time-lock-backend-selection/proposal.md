## Why

`remove-shared-wrapper-factories` で runtime injection（18 個の `*SharedFactory` trait）を廃止し、actor runtime の shared wrapper 構築を `SharedLock::new_with_driver::<SpinSyncMutex<_>>(...)` による direct construction へ回帰させた。しかし 95 箇所で `SpinSyncMutex` / `SpinSyncRwLock` がハードコードされているため、テスト時に再入検知付きの debug mutex へ切り替える手段がない。

本変更は compile-time type alias + feature flag でロックバックエンドを切り替えられるようにする。runtime injection を再導入せず、generic parameter の伝播も起こさない。

## What Changes

- `fraktor-utils-core-rs` に no_std 互換の `CheckedSpinSyncMutex` / `CheckedSpinSyncRwLock` を新設する（`AtomicBool` / `AtomicU8` ベースの再入検知、`std::thread` 不要）
- `fraktor-utils-core-rs` に `debug-locks` feature を追加し、`DefaultMutex<T>` / `DefaultRwLock<T>` type alias を導入する
  - `debug-locks` 無効時: `SpinSyncMutex<T>` / `SpinSyncRwLock<T>`
  - `debug-locks` 有効時: `CheckedSpinSyncMutex<T>` / `CheckedSpinSyncRwLock<T>`
- production code の `SpinSyncMutex` / `SpinSyncRwLock` 直書き 95 箇所を `DefaultMutex` / `DefaultRwLock` に置換する
- 各クレートの `[dev-dependencies]` に `features = ["debug-locks"]` を追加し、`cargo test` 時に自動的に debug backend が有効になるようにする

## Capabilities

### New Capabilities
- `compile-time-lock-backend`: ロックバックエンドを feature flag で compile-time に切り替える
- `no-std-checked-mutex`: no_std 環境で動作する再入検知付き checked mutex

### Modified Capabilities
- `actor-builtin-spin-shared-construction`: shared wrapper の direct construction が `DefaultMutex` / `DefaultRwLock` 経由になる

## Impact

- 対象コード:
  - `modules/utils-core/src/core/sync/` — type alias 定義と debug mutex/rwlock 新設
  - `modules/actor-core/src/` — 71 箇所の SpinSyncMutex/SpinSyncRwLock 直書き置換
  - `modules/cluster-core/src/` — 20 箇所の置換
  - `modules/persistence-core/src/` — 2 箇所の置換
  - `modules/utils-core/src/` — 2 箇所の置換
  - 各クレートの `Cargo.toml` — dev-dependencies の feature 追加
- 影響内容:
  - production binary は変更なし（`DefaultMutex` = `SpinSyncMutex`）
  - `cargo test` 時に debug backend が有効になり、再入ロックが panic として検知される
  - `utils-adaptor-std` の既存 `DebugSpinSyncMutex`（`std::thread` ベース）は残す。スレッド ID 付き診断が必要な場合に直接使える。utils-core 版は `CheckedSpinSyncMutex` と命名し名前衝突を回避する
- 非目標:
  - runtime injection の再導入
  - `ActorSystem<B>` のような公開 generic 型の導入
  - `StdSyncMutex` への切替機構を今回組み込むこと（将来 `std-locks` feature で対応可能）

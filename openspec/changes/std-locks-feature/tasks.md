## 1. utils-core に StdSyncMutex / StdSyncRwLock を新設

- [ ] 1.1 `fraktor-utils-core-rs/Cargo.toml` に `std-locks = ["std"]` feature を追加する（`std` feature と `extern crate std` は debug-locks 対応で追加済み）
- [ ] 1.2 `fraktor-utils-core-rs` に `StdSyncMutex<T>` を `cfg(feature = "std-locks")` で新設する（`std::sync::Mutex` ラッパー、`LockDriver<T>` impl、poison 吸収、`#![allow(cfg_std_forbid)]` 付与）
- [ ] 1.3 `fraktor-utils-core-rs` に `StdSyncRwLock<T>` を `cfg(feature = "std-locks")` で新設する（`std::sync::RwLock` ラッパー、`RwLockDriver<T>` impl、poison 吸収、`#![allow(cfg_std_forbid)]` 付与）
- [ ] 1.4 `StdSyncMutex` / `StdSyncRwLock` の単体テストを追加する

## 2. DefaultMutex / DefaultRwLock の分岐を更新

- [ ] 2.1 `DefaultMutex` / `DefaultRwLock` type alias に `std-locks` 分岐を追加する（debug-locks > std-locks > default の優先順位）

## 3. utils-adaptor-std の StdSyncMutex / StdSyncRwLock を re-export に置換

- [ ] 3.1 `utils-adaptor-std` の `StdSyncMutex` / `StdSyncRwLock` の自前実装を削除し、`utils-core` からの re-export に置き換える（`pub use fraktor_utils_core_rs::core::sync::{StdSyncMutex, StdSyncRwLock};`）
- [ ] 3.2 `utils-adaptor-std` の `StdSyncMutex` / `StdSyncRwLock` の単体テストを削除する（utils-core 側のテストで担保）

## 4. actor-adaptor-std 等で feature を有効化

- [ ] 4.1 `fraktor-actor-adaptor-std-rs/Cargo.toml` の `[dependencies]` で `fraktor-utils-core-rs` に `std-locks` feature を追加する
- [ ] 4.2 `utils-adaptor-std`, `cluster-adaptor-std`, `remote-adaptor-std`, `showcases-std` の `[dependencies]` で `fraktor-utils-core-rs` に `std-locks` feature を追加する

## 5. 検証

- [ ] 5.1 `cargo check --lib --workspace` がクリーンにビルドされることを確認する（std-locks が adaptor-std 経由で有効）
- [ ] 5.2 `cargo check --tests --workspace` がクリーンにビルドされることを確認する（debug-locks が dev-deps 経由で優先）
- [ ] 5.3 no_std ターゲット（`thumbv8m.main-none-eabi`）で `std-locks` なしのビルドが通ることを確認する
- [ ] 5.4 `cargo check -p fraktor-actor-core-rs` 単体（adaptor-std なし）で `DefaultMutex` = `SpinSyncMutex` のままビルドが通ることを確認する

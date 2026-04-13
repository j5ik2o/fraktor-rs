## 1. utils-core に StdSyncMutex / StdSyncRwLock を新設

- [ ] 1.1 `fraktor-utils-core-rs/Cargo.toml` に `std` と `std-locks = ["std"]` feature を追加し、`lib.rs` に `#[cfg(feature = "std")] extern crate std;` を追加する
- [ ] 1.2 `fraktor-utils-core-rs` に `StdSyncMutex<T>` を `cfg(feature = "std-locks")` で新設する（`std::sync::Mutex` ラッパー、`LockDriver<T>` impl、poison 吸収）
- [ ] 1.3 `fraktor-utils-core-rs` に `StdSyncRwLock<T>` を `cfg(feature = "std-locks")` で新設する（`std::sync::RwLock` ラッパー、`RwLockDriver<T>` impl、poison 吸収）
- [ ] 1.4 `StdSyncMutex` / `StdSyncRwLock` の単体テストを追加する

## 2. DefaultMutex / DefaultRwLock の分岐を更新

- [ ] 2.1 `DefaultMutex` / `DefaultRwLock` type alias に `std-locks` 分岐を追加する（debug-locks > std-locks > default の優先順位）

## 3. actor-adaptor-std で feature を有効化

- [ ] 3.1 `fraktor-actor-adaptor-std-rs/Cargo.toml` の `[dependencies]` で `fraktor-utils-core-rs` に `std-locks` feature を追加する
- [ ] 3.2 `utils-adaptor-std`, `cluster-adaptor-std`, `remote-adaptor-std`, `showcases-std` の `[dependencies]` で `fraktor-utils-core-rs` に `std-locks` feature を追加する

## 4. 検証

- [ ] 4.1 `cargo check --lib --workspace` がクリーンにビルドされることを確認する（std-locks が adaptor-std 経由で有効）
- [ ] 4.2 `cargo check --tests --workspace` がクリーンにビルドされることを確認する（debug-locks が dev-deps 経由で優先）
- [ ] 4.3 no_std ターゲット（`thumbv8m.main-none-eabi`）で `std-locks` なしのビルドが通ることを確認する
- [ ] 4.4 `cargo check -p fraktor-actor-core-rs` 単体（adaptor-std なし）で `DefaultMutex` = `SpinSyncMutex` のままビルドが通ることを確認する

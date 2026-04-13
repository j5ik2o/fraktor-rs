## 1. no_std checked mutex / rwlock の実装

- [x] 1.1 `fraktor-utils-core-rs` に `CheckedSpinSyncMutex<T>` を実装する（`AtomicBool` ベース再入検知、`LockDriver<T>` impl、Guard の Drop で flag リセット）
- [x] 1.2 `fraktor-utils-core-rs` に `CheckedSpinSyncRwLock<T>` を実装する（`AtomicU8` ベース `0=free/1=read/2=write` 状態管理、write 再入検知、read→write 昇格検知、`RwLockDriver<T>` impl）
- [x] 1.3 `CheckedSpinSyncMutex` / `CheckedSpinSyncRwLock` の単体テストを追加する（再入 panic、read→write panic、通常 lock/unlock、SharedLock/SharedRwLock 経由の構築）

## 2. type alias と feature flag の導入

- [x] 2.1 `fraktor-utils-core-rs/Cargo.toml` に `debug-locks` feature を追加する
- [x] 2.2 `fraktor-utils-core-rs/src/core/sync.rs` に `DefaultMutex<T>` / `DefaultRwLock<T>` type alias を追加し、`cfg(feature = "debug-locks")` で `CheckedSpinSyncMutex` / `CheckedSpinSyncRwLock` に切り替える
- [x] 2.3 `DefaultMutex` / `DefaultRwLock` を `pub use` で公開する

## 3. production code の置換

- [ ] 3.1 `modules/utils-core/src/` 内の `SpinSyncMutex` 直書き call site を `DefaultMutex` に置換する（2 箇所）
- [ ] 3.2 `modules/actor-core/src/` 内の `SpinSyncMutex` / `SpinSyncRwLock` 直書き call site を `DefaultMutex` / `DefaultRwLock` に置換する（71 箇所。テストファイルはそのまま残す）
- [ ] 3.3 `modules/cluster-core/src/` 内の call site を置換する（20 箇所。テストファイルはそのまま残す）
- [ ] 3.4 `modules/persistence-core/src/` 内の call site を置換する（2 箇所）

## 4. dev-dependencies と検証

- [ ] 4.1 各クレートの `[dev-dependencies]` に `fraktor-utils-core-rs = { ..., features = ["debug-locks"] }` を追加する
- [ ] 4.2 `cargo check --lib --workspace` が `debug-locks` なしでクリーンにビルドされることを確認する
- [ ] 4.3 `cargo check --tests --workspace` が `debug-locks` ありでクリーンにビルドされることを確認する
- [ ] 4.4 no_std ターゲット（`thumbv8m.main-none-eabi`）で `debug-locks` なしのビルドが通ることを確認する

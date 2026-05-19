## 1. 事前調査と確認

- [x] 1.1 `actor-core/src` 全体で `spin` 直接利用箇所を再 Grep（`use spin::` および `spin::`）。起案時調査では `coordinated_shutdown.rs:23` と `mailbox/base.rs:10` の `Once` のみ。実装直前に再確認
- [x] 1.2 `actor-core/Cargo.toml` の `[dependencies]` および `[dev-dependencies]` の `spin` 利用を確認（`:35` の `spin = { workspace = true, default-features = false, features = ["mutex", "spin_mutex", "once"] }`、`[dev-dependencies]` 側には `spin` が無いことを再確認）
- [x] 1.3 `utils-core/src/core/sync/` の既存構成（`lock_driver.rs`、`spin_sync_mutex.rs`、`shared_lock.rs` 等）を再確認し、`once_driver.rs` / `spin_once.rs` / `sync_once.rs` の配置（design Decision 1〜3 で確定済み）と `pub use` 経路を確認
- [x] 1.4 `spin::Once<T>` の正確な API（`call_once`、`get`、`is_completed` 等）を `spin` crate ドキュメントで確認

## 2. utils-core への OnceDriver trait + SpinOnce backend 新設

- [x] 2.1 `modules/utils-core/src/core/sync/once_driver.rs` を新規作成。`OnceDriver<T>` trait を `LockDriver<T>` / `RwLockDriver<T>` と同じ形式で定義（`new`/`call_once`/`get`/`is_completed` を要求）
- [x] 2.2 `modules/utils-core/src/core/sync/spin_once.rs` を新規作成。`SpinOnce<T>` 構造体を `spin::Once<T>` の thin wrapper として実装（`SpinSyncMutex<T>` と同じ形式、ファイル先頭の `mod tests;` 宣言と `unsafe impl Send`/`Sync` 含む）
- [x] 2.3 `SpinOnce<T>: OnceDriver<T>` を実装。`Send`/`Sync` 制約も `SpinSyncMutex` に合わせる（`T: Send + Sync` で `SpinOnce<T>: Send + Sync`）
- [x] 2.4 `spin_once.rs` に `#[cfg(test)] mod tests;` を添え、`call_once` の 1 度だけ実行、`get` の None → Some(...) 遷移、`is_completed` の状態遷移を単体テスト
- [x] 2.5 `modules/utils-core/src/core/sync.rs` に既存パターン（`mod` private + `pub use` 公開）に従って alphabetical 順序で挿入: `mod once_driver;`（`mod lock_driver_factory;` の後）、`#[allow(clippy::disallowed_types)] mod spin_once;`（`mod spin_sync_rwlock_factory;` の後、`spin_sync_mutex` と同じ allow 属性必須）、`pub use once_driver::OnceDriver;` および `pub use spin_once::SpinOnce;` を `pub use` ブロックの alphabetical 位置に追加

## 3. utils-core への SyncOnce abstraction 新設

- [x] 3.1 `modules/utils-core/src/core/sync/sync_once.rs` を新規作成。`SyncOnce<T>` 構造体を `SpinOnce<T>` を内部保持する形で定義（design Decision 3 通り、`ArcShared` 層は挟まない単段構成）
- [x] 3.2 公開 API: `const fn new() -> Self`、`call_once<F>(...)`、`get(...)`、`is_completed(...)`
- [x] 3.3 `SyncOnce<T>: Send + Sync` bound を `SpinOnce<T>` と同じ制約で継承
- [x] 3.4 `modules/utils-core/src/core/sync.rs` に既存パターンに従って `mod sync_once;`（alphabetical 位置: `mod std_sync_rwlock;` の後、`mod weak_shared;` の前）と `pub use sync_once::SyncOnce;` を `pub use` ブロックの alphabetical 位置に追加（`SyncOnce` は `spin::Once` を直接使わないため `#[allow(clippy::disallowed_types)]` は不要）
- [x] 3.5 `sync_once.rs` に `#[cfg(test)] mod tests;` を添え、`SyncOnce` 経由でも `call_once`/`get`/`is_completed` が期待動作することを確認
- [x] 3.6 `cargo build -p fraktor-utils-core-rs` で utils-core のビルド成功を確認
- [x] 3.7 `cargo test -p fraktor-utils-core-rs` で新規テスト含む全テスト pass を確認

## 4. actor-core の置換

- [x] 4.1 `modules/actor-core/src/core/kernel/system/coordinated_shutdown.rs:23` の `use spin::Once;` を `use fraktor_utils_core_rs::core::sync::SyncOnce;` に置換
- [x] 4.2 `coordinated_shutdown.rs` 内の `Once<...>` 型参照と利用を `SyncOnce<...>` に置換
- [x] 4.3 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs:10` の `use spin::Once;` を `use fraktor_utils_core_rs::core::sync::SyncOnce;` に置換
- [x] 4.4 `mailbox/base.rs` の 3 フィールド `instrumentation`、`invoker`、`actor` の型を `Once<T>` → `SyncOnce<T>` に置換
- [x] 4.5 `mailbox/base.rs` 内のドキュメントコメント（`spin::Once::get()` 言及 2 箇所、line 52 / 56）を `SyncOnce::get()` に修正
- [x] 4.6 `modules/actor-core/src/core/kernel/system/shared_factory/mailbox_shared_set.rs:11` のドキュメントコメント `spin::Once<T>` 言及も `SyncOnce<T>` に置換
- [x] 4.7 `modules/actor-core/` 全体のコメント / docstring / `use spin::` で残存する言及を最終 Grep で確認（漏れがあれば追加置換）
- [x] 4.8 `cargo fmt -p fraktor-actor-core-rs --check` で format 整合性を確認
- [x] 4.9 `cargo build -p fraktor-actor-core-rs` でビルド成功を確認

## 5. actor-core の spin 直接依存削除と clippy safety net 強化

- [x] 5.1 `modules/actor-core/Cargo.toml:35` の `spin = { workspace = true, default-features = false, features = ["mutex", "spin_mutex", "once"] }` 行を完全削除
- [x] 5.2 `modules/actor-core/clippy.toml` の `disallowed-types` に `spin::Once` エントリを追加（既存の `spin::Mutex` / `spin::RwLock` 同様の形式、`replacement = "fraktor_utils_core_rs::core::sync::SyncOnce"`）— safety net として将来の再発防止
- [x] 5.3 `cargo build -p fraktor-actor-core-rs --no-default-features` で no_std ビルド成功を確認
- [x] 5.4 `cargo build -p fraktor-actor-core-rs --features test-support` でビルド成功を確認
- [x] 5.5 `cargo tree -p fraktor-actor-core-rs --no-default-features --depth 1` で `spin` が direct dep に出現しないことを確認（utils-core 経由の transitive のみ残る）
- [x] 5.6 `cargo clippy -p fraktor-actor-core-rs --all-targets -- -D warnings` で新 clippy ルールと整合することを確認

## 6. テスト確認

- [x] 6.1 `cargo test -p fraktor-utils-core-rs` で utils-core テスト成功（新規 SpinOnce / SyncOnce テスト含む）
- [x] 6.2 `cargo test -p fraktor-actor-core-rs --lib` で actor-core lib テスト成功
- [x] 6.3 `cargo test -p fraktor-actor-core-rs --features test-support` で test-support 経由テスト成功
- [x] 6.4 `mailbox/base.rs` の `instrumentation`/`invoker`/`actor` 関連の動作テスト（既存 `mailbox/tests.rs` 等）が引き続き pass

## 7. 全体 CI 確認

- [x] 7.1 `./scripts/ci-check.sh ai all` を実行し、エラーがないことを確認（CLAUDE.md ルールに従い完了を待つ）
- [x] 7.2 失敗があれば原因を特定し、修正してから再実行する
- [x] 7.3 すべて green になったら、コミット・PR 作成の前にユーザー確認を取る

## 8. spec 整合確認

- [x] 8.1 `openspec validate step01-eliminate-actor-core-spin-direct-dep --strict` を実行し、artifact 整合を確認
- [x] 8.2 `SyncOnce` 経由の利用が spec delta（spin 固有 Scenario）と一致しているかを目視確認

## 9. ドキュメント更新

- [x] 9.1 `docs/plan/2026-04-21-actor-core-critical-section-followups.md` の残課題 4（`actor-core/Cargo.toml:35` `spin` 直接依存）を「解消済み」に更新
- [x] 9.2 hand-off メモに「他 actor-* クレートの `spin` 直接依存も同様の方針で別 change で対応」を追記（任意）

## 1. 事前調査

- [x] 1.1 `tick_feed.rs` の単体テスト位置を確認する（`tick_driver/tests.rs:79-` に `enqueue_from_isr_preserves_order_and_metrics` を確認。加えて `tick_feed/tests.rs` も発見、内容は public API のみ使用）
- [x] 1.2 `TickFeed` 構造体直接生成探索結果: `TickFeed { ... }` 直接生成は 0 件。`TickFeed::new(` は 8 箇所（system_state.rs:269, bootstrap.rs:53, tick_feed/tests.rs:13/28, tick_metrics_probe/tests.rs:14, tick_driver_bundle/tests.rs:10, scheduler_tick_executor/tests.rs:31, tick_driver/tests.rs:81）。すべて public API 経由のため追従修正不要
- [x] 1.3 `actor-core` 全体で `critical_section` の他の参照箇所なし（`tick_feed.rs:11, :157, :169` のみ）
- [x] 1.4 `enqueue_from_isr` caller は `tick_driver/tests.rs:83-84` のみ。production caller なし（design.md Decision 3 根拠維持）
- [x] 1.5 `compile-time-lock-backend/spec.md` 確認済み。`DefaultMutex` 利用要求で本 change と整合
- [x] 1.6 `actor-core/Cargo.toml` の `[dependencies]` 確認: **`spin = { workspace = true, ... }` が :36 で直接宣言されている**（本 change スコープ外、別 change で対応すべき hand-off 事項）。`parking_lot` は無し
- [x] 1.7 `portable-atomic` features 確認結果: `critical-section, default, disable-fiq, fallback, float, force-amo, require-cas, s-mode, serde, std, unsafe-assume-privileged, unsafe-assume-single-core`。`critical-section/std` impl を有効化する代替経路は **存在しない**。本 change は主選択 X で進行

## 2. tick_feed.rs の実装置換

- [x] 2.1 `use critical_section::Mutex;` を削除した
- [x] 2.2 `use core::cell::RefCell;` を削除した
- [x] 2.3 `use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};` を `ArcShared` import の隣に追加（`{ArcShared, DefaultMutex, SharedLock}` で alphabetical）
- [x] 2.4 `queue: SharedLock<VecDeque<u32>>` に変更
- [x] 2.5 `queue: SharedLock::new_with_driver::<DefaultMutex<_>>(queue)` に変更
- [x] 2.6 `try_push` を `self.queue.with_lock(|queue| ...)` に置換
- [x] 2.7 `pop_front` を `self.queue.with_lock(|queue| queue.pop_front())` に置換
- [x] 2.8 `cargo fmt --check` で差分なし（追加修正不要）
- [x] 2.9 `cargo build -p fraktor-actor-core-rs` 成功

## 3. テスト追従

- [x] 3.1 `tick_feed/tests.rs` は public API のみ使用で追従修正不要（task 1.1 で確認）
- [x] 3.2 `cargo test -p fraktor-actor-core-rs --lib tick_driver` 24 passed
- [x] 3.3 `cargo test -p fraktor-actor-core-rs --features test-support --tests --lib` 1846 passed, 3 ignored, 13 suites

## 4. Cargo.toml クリーンアップ

- [x] 4.1 `critical-section` を `optional = true` に変更
- [x] 4.2 `test-support = ["dep:critical-section", "critical-section/std"]` に変更
- [x] 4.3 `cargo build --no-default-features` 成功。`cargo tree --no-default-features --depth 1` で `critical-section` が direct dep に現れず、`portable-atomic v1.13.1` 経由の transitive のみ
- [x] 4.4 `cargo build --features test-support` 成功、`invalid feature` エラーなし

## 5. spec の整合確認

- [x] 5.1 `openspec validate drop-actor-core-critical-section-dep --strict` valid
- [x] 5.2 spec delta 視認: 2 要件（要件 A: primitive lock crate 直接 use 禁止、要件 B: Cargo.toml non-optional 直接依存禁止）が ADDED Requirements として merge 対象
- [x] 5.3 Scenario の WHEN/THEN/AND は機械的検証可能な形（grep ベースの lint で実装可能な記述）

## 6. 全体 CI 確認

- [x] 6.1 `./scripts/ci-check.sh ai all` 成功（EXIT=0、全テスト pass、全 lint pass）
- [x] 6.2 失敗なし
- [x] 6.3 コミット (182fd9d1)、PR #1605 作成、main へ merge 完了

## 7. ドキュメント更新（必要なら）

- [x] 7.1 `docs/plan/` に hand-off メモを作成: ① `test-support` 責務分離、② `portable-atomic` の `critical-section` feature 再評価、③ `enqueue_from_isr` API 名と実装意図の乖離、④ `actor-core/Cargo.toml:36` `spin` 直接依存の Requirement 2 違反

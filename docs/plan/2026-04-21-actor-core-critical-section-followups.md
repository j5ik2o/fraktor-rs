# actor-core critical-section 関連の hand-off 残課題

`drop-actor-core-critical-section-dep` change の実装で対象外とした残課題のメモ。

## 背景

`drop-actor-core-critical-section-dep` は「`actor-core` の production code から `critical-section` クレートへの直接利用を撤去」を目的とした change。`tick_feed.rs` の `critical_section::Mutex<RefCell<VecDeque<u32>>>` を `SharedLock + DefaultMutex` 抽象に置換し、`Cargo.toml` の `critical-section` 依存を `optional = true` 化、`test-support` feature を `["dep:critical-section", "critical-section/std"]` に更新した。

「源を絶つ」目標は達成したが、関連する残課題が複数発見された。

## 残課題

### 1. `test-support` feature の責務分離（解消済み）

**解消済み**: `step06-remove-actor-core-test-support-feature` change（2026-04-22）で feature 定義そのものを削除し、責務 A〜C + feature 削除すべて完了。

`actor-core/test-support` feature は当初 3 つの異なる責務を抱えていた:

- 責務 A: `critical-section/std` impl provider 提供（std 環境でリンクを通すため） → **完了** (`retire-actor-core-test-support-critical-section-impl` change で各バイナリ側へ移譲、2026-04-21)
- 責務 B: ダウンストリーム統合テスト用 API 公開（`TestTickDriver`, `new_empty` 等）
  - **B-1 完了** (`step03-move-test-tick-driver-to-adaptor-std`、2026-04-21): `TestTickDriver` と `new_empty*` の **公開 API** を `actor-adaptor-std` 側へ移設。`actor-core/test-support` feature の **公開 API には含まれなくなった**。inline test 用に `pub(crate)` 内部版が `tick_driver/tests/test_tick_driver.rs` と `base/tests.rs` / `typed/system/tests.rs` 内に残るが、これは外部から見えない。caller の使い分け: `actor-core` の inline test → 内部版、`actor-core` の integration test + 下流 crate → `actor-adaptor-std` 公開版。詳細は当該 change の design.md「実装後の補足」を参照
  - **B-2 完了** (`step05-hide-actor-core-internal-test-api`、2026-04-22): step04 (CLOSED) で当初想定していた mock/probe 等の test fixture は実在せず、`feature = "test-support"` 公開シンボル (`ActorRef::new_with_builtin_lock`、`SchedulerRunner::manual`、`state::booting_state`/`running_state` 等) はすべて actor-core 内部 inline test のみが caller と判明。step05 で全 11 シンボルを `pub(crate)` 化して feature ゲートを削除した
- 責務 C: 内部 API の `pub(crate)` → `pub` 格上げ（`Behavior::handle_message` 等）
  - **完了** (`step05-hide-actor-core-internal-test-api`、2026-04-22): `Behavior::handle_*` (3)、`TypedActorContext::from_untyped`、`TickDriverBootstrap` 関連 (struct/method/re-export) を `pub(crate)` に縮小。dual-cfg pattern (`#[cfg(any(test, feature = "test-support"))] pub fn` + `#[cfg(not(...))] pub(crate) fn`) を全廃
- feature 削除: **完了** (`step06-remove-actor-core-test-support-feature`、2026-04-22): `actor-core/Cargo.toml` から `test-support = []` 行と 8 個の `[[test]] required-features = ["test-support"]` を削除。下流 8 crate (`actor-adaptor-std`、`cluster-core`、`cluster-adaptor-std`、`persistence-core`、`remote-adaptor-std`、`stream-core`、`stream-adaptor-std`、`showcases/std`) の `Cargo.toml` から `fraktor-actor-core-rs/test-support` への参照も全廃。`actor-test-driver-placement` capability に検証 Scenario を追加し、再侵入を spec で機械的にブロック
- 下流 dead test-support 退役: **完了** (`step09-remove-dead-downstream-test-support-features`、2026-04-22): step06 archive 後の調査で `cluster-core/test-support`、`cluster-adaptor-std/test-support`、`remote-adaptor-std/test-support` が `src` 内 0 cfg gate の dead code と確定。3 件の feature 定義 + 3 件の参照 (showcases/std × 2、cluster-adaptor-std dev-dep × 1) を全廃。`actor-adaptor-std/test-support` のみ実用ゲート 4 箇所を持つため保持。`actor-test-driver-placement` capability に「下流 crate の test-support feature は実用ゲートを持つ場合のみ存在してよい」Scenario を追加し、空 / forward 専用の test-support 復活を spec で機械的に禁止

### 2. `portable-atomic` の `critical-section` feature 再評価（解消済み）

**解消済み**: `step07-evaluate-portable-atomic-critical-section-need` change（2026-04-22）で評価完了。結論は **案 Y (現状維持)**。

評価レポート: `docs/plan/2026-04-22-portable-atomic-critical-section-evaluation.md`

判定根拠:
- CI が `thumbv8m.main-none-eabi` (32-bit ARMv8-M Mainline、Cortex-M33 等) で `actor-core` を check している (`scripts/ci-check.sh:1056`)
- `thumbv8m.main` は AtomicU64 のハードウェアサポートを持たないため emulated atomic が必須
- production code 内に `portable_atomic::AtomicU64` 利用が **8 ファイル** ある (test 除く)
- 退役した瞬間に no-std CI ジョブが compile error で破綻する

対応:
- `actor-core/Cargo.toml:24` の `portable-atomic` 行に直前コメントで対象ターゲット (`thumbv8m.main`) と判定根拠 (8 ファイルの AtomicU64 利用) を記録、評価レポートへの参照を追加
- step08 (`step08-retire-portable-atomic-critical-section`) は中止 (本評価で「退役不可」が確定したため)

### 3. `enqueue_from_isr` API 名と実装意図の乖離（解消済み）

**解消済み**: `step02-align-enqueue-from-isr-implementation` change（2026-04-21）で選択肢 A（`enqueue_from_isr` 削除 → `enqueue` 一本化）を採用して対応完了。

対応内容:
- `TickFeed::enqueue_from_isr` public API を削除
- `tick_driver/tests.rs` の test 関数 `enqueue_from_isr_preserves_order_and_metrics` を `enqueue_tracks_driver_active_and_drop_metrics` にリネーム（既存 `tick_feed/tests.rs::enqueue_wakes_signal_and_preserves_order` とユニーク検証点を区別する命名）
- `actor-lock-construction-governance` spec に「ISR セーフに見せかけながら中身が通常ロック」の public API を禁止する Scenario を追加
- 将来 ISR セーフ経路が真に必要になった場合は、`DefaultMutex` の ISR セーフ feature variant（例: `irq-locks`）を伴う形で **新規 API として** 設計する方針（名前だけ先行した API を温存する方向は選ばない）

### 4. `actor-core/Cargo.toml:35` `spin` 直接依存の Requirement 2 違反（解消済み）

**解消済み**: `step01-eliminate-actor-core-spin-direct-dep` change（2026-04-21）で対応完了。

対応内容:
- `utils-core` に Once 系の 3 段構造（`OnceDriver<T>` trait、`SpinOnce<T>` backend、`SyncOnce<T>` 公開抽象）を新設（既存 `LockDriver` / `SpinSyncMutex` / `SharedLock` パターンと相似形）
- `actor-core` の `spin::Once` 利用 2 箇所（`coordinated_shutdown.rs`、`mailbox/base.rs`）を `SyncOnce` に置換
- `actor-core/Cargo.toml` から `spin` 直接依存行を完全削除
- `actor-core/clippy.toml` の `disallowed-types` に `spin::Once` を追加（safety net 強化）
- `actor-lock-construction-governance` spec に `spin` 固有 Scenario を追加して検査カバレッジを拡張

他 actor-\* クレート（`cluster-*`、`remote-*`、`stream-*`、`persistence-*`、`actor-adaptor-std`）の `spin` 直接依存調査は本 change のスコープ外。必要があれば同じ方針で別 change として対応する。

### 5. `dev-dependencies` の `critical-section` 直接宣言（解消済み）

`retire-actor-core-test-support-critical-section-impl` change（2026-04-21）により、impl provider 供給は各バイナリ（tests/benches/showcases）の責務に統一された。`actor-core/Cargo.toml` の `[dependencies]` から `critical-section` は完全削除済み。`[dev-dependencies]` に残る `critical-section = { workspace = true, features = ["std"] }` は、`actor-core` 自身の `cargo test` が impl を必要とするため引き続き必要（除去しない）。

## 参照

- 元 change: `openspec/changes/drop-actor-core-critical-section-dep/`
- design.md Decision 3: ISR セマンティクスの判断
- design.md Decision 6: optional 化の主選択 X
- 関連 spec: `actor-lock-construction-governance`、`compile-time-lock-backend`

# actor-core critical-section 関連の hand-off 残課題

`drop-actor-core-critical-section-dep` change の実装で対象外とした残課題のメモ。

## 背景

`drop-actor-core-critical-section-dep` は「`actor-core` の production code から `critical-section` クレートへの直接利用を撤去」を目的とした change。`tick_feed.rs` の `critical_section::Mutex<RefCell<VecDeque<u32>>>` を `SharedLock + DefaultMutex` 抽象に置換し、`Cargo.toml` の `critical-section` 依存を `optional = true` 化、`test-support` feature を `["dep:critical-section", "critical-section/std"]` に更新した。

「源を絶つ」目標は達成したが、関連する残課題が複数発見された。

## 残課題

### 1. `test-support` feature の責務分離

`actor-core/Cargo.toml:19` の `test-support` feature は当初 3 つの異なる責務を抱えていた:

- 責務 A: `critical-section/std` impl provider 提供（std 環境でリンクを通すため） → **完了** (`retire-actor-core-test-support-critical-section-impl` change で各バイナリ側へ移譲、2026-04-21)
- 責務 B: ダウンストリーム統合テスト用 API 公開（`TestTickDriver`, `new_empty` 等）
  - **B-1 完了** (`step03-move-test-tick-driver-to-adaptor-std`、2026-04-21): `TestTickDriver` と `new_empty*` の **公開 API** を `actor-adaptor-std` 側へ移設。`actor-core/test-support` feature の **公開 API には含まれなくなった**。inline test 用に `pub(crate)` 内部版が `tick_driver/tests/test_tick_driver.rs` と `base/tests.rs` / `typed/system/tests.rs` 内に残るが、これは外部から見えない。caller の使い分け: `actor-core` の inline test → 内部版、`actor-core` の integration test + 下流 crate → `actor-adaptor-std` 公開版。詳細は当該 change の design.md「実装後の補足」を参照
  - **B-2 未着手**: mock / probe / その他ヘルパ → step04 で対応
- 責務 C: 内部 API の `pub(crate)` → `pub` 格上げ（`Behavior::handle_message` 等） → 未着手

責務 A 退役後、`actor-core/test-support` は `[]`（空配列）になり、責務 B/C のための `#[cfg(any(test, feature = "test-support"))]` がコード側で利用される構造のみが残った。最終的に責務 B/C も退役できれば `test-support` feature 自体を削除できる。

### 2. `portable-atomic` の `critical-section` feature 再評価

`actor-core/Cargo.toml:26` の `portable-atomic = { features = ["critical-section"] }` は組み込み 32-bit ターゲット向けの `AtomicU64` fallback として有効化されている。実際に組み込み 32-bit ターゲットでビルド・運用される実績があるか不明な場合は、std/64-bit 系のみ想定として `core::sync::atomic` への置換を検討する余地あり。

ただし、`actor-core` の他 16 ファイルで `portable_atomic` が使われており、影響範囲は広い。

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

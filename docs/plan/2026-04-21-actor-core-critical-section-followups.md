# actor-core critical-section 関連の hand-off 残課題

`drop-actor-core-critical-section-dep` change の実装で対象外とした残課題のメモ。

## 背景

`drop-actor-core-critical-section-dep` は「`actor-core` の production code から `critical-section` クレートへの直接利用を撤去」を目的とした change。`tick_feed.rs` の `critical_section::Mutex<RefCell<VecDeque<u32>>>` を `SharedLock + DefaultMutex` 抽象に置換し、`Cargo.toml` の `critical-section` 依存を `optional = true` 化、`test-support` feature を `["dep:critical-section", "critical-section/std"]` に更新した。

「源を絶つ」目標は達成したが、関連する残課題が複数発見された。

## 残課題

### 1. `test-support` feature の責務分離

`actor-core/Cargo.toml:19` の `test-support` feature は当初 3 つの異なる責務を抱えていた:

- 責務 A: `critical-section/std` impl provider 提供（std 環境でリンクを通すため） → **完了** (`retire-actor-core-test-support-critical-section-impl` change で各バイナリ側へ移譲、2026-04-21)
- 責務 B: ダウンストリーム統合テスト用 API 公開（`TestTickDriver`, `new_empty` 等） → 未着手
- 責務 C: 内部 API の `pub(crate)` → `pub` 格上げ（`Behavior::handle_message` 等） → 未着手

責務 A 退役後、`actor-core/test-support` は `[]`（空配列）になり、責務 B/C のための `#[cfg(any(test, feature = "test-support"))]` がコード側で利用される構造のみが残った。最終的に責務 B/C も退役できれば `test-support` feature 自体を削除できる。

### 2. `portable-atomic` の `critical-section` feature 再評価

`actor-core/Cargo.toml:26` の `portable-atomic = { features = ["critical-section"] }` は組み込み 32-bit ターゲット向けの `AtomicU64` fallback として有効化されている。実際に組み込み 32-bit ターゲットでビルド・運用される実績があるか不明な場合は、std/64-bit 系のみ想定として `core::sync::atomic` への置換を検討する余地あり。

ただし、`actor-core` の他 16 ファイルで `portable_atomic` が使われており、影響範囲は広い。

### 3. `enqueue_from_isr` API 名と実装意図の乖離

`tick_feed.rs:88` の `pub fn enqueue_from_isr(&self, ticks: u32)` は API 名から ISR セーフティを示唆するが、実装は `try_push` を呼ぶだけで `enqueue` と完全に同じパス。本 change の起案時調査で production caller が存在しない（`tick_driver/tests.rs:83-84` のテストのみ）ことが判明した。

選択肢:
- A: `enqueue_from_isr` を削除し、`enqueue` に一本化
- B: ISR 専用の backend を `DefaultMutex` の feature variant（例: `irq-locks`）として追加し、`enqueue_from_isr` の実装を分岐
- C: API 名を維持し、ドキュメントで「実装上は `enqueue` と同じ」を明記

### 4. `actor-core/Cargo.toml:36` `spin` 直接依存の Requirement 2 違反

本 change で追加した `actor-lock-construction-governance` Requirement 2「`actor-*` の `Cargo.toml` は primitive lock crate を non-optional な直接依存として宣言してはならない」に対し、`actor-core/Cargo.toml:36` の `spin = { workspace = true, default-features = false, features = ["mutex", "spin_mutex", "once"] }` が違反している。

本 change のスコープは `critical-section` のみだったため、`spin` 直接依存の解消は別 change として対応する必要がある。検討事項:

- `spin` の利用箇所を `actor-core` 内で grep
- `utils-core` の `SpinSyncMutex` / `SpinSyncRwLock` 抽象に置き換え可能か検証
- 置き換え不能なケースがあれば、そのケースだけ allow-list 化

### 5. `dev-dependencies` の `critical-section` 直接宣言（解消済み）

`retire-actor-core-test-support-critical-section-impl` change（2026-04-21）により、impl provider 供給は各バイナリ（tests/benches/showcases）の責務に統一された。`actor-core/Cargo.toml` の `[dependencies]` から `critical-section` は完全削除済み。`[dev-dependencies]` に残る `critical-section = { workspace = true, features = ["std"] }` は、`actor-core` 自身の `cargo test` が impl を必要とするため引き続き必要（除去しない）。

## 参照

- 元 change: `openspec/changes/drop-actor-core-critical-section-dep/`
- design.md Decision 3: ISR セマンティクスの判断
- design.md Decision 6: optional 化の主選択 X
- 関連 spec: `actor-lock-construction-governance`、`compile-time-lock-backend`

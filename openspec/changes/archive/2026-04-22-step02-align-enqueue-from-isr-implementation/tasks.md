## 1. 事前調査と確認

- [x] 1.1 `modules/actor-core/src` 全体で `enqueue_from_isr` の残存言及を再 Grep（起案時は `tick_feed.rs:86` の定義 + `tests.rs:83-84` の呼び出し 2 箇所のみ想定）
- [x] 1.2 workspace 全体（`modules/` 配下の `src/` と `benches/`、`showcases/`、`scripts/`、`.github/`）で `enqueue_from_isr` を Grep し、production caller / CI allowlist が存在しないことを最終確認。workspace 直下には `benches/` ディレクトリは無いため対象外
- [x] 1.3 `tick_feed.rs` の `enqueue` と `enqueue_from_isr` の実装が依然同一であることを目視確認（前回調査から差分が入っていないか）
- [x] 1.4 `tests.rs:79-96` の `enqueue_from_isr_preserves_order_and_metrics` テストの期待値が `enqueue` 呼び出しでも同じ結果になることを確認（容量 1 → 2 回 push で 1 dropped、`driver_active` 立ち上がり、`signal.arm()` 通知）

## 2. tick_feed.rs からの API 削除

- [x] 2.1 `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tick_feed.rs` の `pub fn enqueue_from_isr(&self, ticks: u32) { ... }` method（line 85-92 付近、docstring 含む）を削除
- [x] 2.2 削除後、`enqueue` method が残り tick 受付 API として唯一の public 経路になっていることを確認
- [x] 2.3 `cargo fmt -p fraktor-actor-core-rs` で format 整形

## 3. テストの更新

- [x] 3.1 `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests.rs` の test 関数名 `enqueue_from_isr_preserves_order_and_metrics` を `enqueue_tracks_driver_active_and_drop_metrics` にリネーム（design Decision 3 通り、既存 `tick_feed/tests.rs::enqueue_wakes_signal_and_preserves_order` と役割を区別する命名）
- [x] 3.2 同関数内の `feed.enqueue_from_isr(1);` 呼び出し 2 箇所を `feed.enqueue(1);` に置換
- [x] 3.3 テスト本体の assertion（`drained`, `driver_active`, `metrics.enqueued_total()`, `metrics.dropped_total()`, `signal.arm()`）が変更不要であることを確認
- [x] 3.4 その他の test 関数が `enqueue_from_isr` を間接参照していないか再 Grep

## 4. 残存言及の整理

- [x] 4.1 `modules/actor-core/` 配下の docstring / コメント / ドキュメントで `enqueue_from_isr` 言及が残っていないか最終 Grep（削除対象 API は本 change で完全消去）
- [x] 4.2 CI allowlist / `.github/workflows/**.yml` / `scripts/*.sh` に `enqueue_from_isr` を指定する test filter が無いことを Grep で最終確認（tasks 1.2 の補足）
- [x] 4.3 `docs/` 配下で `enqueue_from_isr` に言及する箇所があれば、履歴的文脈（hand-off メモ / archive 済み change）以外は更新 or 削除判断

## 5. ビルド・テスト検証

- [x] 5.1 `cargo build -p fraktor-actor-core-rs --no-default-features` で no_std ビルド成功を確認
- [x] 5.2 `cargo build -p fraktor-actor-core-rs --features test-support` で test-support ビルド成功を確認
- [x] 5.3 `cargo test -p fraktor-actor-core-rs --lib` で lib テスト成功（リネーム後のテスト含む）
- [x] 5.4 `cargo test -p fraktor-actor-core-rs --features test-support` で統合テスト成功
- [x] 5.5 `cargo clippy -p fraktor-actor-core-rs --lib` で clippy clean を確認

## 6. spec 整合確認

- [x] 6.1 `openspec validate step02-align-enqueue-from-isr-implementation --strict` を実行し artifact 整合を確認
- [x] 6.2 追加した Scenario「actor-\* は ISR セーフに見せかけた通常ロック API を公開しない」が本 change 後の状態（`enqueue_from_isr` 削除済み）と一致していることを目視確認

## 7. 全体 CI 確認

- [x] 7.1 `./scripts/ci-check.sh ai all` を実行しエラーがないことを確認（CLAUDE.md ルールに従い完了を待つ）
- [x] 7.2 失敗があれば原因を特定し、修正してから再実行する
- [x] 7.3 すべて green になったら、コミット・PR 作成の前にユーザー確認を取る

## 8. ドキュメント更新

- [x] 8.1 `docs/plan/2026-04-21-actor-core-critical-section-followups.md` の残課題 3（`enqueue_from_isr` API 名と実装意図の乖離）を「解消済み（選択肢 A で削除一本化、step02 で対応）」に更新
- [x] 8.2 hand-off メモに「将来 ISR セーフ経路が必要になった場合は、`DefaultMutex` の ISR セーフ feature variant と合わせて新規 API として設計する」方針を追記

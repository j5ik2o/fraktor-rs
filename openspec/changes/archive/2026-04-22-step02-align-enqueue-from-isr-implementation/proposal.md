## Why

`modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tick_feed.rs:88` の `pub fn enqueue_from_isr(&self, ticks: u32)` は API 名から ISR（割り込みハンドラ）セーフティを示唆するが、実装は `try_push` を呼ぶだけで通常の `enqueue` と完全に同じパスを通る。production caller は存在せず、利用箇所は `tick_driver/tests.rs:83-84` のテストのみ。

「名前が実装意図から乖離している」状態は、将来の読者や新規 integrator を誤解させる。特に embedded 系で ISR セーフな API を期待して採用された場合、実装は通常のロックを取る `SharedLock` 経由であり、期待を満たさない可能性がある。本残課題は `drop-actor-core-critical-section-dep` change の hand-off メモ（`docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 3）として記録されている。

## What Changes

本 change は上記残課題の選択肢 A〜C のうち、**選択肢 A（`enqueue_from_isr` を削除し `enqueue` に一本化）** を採用する（詳細根拠は design フェーズで確定）。

- **BREAKING（workspace-internal）**: `TickFeed::enqueue_from_isr(&self, ticks: u32)` public API を削除
- `tick_driver/tests.rs:83-84` の `enqueue_from_isr` 呼び出しを `enqueue` に置換
- 他の caller が workspace 内で存在しないことを `Grep` で全数確認（ゼロ件である想定）
- `docs/plan/2026-04-21-actor-core-critical-section-followups.md` の残課題 3 を「解消済み」に更新

**Non-Goals**:
- 選択肢 B（ISR 専用 backend を `DefaultMutex` の feature variant として追加）の追求（YAGNI。実需要が顕在化してから対応）
- 選択肢 C（API 名維持 + ドキュメント注記）の追求（実装乖離を温存するため却下）
- `tick_feed.rs` の他部分（`SharedLock` 利用パターン、`peek`/`drain` API）の見直し

## Capabilities

### New Capabilities
- なし

### Modified Capabilities
- なし（`actor-lock-construction-governance` および `compile-time-lock-backend` spec は本 change の影響を受けない。`TickFeed` の public API は spec 化されていない workspace-internal の実装詳細）

ただし OpenSpec validation 要件により最低 1 件の delta が必要なため、design / specs フェーズで適切な capability（例: `actor-runtime-public-api-hygiene` のような新規 capability、または既存 spec への Scenario 追加）を設計する。スコープが大きくなる場合は本 change を「実装のみ」にとどめ、capability spec 化は別 change として切り分けることを検討する。

## Impact

- **Affected code**:
  - `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tick_feed.rs`（public API 削除）
  - `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests.rs`（呼び出し箇所差し替え）
- **Affected dependencies**: なし
- **Affected systems**: `actor-core` 外部に露出する危険性のある紛らわしい API が消える（API surface の健全化）
- **Release impact**: pre-release phase につき外部影響は軽微。`fraktor-actor-core-rs` の minor bump で十分

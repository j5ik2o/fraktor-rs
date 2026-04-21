## Context

`modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tick_feed.rs:86` の `pub fn enqueue_from_isr(&self, ticks: u32)` は API 名から ISR（割り込みハンドラ）セーフな経路であることを示唆するが、実装は以下のとおり `enqueue` と完全に同一:

```rust
// 現状 (tick_feed.rs:77-92)
pub fn enqueue(&self, ticks: u32) {
  if ticks == 0 { return; }
  let pushed = self.try_push(ticks);
  self.finalize_enqueue(pushed, ticks);
}

pub fn enqueue_from_isr(&self, ticks: u32) {
  if ticks == 0 { return; }
  let pushed = self.try_push(ticks);
  self.finalize_enqueue(pushed, ticks);
}
```

- 内部で使っている `try_push` は `self.queue: SharedLock<VecDeque<u32>>` に対し `with_lock` で通常のロックを取る → ISR 内で呼んだら lock acquisition でデッドロック/優先度逆転のリスクを内在する（現状の実装は ISR セーフではない）
- `enqueue_from_isr` の workspace 内 caller は `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests.rs:83-84` のみ（前回 change `drop-actor-core-critical-section-dep` の起案時調査で確認済み、本 change 起案時 Grep でも再確認）
- 該当テスト関数名は `enqueue_from_isr_preserves_order_and_metrics`（`tests.rs:79`）

proposal は選択肢 A（削除して `enqueue` に一本化）を採用する方針を示しているが、代替案（B: ISR 専用 backend 追加 / C: ドキュメント注記のみ）との比較と最終決定は本 design で確定する。

## Goals / Non-Goals

**Goals:**
- `TickFeed::enqueue_from_isr` public API を完全に削除し、API 名と実装の乖離を解消する
- 既存テスト `enqueue_from_isr_preserves_order_and_metrics` を `enqueue` 呼び出しに差し替え、適切なテスト名にリネームする
- `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 3 を解消済みに更新する
- OpenSpec validation 要件を満たす最小限の spec delta を追加する

**Non-Goals:**
- 選択肢 B（ISR 専用 `DefaultMutex` feature variant 追加）の設計・実装（実需要が顕在化してから別 change で対応）
- 選択肢 C（API 名維持 + ドキュメント注記）の追求（実装乖離を温存するため却下）
- `tick_feed.rs` の他部分（`SharedLock` 利用パターン、`drain_pending` / `snapshot` / `driver_active` 等）の見直し
- 他クレートにおける「API 名と実装意図の乖離」全般の棚卸し

## Decisions

### Decision 1: 選択肢 A（`enqueue_from_isr` の完全削除）を採用する

**選択**: `TickFeed::enqueue_from_isr` を完全に削除し、`enqueue` のみを残す。

**根拠**:
- 現状の `enqueue_from_isr` は実装上 `enqueue` と完全同一であり、名前だけが誤解を招く状態
- workspace 内の production caller ゼロ、caller はテストのみ。削除の影響範囲は最小
- pre-release phase につき外部依存者からの breaking 影響は極小
- API surface を小さく保つこと自体が YAGNI 原則に整合
- 将来 ISR セーフな経路が本当に必要になった時点で、適切なセマンティクスを持つ API として新設する方が健全（名前だけ先行した API を温存するより設計判断が明確になる）

**代替案と却下理由**:
- 案 B（ISR 専用 backend を `DefaultMutex` の feature variant として追加し、`enqueue_from_isr` を ISR セーフに実装する）:
  - 現在の workspace 内に ISR から呼び出される production caller がゼロ。実需要ゼロの機能を先行実装するのは YAGNI に反する
  - ISR セーフな lock は `critical_section::with` または割り込み禁止を伴う専用 backend を要し、`SharedLock` 抽象の拡張範囲が大きい
  - 組み込み 32-bit ターゲットの運用実績自体が未確認（step07 で評価予定）。この基盤評価より先に ISR 経路を作るのは順序として不適切
- 案 C（API 名維持 + ドキュメントで「実装上は enqueue と同じ」を明記）:
  - 「名前 is 嘘」状態を温存する。新規 integrator が誤解して ISR から呼ぶ危険が残る
  - lint や型レベルで誤用を防ぐ手段がない
  - pre-release phase で誤解容認を正当化する根拠に乏しい

### Decision 2: spec delta は既存 Requirement への Scenario 追加で最小化する

**選択**: `actor-lock-construction-governance` spec の既存 Requirement「actor-\* の production code は primitive lock crate を直接 use してはならない」に、**ISR セーフな経路に見せかける API を production で持たない** という趣旨の Scenario を ADDED（MODIFIED Requirement 形式）で追加する。

**根拠**:
- 本 change は根底では「governance spec の精神（ISR / lock 経路の誤解を招く API を置かない）」の適用
- `actor-lock-construction-governance` Requirement 1（primitive lock crate 直接 use 禁止）と同じ Requirement 群に属する beacon として Scenario 化するのが自然
- OpenSpec validation 要件（最低 1 件の delta）を満たしつつ、新規 capability 作成の重厚さを避ける
- 将来他クレートで類似の「ISR セーフに見せかけて中身は通常ロック」という API が出た場合に、spec の Scenario で捕捉できる

**代替案と却下理由**:
- 案 A: 新規 capability `actor-runtime-public-api-hygiene` を作成 → YAGNI、本 change の範囲に対して重い
- 案 B: spec delta 無し → OpenSpec strict validation を通らない

### Decision 3: 既存テストはユニークな検証点を反映した名前にリネーム + 呼び出し差し替え

**選択**: `enqueue_from_isr_preserves_order_and_metrics`（`tests.rs:79`）を `enqueue_tracks_driver_active_and_drop_metrics` にリネームし、関数内の `feed.enqueue_from_isr(1)` を `feed.enqueue(1)` に置換する。テスト本体の期待値（queue 容量 1 に 2 回 enqueue → 1 件 dropped）は `enqueue` でも同じ挙動なので変更不要。

**根拠**:
- 既に `tick_feed/tests.rs:11` に `enqueue_wakes_signal_and_preserves_order`、`tick_feed/tests.rs:25` に `snapshot_reports_dropped_ticks` がある。単純に `enqueue_preserves_order_and_metrics` とすると既存テストと役割が混同する
- 当該テストのユニークな検証点は以下 3 つ:
  - `driver_active` の false → true 立ち上がり（他テストは検証していない）
  - `metrics.enqueued_total()` と `metrics.dropped_total()` の具体値同時検証
  - `signal.arm()` の pending flag 消費セマンティクス
- この 3 点を端的に示す名前 `enqueue_tracks_driver_active_and_drop_metrics` を採用
- テストが検証している不変条件は API 名に依存しない
- テストを削除するとカバレッジを失う。リネームして残すのが最小差分
- 現状すでに `enqueue_from_isr` 内部は `enqueue` と同じなので、置換しても assertion はそのまま通る想定

**代替案と却下理由**:
- 案 A: テスト削除 → カバレッジ損失（`driver_active` 検証が消える）、本 change の意図は API 削除であってテスト削除ではない
- 案 B: 新規テスト追加 + 既存テスト削除 → 冗長
- 案 C: `enqueue_preserves_order_and_metrics` にリネーム → 既存 `enqueue_wakes_signal_and_preserves_order` と命名空間的に近すぎ、ユニーク検証点が名前から読み取れない

## Risks / Trade-offs

- **[Risk] workspace 外の外部 crate が `TickFeed::enqueue_from_isr` を利用していた場合の breaking** → Mitigation: pre-release phase であり CLAUDE.md で「後方互換不要」方針が確立済み。リリースノート相当のコミットメッセージで API 削除を明記する
- **[Risk] 将来 ISR セーフ経路が本当に必要になった時の再導入コスト** → Mitigation: 再導入時は新しい API 名（例: `enqueue_from_isr_safe` や `isr::enqueue` など別モジュール）で、ISR セーフな backend を伴う形で設計できる。今回の削除は「名前だけ先行して中身空」の後ろ向き負債を解消するもの
- **[Risk] テストのリネームで CI の test filter / allowlist が壊れる** → Mitigation: workspace 内で `enqueue_from_isr_preserves_order_and_metrics` を参照する `.github/workflows/`、`scripts/` 等を Grep で確認（tasks 1.2 / 4.2 に含める）
- **[Trade-off] spec への Scenario 追加は「将来の違反捕捉」を目的としており、本 change 自体の検証は CI の通過に依存** → 受容: 設計ガバナンスとしての記述価値があれば十分

## Migration Plan

1. **Phase 1**: `tick_feed.rs` の `enqueue_from_isr` method 定義（line 85-92 付近）を削除
2. **Phase 2**: `tick_driver/tests.rs` の `enqueue_from_isr_preserves_order_and_metrics` を `enqueue_tracks_driver_active_and_drop_metrics` にリネーム（Decision 3 通り）し、`feed.enqueue_from_isr(1)` を `feed.enqueue(1)` に置換
3. **Phase 3**: workspace 全体で `enqueue_from_isr` の残存言及を Grep（production / tests / docs / CI 設定すべて）
4. **Phase 4**: `cargo build -p fraktor-actor-core-rs` と `cargo test -p fraktor-actor-core-rs --features test-support` で動作確認
5. **Phase 5**: 既存の spec delta（`specs/actor-lock-construction-governance/spec.md`、本 change 起案時に生成済み）と実装の整合を最終確認し、`openspec validate --strict` を通す
6. **Phase 6**: `./scripts/ci-check.sh ai all` で workspace 全体確認
7. **Phase 7**: `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 3 を解消済みに更新

ロールバックは git revert で完結する。削除した method は再導入も容易（Git 履歴から復元可能）。

## Open Questions

- なし（必要な設計判断は本 design で確定済み）

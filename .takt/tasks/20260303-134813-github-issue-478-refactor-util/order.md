## GitHub Issue #478: refactor: utils の dead_line_timer/time 未使用型 + queue Async系削除

Parent issue: #410

## 目的

`modules/utils/` から他モジュール未参照の `timing/dead_line_timer/`、`time/` 未使用型、`queue/` Async系型を削除する。

## 背景

- `timing/dead_line_timer/` 全5型: `DeadLineTimer`, `DeadLineTimerError` 等 — 他モジュールから参照ゼロ
- `time/` 未使用型 5型: `ClockKind`, `DriftMonitor`, `DriftStatus`, `TickEvent`, `TimerEntryMode` 等
- `queue/` Async系 ~12型: `AsyncFifoQueue`, `AsyncMpscQueue`, `AsyncSpscQueue`, `AsyncPriorityQueue`, `BinaryHeapBackend` 等 — 他モジュールから参照ゼロ

## タスク

- [ ] `modules/utils/src/core/timing/dead_line_timer/` を削除
- [ ] `modules/utils/src/core/time/` の未使用型を削除
- [ ] `modules/utils/src/core/collections/queue/` の Async系型を削除
- [ ] 関連する `mod.rs` のリエクスポート・モジュール宣言を除去
- [ ] 関連テストファイルを削除
- [ ] `./scripts/ci-check.sh all` でグリーン確認

## 受け入れ基準

- 対象の未使用型が `pub` エクスポートから消えていること
- CI が全パスすること
- 他モジュールのコンパイル・テストに影響がないこと

## 推定変更ファイル数

~46ファイル

### Labels
refactoring
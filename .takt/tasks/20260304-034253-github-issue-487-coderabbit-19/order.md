## GitHub Issue #487: CodeRabbit 指摘事項の棚卸し（19件）

## 概要

CodeRabbit が過去の PR レビューで検出した未対応の指摘事項をまとめたトラッキング issue です。

---

## Major（5件）

- [ ] #468 — 手動 `Unpin` 実装の削除（`handle_shared.rs`）
- [ ] #459 — `inner()` 公開で `SharedAccess` の封じ込めが崩壊（`grain_metrics_shared.rs`）
- [ ] #457 — ロック保持中に `send_system_message` を実行（`context_pipe_waker.rs`）
- [ ] #429 — 再起動境界で `last_topology_hash` が残留（`cluster_core.rs`）
- [ ] #419 — 公開コンストラクタでの panic（`cluster_router_group_settings.rs`）

## Minor / Unknown（5件）

- [ ] #458 — 未使用 import `SyncRwLockLike`（`event_stream_shared.rs`）
- [ ] #447 — `unsafe impl Send/Sync` の境界条件未明示（`dispatch_executor_runner.rs`）
- [ ] #438 — 送信 window eviction のサイレントドロップ
- [ ] #423 — `OutOfWindow` 時のフロー制御フィードバック欠落（`bridge.rs`）
- [ ] #417 — テスト名に対して検証が不足

## Trivial（9件）

- [ ] #461 — `shared_mutex` ヘルパーの重複
- [ ] #460 — Mutex 生成パターンの一貫性（スタイル）
- [ ] #449 — `pub inner` フィールドの公開範囲
- [ ] #446 — ロック粒度（将来の競合ポイント）
- [ ] #435 — 未使用戦略での状態常時確保
- [ ] #434 — ロック保持中のハンドラ実行
- [ ] #430 — import の crate ルート基準統一
- [ ] #424 — テストヘルパーの重複
- [ ] #416 — ハンドラ検索が線形 O(n)

---

## 対応方針

- **Major**: 優先的に対応
- **Minor/Unknown**: Major 対応後に順次
- **Trivial**: リファクタリング機会に併せて対応

### Labels
bug
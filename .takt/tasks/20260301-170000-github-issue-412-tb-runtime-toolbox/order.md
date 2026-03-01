## Phase A レビュー: RuntimeMutex/RuntimeRwLock 導入（PR #441）

## 背景
GitHub Issue #412 の Phase A として、PR #441 で以下の変更が実装されマージ済みである。
しかし takt のレビューサイクル（ai_review → reviewers → supervise）が未完了のまま止まっていた。
本タスクは既存の変更に対するレビューのみを行い、Phase A の品質を確認する。

## レビュー対象の変更内容（PR #441、既にマージ済み）
- `RuntimeMutex<T>` / `RuntimeRwLock<T>` 型エイリアスを feature flag（`#[cfg(feature = "std")]`）で定義
- `SyncMutexFamily` / `SyncRwLockFamily` の Family パターン（trait ベースの抽象化）を廃止
- `ToolboxMutex<T, TB>` / `ToolboxRwLock<T, TB>` を `RuntimeMutex<T>` / `RuntimeRwLock<T>` に置換
- 主な変更箇所: `modules/utils/src/core/runtime_toolbox.rs`, `modules/utils/src/lib.rs`

## 実施内容
- **新規実装は不要**（既にマージ済み）
- レビューで問題が見つかった場合のみ修正を行う
- Phase B〜D（TB パラメータ除去、Generic サフィックス廃止、RuntimeToolbox trait 廃止）は別 Issue（#442, #443, #444）で対応するため、本タスクのスコープ外

## 完了条件
- ai_review + arch-review + qa-review がすべて approved
- 修正があった場合は `./scripts/ci-check.sh all` がパス
- 修正がなかった場合は既存の CI 状態が green であることの確認で可

# stream-island-actors 実装計画 6.6 / 6.10 / 6.11

## 対象変更
- change: `stream-island-actors`
- tasks_path: `openspec/changes/stream-island-actors/tasks.md`

## 今回のバッチ
- 6.6 actor 分離後も downstream island が busy loop しないことを regression test で固定する。
- 6.10 `IslandBoundaryShared` が actor 越境の並行アクセス下でも要素ロス・二重配送・不整合 terminal state を起こさないことを compile-time / stress test で固定する。
- 6.11 `cancel` / `shutdown` / `abort` の 3 経路について、in-flight 要素の扱いを matrix test で固定する。

## 実装方針
- 既に追加済みの 6.6 / 6.10 テストは維持し、実装変更は 6.11 の失敗契約に絞る。
- downstream cancellation は失敗ではなく cancellation として扱い、boundary detached による upstream failure を避ける。
- graph-wide shutdown は downstream boundary source を cancel せず、in-flight boundary elements を drain できるようにする。
- abort は既存どおり failure priority を維持する。

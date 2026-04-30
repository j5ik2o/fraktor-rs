# stream-island-actors 実装計画 4.6

## 対象変更
- change: stream-island-actors
- tasks_path: `openspec/changes/stream-island-actors/tasks.md`

## 今回のバッチ
- 4.6: `Drive` が coalescing され、1 island actor に未処理 `Drive` が複数積み上がらないことを unit / integration test で固定する。

## 実装方針
- island actor ごとに `Drive` 用の coalescing state を持たせる。
- scheduler tick は `Drive` を直接無条件送信せず、gate を取得できた場合だけ対象 actor に `Drive` を送る。
- `Drive` 処理完了後、terminal stream の early return、送信失敗時はいずれも gate を idle に戻す。
- scheduler callback から `stream.drive()` は直接呼ばず、既存どおり island actor の mailbox 内で実行する。
- `Cancel` / `Shutdown` / `Abort` の意味は変更しない。

## 検証
- `rtk cargo test -p fraktor-stream-core-rs stream_island`
- `rtk cargo test -p fraktor-stream-core-rs actor_materializer`
- `rtk git diff --check`

`./scripts/ci-check.sh ai all` は TAKT の `final-ci` ムーブメント以外では実行しない。

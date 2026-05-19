# stream-island-actors write_tests 計画

## 対象

`stream-island-actors` change の 2.1 から 2.4 を対象に、プロダクションコード実装前の先行テストだけを追加する。

## テスト方針

- `StreamIslandCommand` は `Drive`、`Cancel { cause: Option<StreamError> }`、`Shutdown`、`Abort(StreamError)` を構築できることを固定する。
- `StreamIslandActor` は 1 つの `StreamShared` を受け取り、`Drive` command を `receive` 経由で処理したときに `stream.drive()` が進むことを固定する。
- terminal state の `StreamShared` に対して `Drive` command を受けても、追加の pull が発生しないことを固定する。
- `Cancel` と `Shutdown` は対象 stream を cancel へ遷移させることを固定する。
- `Abort` は command としてエラーを保持することを先に固定し、graph-wide failure 伝播は後続タスクの範囲に残す。

## 対象ファイル

- `modules/stream-core/src/core/impl/materialization/stream_island_actor/tests.rs`
- `modules/stream-core/src/core/impl/materialization/stream_island_command/tests.rs`

## 範囲外

- `StreamIslandActor` / `StreamIslandCommand` の本体作成
- `materialization.rs` の module wiring
- `ActorMaterializer` から island actor を spawn する配線
- `StreamDriveActor` の削除
- `./scripts/ci-check.sh ai all` の実行

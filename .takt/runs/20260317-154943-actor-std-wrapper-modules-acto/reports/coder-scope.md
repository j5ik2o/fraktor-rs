# 実装スコープ

## 対象
- `modules/actor/src/std.rs`
- `modules/actor/src/std/actor.rs`
- `modules/actor/src/std/dispatch.rs`
- `modules/actor/src/std/dispatch/dispatcher.rs`
- `modules/actor/src/std/event.rs`
- `modules/actor/src/std/event/logging.rs`
- `modules/actor/src/std/event/stream.rs`
- `modules/actor/src/std/props.rs`
- `modules/actor/src/std/scheduler.rs`
- `modules/actor/src/std/system.rs`
- `modules/actor/src/std/typed.rs`
- `modules/actor/src/std/typed/actor.rs`
- `modules/actor/src/std/tests.rs`

## 非対象
- なし

## スコープ判断の理由
- order.md で指定された11件の wrapper ファイル削除と std.rs への再エクスポート集約が全て既に完了済みであり、テストによる削除固定も実装済み
- ビルド成功・テスト2件全パスを確認し、追加のコード変更は不要と判断
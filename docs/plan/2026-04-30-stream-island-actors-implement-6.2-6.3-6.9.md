# stream-island-actors 実装計画 6.2 / 6.3 / 6.9

## 対象

- change: `stream-island-actors`
- tasks: `openspec/changes/stream-island-actors/tasks.md`
- 今回のバッチ: 6.2 / 6.3 / 6.9

## 方針

- downstream cancellation を upstream completion と区別できる boundary state として表現する。
- downstream cancellation は boundary state だけで完結させず、materialized graph 側の control plane から upstream island actor へ `StreamIslandCommand::Cancel { cause }` を配送する。
- `BoundarySourceLogic::on_cancel()` は local cleanup に限定し、upstream lifecycle command の唯一の伝播経路にしない。
- 既存の shutdown / abort / kill switch の graph-wide lifecycle とは混同しない。

## 検証

- boundary unit test で `DownstreamCancelled` state を固定する。
- actor materializer integration test で downstream cancel が upstream island actor の `Cancel` command として観測できることを固定する。
- 実装後に対象テストと formatting / diff check を実行する。

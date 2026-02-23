# fraktor-rs streams モジュール知識

## プロジェクト概要

fraktor-rs は Akka/Pekko と protoactor-go のパターンを Rust に移植したアクターランタイム。
`no_std` と `std/Tokio` の双方で同一 API を提供する二層構造。

## streams モジュール構造

```
modules/streams/src/
├── core/                          # no_std 実装
│   ├── stage/                     # コアDSL型
│   │   ├── source/                # Source<Out, Mat>
│   │   ├── flow/                  # Flow<In, Out, Mat>
│   │   ├── sink/                  # Sink<In, Mat>
│   │   ├── bidi_flow/             # BidiFlow<InTop, OutTop, InBottom, OutBottom>
│   │   ├── source_sub_flow/       # SubFlowソース
│   │   ├── flow_sub_flow/         # SubFlowフロー
│   │   ├── stage_context.rs       # StageContext trait
│   │   └── stream_stage.rs        # StreamStage trait
│   ├── graph/                     # Graph DSL, GraphStage
│   ├── shape/                     # シェイプ型（Inlet, Outlet, SourceShape等）
│   ├── lifecycle/                 # KillSwitch, StreamHandle, StreamState
│   ├── hub/                       # MergeHub, BroadcastHub, PartitionHub
│   ├── mat/                       # Materializer, ActorMaterializer
│   ├── mat_combine_rule/          # MatCombineRule trait
│   ├── testing/                   # TestSourceProbe, TestSinkProbe
│   └── ...                        # StreamError, StreamNotUsed, Keep*, etc.
└── std/                           # std依存の拡張
```

## 参照実装の場所

| 実装 | パス |
|------|------|
| Apache Pekko Streams | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/` |
| Pekko FlowOps | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/scaladsl/FlowOps.scala` |
| Pekko BidiFlow | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/scaladsl/BidiFlow.scala` |
| Pekko Hub | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/scaladsl/Hub.scala` |

## Phase 2 ギャップ一覧

1. ~~`distinct` / `distinctBy`~~ — 実装済み
2. ~~`drop_within`~~ — 実装済み
3. ~~`BidiFlow::fromFunction/fromFunctions`~~ — 実装済み
4. ~~`BidiFlow::atop/join`~~ — 実装済み
5. ~~`DrainingControl`~~ — 実装済み
6. ~~`alsoToMat` / `wireTapMat`~~ — 実装済み
7. ~~`futureFlow` / `lazyFutureFlow`~~ — 実装済み
8. ~~`BidiFlow` に `Mat` 型パラメータ追加~~ — 実装済み

## Phase 2 追加ギャップ（未実装）

gap-analysis セクション5「完全未実装ギャップ」より:

- `monitorMat` — FlowMonitorとのMat合成。既存の `monitor` メソッド（`flow.rs`）を拡張し、`MatCombineRule` でMat値を合成するバージョンを追加。`also_to_mat` / `wire_tap_mat` の実装パターンを参考にする。
- `mapMaterializedValue` — Source, Flow, Sink のマテリアライゼーション値を変換するメソッド。Pekko `FlowOps.scala` の `mapMaterializedValue[Mat2](f: Mat => Mat2)` に対応。内部的には `into_parts` → クロージャ適用 → 再構築で実装可能。

## 既存パターン

- tick ベース同期実行モデル（`process_tick` で1要素ずつ処理）
- `StreamNotUsed` = Mat のデフォルト型
- `MatCombineRule` trait でマテリアライゼーション合成
- テストは `modules/streams/src/core/stage/{type}/tests.rs`
- `CLAUDE.md` と `.claude/rules/rust/` のルールに必ず従うこと

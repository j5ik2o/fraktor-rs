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
│   │   ├── graph_dsl/             # GraphDsl<In,Out,Mat>
│   │   ├── graph_interpreter/     # GraphInterpreter
│   │   ├── graph_stage.rs         # GraphStage trait
│   │   └── graph_stage_logic.rs   # GraphStageLogic trait
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
| Pekko Source | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala` |
| Pekko Flow | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala` |
| Pekko Sink | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/scaladsl/Sink.scala` |
| Pekko BidiFlow | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/scaladsl/BidiFlow.scala` |
| Pekko KillSwitch | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/KillSwitch.scala` |
| Pekko Hub | `references/pekko/pekko-stream/src/main/scala/org/apache/pekko/stream/scaladsl/Hub.scala` |
| ギャップ分析 | `docs/gap-analysis/streams-gap-analysis.md` |

## 実行モデルの違い（重要）

fraktor-rs は **tick ベースの同期実行モデル** を採用：
- Pekko の `FiniteDuration` → fraktor-rs の `ticks: usize` パラメータ
- レート差を前提とするオペレーター（conflate, expand）→ no-op/identity
- 時間ベース操作（debounce, sample）→ tick ベースに再設計が必要

## Phase 2 ギャップ一覧（easy: 単純な新規実装）

| # | 項目 | 概要 | Pekko参照 |
|---|------|------|-----------|
| 1 | `distinct` / `distinctBy` | HashSetベースの重複排除フィルタ | `FlowOps.scala` |
| 2 | `drop_within` | `take_within` の逆（時間内要素スキップ） | `FlowOps.scala` |
| 3 | `BidiFlow::fromFunction/fromFunctions` | 関数からBidiFlow構築 | `BidiFlow.scala` |
| 4 | `BidiFlow::atop/join` | BidiFlow合成・結合 | `BidiFlow.scala` |
| 5 | `DrainingControl` | Hub用ドレイン制御 | `Hub.scala` |
| 6 | `alsoToMat` / `wireTapMat` | Mat合成のサイドチャネル | `FlowOps.scala` |
| 7 | `futureFlow` / `lazyFutureFlow` | Future-based Flowファクトリ | `Flow.scala` |
| 8 | `BidiFlow` に `Mat` 型パラメータ追加 | 型パラメータ拡張 | `BidiFlow.scala` |

## 既存パターン（実装時に準拠すること）

### オペレーター追加パターン

`Source`, `Flow`, `Sink` のオペレーターは `modules/streams/src/core/stage/{source,flow,sink}/` 内で
メソッドとして定義。既存の類似オペレーターを参考にすること。

例: `take_within` の実装を参考に `drop_within` を実装する。

### テストパターン

テストは `{type_name}/tests.rs` ファイルに配置。`#[cfg(test)]` で囲む。
`TestSourceProbe` / `TestSinkProbe` を活用したストリームテストを書く。

### BidiFlowパターン

`modules/streams/src/core/stage/bidi_flow.rs` に既存の `from_flows`, `identity`, `reversed`, `split` の実装がある。
新しいファクトリやコンビネータはこのパターンに従う。

### Hubパターン

`modules/streams/src/core/hub/` に `MergeHub`, `BroadcastHub`, `PartitionHub` がある。
`DrainingControl` はこれらと連携するドレイン制御型。

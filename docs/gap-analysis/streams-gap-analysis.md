# streams モジュール ギャップ分析

> 分析日: 2026-02-28
> 対象: `modules/streams/src/` vs `references/pekko/stream/src/main/`

## サマリー

| 指標 | 値 |
|---|---:|
| Pekko 公開型数 | 442 |
| fraktor-rs 公開型数 | 80 |
| 同名型カバレッジ | 33/442 (7.5%) |
| ギャップ数（同名差分） | 409 |

> 注: 同名一致ベースのため、`via_mat` など別名・Rust命名の実装は低く見積もられる。

## 主要ギャップ

| Pekko API | fraktor対応 | 難易度 | 判定 |
|---|---|---|---|
| RestartFlow / RestartSource / RestartSink | `RestartSettings` のみ | easy | 部分実装 |
| GraphDSL 高度構築（fan-in/fan-out） | `GraphDsl`（`via/to/build`中心） | medium | 部分実装 |
| Attributes API | 未対応 | medium | 未実装 |
| viaMat / toMat / watchTermination | `via_mat` / `to_mat` / `watch_termination_mat` | - | 別名で実装済み |
| mapAsyncUnordered / statefulMapConcat / groupBy / mergeSubstreams | 実装済み | - | 実装済み |

## 根拠（主要参照）

- Pekko:
  - `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/RestartFlow.scala:38`
  - `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Graph.scala:1577`
  - `references/pekko/stream/src/main/scala/org/apache/pekko/stream/Attributes.scala:49`
- fraktor-rs:
  - `modules/streams/src/core/restart_settings.rs:6`
  - `modules/streams/src/core/graph/graph_dsl.rs:7`
  - `modules/streams/src/core/stage/flow.rs:86`
  - `modules/streams/src/core/stage/flow.rs:114`
  - `modules/streams/src/core/stage/flow.rs:2080`

## 実装優先度提案

1. Phase 1 (easy): `RestartFlow/RestartSource/RestartSink` の薄い DSL 追加
2. Phase 2 (medium): GraphDSL fan-in/fan-out 強化
3. Phase 3 (medium): Attributes 基盤 (`withAttributes` / `addAttributes`) 追加

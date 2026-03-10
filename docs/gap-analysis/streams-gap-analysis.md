# streams モジュール ギャップ分析

分析日: 2026-03-10  
対象: `modules/streams/src/` vs `references/pekko/stream/src/main/scala/`

## サマリー

| 指標 | 値 |
|---|---:|
| Pekko 公開型数 | 272 |
| fraktor-rs 公開型数 | 91 |
| カバレッジ（型単位） | 91/272 (33.5%) |
| ギャップ数 | 12（要対応 9 / n/a 3） |

注記:
- 公開型数は機械抽出（Pekko側は `impl` / `javadsl` / `snapshot` / `serialization` を除外）。
- `snake_case` と `camelCase` の命名差は「別名で実装済み」として個別判定した。

## カテゴリ別ギャップ

### 型・トレイト

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `Source` / `Flow` / `Sink` / `BidiFlow` | `scaladsl/Source.scala:46`, `Flow.scala:60`, `Sink.scala:39`, `BidiFlow.scala:23` | `core/stage/source.rs:39`, `flow.rs:29`, `sink.rs:20`, `bidi_flow.rs:7` | - | 実装済み |
| `Shape` 系 (`Inlet`, `Outlet`, `Shape`, `SourceShape`, `FlowShape`, `SinkShape`, `BidiShape`, `ClosedShape`) | `stream/Shape.scala:100,142,183,293,311,327,353,265` | `core/shape/inlet.rs:7`, `outlet.rs:7`, `shape.rs:2`, `source_shape.rs:5`, `flow_shape.rs:5`, `sink_shape.rs:5`, `bidi_shape.rs:5`, `closed_shape.rs:2` | - | 実装済み |
| `KillSwitches` / `KillSwitch` / `SharedKillSwitch` / `UniqueKillSwitch` | `stream/KillSwitch.scala:37,153,257,216` | `core/lifecycle/kill_switches.rs:11`, `kill_switch.rs:7`, `shared_kill_switch.rs:16`, `unique_kill_switch.rs:11` | - | 実装済み |
| `Materializer` のスケジューリング/停止API | `stream/Materializer.scala:48,85,108,144,151,156` | `core/mat/materializer.rs:4`（`materialize` のみ） | hard | `withNamePrefix` / `schedule*` / `shutdown` / `isShutdown` が未対応 |
| `StreamRefs` (`SourceRef`, `SinkRef`) | `stream/StreamRefs.scala:55,89`, `scaladsl/StreamRefs.scala:24` | 未対応 | hard | リモート連携を伴うため実装コストが高い |
| `Tcp` / `TLS` DSL | `scaladsl/Tcp.scala:47`, `scaladsl/TLS.scala:62` | 未対応 | hard | 現状 `streams` モジュール外の責務 |

### オペレーター

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `map` / `filter` / `mapAsync` / `flatMapMerge` | `Flow.scala:1088,1562,1327,2965` | `core/stage/flow.rs:139,247,161,428` | - | 実装済み |
| `groupBy` / `splitWhen` / `splitAfter` / `mergeSubstreamsWithParallelism` | `Flow.scala:2681,2797,2883`, `SubFlow.scala:61` | `core/stage/flow.rs:681,703,718,954`, `flow_sub_flow.rs:33` | - | 実装済み |
| `concatMat` / `prependMat` / `orElseMat` / `zipLatestMat` / `zipLatestWithMat` | `Flow.scala:4395,4436,4483,4235,4248` | `concat/prepend/or_else/zip_latest/zip_latest_with` のみ (`flow.rs:904,838,2096,2129,2140`) | medium | `Mat` 合成付きオーバーロードが未対応 |
| `withFilter` | `Flow.scala:1572` | `filter` のみ (`flow.rs:247`) | trivial | for-comprehension 互換のエイリアス未対応 |
| `alsoToAll` | `Flow.scala:3996` | `also_to_all` (`flow.rs:2203`) | easy | 実装は `sinks` を消費して `self` を返すのみ |
| `divertTo` | `Flow.scala:4020` | `divert_to` (`flow.rs:2212`) | easy | `sink` を `drop` し `filter_not` にフォールバック |
| `watch(ref)` | `Flow.scala:1546` | `watch` (`flow.rs:2241`) | easy | 現状 no-op |
| `zipLatest` / `zipLatestWith` | `Flow.scala:3381,3431` | `zip_latest` / `zip_latest_with` (`flow.rs:2129,2140`) | - | 別名で実装済み |

### マテリアライゼーション

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `viaMat` / `toMat` / `mapMaterializedValue` | `Source.scala:62,93,100`, `Flow.scala:78,150,162`, `Sink.scala:68` | `source.rs:561,614,574`, `flow.rs:99,127,112`, `sink.rs:388` | - | 実装済み |
| `materializeIntoSource` | `Source.scala:122`, `Flow.scala:186` | `flow.rs:1471` | - | 実装済み |
| `watchTermination` | `Flow.scala:4536` | `flow.rs:2267`, `source.rs:587` | - | `watch_termination_mat` で実装 |
| `Source.run/runFold/runFoldAsync/runReduce/runForeach` | `Source.scala:133,157,171,190,204` | `run_with` (`source.rs:633`) + `RunnableGraph::run` (`core/mat/runnable_graph.rs:33`) | easy | 便利メソッド群は未対応 |
| `getAttributes` | `Source.scala:260`, `Flow.scala:836`, `Sink.scala:131` | 未対応（`with_attributes` / `add_attributes` のみ） | easy | 参照系APIが不足 |

### グラフDSL

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `GraphDSL.Builder` と `~>` / `<~` / `<~>` | `scaladsl/Graph.scala:1577,1596,1742,1788,1894` | `core/graph.rs:3`, `core/graph/flow_fragment.rs:11` | n/a | 意図的に非公開（`GraphDSL.Builder` 互換を提供しない設計） |

### その他相互運用

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `fromProcessor` / `toProcessor` (Java Flow interop) | `Flow.scala:429,381`, `JavaFlowSupport.scala:75,95` | 未対応 | n/a | JVM/Java Flow 依存のため Rust/no_std では優先度低 |

## 実装優先度の提案

### Phase 1: trivial/easy（短期で埋められる差分）

- `with_filter` 互換エイリアス追加
- `Source::run_*` 便利メソッド（`run_fold` など）追加
- `get_attributes` 参照系API追加
- `also_to_all` / `divert_to` / `watch` のセマンティクス実装

### Phase 2: medium（API拡張）

- `concat/prepend/or_else/zip_latest` 系に `*Mat` バリアントを追加

### Phase 3: hard（基盤拡張）

- `Materializer` のスケジューリング/停止API拡張
- `StreamRefs` 導入（remote 連携含む）
- `Tcp` / `TLS` ストリームDSL導入

### 対象外（n/a）

- `GraphDSL.Builder` スタイルの任意ポート配線（現方針: `FlowFragment` 中心）
- Java Flow (`Processor`) 直接相互運用


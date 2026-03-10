# streams モジュール ギャップ分析

分析日: 2026-03-09
対象: `modules/streams/src/` vs `references/pekko/stream/src/main/scala/`

途中で止まったので再開してほしい。ほとんど実装はできているはず

## サマリー

| 指標 | 値 |
|---|---:|
| Pekko 公開型数 | 417 |
| fraktor-rs 公開型数 | 152 |
| カバレッジ（型単位） | 152/417 (36.5%) |
| ギャップ数 | 16 |

注記:
- 公開型数は `pub struct/trait/enum/type` と Scala 側 `class/trait/object/enum` のユニーク型名を機械抽出して計数。
- ギャップ数は「未実装 + 部分実装」の主要差分のみを対象（YAGNI観点で優先順位づけ可能な粒度）。

## カテゴリ別ギャップ

### 型・トレイト

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `Source` / `Flow` / `Sink` | `scaladsl/Source.scala:46`, `scaladsl/Flow.scala:60`, `scaladsl/Sink.scala:39` | `core/stage/source.rs:40`, `core/stage/flow.rs:26`, `core/stage/sink.rs:20` | - | 実装済み |
| `RunnableGraph` | `scaladsl/Flow.scala:789` | `core/mat/runnable_graph.rs:11` | - | 実装済み |
| `Shape` 系 (`SourceShape`, `FlowShape`, `SinkShape`, `BidiShape`, `ClosedShape`) | `stream/Shape.scala:183,293,311,327,353` | `core/shape/*.rs` (`shape.rs:2`, `source_shape.rs:5`, `flow_shape.rs:5`, `sink_shape.rs:5`, `bidi_shape.rs:5`, `closed_shape.rs:2`) | - | 実装済み |
| `SubFlow extends FlowOps` | `scaladsl/SubFlow.scala:30` | `core/stage/flow_sub_flow.rs:9`, `core/stage/source_sub_flow.rs:9` | medium | `merge/concat_substreams`中心で、`FlowOps` 相当の演算面は限定的 |
| `KillSwitch` trait + `SharedKillSwitch.flow` | `stream/KillSwitch.scala:153,257,290` | `core/lifecycle/shared_kill_switch.rs:13`, `core/lifecycle/unique_kill_switch.rs:9` | medium | `shutdown/abort` は実装済みだが共通 trait と `flow()` が未提供 |

### オペレーター

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `map` / `filter` / `mapAsync` | `scaladsl/Flow.scala:1088,1562,1327` | `core/stage/flow.rs:125,233,147` | - | 実装済み |
| `mergeLatest` / `mergePreferred` / `mergePrioritized` / `mergeSorted` | `scaladsl/Flow.scala:3634,3655,3680,3713` | `core/stage/flow.rs:1840,1859,1876,2261` | - | 実装済み（専用 `logic` あり） |
| `concatLazy` / `prependLazy` / `orElse` | `scaladsl/Flow.scala:3782,3889,3914` | `core/stage/flow.rs:1809,1941,1932` | easy | `concat` / `prepend` への委譲で lazy/fallback 契約を満たしていない |
| `zipLatest` / `zipLatestWith` | `scaladsl/Flow.scala:3381,3431` | `core/stage/flow.rs:1950,1961` | medium | `zip_all` への委譲・`fill_value` 必須で契約差分あり |
| `limitWeighted` / `batchWeighted` / `groupedWeightedWithin` | `scaladsl/Flow.scala:1937,2490,2214` | `core/stage/flow.rs:1342,1557,1514` | easy | 重み関数と時間窓が実質未使用 |
| `mapAsyncPartitioned` | `scaladsl/Flow.scala:1392` | `core/stage/flow.rs:1433,1451` | medium | `map_async` へ委譲し、partition セマンティクス未反映 |
| `flatMapPrefix` / `prefixAndTail` | `scaladsl/Flow.scala:2622,2597` | `core/stage/flow.rs:1646,1673` | medium | prefix 専用契約ではなく簡略化実装 |

### マテリアライゼーション

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `viaMat` / `toMat` / `mapMaterializedValue` | `scaladsl/Flow.scala:78,150,162` | `core/stage/flow.rs:85,113,98` | - | 実装済み |
| `watchTermination` / `monitorMat` | `scaladsl/Flow.scala:4536,4551` | `core/stage/flow.rs:2093,2079`, `core/stage/source.rs:588` | - | 実装済み |
| `materializeIntoSource` | `scaladsl/Source.scala:122`, `scaladsl/Flow.scala:186` | `core/stage/flow.rs:1381` | easy | no-op（`self` 返却）で同等機能は未提供 |

### グラフDSL

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `GraphDSL.create` + `Builder.add/addEdge/materializedValue` | `scaladsl/Graph.scala:1583,1596,1605,1624,1681` | `core/graph/graph_dsl.rs:10-93` | hard | 現状は `Flow` 連結ラッパーで、ポート単位配線 DSL は未対応 |
| `~>`, `<~`, `<~>` 演算子 | `scaladsl/Graph.scala:1742,1788,1894` | 未対応 | hard | Fan-in/Fan-out/Bidi の DSL 表現を欠く |

### ライフサイクル

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `initial/completion/idle/backpressure timeout` | `scaladsl/Flow.scala:3017,3031,3046,3061` | `core/stage/flow.rs:1698,1719,1741,1762` | - | 実装済み（`logic/*.rs` で動作） |
| `KillSwitches.shared/single/singleBidi` | `stream/KillSwitch.scala:45,53,62` | `core/lifecycle/kill_switches.rs:12,18,24` | - | 実装済み |
| `DrainingControl.drainAndComplete` | `scaladsl/Hub.scala:54,60` | `core/hub/draining_control.rs:9,32` | easy | `drain()` はあるが completion 契約が異なる |

### エラー処理

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `recover` / `recoverWith` / `recoverWithRetries` | `scaladsl/Flow.scala:897,918,946` | `core/stage/flow.rs:1037,1071,1054` | medium | `PartialFunction`/代替 `Source` 契約ではなく固定 fallback 値中心 |
| `onErrorComplete` / `onErrorContinue`（条件付き） | `scaladsl/Flow.scala:966,985,1014,1045` | `core/stage/flow.rs:1003,1009,1015` | easy | 条件付きオーバーロード未対応 |
| `mapError` | `scaladsl/Flow.scala:1072` | `core/stage/flow.rs:995` | medium | エラー変換契約が `Result` ラップ中心で差分あり |

### その他

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `Compression.gzip/gzipDecompress/deflate/inflate` | `scaladsl/Compression.scala:34,50,61,77` | `core/stage/flow.rs:2110,2122,2116,2128` | medium | no-op 実装 |
| `log` / `logWithMarker` | `scaladsl/Flow.scala:3280,3303` | `core/stage/flow.rs:1354,1360` | easy | `wire_tap(|_| {})` のみで実質 no-op |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `concatLazy` / `prependLazy` / `orElse` の契約分離（現在の単純委譲を置換）
- `limitWeighted` / `batchWeighted` / `groupedWeightedWithin` で weight/ticks を実際に使用
- `log` / `logWithMarker` を no-op から最小限の観測可能挙動へ改善

### Phase 2: easy（単純な新規実装）
- `DrainingControl` に `drain_and_complete` 相当契約を追加
- `on_error_*` の条件付きオーバーロード追加
- `materialize_into_source` を no-op から実体化

### Phase 3: medium（中程度の実装工数）
- `map_async_partitioned` 系の partition セマンティクス導入
- `zip_latest` / `zip_latest_with` の契約整合
- `recover_with` 系を source ベースの復旧モデルに拡張
- `SharedKillSwitch::flow` と共通 `KillSwitch` 抽象の導入
- `flat_map_prefix` / `prefix_and_tail` の契約整合

### Phase 4: hard（アーキテクチャ変更を伴う）
- `GraphDSL` の `Builder` モデル（`create/add/addEdge/materializedValue`）導入
- `~>` / `<~` / `<~>` 相当の配線 DSL を no_std 制約内で設計

### 対象外（n/a）
- JVM 固有型や Java/Scala 相互運用専用 API（`CompletionStage`, `Java DSL` 依存の一部）は Rust/no_std 直輸入対象外として保守的に除外可能

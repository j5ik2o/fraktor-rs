# stream モジュール ギャップ分析

## サマリー

主要な比較対象は `references/pekko/stream/src/main/scala/org/apache/pekko/stream/` と
`references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/` の public API とし、
`javadsl` 重複、`impl`、例外専用型は型カバレッジ集計から除外した。

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 57 |
| fraktor-rs 公開型数 | 43（core: 39, std: 4） |
| カバレッジ（型単位） | 43/57 (75%) |
| ギャップ数 | 18（core: 13, std: 5） |

補足:
- `Source` の主要 public API は、`scaladsl/Source.scala` の固有メソッド 44 個に対して、正規化比較で fraktor 側 35 個相当を確認した
- `Flow` の主要 public API は、`scaladsl/Flow.scala` の固有メソッド 135 個に対して、正規化比較で fraktor 側 117 個相当を確認した
- ただし名前一致だけでは十分ではないため、下表ではセマンティクス差分と placeholder 実装を別途ギャップとして計上した

## 層別カバレッジ

`stream` モジュールには `core/typed` 相当の層が存在しないため、typed ラッパー層は `0/0` とした。

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 51 | 39 | 76% |
| core / typed ラッパー | 0 | 0 | - |
| std / アダプタ | 6 | 4 | 67% |

## カテゴリ別ギャップ

### 型・トレイト ✅ 実装済み 22/31 (71%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `StreamRefs` / `SinkRef` / `SourceRef` / `StreamRefResolver` | `scaladsl/StreamRefs.scala:L34,L45`, `StreamRefs.scala:L55,L89,L146` | 未対応 | std | hard | リモート越しの back-pressured stream reference が存在しない |
| `Tcp` | `scaladsl/Tcp.scala:L47` | 未対応 | std | hard | TCP ストリーム DSL と transport 実装がない |
| `TLS` / `SslTlsOptions` | `scaladsl/TLS.scala:L62`, `SslTlsOptions.scala:L29` | 未対応 | std | hard | TLS bidi stage と TLS 設定型がない |
| `MaterializerLoggingProvider` | `MaterializerLoggingProvider.scala:L24` | 未対応 | std | medium | materializer とロギングの統合フックがない |
| `GraphStage` 実行 API 一式 | `stage/GraphStage.scala:L52,L321,L1451,L1532,L1642,L1700,L1863,L1888` | 部分実装 | core | hard | fraktor 側は `GraphStage` / `GraphStageLogic` / `AsyncCallback` / `TimerGraphStageLogic` の最小集合に留まり、`StageActor`、`SubSinkInlet`、`SubSourceOutlet`、`InHandler`、`OutHandler` などの stage authoring API が不足している（`modules/stream/src/core/graph/graph_stage.rs:6`, `modules/stream/src/core/graph/graph_stage_logic.rs:4`, `modules/stream/src/core/stage/async_callback.rs:9`, `modules/stream/src/core/stage/timer_graph_stage_logic.rs:7`） |

### オペレーター ✅ 実装済み 35/44 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.mapAsyncUnordered` | `Flow.scala:L1363` | 未対応 | core | medium | `Flow` には `map_async_unordered` があるが、`Source` 側には `map_async` までしかない（`modules/stream/src/core/stage/source.rs:899`） |
| `Source.groupedWithin` / `Source.groupedWeightedWithin` / `Source.batchWeighted` | `Flow.scala:L2191,L2214,L2490` | 未対応 | core | medium | `Flow` 側には実装がある一方で、`Source` 側には同等 API がない（`modules/stream/src/core/stage/flow.rs:2117`, `modules/stream/src/core/stage/flow.rs:2146`, `modules/stream/src/core/stage/flow.rs:2177`） |
| `Source.flatMapPrefix` | `Flow.scala:L2622` | 部分実装 | core | easy | `flat_map_prefix_mat` のみ公開されており、非 `*_mat` の parity API がない（`modules/stream/src/core/stage/source.rs:2322`） |
| `Source.mergeLatest` / `Source.zipLatest` / `Source.mergeSorted` | `javadsl/Source.scala:L1703,L1800,L1895` | 部分実装 | core | easy | `merge_latest_mat` / `zip_latest_mat` / `merge_sorted_mat` のみで、直接 API が不足している（`modules/stream/src/core/stage/source.rs:2145`, `modules/stream/src/core/stage/source.rs:2092`, `modules/stream/src/core/stage/source.rs:2496`） |
| `Flow.join` / `Flow.joinMat` | `Flow.scala:L236,L253,L277,L298` | 未対応 | core | medium | `BidiFlow::join` はあるが、`Flow` 自身の join API がない |
| `Flow.toProcessor` / processor interop | `Flow.scala:L381` | 未対応 | std | hard | Reactive Streams `Processor` との相互運用がない |
| `Source.optionalVia` | `Source.scala:L339` | 未対応 | core | medium | `Flow` には `optional_via` があるが、`Source` には対応する escape hatch がない |

### マテリアライゼーション ✅ 実装済み 6/12 (50%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.materializeIntoSource` | `Source.scala:L122` | 未対応 | core | medium | `Flow::materialize_into_source` はあるが、`Source` 固有 API がない |
| `Source.run` | `Source.scala:L133` | 未対応 | core | easy | `run_with` / `run_fold` / `run_foreach` はあるが、`Sink.ignore` に接続する省略 API がない（`modules/stream/src/core/stage/source.rs:791`） |
| `Flow.preMaterialize` | `Flow.scala:L169` | 未対応 | core | medium | `Flow` 側は pre-materialize parity がない |
| `Flow.run` / `Flow.runWith` | `Flow.scala:L370,L804` | 未対応 | core | easy | materializer に直接流し込む shorthand がない |
| `Sink.preMaterialize` | `Sink.scala:L77` | 部分実装 | core | medium | `fraktor` は新しい `StreamCompletion` を返すだけで、実行中 sink との接続を作っていない（`modules/stream/src/core/stage/sink.rs:187`） |
| `Source.preMaterialize` | `Source.scala:L107` | 部分実装 | core | medium | `fraktor` は graph/mat を clone して返すだけで、Pekko の先行 materialization 契約を満たしていない（`modules/stream/src/core/stage/source.rs:2785`） |

### グラフDSL ✅ 実装済み 6/10 (60%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GraphDSL.Builder` + `Implicits.PortOps` / `ReversePortOps` | `Graph.scala:L1577,L1596,L1827,L1861` | 部分実装 | core | medium | `GraphDsl` / `GraphDslBuilder` / `PortOps` / `ReversePortOps` はあるが、generic `add`、`createGraph`、暗黙配線 DSL は縮退している（`modules/stream/src/core/graph/graph_dsl.rs:8`, `modules/stream/src/core/graph/graph_dsl_builder.rs:24`, `modules/stream/src/core/graph/port_ops.rs:17`, `modules/stream/src/core/graph/reverse_port_ops.rs:18`） |
| `Merge` / `Broadcast` / `Balance` / `Partition` reusable graph stages | `Graph.scala:L75,L595,L775,L907` | 未対応 | core | medium | 現状はメソッドベースの DSL で表現しており、公開 reusable stage 型がない |
| `ZipN` / `ZipWithN` / `Concat` / `OrElse` / `MergeSequence` reusable graph stages | `Graph.scala:L1166,L1213,L1269,L1361,L1446` | 未対応 | core | medium | 互換 API の一部はメソッドであるが、Pekko のような graph stage 型としては公開されていない |

### ライフサイクル ✅ 実装済み 5/8 (63%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `watchTermination` | `Flow.scala:L4536` | 部分実装 | core | medium | `watch_termination_mat` はあるが、Pekko の `Future[Done]` 契約ではなく独自 `StreamCompletion<()>` である（`modules/stream/src/core/stage/flow.rs:3294`, `modules/stream/src/core/stage/source.rs:741`） |
| `monitor` / `monitorMat` | `Flow.scala:L4551` | 部分実装 | core | medium | `monitor()` は `zip_with_index` ベース、`monitor_mat()` は `FlowMonitorImpl` を合成するだけで live monitor 契約とは異なる（`modules/stream/src/core/stage/flow.rs:3274`, `modules/stream/src/core/stage/flow.rs:3280`） |
| `watch(ref)` | `Flow.scala:L1546` | 部分実装 | core | easy | `watch()` が no-op で actor 終了監視を行わない（`modules/stream/src/core/stage/flow.rs:3268`） |

### エラー処理 ✅ 実装済み 4/7 (57%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Flow.contramap` | `Flow.scala:L204` | 部分実装 | core | medium | `fraktor` は入力変換を行わず `self` を返すだけ（`modules/stream/src/core/stage/flow.rs:1610`） |
| `Flow.fold` | `Flow.scala:L2035` | 部分実装 | core | medium | Pekko は最終値のみを emit するが、`fraktor` は running accumulation を流すため `scan` 寄りの意味になっている（`modules/stream/src/core/stage/flow.rs:1662`） |
| `Flow.doOnCancel` | `Flow.scala:L1623` | 部分実装 | core | easy | callback を保持せず no-op で返す（`modules/stream/src/core/stage/flow.rs:1637`） |
| `Flow.alsoToAll` | `Flow.scala:L3996` | 部分実装 | core | easy | sink 群を数えるだけで配線せず `self` を返す（`modules/stream/src/core/stage/flow.rs:3163`） |

### その他・統合 ✅ 実装済み 4/10 (40%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Sink.fromMaterializer` / `fromSubscriber` / `futureSink` | `Sink.scala` 系 API 群 | 部分実装 | core | medium | いずれも `Sink::ignore()` へフォールバックする placeholder 実装（`modules/stream/src/core/stage/sink.rs:130`, `modules/stream/src/core/stage/sink.rs:136`, `modules/stream/src/core/stage/sink.rs:142`） |
| `Sink.source` / `Sink.asPublisher` | `Sink.scala` 系 bridge API | 部分実装 | core | medium | どちらも `Source::empty()` を返すだけで bridge を構築していない（`modules/stream/src/core/stage/sink.rs:193`, `modules/stream/src/core/stage/sink.rs:199`） |
| `Sink.combine` | `Sink.combine` | 部分実装 | core | medium | 先頭 sink のみ使う stub と明記されている（`modules/stream/src/core/stage/sink.rs:428`） |
| `StreamConverters` の blocking IO 相互運用 | `StreamConverters.scala:L35` | 部分実装 | std | medium | `from_input_stream` / `from_output_stream` は単なる iterator 化、`as_input_stream` / `as_output_stream` は `Vec` 収集であり、Pekko の blocking IO bridge ではない（`modules/stream/src/core/stage/source.rs:324`, `modules/stream/src/core/stage/source.rs:333`, `modules/stream/src/core/stage/source.rs:2457`, `modules/stream/src/core/stage/source.rs:2475`） |

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため省略する。

判定根拠:
- 型単位カバレッジが 75% で、内部構造分析へ進む基準の 80% に届いていない
- `hard` / `medium` の主要未実装ギャップが `StreamRefs`, `Tcp`, `TLS`, `GraphStage` rich API, `Flow.join`, `Flow.toProcessor`, `Source.materializeIntoSource` など 5 件を大きく超える
- placeholder 実装が複数あり、公開契約の parity が未充足な段階である

## 実装優先度

### Phase 1

- `Source.run` を追加する。実装先層: `core`
- `Source.flatMapPrefix` を `flat_map_prefix_mat` の薄いラッパーとして追加する。実装先層: `core`
- `Source.mergeLatest` / `Source.zipLatest` / `Source.mergeSorted` の非 `*_mat` API を追加する。実装先層: `core`
- `Flow.alsoToAll` の no-op をやめ、少なくとも既存 `also_to_mat` の繰り返し合成で意味を持つ実装へ置き換える。実装先層: `core`
- `Flow.doOnCancel` の no-op をやめ、最低限 callback 発火契約を満たす。実装先層: `core`

### Phase 2

- `Source.materializeIntoSource` を追加する。実装先層: `core`
- `Flow.preMaterialize` を追加する。実装先層: `core`
- `Flow.run` / `Flow.runWith` を追加する。実装先層: `core`
- `Source.mapAsyncUnordered` を追加する。実装先層: `core`
- `Source.groupedWithin` / `groupedWeightedWithin` / `batchWeighted` を追加する。実装先層: `core`
- `Flow.join` / `Flow.joinMat` を追加する。実装先層: `core`
- `watchTermination` / `monitor` の materialized contract を Pekko 寄りに揃える。実装先層: `core`
- `Sink.combine` / `Sink.fromMaterializer` / `Sink.fromSubscriber` / `Sink.asPublisher` の placeholder を排除する。実装先層: `core`
- `StreamConverters` の blocking IO bridge を実装し直す。実装先層: `std`

### Phase 3

- `StreamRefs` / `SinkRef` / `SourceRef` / `StreamRefResolver` を実装する。実装先層: `std`
- `Tcp` を実装する。実装先層: `std`
- `TLS` / `SslTlsOptions` を実装する。実装先層: `std`
- `GraphStage` rich API（`StageActor`, `SubSinkInlet`, `SubSourceOutlet`, `InHandler`, `OutHandler`）を実装する。実装先層: `core`
- `Flow.toProcessor` と Reactive Streams processor 相互運用を実装する。実装先層: `std`

### 対象外（n/a）

- 今回の主要ギャップ表では `n/a` 判定項目はなし

## まとめ

- 全体評価: 主要 DSL と基本演算子はかなり揃っているが、Pekko parity の観点では materialization 契約、graph stage authoring、network/remote 系の基盤がまだ手薄である
- 低コストで parity を前進できる項目: `Source.run`、`Source.flatMapPrefix`、`Source.mergeLatest`/`zipLatest`/`mergeSorted`、`Flow.alsoToAll` の実装化
- parity 上の主要ギャップ: `StreamRefs`、`Tcp`、`TLS`、`GraphStage` rich API、`Flow.toProcessor`
- 次のボトルネック評価: まだ API ギャップが支配的であり、内部構造差分より先に公開契約の穴と placeholder 実装の解消を優先すべき段階である

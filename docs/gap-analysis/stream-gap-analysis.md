# stream モジュール ギャップ分析

## 前提と集計範囲

- 比較対象:
  - fraktor-rs: `modules/stream/src/core`, `modules/stream/src/std`
  - Pekko: `references/pekko/stream/src/main/scala/org/apache/pekko/stream`, `.../scaladsl`, `.../stage`
- 集計対象は parity に直接効く公開 DSL・主要型・materialization 入口に限定した。
- `javadsl` 重複、内部実装、例外専用型、テスト専用型は型カバレッジの主集計から除外した。
- `stream` モジュールには `core/typed` 相当の層がないため、typed ラッパー層は `0/0` 扱いとする。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 約 57 |
| fraktor-rs 公開型数 | 約 45（core: 41, std: 4） |
| カバレッジ（型単位） | 約 45/57 (79%) |
| ギャップ数 | 24（core: 18, std: 6） |

補足:
- `Source` / `Flow` / `Sink` の主経路 DSL はかなり揃っている。
- 一方で materializer-aware API、bridge API、GraphStage authoring API、network/remote 系はまだ薄い。
- placeholder 実装が残るため、現段階では API ギャップが構造ギャップより支配的である。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 52 | 41 | 79% |
| core / typed ラッパー | 0 | 0 | - |
| std / アダプタ | 5 | 4 | 80% |

## カテゴリ別ギャップ

### 型・トレイト ✅ 実装済み 23/27 (85%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `StreamRefs` / `SinkRef` / `SourceRef` / `StreamRefResolver` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/StreamRefs.scala:33,55,67,89,133` | 未対応 | std | hard | リモート越しの back-pressured stream reference 群が存在しない |
| `Tcp` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Tcp.scala:47` | 未対応 | std | hard | TCP stream DSL と transport 拡張がない |
| `TLS` / `SslTlsOptions` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/TLS.scala:62`, `references/pekko/stream/src/main/scala/org/apache/pekko/stream/SslTlsOptions.scala:29` | 未対応 | std | hard | TLS bidi stage と TLS 設定型がない |
| `GraphStageLogic` rich API | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/stage/GraphStage.scala:321,1855` | 部分実装 | core | hard | fraktor は `modules/stream/src/core/stage/graph_stage.rs:7`, `.../graph_stage_logic.rs:4`, `.../async_callback.rs:9`, `.../timer_graph_stage_logic.rs:7` の最小集合に留まり、`StageActor`、`SubSinkInlet`、`SubSourceOutlet`、handler 群がない |

### オペレーター ✅ 実装済み 39/44 (89%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.mapAsyncUnordered` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:1363` | 未対応 | core | medium | `Flow` 側には `modules/stream/src/core/dsl/flow.rs:1768` があるが `Source` 表層には露出していない |
| `Source.groupedWithin` / `groupedWeightedWithin` / `batchWeighted` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:2191,2214,2490` | 未対応 | core | medium | `Flow` 側では `modules/stream/src/core/dsl/flow.rs:1896,1921,1948` にあるが `Source` 側には対応 API がない |
| `Source.flatMapPrefix` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:2622` | 部分実装 | core | easy | `modules/stream/src/core/dsl/source.rs:2110` に `flat_map_prefix_mat` はあるが、非 `*_mat` API がない |
| `Source.mergeLatest` / `zipLatest` / `mergeSorted` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:3381,3634,3713` | 部分実装 | core | easy | `modules/stream/src/core/dsl/source.rs:1908,1957,2280` の `*_mat` 系のみ。直接 API が不足 |
| `Flow.join` / `joinMat` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:236,253,277,298` | 未対応 | core | medium | `BidiFlow` はあるが `Flow` 自身の join surface がない |
| `Source.optionalVia` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala:339` | 未対応 | core | medium | `Flow` 側には `optionalVia` があるが `Source` 側の escape hatch がない |

### マテリアライゼーション ✅ 実装済み 8/12 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.run` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala:133` | 未対応 | core | easy | `modules/stream/src/core/dsl/source.rs:795` に `run_with` はあるが `Sink::ignore()` へ接続する shorthand がない |
| `Flow.preMaterialize` / `runWith` / `run` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:169,370,804` | 未対応 | core | medium | `Flow` は DSL 合成に寄っており、materializer 直結の shorthand がない |
| `Source.preMaterialize` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala:107` | 部分実装 | core | medium | `modules/stream/src/core/dsl/source.rs:2565` は graph と mat をそのまま複製して返すだけで、先行 materialization 契約を満たしていない |
| `Sink.preMaterialize` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Sink.scala:77` | 部分実装 | core | medium | `modules/stream/src/core/dsl/sink.rs:189` は `(self, StreamCompletion::new())` を返すだけで、Pekko の materialized sink bridge になっていない |

### グラフDSL ✅ 実装済み 6/8 (75%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GraphDSL.Builder` / `Implicits.PortOps` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Graph.scala:1577,1596,1721,1827` | 部分実装 | core | medium | fraktor 側の `GraphDsl` / `GraphDslBuilder` / `PortOps` は `modules/stream/src/core/impl/graph_dsl.rs:10`, `.../graph_dsl_builder.rs:22`, `.../port_ops.rs:20` に internal 実装としてあるが public DSL になっていない |
| reusable graph stage 型 (`Merge`, `Broadcast`, `Balance`, `Partition`, `ZipN`, `Concat`, `OrElse`) | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Graph.scala:75,595,907,1166,1269,1361` | 未対応 | core | medium | ロジック自体は `modules/stream/src/core/impl/fusing/*.rs` にあるが、Pekko のような reusable public graph stage 型としては出ていない |

### ライフサイクル・互換 ✅ 実装済み 4/9 (44%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Flow.contramap` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:204` | 部分実装 | core | medium | `modules/stream/src/core/dsl/flow.rs:1413` は入力変換を行わず `self` を返す |
| `Flow.doOnCancel` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:1623` | 部分実装 | core | easy | `modules/stream/src/core/dsl/flow.rs:1440` は callback を保持せず `self` を返す |
| `Flow.alsoToAll` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:3996` | 部分実装 | core | easy | `modules/stream/src/core/dsl/flow.rs:2814` は sink 数を数えるだけで配線しない |
| `Flow.watch(ref)` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:1546` | 部分実装 | core | easy | `modules/stream/src/core/dsl/flow.rs:2919` は no-op |
| `watchTermination` / `monitorMat` / `monitor` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:4536,4551,4564` | 部分実装 | core | medium | `modules/stream/src/core/dsl/flow.rs:2925-2945` は `FlowMonitor` / `Future[Done]` 契約ではなく `(u64, Out)` と独自 `StreamCompletion<()>` ベース |

### その他・統合 ✅ 実装済み 5/9 (56%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| materializer-aware factory APIs (`Source.fromMaterializer`, `Flow.fromMaterializer`, `Sink.fromMaterializer`) | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala:393`, `.../Flow.scala:524`, `.../Sink.scala:170` | 部分実装 | core | medium | `modules/stream/src/core/dsl/source.rs:168` は `lazy_source` 相当、`modules/stream/src/core/dsl/flow.rs:102` は即時 factory 実行、`modules/stream/src/core/dsl/sink.rs:132` は `ignore()` へフォールバック |
| `Sink.fromSubscriber` / `futureSink` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Sink.scala:182` | 部分実装 | core | medium | `modules/stream/src/core/dsl/sink.rs:138,144` はともに `ignore()` へフォールバック |
| `Sink.source` / `Sink.asPublisher` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Sink.scala:310,326` | 部分実装 | core | medium | `modules/stream/src/core/dsl/sink.rs:195,201` は `Source::empty()` を返すだけ |
| `Source.asSubscriber` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala:637` | 部分実装 | core | medium | `modules/stream/src/core/dsl/source.rs:186` は `Sink::ignore()` を返すだけで subscriber bridge を作らない |

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため省略する。

判定根拠:
- 型単位カバレッジが約 79% で、内部構造分析へ進む基準の 80% に届いていない
- `hard` / `medium` の主要未実装ギャップが `StreamRefs`, `Tcp`, `TLS`, `GraphStage` rich API, `Flow.join`, materializer-aware API など 5 件を大きく超える
- placeholder 実装がまだ複数残っており、公開契約 parity の穴埋めが先行課題

## 実装優先度

### Phase 1

- `Source.run` を追加する。実装先層: `core`
- `Source.flatMapPrefix` を `flat_map_prefix_mat` の薄いラッパーとして追加する。実装先層: `core`
- `Source.mergeLatest` / `zipLatest` / `mergeSorted` の非 `*_mat` API を追加する。実装先層: `core`
- `Flow.also_to_all` の no-op をやめ、既存 `also_to_mat` 合成で少なくとも意味を持つ実装にする。実装先層: `core`
- `Flow.do_on_cancel` の no-op をやめ、cancel callback を観測可能にする。実装先層: `core`

### Phase 2

- `Source.mapAsyncUnordered` を追加する。実装先層: `core`
- `Source.groupedWithin` / `groupedWeightedWithin` / `batchWeighted` を追加する。実装先層: `core`
- `Flow.join` / `joinMat` を追加する。実装先層: `core`
- `Source.preMaterialize` / `Sink.preMaterialize` / `Flow.preMaterialize` を Pekko 寄りの契約に揃える。実装先層: `core`
- `watchTermination` / `monitor` / `monitorMat` の materialized contract を Pekko 寄りに揃える。実装先層: `core`
- `Flow.contramap` を実入力変換として実装する。実装先層: `core`
- `Sink.fromMaterializer` / `fromSubscriber` / `futureSink` / `source` / `into_publisher` / `Source.as_subscriber` の placeholder を排除する。実装先層: `core`

### Phase 3

- `StreamRefs` / `SinkRef` / `SourceRef` / `StreamRefResolver` を実装する。実装先層: `std`
- `Tcp` を実装する。実装先層: `std`
- `TLS` / `SslTlsOptions` を実装する。実装先層: `std`
- `GraphStage` rich API（`StageActor`, `SubSinkInlet`, `SubSourceOutlet`, handler 群）を実装する。実装先層: `core`

### 対象外（n/a）

- 今回の主要ギャップ表では `n/a` 判定項目はなし

## まとめ

- 全体評価: 基本 DSL、shape、kill switch、restart、主要な fusing stage はかなり揃っているが、Pekko parity の観点では bridge API と materialization 契約がまだ弱い
- 低コストで前進できる項目: `Source.run`、`Source.flatMapPrefix`、`Source.mergeLatest` / `zipLatest` / `mergeSorted`、`Flow.also_to_all`、`Flow.do_on_cancel`
- parity 上の主要ギャップ: `StreamRefs`、`Tcp`、`TLS`、`GraphStage` rich API、materializer-aware factory API
- 次のボトルネック評価: まだ API ギャップが支配的であり、内部責務分割より先に公開契約と placeholder 実装の解消を優先すべき段階である

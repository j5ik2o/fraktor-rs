# streams モジュール ギャップ分析

生成日: 2026-03-16（前回: 2026-03-15）

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | ~79（コアDSL 8、シェイプ 11、GraphDSL系 22、ハブ/ライフサイクル 10、IO 4、その他 24） |
| fraktor-rs 公開型数 | 101（core: 100, std: 1） |
| カバレッジ（型単位） | 74/79 (94%) |
| ギャップ数（型単位・実装対象） | 5 |
| ギャップ数（メソッド単位・実装対象） | 34 |
| 対象外（n/a） | 7 |

**結論：** 型単位カバレッジは 94% で高水準。しかし今回の精査でメソッドレベルの重要なギャップが判明した。特に (1) *Mat バリアント（合成オペレーターのマテリアライゼーション制御）18個、(2) アクター連携（`actorRef` ソース/シンク）4個、(3) Sink ファクトリメソッド（`never`, `foldAsync`, `combine` 等）6個が未実装。これらは Pekko ユーザーが日常的に使う API であり、対応が必要。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core（コアロジック・ポート定義） | 75 | 74 | 99% |
| std（tokio アダプタ） | 4 | 1 | 25% |

**注記**: fraktor-rs の streams モジュールには `typed/` サブ層は存在しない。
std 層には `FileIO` が実装済み（`from_path` / `to_path` に `IOResult` マテリアライズドバリュー付き）。
IO 関連のうち StreamConverters / Tcp / TLS は Pekko では std 相当だが、Rust では外部クレートで代替するため n/a。

---

## カテゴリ別ギャップ

### コアDSL（Source / Flow / Sink）　✅ 実装済み 8/8 (100%)

Source, Flow, Sink, BidiFlow, FlowWithContext, SourceWithContext, SubFlow, RunnableGraph すべて実装済み。ギャップなし。

---

### Source ファクトリメソッド　✅ 実装済み 36/40 (90%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.actorRef` | `Source.scala:L678` | 未対応 | core | medium | アクターへの直接参照でメッセージをストリーム化。actor モジュールとの連携が必要 |
| `Source.actorRefWithBackpressure` | `Source.scala:L715` | 未対応 | core | medium | バックプレッシャー付きアクターソース。actor モジュールとの連携が必要 |
| `Source.create` | `Source.scala:L853` | 未対応 | core | easy | プロデューサーコールバックでキューを制御する Source 生成 |
| `Source.mergePrioritizedN` | `Source.scala:L1015` | 未対応 | core | trivial | Flow には `merge_prioritized_n` があるが Source コンパニオンにはない |

実装済み: `empty`, `fromOption`, `fromIterator`, `from`, `fromArray`, `single`, `failed`, `never`, `repeat`, `cycle`, `iterate`, `range`, `tick`, `unfold`, `unfoldAsync`, `unfoldResource`, `unfoldResourceAsync`, `future`, `futureSource`, `completionStage`, `completionStageSource`, `lazyFuture`, `lazyFutureSource`, `lazyCompletionStage`, `lazyCompletionStageSource`, `lazySingle`, `lazySource`, `maybe`, `queue`, `queueWithOverflow`, `queueUnbounded`, `fromMaterializer`, `fromPublisher`, `combine`, `combineMat`, `zipN`, `zipWithN`

---

### Source 便利メソッド　✅ 実装済み 1/4 (25%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.runFold` | `Source.scala:L157` | 未対応 | core | trivial | `source.fold(zero)(f).to(Sink.head).run()` のショートカット |
| `Source.runFoldAsync` | `Source.scala:L171` | 未対応 | core | trivial | `source.foldAsync(zero)(f).to(Sink.head).run()` のショートカット |
| `Source.runReduce` | `Source.scala:L190` | 未対応 | core | trivial | `source.reduce(f).to(Sink.head).run()` のショートカット |
| `Source.runForeach` | `Source.scala:L204` | 未対応 | core | trivial | `source.to(Sink.foreach(f)).run()` のショートカット |

実装済み: `runWith`

---

### Sink ファクトリメソッド　✅ 実装済み 28/34 (82%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Sink.never` | `Sink.scala:L338` | 未対応 | core | trivial | 永遠に完了しない Sink。テスト等で使用 |
| `Sink.foldAsync` | `Sink.scala:L449` | 未対応 | core | easy | 非同期 fold を行う Sink。`fold` は実装済みだが async 版がない |
| `Sink.combine` | `Sink.scala:L362` | 未対応 | core | medium | 複数 Sink をファンアウトで合成。Broadcast 等と組み合わせ |
| `Sink.combineMat` | `Sink.scala:L383` | 未対応 | core | medium | `combine` のマテリアライズドバリュー制御版 |
| `Sink.actorRef` | `Sink.scala:L605` | 未対応 | core | medium | アクターにメッセージを送信する Sink。actor モジュール連携が必要 |
| `Sink.actorRefWithBackpressure` | `Sink.scala:L658` | 未対応 | core | medium | バックプレッシャー付きアクター Sink。actor モジュール連携が必要 |

実装済み: `ignore`, `foreach`, `foreachAsync`, `cancelled`, `none`, `onComplete`, `fromSubscriber`, `futureSink`, `lazySink`, `lazyFutureSink`, `lazyCompletionStageSink`, `collect`, `collection`, `seq`, `javaCollector`, `takeLast`, `toPath`, `count`, `exists`, `forall`, `headOption`, `lastOption`, `queue`, `fold`, `foldWhile`, `head`, `last`, `reduce`, `fromGraph`, `fromMaterializer`, `source`, `asPublisher`, `preMaterialize`

---

### Flow ファクトリメソッド　✅ 実装済み 8/11 (73%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Flow.fromSinkAndSourceMat` | `Flow.scala:L579` | 未対応 | core | easy | `fromSinkAndSource` のマテリアライズドバリュー制御版 |
| `Flow.fromSinkAndSourceCoupledMat` | `Flow.scala:L674` | 未対応 | core | easy | `fromSinkAndSourceCoupled` のマテリアライズドバリュー制御版 |
| `Flow.toProcessor` | `Flow.scala:L381` | 未対応 | core | n/a | Reactive Streams `Processor` 変換。JVM 固有 |

実装済み: `new` (identity), `fromFunction`, `fromGraph`, `fromMaterializer`, `fromSinkAndSource`, `fromSinkAndSourceCoupled`, `lazyFlow`, `lazyFutureFlow`, `lazyCompletionStageFlow`, `optionalVia`

---

### オペレーター（変換・フィルタ）　✅ 実装済み 約89/91 (98%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `collect(pf)` | `FlowOps:L1753` | 未対応 | core | trivial | Rust に PartialFunction はないが `filter_map` 相当として実装可。`collectFirst`/`collectWhile`/`collectType` は実装済み |
| `flatten` | `FlowOps:L2950` | 未対応 | core | trivial | ネストされた Source をフラットにする。`flatMapConcat(identity)` で代替可能 |

**主要オペレーター実装済みリスト**（67個のロジックファイル + インライン実装）:

`map`, `mapConcat`, `mapAsync`, `mapAsyncUnordered`, `mapAsyncPartitioned`, `mapAsyncPartitionedUnordered`, `mapWithResource`, `mapOption`, `mapError`, `statefulMap`, `statefulMapConcat`, `filter`, `filterNot`, `take`, `takeWhile`, `takeUntil`, `takeWithin`, `drop`, `dropWhile`, `dropWithin`, `dropRepeated`, `scan`, `scanAsync`, `fold`, `foldAsync`, `reduce`, `grouped`, `groupedWithin`, `groupedWeighted`, `groupedWeightedWithin`, `groupedAdjacentBy`, `groupedAdjacentByWeighted`, `sliding`, `buffer`, `throttle`, `debounce`, `delay`, `delayWith`, `initialDelay`, `expand`, `extrapolate`, `conflateWithSeed`, `batch`, `batchWeighted`, `sample`, `log`, `logWithMarker`, `flatMapConcat`, `flatMapMerge`, `flatMapPrefix`, `flattenMerge`, `switchMap`, `intersperse`, `wireTap`, `alsoTo`, `alsoToAll`, `divertTo`, `aggregateWithBoundary`, `collectFirst`, `collectWhile`, `collectType`, `doOnFirst`, `doOnCancel`, `mergeSequence`, `ask`, `keepAlive`, `limitWeighted`, `prefixAndTail`, `backpressureTimeout`, `completionTimeout`, `idleTimeout`, `initialTimeout`, `onErrorComplete`, `onErrorContinue`, `onErrorResume`, `recover`, `recoverWith`, `recoverWithRetries`, `contramap`, `flattenOptional`, `concatLazy`, `concatAllLazy`, `prependLazy`, `orElse`, `monitor`, `monitorMat`, `detach`, `asyncBoundary`

**注意**: 一部のオペレーター（`zipLatest`, `mergeAll`, `interleaveAll`, `switchMap`, `extrapolate`, `keepAlive`, `dropWithin`, `delayWith`, `groupedAdjacentBy` 等）は既存オペレーターへの委譲で実装されており、独立したロジックファイルを持たない。

---

### エラーハンドリング　✅ 実装済み 7/7 (100%)

`recover`, `recoverWith`, `recoverWithRetries`, `onErrorComplete`, `onErrorContinue`, `onErrorResume`, `mapError` すべて実装済み。ギャップなし。

---

### ファンイン / ファンアウト　✅ 実装済み 23/23 (100%)

グラフプリミティブ実装済み：`Merge`, `MergePreferred`, `MergePrioritized`, `MergeSorted`, `MergeLatest`, `MergeAll`, `MergeSequence`, `Interleave`, `InterleaveAll`, `Broadcast`, `WireTap`, `Partition`, `Balance`, `Zip`, `ZipLatest`, `ZipWith`, `ZipLatestWith`, `ZipN`, `ZipWithN`, `ZipWithIndex`, `ZipAll`, `Unzip`, `UnzipWith`, `Concat`, `ConcatLazy`, `ConcatAllLazy`, `OrElse`, `PrependLazy`, `AlsoTo`, `DivertTo`

---

### *Mat バリアント（FlowOpsMat）　✅ 実装済み 6/24 (25%)

Pekko の `FlowOpsMat` trait は合成オペレーター（zip, merge, concat 等）にマテリアライズドバリュー制御版を提供する。fraktor-rs では一部のみ実装済み。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `flatMapPrefixMat` | `FlowOpsMat:L4179` | 未対応 | core | easy | `flatMapPrefix` の Mat 版 |
| `zipMat` | `FlowOpsMat:L4192` | 未対応 | core | easy | `zip` の Mat 版 |
| `zipAllMat` | `FlowOpsMat:L4208` | 未対応 | core | easy | `zipAll` の Mat 版 |
| `zipWithMat` | `FlowOpsMat:L4222` | 未対応 | core | easy | `zipWith` の Mat 版 |
| `zipLatestMat` | `FlowOpsMat:L4235` | 未対応 | core | easy | `zipLatest` の Mat 版 |
| `zipLatestWithMat` | `FlowOpsMat:L4248` | 未対応 | core | easy | `zipLatestWith` の Mat 版 |
| `mergeMat` | `FlowOpsMat:L4274` | 未対応 | core | easy | `merge` の Mat 版 |
| `interleaveMat` | `FlowOpsMat:L4292` | 未対応 | core | easy | `interleave` の Mat 版 |
| `mergeLatestMat` | `FlowOpsMat:L4330` | 未対応 | core | easy | `mergeLatest` の Mat 版 |
| `mergePreferredMat` | `FlowOpsMat:L4342` | 未対応 | core | easy | `mergePreferred` の Mat 版 |
| `mergePrioritizedMat` | `FlowOpsMat:L4354` | 未対応 | core | easy | `mergePrioritized` の Mat 版 |
| `mergeSortedMat` | `FlowOpsMat:L4373` | 未対応 | core | easy | `mergeSorted` の Mat 版 |
| `concatMat` | `FlowOpsMat:L4395` | 未対応 | core | easy | `concat` の Mat 版 |
| `concatLazyMat` | `FlowOpsMat:L4415` | 未対応 | core | easy | `concatLazy` の Mat 版 |
| `prependMat` | `FlowOpsMat:L4436` | 未対応 | core | easy | `prepend` の Mat 版 |
| `prependLazyMat` | `FlowOpsMat:L4457` | 未対応 | core | easy | `prependLazy` の Mat 版 |
| `orElseMat` | `FlowOpsMat:L4483` | 未対応 | core | easy | `orElse` の Mat 版 |
| `divertToMat` | `FlowOpsMat:L4508` | 未対応 | core | easy | `divertTo` の Mat 版 |

実装済み: `viaMat`, `toMat`, `alsoToMat`, `wireTapMat`, `watchTermination` (`watchTerminationMat`), `mapMaterializedValue`, `monitorMat`, `monitor`

---

### シェイプ（Shapes）　✅ 実装済み 9/11 (82%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FanInShape3`〜`FanInShape22` | `FanInShape.scala` | 未対応 | core | easy | Pekko はマクロ生成。`FanInShape2` は実装済みだが、3以上のバリアントはなし |
| `UniformFanOutShape` | `UniformFanOutShape.scala` | 未対応 | core | easy | `UniformFanInShape` は実装済みだが、FanOut 側が欠けている |

実装済み：`Shape`, `SourceShape`, `SinkShape`, `FlowShape`, `BidiShape`, `ClosedShape`, `UniformFanInShape`, `FanInShape2`, `FanOutShape2`, `StreamShape`

---

### グラフDSL（GraphDSL）　❌ 実装済み 0/1 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GraphDSL` + `GraphDSL.Builder` | `scaladsl/Graph.scala:L1577` | 未対応 | core | hard | Pekko で複雑なグラフを宣言的に組み立てる主要パターン。fraktor-rs は Flow/Source メソッドによる命令的組み立てのみ |

> **注意**: fraktor-rs では Merge/Broadcast/Balance/Partition 等のグラフプリミティブはすべて Flow/Source のメソッドとして提供されており、GraphDSL が必須でないユースケースの多くはカバー済み。`core/graph/graph_dsl_builder` ディレクトリは存在するが空であり、まだ公開 API として利用可能ではない。

---

### ハブ（Hub）　✅ 実装済み 3/3 (100%)

MergeHub, BroadcastHub, PartitionHub すべて実装済み。（Pekko にも BalanceHub はない）ギャップなし。

---

### ライフサイクル（KillSwitch / Restart）　✅ 実装済み 8/8 (100%)

`UniqueKillSwitch`, `SharedKillSwitch`, `KillSwitches`, `RestartSource`, `RestartFlow`, `RestartSink`, `RestartSettings`, `watchTermination` すべて実装済み。ギャップなし。

---

### RetryFlow　✅ 実装済み 2/2 (100%)

`RetryFlow.with_backoff` および `RetryFlow.with_backoff_and_context` が実装済み。`core/retry_flow.rs` にて指数バックオフ付きリトライを提供。ギャップなし。

---

### フレーミング（Framing）　✅ 実装済み 4/6 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Framing.simpleFramingProtocol` | `Framing.scala:L87` | 未対応 | core | easy | BidiFlow ベースの対称フレーミングプロトコル。`BidiFlow` + 既存 `Framing` の合成で実装可能 |
| `JsonFraming.arrayScanner` | `JsonFraming.scala:L40` | 未対応 | core | easy | JSON 配列要素のストリーミングパーサー。`objectScanner` の姉妹メソッド |

実装済み: `Framing.delimiter`, `Framing.lengthField`, `JsonFraming.objectScanner`, `Compression`（gzip/deflate/inflate）

---

### マテリアライゼーション（Materialization）　✅ 実装済み 5/5 (100%)

`Materializer`, `ActorMaterializer`, `Keep variants` (KeepLeft/KeepRight/KeepBoth/KeepNone), `Attributes`, `FlowMonitor` すべて実装済み。ギャップなし。

---

### キュー（Queue）　✅ 実装済み 5/5 (100%)

`BoundedSourceQueue`, `SourceQueue`, `SourceQueueWithComplete`, `SinkQueue`（`cancel()` メソッド付き）すべて実装済み。ギャップなし。

---

### IO（ファイル・ネットワーク）　✅ 実装済み 2/4 (50%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `StreamConverters` | `scaladsl/StreamConverters.scala` | 未対応 | std | n/a | Java Iterator/InputStream/OutputStream 変換。Rust では不要 |
| `Tcp` | `scaladsl/Tcp.scala` | 未対応 | std | n/a | Akka/Pekko の ActorSystem ベース TCP。Rust では tokio::net で代替 |
| `TLS` | `scaladsl/TLS.scala` | 未対応 | std | n/a | JVM TLS 統合。Rust では rustls 等で代替 |

実装済み：`FileIO`（`from_path` / `to_path` に `IOResult` マテリアライズドバリュー付き）、`IOResult` 型

---

### StreamRefs（分散ストリーム）　❌ 実装済み 0/3 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SinkRef[T]` | `StreamRefs.scala` | 未対応 | core+std | hard | 別ノードへのリモートマテリアライゼーション。actor/remote モジュール連携が必要 |
| `SourceRef[T]` | `StreamRefs.scala` | 未対応 | core+std | hard | 同上 |
| `StreamRefSettings` | `StreamRefSettings.scala` | 未対応 | core | hard | 設定型。上2つに依存 |

---

### その他の型・設定　✅ 実装済み 8/9 (89%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SubstreamCancelStrategy` | `SubstreamCancelStrategy.scala` | 未対応 | core | easy | `groupBy` のキャンセル戦略（Drain/Propagate）。fraktor-rs の `group_by` は現状この制御がない |

実装済み：`DelayStrategy<T>` trait + `FixedDelay` + `LinearIncreasingDelay`、`IOResult`、`Source.combine_mat`、`MergeLatest`、`StatefulMapConcatAccumulator` trait、`CoupledTerminationFlow`、`NeverMaterializedException`、`TooManySubstreamsOpenException`

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）— 7項目

- **`collect`（filter_map 相当）** [core]: `filter` + `map` の合成。`map_option` で代替可能なケースも多い
- **`flatten`** [core]: `flatMapConcat(identity)` への委譲のみ
- **`Sink.never`** [core]: 永遠に完了しない Sink。`futures::future::pending()` 相当
- **`Source.runFold`** [core]: `source.fold(zero)(f).to(Sink.head).run()` のショートカット
- **`Source.runFoldAsync`** [core]: 同上の async 版
- **`Source.runReduce`** [core]: `source.reduce(f).to(Sink.head).run()` のショートカット
- **`Source.runForeach`** [core]: `source.to(Sink.foreach(f)).run()` のショートカット

### Phase 2: easy（単純な新規実装）— 25項目

- **`SubstreamCancelStrategy`** [core]: `group_by` のキャンセル動作を制御する enum（Drain/Propagate）
- **`UniformFanOutShape`** [core]: `UniformFanInShape` と対になる型
- **`FanInShape3`〜`FanInShapeN`** [core]: Rust マクロで生成可能
- **`Sink.foldAsync`** [core]: 非同期 fold Sink。`fold` + `mapAsync` の合成で実装可能
- **`Source.create`** [core]: プロデューサーコールバック版 Source 生成
- **`Source.mergePrioritizedN`** [core]: Source コンパニオンでの N入力優先度付きマージ
- **`Flow.fromSinkAndSourceMat`** [core]: 既存 `fromSinkAndSource` に Mat 制御を追加
- **`Flow.fromSinkAndSourceCoupledMat`** [core]: 同上の Coupled 版
- **`Framing.simpleFramingProtocol`** [core]: `BidiFlow` + 既存 `Framing` の合成
- **`JsonFraming.arrayScanner`** [core]: `objectScanner` と同パターンの配列版
- **`*Mat` バリアント 18個** [core]: 合成オペレーター（`zip`, `merge`, `concat`, `interleave`, `prepend`, `orElse`, `divertTo` 等）のマテリアライズドバリュー制御版。既存の `via_mat` / `also_to_mat` / `wire_tap_mat` と同パターン
  - `zipMat`, `zipAllMat`, `zipWithMat`, `zipLatestMat`, `zipLatestWithMat`
  - `mergeMat`, `mergeLatestMat`, `mergePreferredMat`, `mergePrioritizedMat`, `mergeSortedMat`
  - `interleaveMat`
  - `concatMat`, `concatLazyMat`
  - `prependMat`, `prependLazyMat`
  - `orElseMat`
  - `divertToMat`
  - `flatMapPrefixMat`

### Phase 3: medium（中程度の実装工数）— 4項目

- **`Sink.combine` / `Sink.combineMat`** [core]: 複数 Sink を Broadcast 等で合成する Sink ファクトリ。Source.combine の対称操作
- **`Source.actorRef`** [core]: アクターシステムからのメッセージを Source 化。actor モジュールの `ActorRef` との連携が必要
- **`Source.actorRefWithBackpressure`** [core]: バックプレッシャー制御付きアクターソース

### Phase 4: hard（アーキテクチャ変更を伴う）— 5項目

- **`GraphDSL.Builder`** [core]: 宣言的グラフ構築 DSL。Flow メソッドで代替可能なケースが多いが、複雑なダイヤモンドグラフや非線形パイプラインでは有用
- **`StreamRefs (SinkRef/SourceRef/StreamRefSettings)`** [core+std]: remote モジュールとの統合が必要。分散ストリーミングの基盤（3型）
- **`Sink.actorRef` / `Sink.actorRefWithBackpressure`** [core]: アクターにメッセージを送信する Sink。actor モジュール連携が必要（Source.actorRef と同時に検討すべき）

### 対象外（n/a）— 7項目

- `Tcp` / `TLS`: JVM ネットワークスタック固有。tokio::net/rustls で代替
- `StreamConverters`: Java Iterator/Stream 変換。Rust では不要
- `JavaFlowSupport`: Java API 専用
- `SystemMaterializer`: JVM ActorSystem 依存
- `mapWithResource(AutoCloseable)` 2引数版: Rust では `Drop` trait で自動対応
- `Flow.fromProcessor` / `Flow.fromProcessorMat`: JVM Reactive Streams `Processor` 固有
- `Flow.toProcessor`: 同上

---

## まとめ

**全体カバレッジの評価**: 型単位で 94%（前回同様）。メソッドレベルで精査した結果、実装対象ギャップは 34 個あることが判明。ただし、大半は Phase 1〜2（trivial/easy）に分類され、既存パターンの踏襲で実装可能。

**即座に価値を提供できる未実装機能（Phase 1〜2）**:
- `*Mat` バリアント 18個: Pekko ユーザーがマテリアライゼーション制御で日常的に使用する API。既存の `also_to_mat` / `wire_tap_mat` と同パターンでの実装が可能
- `Sink.never` / `Sink.foldAsync`: 基本的な Sink ファクトリ。1〜2行の委譲で実装可能
- `Source.runFold` / `runReduce` / `runForeach`: 便利メソッド。既存オペレーターの合成のみ
- `collect`（filter_map 相当）: `map_option` でほぼ代替可能だが、名前の親和性のため追加推奨

**実用上の主要ギャップ（Phase 3〜4）**:
- アクター連携（`actorRef` ソース/シンク）: actor モジュールが安定した段階で実装すべき
- `GraphDSL.Builder`: 複雑なグラフ組み立てに必要だが、fraktor-rs は Flow/Source のメソッドチェーンで多くのケースをカバー
- `StreamRefs`: remote モジュールの成熟が前提。分散ストリーミングには必須

## 注記

- fraktor-rs は Pekko に存在しない機能（`mapAsyncPartitioned`, `mapAsyncPartitionedUnordered`, `flatMapPrefix`, `switchMap`, `aggregateWithBoundary`, `groupedAdjacentBy`, `groupedAdjacentByWeighted`, `dropRepeated`, `mergeSequence`, `onErrorResume`, `flattenOptional`, `batchWeighted`, `concatLazy`, `prependLazy`, `distinct`, `distinctBy`, `debounce`, `sample` 等）を実装しており、一部で Pekko を超えている
- `Compression` は `#[cfg(feature="compression")]` フラグで実装済み。デフォルトでは無効
- fraktor-rs の `OperatorCatalog` / `OperatorContract` / `OperatorCoverage` は Pekko に存在しない独自機能（オペレーターの契約管理）
- 一部のオペレーター（`zipLatest`, `mergeAll`, `interleaveAll`, `switchMap`, `extrapolate`, `keepAlive`, `dropWithin` 等）は既存オペレーターへの簡易委譲で実装されており、Pekko と完全に同一のセマンティクスではない場合がある。実用上問題になった時点で個別のロジックファイルを作成することを推奨
- Flow.rs は 4,413 行超に達しており、今後のメソッド追加時（特に *Mat バリアント 18個）にはファイル分割を検討すべき

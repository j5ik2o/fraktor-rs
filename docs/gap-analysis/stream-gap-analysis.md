# stream モジュール ギャップ分析

生成日: 2026-03-20（前回: 2026-03-17）

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | ~79（コアDSL 8、シェイプ 11、GraphDSL系 22、ハブ/ライフサイクル 10、IO 4、その他 24） |
| fraktor-rs 公開型数 | 101（core: 100, std: 1） |
| カバレッジ（型単位） | 74/79 (94%) |
| ギャップ数（型単位・実装対象） | 5 |
| ギャップ数（メソッド単位・実装対象） | 22（前回31 → PR #1124で9項目解消） |
| 対象外（n/a） | 7 |

**結論：** 型単位カバレッジは 94% で高水準。PR #1124 で `flatten`, `collect`, `fold_async`, `merge_prioritized_n`, `simple_framing_protocol`, `Source.run_fold/runReduce/runForeach` 等の9項目を実装し、メソッド単位ギャップが31→22に縮小。残る主要不足は (1) *Mat バリアント（合成オペレーターのマテリアライゼーション制御）18個、(2) Source 側のアクター連携（`actorRef` / `actorRefWithBackpressure`）2個、(3) `Sink.never` / `Sink.combine` / `Sink.combineMat` 3個、(4) GraphDSL / StreamRefs の大型機能である。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core（コアロジック・ポート定義） | 75 | 74 | 99% |
| std（tokio アダプタ） | 4 | 1 | 25% |

**注記**: fraktor-rs の stream モジュールには `typed/` サブ層は存在しない。
std 層には `FileIO` が実装済み（`from_path` / `to_path` に `IOResult` マテリアライズドバリュー付き）。
IO 関連のうち StreamConverters / Tcp / TLS は Pekko では std 相当だが、Rust では外部クレートで代替するため n/a。

---

## カテゴリ別ギャップ

### コアDSL（Source / Flow / Sink）　✅ 実装済み 8/8 (100%)

Source, Flow, Sink, BidiFlow, FlowWithContext, SourceWithContext, SubFlow, RunnableGraph すべて実装済み。ギャップなし。

---

### Source ファクトリメソッド　✅ 実装済み 38/40 (95%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.actorRef` | `Source.scala:L678` | 未対応 | core | medium | アクターへの直接参照でメッセージをストリーム化。actor モジュールとの連携が必要 |
| `Source.actorRefWithBackpressure` | `Source.scala:L715` | 未対応 | core | medium | バックプレッシャー付きアクターソース。actor モジュールとの連携が必要 |

実装済み: `empty`, `fromOption`, `fromIterator`, `from`, `fromArray`, `single`, `failed`, `never`, `repeat`, `cycle`, `iterate`, `range`, `tick`, `unfold`, `unfoldAsync`, `unfoldResource`, `unfoldResourceAsync`, `future`, `futureSource`, `completionStage`, `completionStageSource`, `lazyFuture`, `lazyFutureSource`, `lazyCompletionStage`, `lazyCompletionStageSource`, `lazySingle`, `lazySource`, `maybe`, `queue`, `queueWithOverflow`, `queueUnbounded`, `fromMaterializer`, `fromPublisher`, `combine`, `combineMat`, `zipN`, `zipWithN`, `create`, `mergePrioritizedN`

`Source.create` は [source.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/std/source.rs) に std 層実装として存在する。
`Source.mergePrioritizedN` は PR #1124 で `source.rs:L239` に実装済み。

---

### Source 便利メソッド　✅ 実装済み 5/5 (100%)

PR #1124 で `run_fold`, `run_fold_async`, `run_reduce`, `run_foreach` を実装。ギャップなし。

実装済み: `runWith`, `runFold`（`source.rs:L794`）, `runFoldAsync`（`source.rs:L812`）, `runReduce`（`source.rs:L831`）, `runForeach`（`source.rs:L847`）

---

### Sink ファクトリメソッド　✅ 実装済み 31/34 (91%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Sink.never` | `Sink.scala:L338` | 未対応 | core | trivial | 永遠に完了しない Sink。テスト等で使用 |
| `Sink.combine` | `Sink.scala:L362` | 未対応 | core | medium | 複数 Sink をファンアウトで合成。Broadcast 等と組み合わせ |
| `Sink.combineMat` | `Sink.scala:L383` | 未対応 | core | medium | `combine` のマテリアライズドバリュー制御版 |
実装済み: `ignore`, `foreach`, `foreachAsync`, `cancelled`, `none`, `onComplete`, `fromSubscriber`, `futureSink`, `lazySink`, `lazyFutureSink`, `lazyCompletionStageSink`, `collect`, `collection`, `seq`, `javaCollector`, `takeLast`, `toPath`, `count`, `exists`, `forall`, `headOption`, `lastOption`, `queue`, `fold`, `foldWhile`, `foldAsync`, `head`, `last`, `reduce`, `fromGraph`, `fromMaterializer`, `source`, `asPublisher`, `preMaterialize`

`Sink.foldAsync` は PR #1124 で `sink.rs:L356` に実装済み。

別名で実装済み:
- `Sink.actorRef` → [actor_sink.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/stage/actor_sink.rs) の `ActorSink::actor_ref`
- `Sink.actorRefWithBackpressure` → [actor_sink.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/stream/src/core/stage/actor_sink.rs) の `ActorSink::actor_ref_with_backpressure`

---

### Flow ファクトリメソッド　✅ 実装済み 8/11 (73%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Flow.fromSinkAndSourceMat` | `Flow.scala:L579` | 未対応 | core | easy | `fromSinkAndSource` のマテリアライズドバリュー制御版 |
| `Flow.fromSinkAndSourceCoupledMat` | `Flow.scala:L674` | 未対応 | core | easy | `fromSinkAndSourceCoupled` のマテリアライズドバリュー制御版 |
| `Flow.toProcessor` | `Flow.scala:L381` | 未対応 | core | n/a | Reactive Streams `Processor` 変換。JVM 固有 |

実装済み: `new` (identity), `fromFunction`, `fromGraph`, `fromMaterializer`, `fromSinkAndSource`, `fromSinkAndSourceCoupled`, `lazyFlow`, `lazyFutureFlow`, `lazyCompletionStageFlow`, `optionalVia`

---

### オペレーター（変換・フィルタ）　✅ 実装済み 91/91 (100%)

PR #1124 で `collect`（`map_option` への委譲、`flow.rs:L1211`）と `flatten`（Source/Flow 両方、`flow.rs:L1047`, `source.rs:L1894`）を実装。ギャップなし。

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

### フレーミング（Framing）　✅ 実装済み 5/6 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `JsonFraming.arrayScanner` | `JsonFraming.scala:L40` | 未対応 | core | easy | JSON 配列要素のストリーミングパーサー。`objectScanner` の姉妹メソッド |

実装済み: `Framing.delimiter`, `Framing.lengthField`, `Framing.simpleFramingProtocol`, `JsonFraming.objectScanner`, `Compression`（gzip/deflate/inflate）

`Framing.simpleFramingProtocol` は PR #1124 で `framing.rs:L79` に BidiFlow ベースで実装済み。

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

### Phase 1: trivial（既存組み合わせで即実装可能）— 1項目

- **`Sink.never`** [core]: 永遠に完了しない Sink。`futures::future::pending()` 相当

PR #1124 で解消済み: ~~`collect`~~, ~~`flatten`~~, ~~`Source.runFold`~~, ~~`Source.runFoldAsync`~~, ~~`Source.runReduce`~~, ~~`Source.runForeach`~~

### Phase 2: easy（単純な新規実装）— 24項目

- **`SubstreamCancelStrategy`** [core]: `group_by` のキャンセル動作を制御する enum（Drain/Propagate）
- **`UniformFanOutShape`** [core]: `UniformFanInShape` と対になる型
- **`FanInShape3`〜`FanInShapeN`** [core]: Rust マクロで生成可能
- **`Flow.fromSinkAndSourceMat`** [core]: 既存 `fromSinkAndSource` に Mat 制御を追加
- **`Flow.fromSinkAndSourceCoupledMat`** [core]: 同上の Coupled 版
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

PR #1124 で解消済み: ~~`Sink.foldAsync`~~, ~~`Source.mergePrioritizedN`~~, ~~`Framing.simpleFramingProtocol`~~

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

**全体カバレッジの評価**: 型単位で 94%。メソッドレベルの実装対象ギャップは 22 個（前回34 → PR #1124で9項目解消）。大半は Phase 2（easy）の `*Mat` バリアント 18個で、既存パターンの踏襲で実装可能。

**PR #1124 で解消された項目（9個）**:
- `collect`（filter_map 相当）、`flatten`（Source/Flow 両方）
- `Sink.foldAsync`（非同期 fold Sink）
- `Source.mergePrioritizedN`（N入力優先度付きマージ）
- `Framing.simpleFramingProtocol`（BidiFlow ベースのフレーミングプロトコル）
- `Source.runFold` / `runFoldAsync` / `runReduce` / `runForeach`（便利メソッド）

**即座に価値を提供できる未実装機能（Phase 1〜2）**:
- `*Mat` バリアント 18個: Pekko ユーザーがマテリアライゼーション制御で日常的に使用する API。既存の `also_to_mat` / `wire_tap_mat` と同パターンでの実装が可能
- `Sink.never`: 永遠に完了しない Sink。1行の委譲で実装可能

**実用上の主要ギャップ（Phase 3〜4）**:
- アクター連携（`actorRef` ソース/シンク）: actor モジュールが安定した段階で実装すべき
- `GraphDSL.Builder`: 複雑なグラフ組み立てに必要だが、fraktor-rs は Flow/Source のメソッドチェーンで多くのケースをカバー
- `StreamRefs`: remote モジュールの成熟が前提。分散ストリーミングには必須

## 並行実行可能タスク一覧

以下のタスクは互いに依存関係がなく、完全に並行して実行できる。
各タスクは変更対象ファイルが競合しないよう分割されている。

### 並行グループ A: 独立した型・enum 追加（ファイル競合なし）

| # | タスク | 変更ファイル | 難易度 | 備考 |
|---|--------|-------------|--------|------|
| A1 | `Sink.never` 追加 | `sink.rs` | trivial | 永遠に完了しない Sink。1メソッド追加 |
| A2 | `UniformFanOutShape` 追加 | `core/shape/uniform_fan_out_shape.rs`（新規）+ `core/shape.rs` | easy | `UniformFanInShape` と対になる型 |
| A3 | `FanInShape3`〜`FanInShapeN` マクロ生成 | `core/shape/fan_in_shape*.rs`（新規）+ `core/shape.rs` | easy | `FanInShape2` をテンプレートにマクロ化 |
| A4 | `SubstreamCancelStrategy` enum 追加 | `core/substream_cancel_strategy.rs`（新規）+ `core/stage/flow.rs`（`group_by` 引数追加） | easy | `Drain`/`Propagate` の2バリアント |
| A5 | `JsonFraming.arrayScanner` 追加 | `core/json_framing.rs` | easy | `objectScanner` と同パターンの配列版 |

### 並行グループ B: Flow メソッド追加（`flow.rs` を分割して並行化）

`*Mat` バリアント 18個は全て `flow.rs` への追加だが、以下のサブグループに分けることで
ファイル末尾への追加位置を調整し並行化できる。ただし **`flow.rs` が 4,400行超のため、
先にファイル分割（`flow_mat_ops.rs` 等）を行うことを強く推奨**する。

ファイル分割を行った場合、以下は完全に並行実行可能:

| # | タスク | 対象オペレーター | 難易度 | 備考 |
|---|--------|----------------|--------|------|
| B1 | Zip 系 Mat バリアント | `zipMat`, `zipAllMat`, `zipWithMat`, `zipLatestMat`, `zipLatestWithMat` | easy | 5メソッド |
| B2 | Merge 系 Mat バリアント | `mergeMat`, `mergeLatestMat`, `mergePreferredMat`, `mergePrioritizedMat`, `mergeSortedMat` | easy | 5メソッド |
| B3 | Concat/Prepend 系 Mat バリアント | `concatMat`, `concatLazyMat`, `prependMat`, `prependLazyMat` | easy | 4メソッド |
| B4 | その他 Mat バリアント | `interleaveMat`, `orElseMat`, `divertToMat`, `flatMapPrefixMat` | easy | 4メソッド |
| B5 | `Flow.fromSinkAndSourceMat` | 既存 `fromSinkAndSource` に Mat 制御を追加 | easy | 1メソッド |
| B6 | `Flow.fromSinkAndSourceCoupledMat` | Coupled 版 | easy | 1メソッド |

### 並行グループ C: medium 以上（外部依存あり）

| # | タスク | 変更ファイル | 難易度 | 依存 | 備考 |
|---|--------|-------------|--------|------|------|
| C1 | `Sink.combine` / `Sink.combineMat` | `sink.rs` | medium | なし | Broadcast と組み合わせた Sink 合成 |
| C2 | `Source.actorRef` + `Source.actorRefWithBackpressure` | `source.rs` + actor モジュール | medium | actor モジュール安定後 | アクターソース |
| C3 | `Sink.actorRef` + `Sink.actorRefWithBackpressure` | `sink.rs` + actor モジュール | medium | C2 と同時着手可 | アクターシンク |
| C4 | `GraphDSL.Builder` | `core/graph/graph_dsl_builder/`（新規） | hard | なし | 宣言的グラフ構築 DSL |
| C5 | `StreamRefs` (SinkRef/SourceRef/Settings) | `core/` + `std/` + remote モジュール | hard | remote モジュール成熟後 | 分散ストリーミング |

### 最大並行度の実行計画

```
同時実行可能なタスク数: 最大 11（ファイル分割前提）

Phase 1 (即時着手可能 — 全て並行):
  A1 | A2 | A3 | A4 | A5 | B5 | B6 | C1 | C4
  → 9タスクを同時実行可能

Phase 2 (flow.rs 分割後 — 全て並行):
  B1 | B2 | B3 | B4
  → 4タスクを同時実行可能
  → Phase 1 の A1〜A5, C1, C4 と並行しても可

Phase 3 (外部依存解消後):
  C2 | C3 (actor モジュール安定後)
  C5     (remote モジュール成熟後)
```

### 前提条件

- **B1〜B4** を並行実行するには、先に `flow.rs` から Mat オペレーターを別ファイル（例: `flow_mat_ops.rs`）に分離すること。分離しない場合は B1〜B4 を逐次実行する必要がある
- **C2 と C3** は actor モジュールの `ActorRef` 型に依存するため、actor モジュールの安定後に着手
- **C5** は remote モジュールのシリアライゼーション基盤に依存

---

## 注記

- fraktor-rs は Pekko に存在しない機能（`mapAsyncPartitioned`, `mapAsyncPartitionedUnordered`, `flatMapPrefix`, `switchMap`, `aggregateWithBoundary`, `groupedAdjacentBy`, `groupedAdjacentByWeighted`, `dropRepeated`, `mergeSequence`, `onErrorResume`, `flattenOptional`, `batchWeighted`, `concatLazy`, `prependLazy`, `distinct`, `distinctBy`, `debounce`, `sample` 等）を実装しており、一部で Pekko を超えている
- `Compression` は `#[cfg(feature="compression")]` フラグで実装済み。デフォルトでは無効
- fraktor-rs の `OperatorCatalog` / `OperatorContract` / `OperatorCoverage` は Pekko に存在しない独自機能（オペレーターの契約管理）
- 一部のオペレーター（`zipLatest`, `mergeAll`, `interleaveAll`, `switchMap`, `extrapolate`, `keepAlive`, `dropWithin` 等）は既存オペレーターへの簡易委譲で実装されており、Pekko と完全に同一のセマンティクスではない場合がある。実用上問題になった時点で個別のロジックファイルを作成することを推奨
- Flow.rs は 4,413 行超に達しており、今後のメソッド追加時（特に *Mat バリアント 18個）にはファイル分割を検討すべき

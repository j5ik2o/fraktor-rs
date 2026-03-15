# streams モジュール ギャップ分析

Pekko互換仕様を実装する必要があります。
Phase 4: hard（アーキテクチャ変更を伴う）は対象外です。Phase 1から3を対応してください。

生成日: 2026-03-14（前回: 2026-03-12）

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | ~79（コアDSL 8、シェイプ 9、GraphDSL系 22、ハブ/ライフサイクル 10、IO 4、その他 26） |
| fraktor-rs 公開型数 | ~89（core: 89, std: 0） |
| カバレッジ（型単位） | 62/79 (78%) |
| ギャップ数（実装対象） | 9 |
| 対象外（n/a） | 8 |

**結論：** fraktor-rs は Pekko scaladsl のコアAPI（オペレーター・ライフサイクル・ハブ）を高いカバレッジで実装済み。
主なギャップは `RetryFlow`、`GraphDSL`、`JsonFraming`、`DelayStrategy` 独立型、`SubstreamCancelStrategy`、`IOResult` 型、`StreamRefs`。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core（コアロジック・ポート定義） | 75 | 62 | 83% |
| std（tokio アダプタ） | 4 | 0 | 0% |

**注記**: fraktor-rs の streams モジュールには `typed/` サブ層は存在しない。
std 層には `std/source.rs` が1ファイルのみ存在するが、公開型の定義はない。
IO 関連（FileIO, StreamConverters, Tcp, TLS）は Pekko では std 相当だが、Rust では外部クレートで代替するため大半は n/a。

---

## カテゴリ別ギャップ

### コアDSL（Source / Flow / Sink）　✅ 実装済み 8/8 (100%)

Source, Flow, Sink, BidiFlow, FlowWithContext, SourceWithContext, SubFlow, RunnableGraph すべて実装済み。ギャップなし。

**メソッド数比較**（ユニークメソッド名ベース）:

| 型 | Pekko | fraktor-rs | 備考 |
|----|-------|-----------|------|
| Source (コンストラクタ + オペレーター) | 50 | 131 | fraktor-rs は FlowOps 相当を Source に直接実装 |
| Flow (FlowOps + コンストラクタ) | 164 | 163 | ほぼ同数 |
| Sink | 40 | 45 | fraktor-rs がやや多い |

---

### オペレーター（変換・フィルタ）　✅ 実装済み 約87/90 (97%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `collect(pf)` | `Flow.scala:L1753` | 未対応 | core | trivial | Rust に PartialFunction はないが `filter_map` 相当として検討可。`collectFirst`/`collectWhile`/`collectType` は実装済み |
| `delayWith(strategySupplier)` 型安全版 | `Flow.scala:L2301` | 部分実装 | core | easy | `delay_with` はあるが `DelayStrategy` trait が独立型として未定義。現状 `delay` への委譲 |
| `mapWithResource(AutoCloseable)` 2引数版 | `Flow.scala:L1258` | 未対応 | core | trivial | `map_with_resource(factory, f)` の AutoCloseable 版。Rust では `Drop` trait で代替可 |

**主要オペレーター実装済みリスト**（67個のロジックファイル + インライン実装）:

`map`, `mapConcat`, `mapAsync`, `mapAsyncUnordered`, `mapAsyncPartitioned`, `mapAsyncPartitionedUnordered`, `mapWithResource`, `mapOption`, `mapError`, `statefulMap`, `statefulMapConcat`, `filter`, `filterNot`, `take`, `takeWhile`, `takeUntil`, `takeWithin`, `drop`, `dropWhile`, `dropWithin`, `dropRepeated`, `scan`, `scanAsync`, `fold`, `foldAsync`, `reduce`, `grouped`, `groupedWithin`, `groupedWeighted`, `groupedWeightedWithin`, `groupedAdjacentBy`, `groupedAdjacentByWeighted`, `sliding`, `buffer`, `throttle`, `debounce`, `delay`, `delayWith`, `initialDelay`, `expand`, `extrapolate`, `conflateWithSeed`, `batch`, `batchWeighted`, `sample`, `log`, `logWithMarker`, `flatMapConcat`, `flatMapMerge`, `flatMapPrefix`, `flattenMerge`, `switchMap`, `intersperse`, `wireTap`, `alsoTo`, `alsoToAll`, `divertTo`, `aggregateWithBoundary`, `collectFirst`, `collectWhile`, `collectType`, `doOnFirst`, `doOnCancel`, `dropRepeated`, `mergeSequence`, `ask`, `watch`, `keepAlive`, `limitWeighted`, `prefixAndTail`, `backpressureTimeout`, `completionTimeout`, `idleTimeout`, `initialTimeout`, `onErrorComplete`, `onErrorContinue`, `onErrorResume`, `recover`, `recoverWith`, `recoverWithRetries`, `contramap`, `flattenOptional`, `concatLazy`, `concatAllLazy`, `prependLazy`, `orElse`

**注意**: 一部のオペレーター（`zipLatest`, `mergeAll`, `interleaveAll`, `concatAllLazy`, `flattenMerge`, `switchMap`, `extrapolate`, `keepAlive`, `dropWithin`, `delayWith`, `groupedAdjacentBy` 等）は既存オペレーターへの委譲で実装されており、独立したロジックファイルを持たない。セマンティクスが完全に同一でないケースがある可能性に注意。

---

### エラーハンドリング　✅ 実装済み 7/7 (100%)

`recover`, `recoverWith`, `recoverWithRetries`, `onErrorComplete`, `onErrorContinue`, `onErrorResume`, `mapError` すべて実装済み。ギャップなし。

---

### ファンイン / ファンアウト　✅ 実装済み 23/23 (100%)

グラフプリミティブ実装済み：`Merge`, `MergePreferred`, `MergePrioritized`, `MergeSorted`, `MergeLatest`, `MergeAll`, `MergeSequence`, `Interleave`, `InterleaveAll`, `Broadcast`, `WireTap`, `Partition`, `Balance`, `Zip`, `ZipLatest`, `ZipWith`, `ZipLatestWith`, `ZipN`, `ZipWithN`, `ZipWithIndex`, `ZipAll`, `Unzip`, `UnzipWith`, `Concat`, `ConcatLazy`, `ConcatAllLazy`, `OrElse`, `PrependLazy`, `AlsoTo`, `DivertTo`

---

### シェイプ（Shapes）　✅ 実装済み 7/9 (78%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FanInShape2` / `FanInShape3` ... | `FanInShape.scala` | 未対応 | core | medium | Pekko は `FanInShape1`〜`FanInShape22` のバリアント群をマクロ生成。GraphDSL と組み合わせて使用 |
| `FanOutShape2` / `FanOutShape3` ... | `FanOutShape.scala` | 未対応 | core | medium | 同上。GraphDSL と組み合わせて使用 |

実装済み：`Shape`, `SourceShape`, `SinkShape`, `FlowShape`, `BidiShape`, `ClosedShape`, `UniformFanInShape`

---

### グラフDSL（GraphDSL）　❌ 実装済み 0/1 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GraphDSL` + `GraphDSL.Builder` | `scaladsl/Graph.scala:L1577` | 未対応 | core | hard | Pekko で複雑なグラフを宣言的に組み立てる主要パターン。fraktor-rs は Flow/Source メソッドによる命令的組み立てのみ |

> **注意**: fraktor-rs では Merge/Broadcast/Balance/Partition 等のグラフプリミティブはすべて Flow/Source のメソッドとして提供されており、GraphDSL が必須でないユースケースの多くはカバー済み。`core/graph/` に `graph_dsl_builder` ディレクトリは存在するが、まだ公開 API として利用可能ではない。

---

### ハブ（Hub）　✅ 実装済み 3/3 (100%)

MergeHub, BroadcastHub, PartitionHub すべて実装済み。（Pekko にも BalanceHub はない）ギャップなし。

---

### ライフサイクル（KillSwitch / Restart）　✅ 実装済み 8/8 (100%)

`UniqueKillSwitch`, `SharedKillSwitch`, `KillSwitches`, `RestartSource`, `RestartFlow`, `RestartSink`, `RestartSettings`, `watchTermination` すべて実装済み。ギャップなし。

---

### RetryFlow　❌ 実装済み 0/2 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RetryFlow.withBackoff` | `scaladsl/RetryFlow.scala:L53` | 未対応 | core | medium | 個別要素に対するエクスポネンシャルバックオフ付きリトライ。`@ApiMayChange`（実験的） |
| `RetryFlow.withBackoffAndContext` | `scaladsl/RetryFlow.scala:L92` | 未対応 | core | medium | コンテキスト付きバージョン |

---

### フレーミング（Framing）　〜 実装済み 1/2 (50%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `JsonFraming.objectScanner` | `scaladsl/JsonFraming.scala` | 未対応 | core | easy | JSON オブジェクト単位でのフレーミング。バイト列ストリームから JSON オブジェクトを切り出す |

実装済み：`Framing`（バイト境界フレーミング）、`Compression`（gzip/deflate/inflate、`#[cfg(feature="compression")]`）

---

### マテリアライゼーション（Materialization）　✅ 実装済み 4/4 (100%)

`Materializer`, `ActorMaterializer`, `Keep variants` (KeepLeft/KeepRight/KeepBoth/KeepNone), `Attributes` すべて実装済み。ギャップなし。

---

### キュー（Queue）　✅ 実装済み 4/5 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SinkQueueWithCancel[T]` | `scaladsl/Queue.scala` | 部分実装 | core | trivial | `SinkQueue` にキャンセルメソッドを追加するだけ |

実装済み：`BoundedSourceQueue`, `SourceQueue`, `SourceQueueWithComplete`, `SinkQueue`

---

### IO（ファイル・ネットワーク）　〜 実装済み 1/4 (25%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FileIO.toPath` / `fromPath` （完全版） | `scaladsl/FileIO.scala` | 部分実装 | std | easy | `from_path`/`to_path` はあるが IOResult マテリアライズドバリューが未実装。`IOResult` 型自体も未実装 |
| `StreamConverters` | `scaladsl/StreamConverters.scala` | 未対応 | std | n/a | Java Iterator/InputStream/OutputStream 変換。Rust では不要 |
| `Tcp` | `scaladsl/Tcp.scala` | 未対応 | std | n/a | Akka/Pekko の ActorSystem ベース TCP。Rust では tokio::net で代替 |
| `TLS` | `scaladsl/TLS.scala` | 未対応 | std | n/a | JVM TLS 統合。Rust では rustls 等で代替 |

---

### StreamRefs（分散ストリーム）　❌ 実装済み 0/3 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SinkRef[T]` | `StreamRefs.scala` | 未対応 | core+std | hard | 別ノードへのリモートマテリアライゼーション。actor/remote モジュール連携が必要 |
| `SourceRef[T]` | `StreamRefs.scala` | 未対応 | core+std | hard | 同上 |
| `StreamRefSettings` | `StreamRefSettings.scala` | 未対応 | core | hard | 設定型。上2つに依存 |

---

### その他の型・設定　〜 実装済み 6/9 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DelayStrategy[T]` | `scaladsl/DelayStrategy.scala` | 未対応 | core | easy | `delayWith` に渡す戦略型の独立定義。`fixedDelay`, `linearIncreasingDelay` 等の標準実装付き |
| `SubstreamCancelStrategy` | `SubstreamCancelStrategy.scala` | 未対応 | core | easy | `groupBy` のキャンセル戦略（Drain/Propagate）。現状 fraktor は enum 定義なし |
| `Source.combineMat` | `Source.scala:L765` | 部分実装 | core | easy | `combine` はあるが `combineMat`（マット値を結合）が未対応 |
| `IOResult` 型 | `IOResult.scala` | 未対応 | core | easy | FileIO のマテリアライズドバリュー型。バイト数・完了状態を保持 |
| `MergeLatest` 独立型 | `scaladsl/MergeLatest.scala` | ✅ `merge_latest` メソッドで実装 | — | — | — |
| `StatefulMapConcatAccumulator` | `scaladsl/StatefulMapConcatAccumulator.scala` | 未対応 | core | trivial | `statefulMapConcat` のアキュムレーター型。現状不要 |
| `CoupledTerminationFlow` | `scaladsl/CoupledTerminationFlow.scala` | ✅ `from_sink_and_source_coupled` で対応 | — | — | — |
| `NeverMaterializedException` | `NeverMaterializedException.scala` | 未対応 | core | trivial | `Source.maybe` の未解決 Promise 時のエラー型 |
| `TooManySubstreamsOpenException` | `TooManySubstreamsOpenException.scala` | 部分実装 | core | trivial | StreamError に統合済みの可能性あり |

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- **`SinkQueueWithCancel`** [core]: `SinkQueue` に `cancel()` メソッドを追加する
- **`NeverMaterializedException`** [core]: 専用エラー型の追加
- **`StatefulMapConcatAccumulator`** [core]: 必要性が低いが、API 互換性のために追加可

### Phase 2: easy（単純な新規実装）

- **`JsonFraming`** [core]: JSON オブジェクト境界スキャナーの実装
- **`DelayStrategy[T]`** [core] + `LinearIncreasingDelay`: `delay_with` の型安全化
- **`SubstreamCancelStrategy`** [core]: `group_by` のキャンセル動作を制御する enum の追加
- **`IOResult` 型** [core] + **FileIO 完全版** [std]: `from_path`/`to_path` のマテリアライズドバリューを IOResult に
- **`Source.combineMat`** [core]: 複数 Source を結合してマット値を保持するコンストラクタ追加

### Phase 3: medium（中程度の実装工数）

- **`RetryFlow.withBackoff`** [core]: 個別要素の指数バックオフリトライ（`@ApiMayChange` のため優先度低め）
- **`FanInShape2`/`FanOutShape3` 等バリアント** [core]: 多ポート型シェイプの生成（GraphDSL と連動）

### Phase 4: hard（アーキテクチャ変更を伴う）

- **`GraphDSL.Builder`** [core]: 宣言的グラフ構築 DSL（Flow メソッドで代替可能であれば低優先度）
- **`StreamRefs (SinkRef/SourceRef)`** [core+std]: remote モジュールとの統合が必要

### 対象外（n/a）

- `Tcp` / `TLS`: JVM ネットワークスタック固有。tokio::net/rustls で代替
- `StreamConverters`: Java Iterator/Stream 変換。Rust では不要
- `JavaFlowSupport`: Java API 専用
- `ActorRef`-based `Sink.actorRef` / `Source.actorRef`: fraktor-rs の actor モジュール連携で別途対応
- `MaterializerState` / `SystemMaterializer`: JVM ActorSystem 依存

---

## まとめ

**全体カバレッジの評価**: コアDSL・オペレーター（97%）・エラーハンドリング（100%）・ファンイン/ファンアウト（100%）・ライフサイクル（100%）・ハブ（100%）は極めて高いカバレッジ。Pekko の主要ユースケースの大部分は実装済み。

**即座に価値を提供できる未実装機能（Phase 1〜2）**:
- `JsonFraming`: バイトストリームから JSON オブジェクトを切り出す実用的機能
- `DelayStrategy`: `delay_with` の型安全な独立型定義
- `IOResult` + FileIO 完全版: ファイル I/O のマテリアライズドバリュー

**実用上の主要ギャップ（Phase 3〜4）**:
- `GraphDSL.Builder`: 複雑なグラフ組み立てに必要だが、fraktor-rs は Flow/Source のメソッドチェーンで同等のことが可能。必要性を慎重に見極めるべき
- `RetryFlow`: 実験的 API（`@ApiMayChange`）のため、Pekko 側が安定したタイミングで実装を検討

**YAGNI 観点での省略推奨**:
- `GraphDSL.Builder`: fraktor-rs の命令的 API で代替可能なため、現時点では実装不要
- `StreamRefs`: remote モジュールの成熟が前提。現段階では持ち越し推奨
- TCP/TLS/StreamConverters: Rust エコシステムに専用クレートがあり、fraktor-rs が責任を持つべき範囲ではない
- `FanInShape` バリアント群: GraphDSL なしでは使用機会がほぼない

## 注記

- fraktor-rs は Pekko に存在しない機能（`mapAsyncPartitioned`, `mapAsyncPartitionedUnordered`, `flatMapPrefix`, `switchMap`, `aggregateWithBoundary`, `groupedAdjacentBy`, `groupedAdjacentByWeighted`, `dropRepeated`, `mergeSequence`, `onErrorResume`, `flattenOptional`, `batchWeighted`, `concatLazy`, `prependLazy` 等）を実装しており、一部で Pekko を超えている
- `Compression` は `#[cfg(feature="compression")]` フラグで実装済み。デフォルトでは無効
- fraktor-rs の `OperatorCatalog` / `OperatorContract` / `OperatorCoverage` は Pekko に存在しない独自機能（オペレーターの契約管理）
- 一部のオペレーター（`zipLatest`, `mergeAll`, `interleaveAll`, `switchMap`, `extrapolate`, `keepAlive`, `dropWithin` 等）は既存オペレーターへの簡易委譲で実装されており、Pekko と完全に同一のセマンティクスではない場合がある。実用上問題になった時点で個別のロジックファイルを作成することを推奨
- Flow.rs は 4,413 行に達しており、今後のメソッド追加時にはファイル分割を検討すべき

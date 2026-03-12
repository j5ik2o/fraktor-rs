# streams モジュール ギャップ分析

生成日: 2026-03-12

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | ~79（コアDSL 8、シェイプ 9、GraphDSL系 22、ハブ/ライフサイクル 10、IO 4、その他 26） |
| fraktor-rs 公開型数 | ~91 |
| カバレッジ（型単位） | 62/79 (78%) |
| ギャップ数（実装対象） | 9 |
| 対象外（n/a） | 8 |

**結論：** fraktor-rs は Pekko scaladsl のコアAPI（オペレーター・ライフサイクル・ハブ）を高いカバレッジで実装済み。
主なギャップは `RetryFlow`、`GraphDSL`、`JsonFraming`、`DelayStrategy` 独立型、`SubstreamCancelStrategy`、`IOResult` 型、`StreamRefs`。

---

## カテゴリ別ギャップ

### コアDSL（Source / Flow / Sink）　✅ 実装済み 8/8 (100%)

Source, Flow, Sink, BidiFlow, FlowWithContext, SourceWithContext, SubFlow, RunnableGraph すべて実装済み。ギャップなし。

---

### オペレーター（変換・フィルタ）　✅ 実装済み 約83/90 (92%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `mapWithResource(AutoCloseable)` | `Flow.scala:L1258` | 未対応 | trivial | `mapWithResource(factory, f)` の AutoCloseable 2引数版 |
| `collect(pf)` | `Flow.scala:L1753` | 未対応 | trivial | Rust では PartialFunction は使えないが `filter_map` 相当の糖衣構文として検討可 |
| `delayWith(strategySupplier)` 型安全版 | `Flow.scala:L2301` | 部分実装 | easy | `delay_with` はあるが `DelayStrategy` 型が独立型として未定義 |

主要オペレーター実装済みリスト：`map`, `mapConcat`, `mapAsync`, `mapAsyncUnordered`, `mapAsyncPartitioned`, `mapWithResource`, `statefulMap`, `statefulMapConcat`, `filter`, `filterNot`, `take`, `takeWhile`, `takeUntil`, `takeWithin`, `drop`, `dropWhile`, `scan`, `scanAsync`, `fold`, `foldAsync`, `reduce`, `grouped`, `groupedWithin`, `groupedWeighted`, `groupedWeightedWithin`, `groupedAdjacentBy`, `sliding`, `buffer`, `throttle`, `debounce`, `delay`, `initialDelay`, `expand`, `extrapolate`, `conflateWithSeed`, `batch`, `batchWeighted`, `log`, `logWithMarker`, `flatMapConcat`, `flatMapMerge`, `switchMap`, `intersperse`, `wireTap`, `alsoTo`, `alsoToAll`, `divertTo`, `aggregateWithBoundary`, `flatMapPrefix`, `collectFirst`, `collectWhile`, `collectType`, `doOnFirst`, `doOnCancel`, `distinct`, `dropRepeated`, `mergeSequence`

---

### エラーハンドリング　✅ 実装済み 6/6 (100%)

`recover`, `recoverWith`, `recoverWithRetries`, `onErrorComplete`, `onErrorContinue`, `mapError` すべて実装済み。ギャップなし。

---

### シェイプ（Shapes）　✅ 実装済み 7/9 (78%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `FanInShape2` / `FanInShape3` ... | `FanInShape.scala` | 未対応 | medium | Pekko は `FanInShape1`〜`FanInShape22` のバリアント群を生成。GraphDSL と組み合わせて使用 |
| `FanOutShape2` / `FanOutShape3` ... | `FanOutShape.scala` | 未対応 | medium | 同上。GraphDSL と組み合わせて使用 |

実装済み：`Shape`, `SourceShape`, `SinkShape`, `FlowShape`, `BidiShape`, `ClosedShape`, `UniformFanInShape`

---

### グラフDSL（GraphDSL）　❌ 実装済み 0/1 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `GraphDSL` + `GraphDSL.Builder` | `Graph.scala:L1577` | 未対応 | hard | Pekko で複雑なグラフを宣言的に組み立てる主要パターン。fraktor-rs は Flow/Source メソッドによる命令的組み立てのみ |

> **注意**: fraktor-rs では Merge/Broadcast/Balance/Partition 等のグラフプリミティブはすべて Flow/Source のメソッドとして提供されており、GraphDSL が必須でないユースケースの多くはカバー済み。

グラフプリミティブ実装済み：`Merge`, `MergePreferred`, `MergePrioritized`, `MergeSorted`, `MergeLatest`, `MergeSequence`, `Interleave`, `Broadcast`, `WireTap`, `Partition`, `Balance`, `Zip`, `ZipLatest`, `ZipWith`, `ZipLatestWith`, `ZipN`, `ZipWithN`, `ZipWithIndex`, `ZipAll`, `Unzip`, `UnzipWith`, `Concat`, `OrElse`

---

### ハブ（Hub）　✅ 実装済み 3/3 (100%)

MergeHub, BroadcastHub, PartitionHub すべて実装済み。（Pekko にも BalanceHub はない）ギャップなし。

---

### ライフサイクル（KillSwitch / Restart）　✅ 実装済み 8/8 (100%)

`UniqueKillSwitch`, `SharedKillSwitch`, `KillSwitches`, `RestartSource`, `RestartFlow`, `RestartSink`, `RestartSettings`, `watchTermination` すべて実装済み。ギャップなし。

---

### RetryFlow　❌ 実装済み 0/2 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RetryFlow.withBackoff` | `scaladsl/RetryFlow.scala:L53` | 未対応 | medium | 個別要素に対するエクスポネンシャルバックオフ付きリトライ。`@ApiMayChange`（実験的） |
| `RetryFlow.withBackoffAndContext` | `scaladsl/RetryFlow.scala:L92` | 未対応 | medium | コンテキスト付きバージョン |

---

### フレーミング（Framing）　〜 実装済み 1/2 (50%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `JsonFraming.objectScanner` | `scaladsl/JsonFraming.scala` | 未対応 | easy | JSON オブジェクト単位でのフレーミング。バイト列ストリームから JSON オブジェクトを切り出す |

実装済み：`Framing`（バイト境界フレーミング）、`Compression`（gzip/deflate/inflate、`#[cfg(feature="compression")]`）

---

### マテリアライゼーション（Materialization）　✅ 実装済み 4/4 (100%)

`Materializer`, `ActorMaterializer`, `Keep variants` (KeepLeft/KeepRight/KeepBoth/KeepNone), `Attributes` すべて実装済み。ギャップなし。

---

### キュー（Queue）　✅ 実装済み 4/5 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `SinkQueueWithCancel[T]` | `Queue.scala` | 部分実装 | trivial | `SinkQueue` にキャンセルメソッドを追加するだけ |

実装済み：`BoundedSourceQueue`, `SourceQueue`, `SourceQueueWithComplete`, `SinkQueue`

---

### IO（ファイル・ネットワーク）　〜 実装済み 1/4 (25%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `FileIO.toPath` / `fromPath` （完全版） | `scaladsl/FileIO.scala` | 部分実装 | easy | `from_path`/`to_path` はあるが IOResult マテリアライズドバリューが未実装。`IOResult` 型自体も未実装 |
| `StreamConverters` | `scaladsl/StreamConverters.scala` | 未対応 | n/a | Java Iterator/InputStream/OutputStream 変換。Rust では不要 |
| `Tcp` | `scaladsl/Tcp.scala` | 未対応 | n/a | Akka/Pekko の ActorSystem ベース TCP。Rust では tokio::net で代替 |
| `TLS` | `scaladsl/TLS.scala` | 未対応 | n/a | JVM TLS 統合。Rust では rustls 等で代替 |

---

### StreamRefs（分散ストリーム）　❌ 実装済み 0/3 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `SinkRef[T]` | `StreamRefs.scala` | 未対応 | hard | 別 JVM へのリモートマテリアライゼーション。actor/cluster モジュール連携が必要 |
| `SourceRef[T]` | `StreamRefs.scala` | 未対応 | hard | 同上 |
| `StreamRefSettings` | `StreamRefSettings.scala` | 未対応 | hard | 設定型。上2つに依存 |

---

### その他の型・設定　〜 実装済み 6/9 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `DelayStrategy[T]` | `scaladsl/DelayStrategy.scala` | 未対応 | easy | `delayWith` に渡す戦略型の独立定義。`LinearIncreasingDelay` 等の標準実装付き |
| `SubstreamCancelStrategy` | `SubstreamCancelStrategy.scala` | 未対応 | easy | `groupBy` のキャンセル戦略（Drain/Propagate）。現状 fraktor は enum 定義なし |
| `Source.combineMat` | `Source.scala:L765` | 部分実装 | easy | `combine` はあるが `combineMat`（マット値を結合）が未対応 |
| `IOResult` 型 | `IOResult.scala` | 未対応 | easy | FileIO のマテリアライズドバリュー型。バイト数・完了状態を保持 |
| `MergeLatest` 独立型 | `scaladsl/MergeLatest.scala` | ✅ `merge_latest` メソッドで実装 | — | — |
| `StatefulMapConcatAccumulator` | `scaladsl/StatefulMapConcatAccumulator.scala` | 未対応 | trivial | `statefulMapConcat` のアキュムレーター型。現状不要 |
| `CoupledTerminationFlow` | `scaladsl/CoupledTerminationFlow.scala` | ✅ `from_sink_and_source_coupled` で対応 | — | — |
| `NeverMaterializedException` | `NeverMaterializedException.scala` | 未対応 | trivial | `Source.maybe` の未解決 Promise 時のエラー型 |
| `TooManySubstreamsOpenException` | `TooManySubstreamsOpenException.scala` | 部分実装 | trivial | StreamError に統合済みの可能性あり |

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- **`SinkQueueWithCancel`**: `SinkQueue` に `cancel()` メソッドを追加する
- **`mapWithResource(AutoCloseable)`**: `mapWithResource` の 2引数オーバーロード追加
- **`NeverMaterializedException`**: 専用エラー型の追加

### Phase 2: easy（単純な新規実装）

- **`JsonFraming`**: JSON オブジェクト境界スキャナーの実装
- **`DelayStrategy[T]`** + `LinearIncreasingDelay`: `delay_with` の型安全化
- **`SubstreamCancelStrategy`**: `group_by` のキャンセル動作を制御する enum の追加
- **`IOResult` 型 + FileIO 完全版**: `from_path`/`to_path` のマテリアライズドバリューを IOResult に
- **`Source.combineMat`**: 複数 Source を結合してマット値を保持するコンストラクタ追加

### Phase 3: medium（中程度の実装工数）

- **`RetryFlow.withBackoff`**: 個別要素の指数バックオフリトライ（`@ApiMayChange` のため優先度低め）
- **`FanInShape2`/`FanOutShape3` 等バリアント**: 多ポート型シェイプの生成（GraphDSL と連動）

### Phase 4: hard（アーキテクチャ変更を伴う）

- **`GraphDSL.Builder`**: 宣言的グラフ構築 DSL（Flow メソッドで代替可能であれば低優先度）
- **`StreamRefs (SinkRef/SourceRef)`**: cluster/remote モジュールとの統合が必要

### 対象外（n/a）

- `Tcp` / `TLS`: JVM ネットワークスタック固有。tokio::net/rustls で代替
- `StreamConverters`: Java Iterator/Stream 変換。Rust では不要
- `JavaFlowSupport`: Java API 専用
- `ActorRef`-based `Sink.actorRef` / `Source.actorRef`: fraktor-rs の actor モジュール連携で別途対応
- `MaterializerState` / `SystemMaterializer`: JVM ActorSystem 依存

---

## まとめ

**全体カバレッジの評価**: コアDSL・オペレーター（92%以上）・ライフサイクル・ハブはきわめて高カバレッジ。Pekko の主要ユースケースの大部分は実装済み。

**即座に価値を提供できる未実装機能（Phase 1〜2）**:
- `JsonFraming`: バイトストリームから JSON オブジェクトを切り出す実用的機能
- `DelayStrategy`: `delay_with` の型安全な独立型定義
- `IOResult` + FileIO 完全版: ファイル I/O のマテリアライズドバリュー

**実用上の主要ギャップ（Phase 3〜4）**:
- `GraphDSL.Builder`: 複雑なグラフ組み立てに必要だが、fraktor-rs は Flow/Source のメソッドチェーンで同等のことが可能。必要性を慎重に見極めるべき
- `RetryFlow`: 実験的 API（`@ApiMayChange`）のため、Pekko 側が安定したタイミングで実装を検討

**YAGNI 観点での省略推奨**:
- `GraphDSL.Builder`: fraktor-rs の命令的 API で代替可能なため、現時点では実装不要
- `StreamRefs`: cluster モジュールの成熟が前提。現段階では持ち越し推奨
- TCP/TLS/StreamConverters: Rust エコシステムに専用クレートがあり、fraktor-rs が責任を持つべき範囲ではない
- `FanInShape` バリアント群: GraphDSL なしでは使用機会がほぼない

## 注記

- fraktor-rs は Pekko に存在しない機能（`mapAsyncPartitioned`, `mapAsyncPartitionedUnordered`, `flatMapPrefix`, `switchMap`, `aggregateWithBoundary`, `groupedAdjacentBy`, `dropRepeated`, `mergeSequence` 等）を実装しており、一部でPekkoを超えている
- `Compression` は `#[cfg(feature="compression")]` フラグで実装済み。デフォルトでは無効
- fraktor-rs の `OperatorCatalog` / `OperatorContract` / `OperatorCoverage` は Pekko に存在しない独自機能（オペレーターの契約管理）

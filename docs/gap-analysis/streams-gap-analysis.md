# streams モジュール ギャップ分析

生成日: 2026-03-15（前回: 2026-03-14）

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | ~79（コアDSL 8、シェイプ 11、GraphDSL系 22、ハブ/ライフサイクル 10、IO 4、その他 24） |
| fraktor-rs 公開型数 | 101（core: 100, std: 1） |
| カバレッジ（型単位） | 74/79 (94%) |
| ギャップ数（実装対象） | 5 |
| 対象外（n/a） | 5 |

**結論：** 前回分析（2026-03-14）から大幅にカバレッジが向上。`RetryFlow`、`JsonFraming`、`DelayStrategy`、`IOResult`、`FileIO` 完全版、`FanInShape2`、`FanOutShape2`、`StatefulMapConcatAccumulator`、`FlowMonitor`、`Source.combineMat` 等が実装済みとなった。残りの主要ギャップは `GraphDSL.Builder`、`SubstreamCancelStrategy`、`UniformFanOutShape`、`StreamRefs` のみ。

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

**メソッド数比較**（ユニークメソッド名ベース）:

| 型 | Pekko | fraktor-rs | 備考 |
|----|-------|-----------|------|
| Source (コンストラクタ + オペレーター) | 50 | 131 | fraktor-rs は FlowOps 相当を Source に直接実装 |
| Flow (FlowOps + コンストラクタ) | 164 | 163 | ほぼ同数 |
| Sink | 40 | 45 | fraktor-rs がやや多い |

---

### オペレーター（変換・フィルタ）　✅ 実装済み 約89/90 (99%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `collect(pf)` | `Flow.scala:L1753` | 未対応 | core | trivial | Rust に PartialFunction はないが `filter_map` 相当として検討可。`collectFirst`/`collectWhile`/`collectType` は実装済み |

**主要オペレーター実装済みリスト**（67個のロジックファイル + インライン実装）:

`map`, `mapConcat`, `mapAsync`, `mapAsyncUnordered`, `mapAsyncPartitioned`, `mapAsyncPartitionedUnordered`, `mapWithResource`, `mapOption`, `mapError`, `statefulMap`, `statefulMapConcat`, `filter`, `filterNot`, `take`, `takeWhile`, `takeUntil`, `takeWithin`, `drop`, `dropWhile`, `dropWithin`, `dropRepeated`, `scan`, `scanAsync`, `fold`, `foldAsync`, `reduce`, `grouped`, `groupedWithin`, `groupedWeighted`, `groupedWeightedWithin`, `groupedAdjacentBy`, `groupedAdjacentByWeighted`, `sliding`, `buffer`, `throttle`, `debounce`, `delay`, `delayWith`, `initialDelay`, `expand`, `extrapolate`, `conflateWithSeed`, `batch`, `batchWeighted`, `sample`, `log`, `logWithMarker`, `flatMapConcat`, `flatMapMerge`, `flatMapPrefix`, `flattenMerge`, `switchMap`, `intersperse`, `wireTap`, `alsoTo`, `alsoToAll`, `divertTo`, `aggregateWithBoundary`, `collectFirst`, `collectWhile`, `collectType`, `doOnFirst`, `doOnCancel`, `dropRepeated`, `mergeSequence`, `ask`, `watch`, `keepAlive`, `limitWeighted`, `prefixAndTail`, `backpressureTimeout`, `completionTimeout`, `idleTimeout`, `initialTimeout`, `onErrorComplete`, `onErrorContinue`, `onErrorResume`, `recover`, `recoverWith`, `recoverWithRetries`, `contramap`, `flattenOptional`, `concatLazy`, `concatAllLazy`, `prependLazy`, `orElse`, `monitor`, `monitorMat`

**注意**: 一部のオペレーター（`zipLatest`, `mergeAll`, `interleaveAll`, `concatAllLazy`, `flattenMerge`, `switchMap`, `extrapolate`, `keepAlive`, `dropWithin`, `delayWith`, `groupedAdjacentBy` 等）は既存オペレーターへの委譲で実装されており、独立したロジックファイルを持たない。セマンティクスが完全に同一でないケースがある可能性に注意。

**前回からの変更**: `delayWith` は `DelayStrategy` trait の独立定義とともに完全版となった。`mapWithResource` 2引数版は Rust では `Drop` trait で自動対応可能なため n/a に分類変更。

---

### エラーハンドリング　✅ 実装済み 7/7 (100%)

`recover`, `recoverWith`, `recoverWithRetries`, `onErrorComplete`, `onErrorContinue`, `onErrorResume`, `mapError` すべて実装済み。ギャップなし。

---

### ファンイン / ファンアウト　✅ 実装済み 23/23 (100%)

グラフプリミティブ実装済み：`Merge`, `MergePreferred`, `MergePrioritized`, `MergeSorted`, `MergeLatest`, `MergeAll`, `MergeSequence`, `Interleave`, `InterleaveAll`, `Broadcast`, `WireTap`, `Partition`, `Balance`, `Zip`, `ZipLatest`, `ZipWith`, `ZipLatestWith`, `ZipN`, `ZipWithN`, `ZipWithIndex`, `ZipAll`, `Unzip`, `UnzipWith`, `Concat`, `ConcatLazy`, `ConcatAllLazy`, `OrElse`, `PrependLazy`, `AlsoTo`, `DivertTo`

---

### シェイプ（Shapes）　✅ 実装済み 9/11 (82%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FanInShape3`〜`FanInShape22` | `FanInShape.scala` | 未対応 | core | easy | Pekko はマクロ生成。`FanInShape2` は実装済みだが、3以上のバリアントはなし |
| `UniformFanOutShape` | `UniformFanOutShape.scala` | 未対応 | core | easy | `UniformFanInShape` は実装済みだが、FanOut側が欠けている |

実装済み：`Shape`, `SourceShape`, `SinkShape`, `FlowShape`, `BidiShape`, `ClosedShape`, `UniformFanInShape`, `FanInShape2`, `FanOutShape2`, `StreamShape`

**前回からの変更**: `FanInShape2`, `FanOutShape2` が新規実装済み。

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

**前回からの変更**: 2メソッドともフル実装済みとなった。

---

### フレーミング（Framing）　✅ 実装済み 3/3 (100%)

`Framing`（バイト境界フレーミング）、`JsonFraming.object_scanner`（JSON オブジェクト境界スキャナー）、`Compression`（gzip/deflate/inflate、`#[cfg(feature="compression")]`）すべて実装済み。ギャップなし。

**前回からの変更**: `JsonFraming.object_scanner` が `core/json_framing.rs` にフル実装（括弧カウント + 文字列リテラル対応 + サイズ制限付き）。

---

### マテリアライゼーション（Materialization）　✅ 実装済み 5/5 (100%)

`Materializer`, `ActorMaterializer`, `Keep variants` (KeepLeft/KeepRight/KeepBoth/KeepNone), `Attributes`, `FlowMonitor` すべて実装済み。ギャップなし。

**前回からの変更**: `FlowMonitor` が `core/stage/flow_monitor.rs` に追加。`Flow.monitor` / `Flow.monitor_mat` メソッドも実装済み。

---

### キュー（Queue）　✅ 実装済み 5/5 (100%)

`BoundedSourceQueue`, `SourceQueue`, `SourceQueueWithComplete`, `SinkQueue`（`cancel()` メソッド付き）すべて実装済み。ギャップなし。

**前回からの変更**: `SinkQueue.cancel()` が実装済みであることを確認。Pekko の `SinkQueueWithCancel` 相当の機能をカバー。

---

### IO（ファイル・ネットワーク）　✅ 実装済み 2/4 (50%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `StreamConverters` | `scaladsl/StreamConverters.scala` | 未対応 | std | n/a | Java Iterator/InputStream/OutputStream 変換。Rust では不要 |
| `Tcp` | `scaladsl/Tcp.scala` | 未対応 | std | n/a | Akka/Pekko の ActorSystem ベース TCP。Rust では tokio::net で代替 |
| `TLS` | `scaladsl/TLS.scala` | 未対応 | std | n/a | JVM TLS 統合。Rust では rustls 等で代替 |

実装済み：`FileIO`（`from_path` / `to_path` に `IOResult` マテリアライズドバリュー付き）、`IOResult` 型

**前回からの変更**: `IOResult` 型が `core/io_result.rs` に追加。`FileIO`（`std/file_io.rs`）が IOResult をマテリアライズドバリューとして返すようになった。

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

実装済み：`DelayStrategy<T>` trait + `FixedDelay` + `LinearIncreasingDelay`、`IOResult`、`Source.combine_mat`、`MergeLatest`（`merge_latest` メソッド）、`StatefulMapConcatAccumulator` trait、`CoupledTerminationFlow`（`from_sink_and_source_coupled`）、`NeverMaterializedException`（`StreamError::NeverMaterialized`）、`TooManySubstreamsOpenException`（`StreamError` に統合）

**前回からの変更**: `DelayStrategy<T>` trait + 標準実装2種（`FixedDelay`, `LinearIncreasingDelay`）、`IOResult`、`Source.combine_mat`、`StatefulMapConcatAccumulator` trait、`NeverMaterializedException`（`StreamError::NeverMaterialized` バリアント）がすべて新規実装済み。

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- **`collect`（filter_map 相当）** [core]: `filter` + `map` の合成。Rust では `filter_map` クロージャを受け取るオペレーターとして提供

### Phase 2: easy（単純な新規実装）

- **`SubstreamCancelStrategy`** [core]: `group_by` のキャンセル動作を制御する enum（Drain/Propagate）の追加。`group_by` メソッドのシグネチャ拡張を伴う
- **`UniformFanOutShape`** [core]: `UniformFanInShape` と対になる型。Broadcast/Balance/Partition で利用
- **`FanInShape3`〜`FanInShapeN`** [core]: Rust マクロで生成可能。GraphDSL 実装時に有用

### Phase 3: medium（中程度の実装工数）

（該当なし — 前回の Phase 3 項目はすべて実装済み）

### Phase 4: hard（アーキテクチャ変更を伴う）

- **`GraphDSL.Builder`** [core]: 宣言的グラフ構築 DSL。Flow メソッドで代替可能なケースが多いが、複雑なダイヤモンドグラフや非線形パイプラインでは有用。`core/graph/graph_dsl_builder` ディレクトリは存在するが空
- **`StreamRefs (SinkRef/SourceRef)`** [core+std]: remote モジュールとの統合が必要。分散ストリーミングの基盤

### 対象外（n/a）

- `Tcp` / `TLS`: JVM ネットワークスタック固有。tokio::net/rustls で代替
- `StreamConverters`: Java Iterator/Stream 変換。Rust では不要
- `JavaFlowSupport`: Java API 専用
- `SystemMaterializer`: JVM ActorSystem 依存
- `mapWithResource(AutoCloseable)` 2引数版: Rust では `Drop` trait で自動対応

---

## まとめ

**全体カバレッジの評価**: 型単位で 94%（前回 78%）に到達。コアDSL（100%）・オペレーター（99%）・エラーハンドリング（100%）・ファンイン/ファンアウト（100%）・ライフサイクル（100%）・ハブ（100%）・RetryFlow（100%）・フレーミング（100%）・マテリアライゼーション（100%）・キュー（100%）と、主要カテゴリのほぼすべてが完全実装済み。Pekko の主要ユースケースの大部分をカバーしている。

**即座に価値を提供できる未実装機能（Phase 1〜2）**:
- `collect`（filter_map 相当）: 1オペレーターの追加のみ
- `SubstreamCancelStrategy`: `group_by` のキャンセル制御を型安全にする enum
- `UniformFanOutShape`: `UniformFanInShape` の対型

**実用上の主要ギャップ（Phase 4）**:
- `GraphDSL.Builder`: 複雑なグラフ組み立てに必要だが、fraktor-rs は Flow/Source のメソッドチェーンで同等のことが多くのケースで可能
- `StreamRefs`: remote モジュールの成熟が前提。分散ストリーミングには必須だが、現段階では持ち越し推奨

## 注記

- fraktor-rs は Pekko に存在しない機能（`mapAsyncPartitioned`, `mapAsyncPartitionedUnordered`, `flatMapPrefix`, `switchMap`, `aggregateWithBoundary`, `groupedAdjacentBy`, `groupedAdjacentByWeighted`, `dropRepeated`, `mergeSequence`, `onErrorResume`, `flattenOptional`, `batchWeighted`, `concatLazy`, `prependLazy` 等）を実装しており、一部で Pekko を超えている
- `Compression` は `#[cfg(feature="compression")]` フラグで実装済み。デフォルトでは無効
- fraktor-rs の `OperatorCatalog` / `OperatorContract` / `OperatorCoverage` は Pekko に存在しない独自機能（オペレーターの契約管理）
- 一部のオペレーター（`zipLatest`, `mergeAll`, `interleaveAll`, `switchMap`, `extrapolate`, `keepAlive`, `dropWithin` 等）は既存オペレーターへの簡易委譲で実装されており、Pekko と完全に同一のセマンティクスではない場合がある。実用上問題になった時点で個別のロジックファイルを作成することを推奨
- Flow.rs は 4,413 行超に達しており、今後のメソッド追加時にはファイル分割を検討すべき

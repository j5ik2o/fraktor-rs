# streams モジュール ギャップ分析

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 約85（型・トレイト・オブジェクト） |
| fraktor-rs 公開型数 | 約60 |
| カバレッジ（型単位） | 60/85 (約71%) |
| 完全実装オペレーター数 | 約40 |
| スタブ実装オペレーター数 | 約25 |
| 完全未実装ギャップ数 | 約14 |

### 補足: 実行モデルの違い

fraktor-rsはtickベースの同期実行モデルを採用しているため、Pekkoの`FiniteDuration`を使うオペレーター（debounce, sample, 実時間throttle等）は設計上の変換が必要。tickベースのタイミングパラメータ（`ticks: usize`）で代替している箇所が多い。

---

## カテゴリ別ギャップ

### 1. コアDSL型

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Source[+Out, +Mat]` | `Source.scala` | `Source<Out, Mat>` | - | 実装済み |
| `Flow[-In, +Out, +Mat]` | `Flow.scala` | `Flow<In, Out, Mat>` | - | 実装済み |
| `Sink[-In, +Mat]` | `Sink.scala` | `Sink<In, Mat>` | - | 実装済み |
| `BidiFlow[-I1,+O1,-I2,+O2,+Mat]` | `BidiFlow.scala` | `BidiFlow<InTop,OutTop,InBottom,OutBottom>` | easy | Mat型パラメータなし |
| `RunnableGraph[+Mat]` | `RunnableGraph.scala` | `RunnableGraph<Mat>` | - | 実装済み |
| `Graph[S <: Shape, +Mat]` | `Graph.scala` | なし（直接的な対応なし） | medium | 汎用Graph型は未実装 |
| `SubFlow[+Out,+Mat,+F[+_],C]` | `SubFlow.scala` | `SourceSubFlow<Out,Mat>`, `FlowSubFlow<In,Out,Mat>` | - | 別設計で実装済み |

### 2. シェイプ型

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Inlet[T]` | `Shape.scala` | `Inlet<T>` | - | 実装済み |
| `Outlet[T]` | `Shape.scala` | `Outlet<T>` | - | 実装済み |
| `SourceShape[T]` | `Shape.scala` | `SourceShape<Out>` | - | 実装済み |
| `FlowShape[-I,+O]` | `Shape.scala` | `FlowShape<In,Out>` | - | 実装済み |
| `SinkShape[-T]` | `Shape.scala` | `SinkShape<In>` | - | 実装済み |
| `BidiShape[-I1,+O1,-I2,+O2]` | `Shape.scala` | `BidiShape<In1,Out1,In2,Out2>` | - | 実装済み |
| `ClosedShape` | `Shape.scala` | `ClosedShape` (`pub type ClosedShape = ()`) | - | 実装済み |
| `AmorphousShape` | `Shape.scala` | なし | n/a | 動的ポート数は設計対象外 |
| `Shape` trait | `Shape.scala` | `Shape` trait | - | 実装済み |
| `PortId` | - | `PortId` | - | 実装済み |
| `StreamShape<In,Out>` | - | `StreamShape<In,Out>` | - | fraktor独自追加 |

### 3. 変換オペレーター（FlowOps相当）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `map` | `FlowOps.scala` | `map` | - | 完全実装 |
| `mapOption` / `mapConcat` | `FlowOps.scala` | `map_option`, `map_concat` | - | 完全実装 |
| `mapAsync` | `FlowOps.scala` | `map_async` | - | 完全実装 |
| `mapAsyncUnordered` | `FlowOps.scala` | `map_async_unordered` | - | 完全実装 |
| `filter` / `filterNot` | `FlowOps.scala` | `filter`, `filter_not` | - | 完全実装 |
| `collect` | `FlowOps.scala` | なし（`filter` + `map`で代替） | trivial | 部分関数はRustにない |
| `collectFirst` / `collectWhile` | `FlowOps.scala` | `collect_first`, `collect_while` | - | 完全実装 |
| `collectType` | `FlowOps.scala` | `collect_type` | - | 完全実装 |
| `scan` / `scanAsync` | `FlowOps.scala` | `scan`, `scan_async` | - | 完全実装 |
| `fold` / `foldAsync` | `FlowOps.scala` | `fold`, `fold_async` | - | 完全実装 |
| `reduce` | `FlowOps.scala` | `reduce` | - | 完全実装 |
| `statefulMap` / `statefulMapConcat` | `FlowOps.scala` | `stateful_map`, `stateful_map_concat` | - | 完全実装 |
| `take` / `drop` | `FlowOps.scala` | `take`, `drop` | - | 完全実装 |
| `takeWhile` / `dropWhile` | `FlowOps.scala` | `take_while`, `drop_while` | - | 完全実装 |
| `takeWithin` / `dropWithin` | `FlowOps.scala` | `take_within` / なし | easy | `drop_within`未実装 |
| `grouped` | `FlowOps.scala` | `grouped` | - | 完全実装 |
| `sliding` | `FlowOps.scala` | `sliding` | - | 完全実装 |
| `batch` | `FlowOps.scala` | `batch` | - | 完全実装 |
| `intersperse` | `FlowOps.scala` | `intersperse` | - | 完全実装 |
| `flatMapConcat` / `flatMapMerge` | `FlowOps.scala` | `flat_map_concat`, `flat_map_merge` | - | 完全実装 |
| `contramap` | - | `contramap` | - | fraktor独自追加 |
| `dimap` | - | `dimap` | - | fraktor独自追加 |
| `zipWithIndex` | `FlowOps.scala` | `zip_with_index` | - | 完全実装 |

### 4. スタブ実装オペレーター（API互換だがセマンティクスが簡略化）

| Pekko API | Pekko参照 | fraktor対応 | 実際の実装 | 備考 |
|-----------|-----------|-------------|-----------|------|
| `conflate` | `FlowOps.scala` | `conflate` | `self.map(\|v\| v)` (no-op) | 同期モデルではレート差なし |
| `conflateWithSeed` | `FlowOps.scala` | `conflate_with_seed` | `self.map(seed)` | seed関数のみ適用 |
| `expand` | `FlowOps.scala` | `expand` | `self` (const, no-op) | 同期モデルでは不要 |
| `extrapolate` | `FlowOps.scala` | `extrapolate` | `self` (const, no-op) | 同期モデルでは不要 |
| `groupedWithin` | `FlowOps.scala` | `grouped_within` | `self.grouped(size)` | 時間パラメータ無視 |
| `groupedAdjacentBy` | `FlowOps.scala` | `grouped_adjacent_by` | キーベースの隣接グルーピング | 完全実装に近い |
| `switchMap` | `FlowOps.scala` | `switch_map` | `self.flat_map_merge(1, func)` | breadth=1固定 |
| `prefixAndTail` | `FlowOps.scala` | `prefix_and_tail` | `self.grouped(size)` | tail部分なし |
| `keepAlive` | `FlowOps.scala` | `keep_alive` | `self.intersperse(...)` | タイミングなし |
| `mergePreferred` | `FlowOps.scala` | `merge_preferred` | `self.merge(fan_in)` | 優先度なし |
| `mergePrioritized` | `FlowOps.scala` | `merge_prioritized` | `self.merge(fan_in)` | 優先度なし |
| `mergeSorted` | `FlowOps.scala` | `merge_sorted` | `self.merge(fan_in)` | ソート順なし |
| `mergeLatest` | `FlowOps.scala` | `merge_latest` | `self.merge(fan_in)` | Latest保持なし |
| `orElse` | `FlowOps.scala` | `or_else` | `self.prepend(fan_in)` | フォールバック未実装 |
| `zipLatest` | `FlowOps.scala` | `zip_latest` | `self.zip_all(fan_in, fill)` | Latest保持なし |
| `alsoTo` | `FlowOps.scala` | `also_to` | sinkをdropして`self` | サイドチャネル未実装 |
| `wireTap` | `FlowOps.scala` | `wire_tap` | map内でcallback呼び出し | 機能的に実装済み |
| `monitor` | `FlowOps.scala` | `monitor` | カウンタ付きmap | 簡易実装 |
| `watchTermination` | `FlowOps.scala` | `watch_termination` | `self` (const, no-op) | 終了監視なし |
| `log` | `FlowOps.scala` | `log` | `self.wire_tap(\|_\| {})` | ログ出力なし |
| `limit` | `FlowOps.scala` | `limit` | `self.take(max)` | 超過時エラーなし |
| `limitWeighted` | `FlowOps.scala` | `limit_weighted` | `self.take(max)` | 重みなし |
| `batchWeighted` | `FlowOps.scala` | `batch_weighted` | `self.batch(size)` | 重みなし |
| `backpressureTimeout` | `FlowOps.scala` | `backpressure_timeout` | `self.take_within(ticks)` | タイムアウト未実装 |
| `completionTimeout` | `FlowOps.scala` | `completion_timeout` | `self.take_within(ticks)` | タイムアウト未実装 |
| `idleTimeout` | `FlowOps.scala` | `idle_timeout` | `self.take_within(ticks)` | タイムアウト未実装 |
| `initialTimeout` | `FlowOps.scala` | `initial_timeout` | `self.take_within(ticks)` | タイムアウト未実装 |
| `concatLazy` | `FlowOps.scala` | `concat_lazy` | `self.concat(fan_in)` | Lazy評価なし |
| `concatAllLazy` | `FlowOps.scala` | `concat_all_lazy` | `self.concat(fan_in)` | Lazy評価なし |
| `prependLazy` | `FlowOps.scala` | `prepend_lazy` | `self.prepend(fan_in)` | Lazy評価なし |
| `flatMapPrefix` | `FlowOps.scala` | `flat_map_prefix` | prefix=1のflat_map_merge | prefix処理簡略 |
| `dropRepeated` | `FlowOps.scala` | `drop_repeated` | stateful_mapで実装 | ほぼ完全実装 |

### 5. 完全未実装ギャップ

| Pekko API | Pekko参照 | 難易度 | 備考 |
|-----------|-----------|--------|------|
| `debounce` | `FlowOps.scala` | medium | 実時間タイマー必要（同期モデルでは再設計要） |
| `sample` | `FlowOps.scala` | medium | 実時間サンプリング（同期モデルでは再設計要） |
| `distinct` / `distinctBy` | `FlowOps.scala` | easy | HashSetベースの重複排除 |
| `withAttributes` / `addAttributes` | `FlowOps.scala` | hard | Attributes型システム全体の設計が必要 |
| `async` / `async(dispatcher)` | `FlowOps.scala` | hard | 非同期ディスパッチャモデルの設計が必要 |
| `mapMaterializedValue` | `FlowOps.scala` | medium | マテリアライゼーション値の変換 |
| `viaMat` / `toMat` 完全版 | `FlowOps.scala` | - | 存在するが`alsoToMat`, `wireTapMat`はなし |
| `alsoToMat` / `wireTapMat` | `FlowOps.scala` | easy | Mat合成のサイドチャネル |
| `monitorMat` | `FlowOps.scala` | easy | FlowMonitorとのMat合成 |
| `preMaterialize` (Source) | `Source.scala` | medium | 事前マテリアライズ |

### 6. Sourceファクトリ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `apply(iterable)` | `Source.scala` | `from_iterator` / `from` | - | 実装済み |
| `single` | `Source.scala` | `single` | - | 実装済み |
| `empty` | `Source.scala` | `empty` | - | 実装済み |
| `never` | `Source.scala` | `never` | - | 実装済み |
| `failed` | `Source.scala` | `failed` | - | 実装済み |
| `future` / `fromFuture` | `Source.scala` | `future` / `future_source` | - | 実装済み |
| `fromIterator` | `Source.scala` | `from_iterator` | - | 実装済み |
| `cycle` | `Source.scala` | `cycle` | - | 実装済み |
| `tick` | `Source.scala` | `tick` | - | 実装済み（tickベース） |
| `range` | `Source.scala` | `range` | - | 実装済み |
| `repeat` | `Source.scala` | `repeat` | - | 実装済み |
| `unfold` | `Source.scala` | `unfold` | - | 実装済み |
| `unfoldAsync` | `Source.scala` | `unfold_async` | - | 実装済み |
| `unfoldResource` | `Source.scala` | `unfold_resource` | - | 実装済み（unfoldへのalias） |
| `unfoldResourceAsync` | `Source.scala` | `unfold_resource_async` | - | 実装済み（unfold_asyncへのalias） |
| `fromGraph` | `Source.scala` | なし | medium | Graph型未実装のため |
| `fromMaterializer` | `Source.scala` | なし | medium | Materializer参照ファクトリ |
| `asSubscriber` | `Source.scala` | `as_subscriber` | - | 互換stub |
| `asPublisher` | `Source.scala` | なし | n/a | Reactive Streams固有 |
| `queue` | `Source.scala` | `queue` | - | 実装済み |
| `fromPublisher` | `Source.scala` | `from_publisher` | - | 互換stub |
| `combine` | `Source.scala` | `combine` | - | 実装済み |
| `lazySingle` | `Source.scala` | `lazy_single` | - | 実装済み |
| `lazySource` | `Source.scala` | `lazy_source` | - | 実装済み |
| `lazyFuture` | `Source.scala` | `lazy_future` | - | 実装済み |
| `maybe` | `Source.scala` | `maybe` | - | 実装済み |
| `iterate` | - | `iterate` | - | fraktor独自追加 |

### 7. Sinkファクトリ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ignore` | `Sink.scala` | `ignore` | - | 実装済み |
| `foreach` | `Sink.scala` | `foreach` | - | 実装済み |
| `foreachAsync` | `Sink.scala` | `foreach_async` | - | 実装済み |
| `fold` / `foldAsync` | `Sink.scala` | `fold` | - | 実装済み（foldAsyncなし） |
| `reduce` | `Sink.scala` | `reduce` | - | 実装済み |
| `head` / `headOption` | `Sink.scala` | `head`, `head_option` | - | 実装済み |
| `last` / `lastOption` | `Sink.scala` | `last`, `last_option` | - | 実装済み |
| `takeLast` | `Sink.scala` | `take_last` | - | 実装済み |
| `seq` | `Sink.scala` | `seq` / `collect` | - | 実装済み |
| `count` | `Sink.scala` | `count` | - | 実装済み |
| `collection` | `Sink.scala` | `collection` | - | 実装済み |
| `cancelled` | `Sink.scala` | `cancelled` | - | 実装済み |
| `asPublisher` | `Sink.scala` | `as_publisher` | - | 互換stub |
| `fromGraph` | `Sink.scala` | なし | medium | Graph型未実装 |
| `fromMaterializer` | `Sink.scala` | `from_materializer` | - | 互換stub |
| `fromSubscriber` | `Sink.scala` | `from_subscriber` | - | 互換stub |
| `foldWhile` | - | `fold_while` | - | fraktor独自追加 |
| `exists` / `forall` | - | `exists`, `forall` | - | fraktor独自追加 |
| `onComplete` | - | `on_complete` | - | fraktor独自追加 |
| `lazySink` | `Sink.scala` | `lazy_sink` | - | 実装済み |

### 8. Flowファクトリ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `apply[T]` | `Flow.scala` | `Flow::new()` | - | 実装済み |
| `fromFunction` | `Flow.scala` | `from_function` | - | 実装済み |
| `fromGraph` | `Flow.scala` | なし | medium | Graph型未実装 |
| `fromMaterializer` | `Flow.scala` | なし | medium | Materializer参照ファクトリ |
| `fromSinkAndSource` | `Flow.scala` | `from_sink_and_source` | - | 実装済み |
| `fromSinkAndSourceCoupled` | `Flow.scala` | `from_sink_and_source_coupled` | - | 実装済み |
| `lazyFlow` | `Flow.scala` | `lazy_flow` | - | 実装済み |
| `futureFlow` | `Flow.scala` | なし | easy | Future-based Flow |
| `lazyFutureFlow` | `Flow.scala` | なし | easy | Lazy Future Flow |

### 9. Fan-In/Fan-Out グラフステージ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Merge[T]` | `Graph.scala` | `merge(fan_in)` メソッド | - | メソッドチェーンで実装 |
| `MergePreferred[T]` | `Graph.scala` | `merge_preferred` | スタブ | `merge`に委譲、優先度なし |
| `MergePrioritized[T]` | `Graph.scala` | `merge_prioritized` | スタブ | `merge`に委譲 |
| `MergeSorted[T]` | `Graph.scala` | `merge_sorted` | スタブ | `merge`に委譲、ソートなし |
| `Interleave[T]` | `Graph.scala` | `interleave(fan_in)` | - | 実装済み |
| `Broadcast[T]` | `Graph.scala` | `broadcast(fan_out)` | - | 実装済み |
| `Balance[T]` | `Graph.scala` | `balance(fan_out)` | - | 実装済み |
| `Partition[T]` | `Graph.scala` | `partition(predicate)` | - | 実装済み |
| `Zip[A,B]` | `Graph.scala` | `zip(fan_in)` | - | 実装済み |
| `ZipLatest[A,B]` | `Graph.scala` | `zip_latest` | スタブ | `zip_all`に委譲 |
| `ZipWith[A,B,O]` | `Graph.scala` | `zip_with` | - | 実装済み |
| `ZipN[A]` | `Graph.scala` | `zip_n` | - | 実装済み |
| `ZipWithN[A,O]` | `Graph.scala` | `zip_with_n` | - | 実装済み |
| `Concat[T]` | `Graph.scala` | `concat(fan_in)` | - | 実装済み |
| `Unzip[A,B]` | `Graph.scala` | `unzip` | - | 実装済み |
| `UnzipWith[A,O1,O2]` | `Graph.scala` | `unzip_with` | - | 実装済み |
| `WireTap[T]` | `Graph.scala` | `wire_tap` | - | 機能的に実装 |

### 10. マテリアライゼーション

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Keep.left/right/both/none` | `Keep.scala` | `KeepLeft/KeepRight/KeepBoth/KeepNone` | - | 実装済み（独立struct） |
| `Materializer` trait | `Materializer.scala` | `Materializer` trait | - | 実装済み |
| `ActorMaterializer` | `ActorMaterializer.scala` | `ActorMaterializerGeneric<TB>` | - | 実装済み |
| `Materialized` | - | `Materialized<Mat,TB>` | - | fraktor独自追加 |
| `RunnableGraph.run` | `RunnableGraph.scala` | `RunnableGraph.run` | - | 実装済み |
| `mapMaterializedValue` | `FlowOps.scala` | なし | medium | マテリアライゼーション値変換 |
| `NotUsed` | `NotUsed.scala` | `StreamNotUsed` | - | 実装済み |
| `MatCombineRule` trait | - | `MatCombineRule<Left,Right>` | - | fraktor独自追加 |
| `MatCombine` enum | - | `MatCombine` | - | fraktor独自追加 |

### 11. ライフサイクル・KillSwitch

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `KillSwitch` trait | `KillSwitch.scala` | なし（具象型のみ） | trivial | trait化可能 |
| `SharedKillSwitch` | `KillSwitch.scala` | `SharedKillSwitch` | - | 実装済み |
| `UniqueKillSwitch` | `KillSwitch.scala` | `UniqueKillSwitch` | - | 実装済み |
| `KillSwitches.shared` | `KillSwitch.scala` | `KillSwitches::shared()` | - | 実装済み |
| `KillSwitches.single` | `KillSwitch.scala` | `KillSwitches::single()` | - | 実装済み |
| `StreamHandle` trait | - | `StreamHandle` trait | - | fraktor独自 |
| `StreamHandleGeneric<TB>` | - | `StreamHandleGeneric<TB>` | - | fraktor独自 |
| `StreamState` | - | `StreamState` | - | fraktor独自 |
| `DriveOutcome` | - | `DriveOutcome` | - | fraktor独自 |
| `StreamCompletion<T>` | - | `StreamCompletion<T>` | - | fraktor独自 |
| `Completion<T>` | - | `Completion<T>` | - | fraktor独自 |

### 12. エラー処理

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `recover` | `FlowOps.scala` | `recover` | - | 実装済み（Result<T,E>ベース） |
| `recoverWith` | `FlowOps.scala` | `recover_with` | - | 実装済み |
| `recoverWithRetries` | `FlowOps.scala` | `recover_with_retries` | - | 実装済み |
| `onErrorComplete` | `FlowOps.scala` | `on_error_complete` | - | 実装済み |
| `onErrorContinue` | `FlowOps.scala` | `on_error_continue` | - | 実装済み |
| `mapError` | `FlowOps.scala` | `map_error` | - | 実装済み |
| `on_error_resume` | - | `on_error_resume` | - | fraktor独自alias |

### 13. リスタート

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RestartSource.withBackoff` | `RestartSource.scala` | `restart_source_with_backoff` | - | 実装済み |
| `RestartSource.onFailuresWithBackoff` | `RestartSource.scala` | `on_failures_with_backoff` | - | 実装済み |
| `RestartFlow.withBackoff` | `RestartFlow.scala` | `restart_flow_with_backoff` | - | 実装済み |
| `RestartSink.withBackoff` | `RestartSink.scala` | `restart_sink_with_backoff` | - | 実装済み |
| `RestartSettings` | `RestartSettings.scala` | `RestartSettings` | - | 実装済み |

### 14. Hub（動的接続）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `MergeHub.source` | `Hub.scala` | `MergeHub<T>` | - | 実装済み |
| `BroadcastHub.sink` | `Hub.scala` | `BroadcastHub<T>` | - | 実装済み |
| `PartitionHub.statefulSink` | `Hub.scala` | `PartitionHub<T>` | - | 実装済み |
| `DrainingControl` | `Hub.scala` | なし | easy | ドレイン制御 |
| `MergeHub.sourceWithDraining` | `Hub.scala` | なし | easy | ドレイン付きMergeHub |

### 15. Queue（外部供給）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `SourceQueue[T]` trait | `QueueSource.scala` | なし | medium | キュー供給インターフェース |
| `SourceQueueWithComplete[T]` | `QueueSource.scala` | なし | medium | 完了制御付きキュー |
| `SinkQueue[T]` | `QueueSink.scala` | なし | medium | キューpull |
| `SinkQueueWithCancel[T]` | `QueueSink.scala` | なし | medium | キャンセル付きキュー |
| `Source.queue()` | `Source.scala` | `Source::queue()` | - | 互換stub（Iteratorベース） |

### 16. Attributes（ストリーム属性）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Attributes` | `Attributes.scala` | なし | hard | 属性型システム全体 |
| `withAttributes` | `FlowOps.scala` | なし | hard | 属性変更API |
| `addAttributes` | `FlowOps.scala` | なし | hard | 属性追加API |
| `named` | `FlowOps.scala` | `named` | - | 実装済み（no-op、Attributes未導入） |
| `async` | `FlowOps.scala` | `async_boundary` | - | 別名で実装済み |
| `Supervision.Strategy` | `Supervision.scala` | `supervision_stop/resume/restart` | - | メソッドベースで実装 |
| `OverflowStrategy` | `OverflowStrategy.scala` | `overflow_policy` パラメータ | - | buffer内で対応 |

### 17. Graph DSL

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `GraphDSL` object | `GraphDSL.scala` | `GraphDsl<In,Out,Mat>` | - | 実装済み |
| `GraphDSL.Builder[M]` | `GraphDSL.scala` | なし | medium | 汎用ビルダー |
| `GraphDSL.create` | `GraphDSL.scala` | なし | medium | グラフ作成DSL |
| `~>` / `<~` 演算子 | `GraphDSL.scala` | なし | medium | Rustでは演算子制約あり |
| `GraphStage` trait | `GraphStage.scala` | `GraphStage<In,Out,Mat>` trait | - | 実装済み |
| `GraphStageLogic` | `GraphStage.scala` | `GraphStageLogic<In,Out,Mat>` trait | - | 実装済み |
| `StageContext` | - | `StageContext<In,Out>` trait | - | 実装済み |
| `StreamGraph` | - | `StreamGraph` | - | fraktor独自 |
| `GraphInterpreter` | - | `GraphInterpreter` | - | fraktor独自 |

### 18. Stream IO（no_std制約下で対象外）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `FileIO.fromPath/toPath` | `FileIO.scala` | `from_path` / `to_path` | - | std依存、実装済み |
| `StreamConverters` | `StreamConverters.scala` | なし | n/a | JVM固有（InputStream/OutputStream） |
| `Framing` | `Framing.scala` | なし | n/a | バイトストリームフレーミング |
| `Compression` | `Compression.scala` | なし | n/a | gzip/deflate |
| `Tcp` | `Tcp.scala` | なし | n/a | TCP接続（std依存） |
| `TLS` | `TLS.scala` | なし | n/a | セキュア通信（std依存） |

### 19. BidiFlow

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `BidiFlow[I1,O1,I2,O2,Mat]` | `BidiFlow.scala` | `BidiFlow<InTop,OutTop,InBottom,OutBottom>` | easy | Mat型パラメータなし |
| `atop` / `atopMat` | `BidiFlow.scala` | なし | easy | BidiFlow合成 |
| `join` | `BidiFlow.scala` | なし | easy | FlowとのBidi結合 |
| `reversed` | `BidiFlow.scala` | `reversed` | - | 実装済み |
| `identity` | `BidiFlow.scala` | `identity` | - | 実装済み |
| `fromFlows` | `BidiFlow.scala` | `from_flows` | - | 実装済み |
| `fromFunction` / `fromFunctions` | `BidiFlow.scala` | なし | easy | 関数から構築 |

### 20. テストキット

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `TestSource.probe` | `TestSource.scala` | `TestSourceProbe<T>` | - | 実装済み |
| `TestSink.probe` | `TestSink.scala` | `TestSinkProbe<T>` | - | 実装済み |
| `StreamFuzzRunner` | - | `StreamFuzzRunner` | - | fraktor独自追加 |

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）— 実装完了

1. ~~`Flow::from_function`~~ — 実装済み
2. ~~`named`~~ — 実装済み（no-op）
3. ~~`ClosedShape`~~ — 実装済み
4. ~~`KillSwitches::shared/single`~~ — 実装済み
5. ~~`BidiFlow::identity`~~ — 実装済み
6. ~~`BidiFlow::reversed`~~ — 実装済み

### Phase 2: easy（単純な新規実装）

1. `distinct` / `distinctBy` — HashSetベースの重複排除フィルタ
2. `drop_within` — `take_within`の逆（時間内要素スキップ）
3. ~~`BidiFlow::fromFlows`~~ (実装済み) / `BidiFlow::fromFunction/fromFunctions` — BidiFlowファクトリ群
4. `BidiFlow::atop/join` — BidiFlow合成・結合
5. `DrainingControl` — Hub用ドレイン制御
6. `alsoToMat` / `wireTapMat` — マテリアライゼーション合成版サイドチャネル
7. `futureFlow` / `lazyFutureFlow` — Future-based Flowファクトリ
8. `BidiFlow` に `Mat` 型パラメータ追加

### Phase 3: medium（中程度の実装工数）

1. `mapMaterializedValue` — マテリアライゼーション値の変換チェーン
2. `preMaterialize` (Source) — 事前マテリアライゼーション
3. `SourceQueue/SinkQueue` traits — 外部からの供給・取得インターフェース
4. `fromGraph` / `fromMaterializer` ファクトリ群 — 汎用Graph型依存
5. `debounce` — tickベースのデバウンス再設計
6. `sample` — tickベースのサンプリング再設計
7. `GraphDSL.Builder` / `GraphDSL.create` — グラフDSLビルダー

### Phase 4: hard（アーキテクチャ変更を伴う）

1. `Attributes` 型システム — ストリーム属性のメタデータ体系
2. `withAttributes` / `addAttributes` — 属性変更API
3. `async(dispatcher)` — 非同期ディスパッチャ選択
4. 汎用 `Graph[S <: Shape, Mat]` 型 — 任意シェイプのグラフ

### 対象外（n/a）

1. `StreamConverters` — JVM固有（InputStream/OutputStream変換）
2. `Framing` — バイトストリームフレーミング（no_std制約）
3. `Compression` — gzip/deflate（no_std制約）
4. `Tcp` / `TLS` — ネットワークIO（std依存、外部crateで対応）
5. `AmorphousShape` — 動的ポート数（設計対象外）
6. `fromJavaStream` — JVM固有
7. `asPublisher` (Reactive Streams) — JVM固有

---

## 主要な発見事項

### 1. スタブ実装の多さ

約25個のオペレーターがAPI互換スタブとして存在する。これらはPekkoのシグネチャを持つがセマンティクスが簡略化されており、同期実行モデルの制約を反映している。スタブか完全実装かは個別に判断が必要。

### 2. 同期実行モデルの影響

`FiniteDuration`ベースのオペレーター（debounce, sample, 実時間throttle, keepAlive等）はtickベースに変換されている。レート差を前提とするオペレーター（conflate, expand, extrapolate）はno-opまたはidentity関数。

### 3. Fan-In/Fan-Out設計の違い

Pekkoは再利用可能な`GraphStage`（`Merge[T]`, `Broadcast[T]`等）を提供するが、fraktor-rsはメソッドチェーン（`merge(fan_in)`, `broadcast(fan_out)`等）で統合。設計哲学の違いであり、機能的なギャップではない。

### 4. Attributes未実装

Pekkoの`Attributes`型システム全体が未実装。`named`, `withAttributes`, `addAttributes`, `async(dispatcher)` 等が影響を受ける。ただし `supervision_*` メソッドや `async_boundary` は個別に実装されている。

### 5. エラー処理のRust適応

PekkoのThrowableベースのエラー処理はRustの`Result<T, StreamError>`ベースに適応されている。`recover`, `recoverWith`, `onErrorComplete`, `onErrorContinue`はすべて`Result`型の要素に対して動作する。

### 6. BidiFlowの実装状況

BidiFlowは `from_flows`, `split`, `identity`, `reversed` が実装済み。`atop`, `join` 等の合成オペレーターは未実装。双方向通信パターン（プロトコル変換等）が必要な場合は Phase 2 での対応が望ましい。

### 7. fraktor独自の追加

Pekkoにない以下の機能が追加されている：
- `contramap` / `dimap`（関手・双関手操作）
- `take_until`（述語ベースの終了）
- `fold_while`（条件付きfold）
- `exists` / `forall`（述語Sink）
- `StreamFuzzRunner`（ファズテスト）
- `StreamCompletion<T>` / `Completion<T>`（ポーリングベースの完了監視）
- `DriveOutcome`（tick駆動のステップ実行）

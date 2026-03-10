# streams モジュール ギャップ分析

生成日: 2026-03-10

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko scaladsl 公開型数 | 約 48（型・オブジェクト単位、JVM固有除く） |
| fraktor-rs 公開型数 | 約 55 |
| カバレッジ（型単位） | 45/48 (~94%) |
| ギャップ数（実装対象） | 3 |
| 対象外（n/a） | 8 |

**結論：** fraktor-rs は Pekko scaladsl のコアAPI を高いカバレッジで実装済み。
残りのギャップは `RetryFlow`（フィードバックループ型リトライ）、`GraphDSL`（明示的グラフ構築DSL）、`DelayStrategy`（カスタム遅延戦略）の3点。

---

## カテゴリ別ギャップ

### コア型・トレイト

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Source[+Out, +Mat]` | `Source.scala` | `core/stage/source.rs` | - | 実装済み |
| `Flow[-In, +Out, +Mat]` | `Flow.scala` | `core/stage/flow.rs` | - | 実装済み |
| `Sink[-In, +Mat]` | `Sink.scala` | `core/stage/sink.rs` | - | 実装済み |
| `BidiFlow` | `BidiFlow.scala` | `core/stage/bidi_flow.rs` | - | 実装済み |
| `FlowWithContext` | `FlowWithContext.scala` | `core/stage/flow_with_context.rs` | - | 実装済み |
| `SourceWithContext` | `SourceWithContext.scala` | `core/stage/source_with_context.rs` | - | 実装済み |
| `SubFlow` | `SubFlow.scala` | `FlowSubFlow`, `SourceSubFlow` | - | 別名で実装済み |
| `RunnableGraph` | `Flow.scala` | `core/mat/runnable_graph.rs` | - | 実装済み |

### オペレーター（変換・フィルタ）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `map` | `FlowOps` | `flow.rs:map` | - | 実装済み |
| `mapConcat` | `FlowOps` | `flow.rs:map_concat` | - | 実装済み |
| `mapAsync` | `FlowOps` | `flow.rs:map_async` | - | 実装済み |
| `mapAsyncUnordered` | `FlowOps` | `flow.rs:map_async_unordered` | - | 実装済み（L1531） |
| `mapWithResource` | `FlowOps` | `flow.rs:map_with_resource` | - | 実装済み（L1456） |
| `statefulMap` | `FlowOps` | `flow.rs:stateful_map` | - | 実装済み |
| `statefulMapConcat` | `FlowOps` | `flow.rs:stateful_map_concat` | - | 実装済み |
| `filter` | `FlowOps` | `flow.rs:filter` | - | 実装済み |
| `take` | `FlowOps` | `flow.rs:take` | - | 実装済み |
| `takeWhile` | `FlowOps` | `flow.rs:take_while` | - | 実装済み |
| `takeWithin` | `FlowOps` | `flow.rs:take_within` | - | 実装済み |
| `drop` | `FlowOps` | `flow.rs:drop` | - | 実装済み |
| `dropWhile` | `FlowOps` | `flow.rs:drop_while` | - | 実装済み |
| `scan` | `FlowOps` | `flow.rs:scan` | - | 実装済み |
| `grouped` | `FlowOps` | `flow.rs:grouped` | - | 実装済み |
| `sliding` | `FlowOps` | `flow.rs:sliding` | - | 実装済み |
| `buffer` | `FlowOps` | `flow.rs:buffer` | - | 実装済み |
| `throttle` | `FlowOps` | `flow.rs:throttle` | - | 実装済み |
| `debounce` | `FlowOps` | `flow.rs:debounce` | - | 実装済み |
| `delay` | `FlowOps` | `flow.rs:delay` | - | 実装済み（固定値のみ） |
| `delayWith(delayStrategy)` | `DelayStrategy.scala` | 未対応 | easy | カスタム遅延戦略。固定値 `delay` はあるが戦略オブジェクトなし |
| `expand` | `FlowOps` | `expand_logic.rs` | - | 実装済み |
| `conflateWithSeed` | `FlowOps` | `conflate_with_seed_logic.rs` | - | 実装済み |
| `log` / `log(name)` | `FlowOps` | `log_logic.rs` | - | 実装済み |
| `flatMapConcat` | `FlowOps` | `flow.rs:flat_map_concat` | - | 実装済み |
| `flatMapMerge` | `FlowOps` | `flow.rs:flat_map_merge` | - | 実装済み |
| `intersperse` | `FlowOps` | `flow.rs:intersperse` | - | 実装済み |
| `wireTap(f)` | `FlowOps` | `flow.rs:wire_tap` (L2221) | - | 実装済み |
| `wireTap(sink)` | `FlowOps` | `flow.rs:wire_tap_mat` | - | 実装済み |

### エラーハンドリング

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `recover` | `FlowOps` | `flow.rs:recover` | - | 実装済み |
| `recoverWith` | `FlowOps` | `flow.rs:recover_with` | - | 実装済み |
| `recoverWithRetries` | `FlowOps` | `flow.rs:recover_with_retries` | - | 実装済み |
| `onErrorComplete` | `FlowOps` | `flow.rs:on_error_complete` | - | 実装済み |
| `onErrorContinue` | `FlowOps` | `flow.rs:on_error_continue` | - | 実装済み |
| `mapError` | `FlowOps` | `flow.rs:map_error` | - | 実装済み |

### グラフ演算子（ファン・イン/ファン・アウト）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Merge[T]` | `Graph.scala` | `merge_logic.rs` | - | 実装済み |
| `MergePreferred[T]` | `Graph.scala` | `merge_preferred_logic.rs` | - | 実装済み |
| `MergePrioritized[T]` | `Graph.scala` | `merge_prioritized_logic.rs` | - | 実装済み |
| `MergeSorted[T]` | `Graph.scala` | `merge_sorted_logic.rs` | - | 実装済み |
| `MergeLatest[T]` | `MergeLatest.scala` | `merge_latest_logic.rs` | - | 実装済み |
| `Interleave[T]` | `Graph.scala` | `interleave_logic.rs` | - | 実装済み |
| `Broadcast[T]` | `Graph.scala` | `broadcast_logic.rs` | - | 実装済み |
| `WireTap` | `Graph.scala` | `flow.rs:wire_tap` | - | 別名で実装済み |
| `Partition[T]` | `Graph.scala` | `partition_logic.rs` | - | 実装済み |
| `Balance[T]` | `Graph.scala` | `balance_logic.rs` | - | 実装済み |
| `Zip[A, B]` | `Graph.scala` | `zip_logic.rs` | - | 実装済み |
| `ZipLatest[A, B]` | `Graph.scala` | `flow.rs:zip_latest` (L2129) | - | 実装済み |
| `ZipWith` | `Graph.scala` | `flow.rs:zip_with` (L2153) | - | 実装済み |
| `ZipLatestWith` | `Graph.scala` | `flow.rs:zip_latest_with` | - | 実装済み |
| `ZipN` / `ZipWithN` | `Graph.scala` | `source.rs:zip_n`, `zip_with_n` | - | 実装済み |
| `ZipWithIndex` | `FlowOps` | `zip_with_index_logic.rs` | - | 実装済み |
| `ZipAll` | `FlowOps` | `zip_all_logic.rs` | - | 実装済み |
| `Unzip[A, B]` | `Graph.scala` | `unzip_logic.rs` | - | 実装済み |
| `UnzipWith` | `Graph.scala` | `unzip_with_logic.rs` | - | 実装済み |
| `Concat` | `Graph.scala` | `concat_logic.rs` | - | 実装済み |
| `OrElse` | `Graph.scala` | `or_else_source_logic.rs` | - | 別名で実装済み |
| `GraphDSL` | `Graph.scala` | 未対応 | hard | 明示的グラフ構築DSL。fraktor-rsは内部 `StreamGraph` で管理するが、ユーザー向けDSL未実装 |

### ハブ・キュー

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `MergeHub` | `Hub.scala` | `hub/merge_hub.rs` | - | 実装済み |
| `BroadcastHub` | `Hub.scala` | `hub/broadcast_hub.rs` | - | 実装済み |
| `PartitionHub` | `Hub.scala` | `hub/partition_hub.rs` | - | 実装済み |
| `SourceQueue[T]` | `Queue.scala` | `core/source_queue.rs` | - | 実装済み |
| `SourceQueueWithComplete[T]` | `Queue.scala` | `core/source_queue_with_complete.rs` | - | 実装済み |
| `SinkQueue[T]` | `Queue.scala` | `core/sink_queue.rs` | - | 実装済み（cancel未実装） |
| `SinkQueueWithCancel[T]` | `Queue.scala` | 未対応 | trivial | `SinkQueue` にキャンセルメソッドを追加するだけ |
| `BoundedSourceQueue[T]` | `BoundedSourceQueue.scala` | `core/bounded_source_queue.rs` | - | 実装済み |

### ライフサイクル・キルスイッチ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `KillSwitches` | `KillSwitch.scala` | `lifecycle/kill_switches.rs` | - | 実装済み |
| `SharedKillSwitch` | `KillSwitch.scala` | `lifecycle/shared_kill_switch.rs` | - | 実装済み |
| `UniqueKillSwitch` | `KillSwitch.scala` | `lifecycle/unique_kill_switch.rs` | - | 実装済み |
| `watchTermination` | `FlowOps` | `watch_termination_logic.rs` | - | 実装済み |

### 再起動・リトライ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RestartSource` | `RestartSource.scala` | `core/stage/restart_source.rs` | - | 実装済み |
| `RestartFlow` | `RestartFlow.scala` | `core/stage/restart_flow.rs` | - | 実装済み |
| `RestartSink` | `RestartSink.scala` | `core/stage/restart_sink.rs` | - | 実装済み |
| `RetryFlow.withBackoff[In,Out,Mat]` | `RetryFlow.scala:L53` | 未対応 | medium | 失敗した要素をフィードバックして再試行するフロー。RestartFlowとは異なる概念（ストリーム再起動ではなく要素単位のリトライ） |
| `RetryFlow.withBackoffAndContext` | `RetryFlow.scala:L92` | 未対応 | medium | コンテキスト付きバージョン |

### マテリアライゼーション

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Keep` | `Materialization.scala` | `KeepBoth/KeepLeft/KeepRight/KeepNone` | - | 別名で実装済み |
| `Materializer` | `ActorMaterializer.scala` | `mat/materializer.rs` | - | 実装済み |
| `ActorMaterializer` | `ActorMaterializer.scala` | `mat/actor_materializer.rs` | - | 実装済み |

### 変換・エンコーディング

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Framing` | `Framing.scala` | `core/framing.rs` | - | 実装済み |
| `Compression` (gzip/deflate) | `Compression.scala` | `flow.rs:gzip/deflate` | - | `#[cfg(feature="compression")]` で実装済み |
| `FileIO` | `FileIO.scala` | `source.rs:from_path`, `sink.rs:to_path` | - | 別名で実装済み（IOResultマテリアライズ値は未確認） |
| `StreamConverters` | `StreamConverters.scala` | 未対応 | n/a | JVM/Java固有（`java.util.stream` 変換）。Rust不要 |
| `CoupledTerminationFlow` | `CoupledTerminationFlow.scala` | `flow.rs:from_sink_and_source_coupled` | - | 別名で実装済み |

### 対象外（n/a）

| Pekko API | Pekko参照 | 理由 |
|-----------|-----------|------|
| `StreamConverters` | `StreamConverters.scala` | JVM/Java ストリーム変換（`java.io.InputStream` 等）。Rust に相当なし |
| `JavaFlowSupport` | `JavaFlowSupport.scala` | Java 9 Flow API 対応。JVM固有 |
| `Tcp` | `Tcp.scala` | TCPネットワーク接続。fraktor-rsのスコープ外 |
| `TLS` | `TLS.scala` | TLS暗号化ストリーム。fraktor-rsのスコープ外 |
| `StreamRefs` | `StreamRefs.scala` | アクター間リモートストリーム参照。リモーティング層依存 |
| `MaterializerState` | `snapshot/MaterializerState.scala` | デバッグ/診断用スナップショット。JVM ActorSystem 依存 |
| `StreamRefSettings` | `StreamRefSettings.scala` | StreamRefs の設定。上記に準じて対象外 |
| `SystemMaterializer` | `SystemMaterializer.scala` | ActorSystem バインドマテリアライザ。JVM固有 |

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- **`SinkQueueWithCancel`**: `SinkQueue` に `cancel()` メソッドを追加する。既存 `SinkQueue` の薄いラッパーまたは拡張として実装可能。

### Phase 2: easy（単純な新規実装）

- **`DelayStrategy`**: カスタム遅延戦略型の導入。現在 `delay(ticks)` は固定値のみ。Pekko の `DelayStrategy` trait に相当する型（`FixedDelay`, `LinearIncreasingDelay` 等）を追加し、`flow.rs:delay` をオーバーロード。

### Phase 3: medium（中程度の実装工数）

- **`RetryFlow.withBackoff`**: フィードバックループを持つリトライフロー。
  - Pekko の概念: 失敗した `(In, Out)` ペアを受け取り、新たな入力として再投入するループ構造
  - `RestartFlow`（ストリーム全体の再起動）とは異なり、要素レベルのリトライ
  - 新規型 `RetryFlow` と内部ロジック `retry_flow_logic.rs` の追加が必要

### Phase 4: hard（アーキテクチャ変更を伴う）

- **`GraphDSL`**: ユーザー向けの明示的グラフ構築DSL。
  - Pekko では `GraphDSL.create { implicit b => val merge = b.add(Merge(2)); ... }` のように構築
  - fraktor-rs は `StreamGraph` で内部的にDAGを管理しており、現在ユーザー向けDSLは存在しない
  - 設計根拠: fraktor-rsの設計哲学（YAGNI・Less is more）との整合性を人間に確認してから着手すること
  - 実装する場合は `GraphDSL::create(builder: impl FnOnce(&mut GraphBuilder) -> ClosedShape)` 相当のAPIが必要

### 対象外（n/a）

- `StreamConverters`, `JavaFlowSupport`, `Tcp`, `TLS`, `StreamRefs`, `MaterializerState`, `StreamRefSettings`, `SystemMaterializer`

---

## 注記

- fraktor-rs は Pekko に存在しない機能（`mapAsyncPartitioned`, `batch`, `zip_with_index`など）を追加実装しており、一部でPekkoを超えている
- `Compression` は `#[cfg(feature="compression")]` フラグで実装済み。デフォルトでは無効
- fraktor-rs の `OperatorCatalog` / `OperatorContract` / `OperatorCoverage` は Pekko に存在しない独自機能（オペレーターの契約管理）

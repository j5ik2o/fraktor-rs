# stream モジュール ギャップ分析

生成日: 2026-03-23

対象:
- fraktor-rs: `modules/stream/src/`
- Pekko 参照: `references/pekko/stream/src/main/scala/`

計数ポリシー:
- Pekko 側は `org.apache.pekko.stream.*`、`scaladsl/*`、`stage/GraphStage.scala` の top-level public API を母集団とする
- fraktor-rs 側は `modules/stream/src/core` と `modules/stream/src/std` の `pub` 型を母集団とする
- `javadsl/*` と `impl/*` はカバレッジ母集団から除外し、必要なものだけギャップ表で補足する
- カバレッジは「型単位」の概算であり、最終判断はカテゴリ別ギャップを優先する

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 237 |
| fraktor-rs 公開型数 | 125（core: 124, std: 1） |
| カバレッジ（型単位） | 125/237 (53%) |
| ギャップ数 | 17（core: 10, std: 7） |

一言評価:
- 基本 DSL (`Source` / `Flow` / `Sink` / `BidiFlow`) と主要オペレーター群はかなり厚い
- 一方で、Pekko が持つ「GraphDSL 完全互換」「GraphStage 公開 API」「StreamConverters / Tcp / TLS / StreamRefs」などの周辺基盤はまだ薄い
- 実用上の差は「日常の stream 変換」ではなく、「Pekko らしい高度な配線・IO・外部連携」に集中している

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 12 API ファミリ | 9 API ファミリ | 75% |
| core / typed ラッパー | 0 | 0 | n/a |
| std / アダプタ | 5 API ファミリ | 1 API ファミリ | 20% |

補足:
- `modules/stream` には `core/typed` サブ層は存在しない
- `core/` には DSL、shape、materialization、kill switch、queue/hub、framing、compression が集約されている
- `std/` は `FileIO` と producer source のみで、Pekko の IO/transport 系 API を吸収しきれていない

## カテゴリ別ギャップ

### コア DSL / 型  ✅ 実装済み 10/13 (77%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GraphDSL` の完全互換 DSL (`~>`, `<~`, builder add/connect 連鎖) | `scaladsl/Graph.scala` | 部分実装 | core | hard | `graph_dsl.rs` / `graph_dsl_builder.rs` はあるが、Pekko の宣言的 wiring DSL までは未達 |
| `CoupledTerminationFlow` | `scaladsl/CoupledTerminationFlow.scala` | 未対応 | core | medium | `from_sink_and_source_coupled*` はあるが、独立 API としては未提供 |
| `Materializer` の system extension / singleton 取得 (`SystemMaterializer`) | `SystemMaterializer.scala` | 未対応 | std | medium | `ActorMaterializer` はあるが system-level shared materializer はない |

実装済み:
- `Source`, `Flow`, `Sink`, `RunnableGraph`, `BidiFlow`
- `SourceWithContext`, `FlowWithContext`
- `SourceSubFlow`, `FlowSubFlow`, `SourceGroupBySubFlow`
- `KeepLeft`, `KeepRight`, `KeepBoth`, `KeepNone`

### オペレーター / 変換 DSL  ✅ 実装済み 57/70 (81%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `wireTap`, `alsoTo`, `divertTo` の Pekko 契約完全互換 | `scaladsl/Flow.scala`, `scaladsl/Source.scala` | 部分実装 | core | medium | API 名は揃うが、Pekko の遅延・キャンセル契約までの検証が不足 |
| `statefulMap` / `statefulMapConcat` の Pekko 契約完全互換 | `scaladsl/Flow.scala` | 部分実装 | core | medium | `StatefulMapConcatAccumulator` はあるが、契約比較の詰めが必要 |
| `FlowMonitor` / materialization monitor の本格提供 | `FlowMonitor.scala` | 部分実装 | core | medium | `flow_monitor.rs` はあるが、Pekko 同等の公開面は限定的 |
| `mapAsyncPartitioned` の full parity | `MapAsyncPartitioned.scala` | 部分実装 | core | medium | API はあるが、Pekko 実装の partition/backpressure 契約差分確認が必要 |
| `FlowWithContextOps` / `SourceWithContext` 周辺の演算拡充 | `scaladsl/FlowWithContext.scala`, `FlowWithContextOps.scala` | 部分実装 | core | medium | `map`, `filter`, `map_context` はあるが、Pekko の文脈付き演算はまだ薄い |

実装済み代表:
- `map`, `map_async`, `map_async_unordered`, `map_async_partitioned`
- `filter`, `filter_not`, `map_concat`, `map_option`, `recover`, `recover_with_retries`
- `flat_map_concat`, `flat_map_merge`, `prefix_and_tail`
- `group_by`, `split_when`, `split_after`, `merge_substreams`, `concat_substreams`
- `merge`, `merge_latest`, `merge_preferred`, `merge_prioritized`, `merge_sorted`
- `zip`, `zip_all`, `zip_latest`, `zip_with`, `zip_with_index`, `concat`, `prepend`
- `buffer`, `throttle`, `debounce`, `expand`, `conflate`, `grouped`, `sliding`

### マテリアライゼーション / ライフサイクル  ✅ 実装済み 8/12 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorMaterializerSettings` / `IOSettings` / `StreamSubscriptionTimeoutSettings` | `ActorMaterializer.scala` | 未対応 | core / std | medium | `ActorMaterializerConfig` はあるが、Pekko 設定 API 群の層分けが未整備 |
| `Materializer` snapshot / introspection API | `snapshot/MaterializerState.scala` | 未対応 | core | hard | 実行中 graph の可視化・診断 API がない |
| `SystemMaterializer` | `SystemMaterializer.scala` | 未対応 | std | medium | actor system extension としての共有 materializer 不在 |
| `NeverMaterializedException` / detached diagnostics | `NeverMaterializedException.scala`, `StreamDetachedException.scala` | 部分実装 | core | easy | `StreamError` はあるが例外種別までは追っていない |

実装済み:
- `Materializer` trait
- `ActorMaterializer`
- `RunnableGraph::run`
- `watch_termination_mat`
- `KillSwitches`, `UniqueKillSwitch`, `SharedKillSwitch`
- `RestartSettings`, `RestartSource`, `RestartFlow`, `RestartSink`

### Graph / Shape / Stage API  ✅ 実装済み 9/15 (60%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| 公開 `GraphStage` API | `stage/GraphStage.scala` | 部分実装 | core | hard | `graph_stage.rs`, `graph_stage_logic.rs`, `timer_graph_stage_logic.rs`, `async_callback.rs` はあるが Pekko の公開 stage API 面をまだ十分に包めていない |
| `GraphStageLogicWithLogging` / `TimerGraphStageLogicWithLogging` | `stage/GraphStage.scala` | 未対応 | core | medium | logging 付き stage logic の専用抽象なし |
| `SubSinkInlet` / `SubSourceOutlet` | `stage/GraphStage.scala` | 未対応 | core | medium | substream 系は DSL から提供するが、stage authoring API としての low-level 口はない |
| `AmorphousShape` | `Shape.scala` | 未対応 | core | medium | 固定 shape 群は厚いが、動的 shape はない |
| `FlowShape.of`, `SourceShape.of`, `SinkShape.of` 相当の builder ergonomics | `Shape.scala` | 部分実装 | core | easy | 型自体はあるが Pekko 互換の helper 群が少ない |
| `Attributes` の完全互換 | `Attributes.scala` | 部分実装 | core | medium | `attributes.rs` はあるが InputBuffer / LogLevels / cancellation strategy などの全属性は未網羅 |

実装済み:
- `Graph`, `Shape`, `Inlet`, `Outlet`, `SourceShape`, `SinkShape`, `FlowShape`, `BidiShape`
- `UniformFanInShape`, `UniformFanOutShape`
- `FanInShape2` から `FanInShape22` の大量展開
- `TimerGraphStageLogic`, `AsyncCallback`

### Queue / Hub / Actor 連携  ✅ 実装済み 7/10 (70%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.actorRef` | `scaladsl/Source.scala` | 未対応 | core | medium | actor モジュール連携の source 側 API がまだない |
| `Source.actorRefWithBackpressure` | `scaladsl/Source.scala` | 未対応 | core | medium | backpressure handshake 付き source 未提供 |
| `Sink.actorRefWithAck` / legacy actor interop variants | `scaladsl/Sink.scala`, `impl/ActorRef*` | 部分実装 | core | medium | `ActorSink::actor_ref_with_backpressure` はあるが Pekko の actor source / sink 全系統には未達 |

実装済み:
- `BoundedSourceQueue`, `SourceQueue`, `SourceQueueWithComplete`, `SinkQueue`
- `ActorSink::actor_ref`, `ActorSink::actor_ref_with_backpressure`
- `MergeHub`, `BroadcastHub`, `PartitionHub`

### フレーミング / 圧縮 / JSON  ✅ 実装済み 6/8 (75%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Compression` facade の独立名前空間 | `scaladsl/Compression.scala` | 別名で実装済み | core | easy | `Flow::deflate`, `gzip`, `inflate`, `gzip_decompress` として提供。独立 facade は未提供 |
| `JsonFraming` の完全互換スキャナ契約 | `scaladsl/JsonFraming.scala` | 部分実装 | core | medium | `object_scanner`, `array_scanner` はあるが、長大 JSON / strictness 契約差分の詰めが必要 |

実装済み:
- `Framing::delimiter`
- `Framing::length_field`
- `Framing::simple_framing_protocol`
- `JsonFraming::object_scanner`
- `JsonFraming::array_scanner`
- `Flow` ベースの gzip/deflate/inflate/gzip_decompress

### std / IO / 外部連携  ✅ 実装済み 1/8 (12%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `StreamConverters` | `scaladsl/StreamConverters.scala` | 未対応 | std | medium | Rust では `Read` / `Write` / async reader-writer への橋渡しが必要 |
| `Tcp` | `scaladsl/Tcp.scala` | 未対応 | std | hard | `tokio::net` 連携と backpressure を伴う adapter 設計が必要 |
| `TLS` | `scaladsl/TLS.scala` | 未対応 | std | hard | `rustls` 等と stream stage の橋渡しが必要 |
| `StreamRefs` | `scaladsl/StreamRefs.scala` | 未対応 | std | hard | remote / actor / serialization を跨ぐ |
| `FileIO` の append / chunked / path option 拡張 | `scaladsl/FileIO.scala` | 部分実装 | std | easy | `from_path` / `to_path` はあるが Pekko の IO バリエーションは不足 |
| `std::source::create` 以外の std runtime adapters | `scaladsl/Source.scala`, `StreamConverters.scala` | 部分実装 | std | medium | runtime specific source/sink adapter 群が少ない |
| Java Stream / InputStream / OutputStream 直結 | `StreamConverters.scala` | n/a | n/a | n/a | JVM 固有そのままは不要。Rust では `Read` / `Write` に読み替えるべき |

実装済み:
- `FileIO::from_path`
- `FileIO::to_path`

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `Compression` facade を `Framing` / `JsonFraming` と同様の名前空間として切り出す
  実装先層: `core`
- `FlowMonitor` / materialization 監視 API の公開整理
  実装先層: `core`
- `Attributes` の不足 helper を追加し、現行 `attributes.rs` の使い勝手を Pekko へ近づける
  実装先層: `core`

### Phase 2: easy（単純な新規実装）
- `Source.actorRef`
  実装先層: `core`
- `Source.actorRefWithBackpressure`
  実装先層: `core`
- `FileIO` の append / option 拡張
  実装先層: `std`
- `Shape` / `Graph` helper (`of`, copy helper, ergonomic constructor)
  実装先層: `core`

### Phase 3: medium（中程度の実装工数）
- `StreamConverters` の Rust 版最小集合
  実装先層: `std`
- `FlowWithContext` / `SourceWithContext` オペレーター拡充
  実装先層: `core`
- `GraphDSL` builder ergonomics 改善
  実装先層: `core`
- stage authoring API (`SubSinkInlet`, `SubSourceOutlet`, logging variants)
  実装先層: `core`

### Phase 4: hard（アーキテクチャ変更を伴う）
- `GraphDSL` 完全互換 DSL (`~>`, `<~`, fan-in/fan-out wiring)
  実装先層: `core`
- `Tcp`
  実装先層: `std`
- `TLS`
  実装先層: `std`
- `StreamRefs`
  実装先層: `std`
- materializer snapshot / interpreter introspection
  実装先層: `core`

### 対象外（n/a）
- `javadsl/*` 全般
- `CompletionStage`, Java Stream, `InputStream` / `OutputStream` そのものへの直結 API
- legacy `Processor` / `ActorPublisher` / `ActorSubscriber` 系の JVM 依存面

## まとめ

- 全体として、fraktor-rs の `stream` は **基本 DSL と主要オペレーターの厚みが高い**。日常的な `Source` / `Flow` / `Sink` 変換はかなりカバーされている。
- すぐ価値を出せるのは `Source.actorRef*`、`Compression` facade の整理、`Attributes` / `FlowMonitor` の公開面整理。
- 実用上の大きなギャップは **GraphDSL 完全互換** と **StreamConverters / Tcp / TLS / StreamRefs**。ここは Pekko の「配線基盤・IO 基盤」に相当するため、価値は高いが工数も大きい。
- つまり現状は「演算 DSL は強い、外部接続と高度配線は弱い」という状態である。

# stream モジュール ギャップ分析

生成日: 2026-03-23

対象:
- fraktor-rs: `modules/stream/src/`
- Pekko 参照: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/`

計数ポリシー:
- Pekko 側は `scaladsl/*` と `stage/*` の top-level public 型を母集団にする
- `javadsl/*` は Scala DSL の重複 wrapper が多いため、カバレッジ母集団から除外し、必要なものだけ備考で補足する
- `impl/*` は内部実装なので母集団から除外する
- fraktor-rs 側は `modules/stream/src/core` と `modules/stream/src/std` の純粋な `pub` 型を母集団にする
- カバレッジは「型単位」の概算であり、最終判断はカテゴリ別ギャップを優先する

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 161 |
| fraktor-rs 公開型数 | 137（core: 135, std: 2） |
| カバレッジ（型単位） | 137/161 (85%) |
| ギャップ数 | 16（core: 11, std: 5） |

一言評価:
- `Source` / `Flow` / `Sink`、shape 群、GraphDSL 基礎、queue / hub、framing / compression まで含め、`stream` のコア DSL はかなり厚い
- 直近の修正で `fromSinkAndSourceCoupled*` の終端連携、`StreamConverters::from_reader` の lazy 化、`FileIO` / `StreamConverters` の境界値・エラー処理は Pekko 寄りに改善された
- 残る大きな差は「GraphStage authoring API の低レベル互換」「GraphDSL の記法互換」「Tcp / TLS / StreamRefs などの外部接続基盤」に集中している

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 12 API ファミリ | 10 API ファミリ | 83% |
| core / typed ラッパー | 0 | 0 | n/a |
| std / アダプタ | 8 API ファミリ | 3 API ファミリ | 38% |

補足:
- `modules/stream` には `core/typed` サブ層は存在しない
- `core/` に DSL、shape、graph、queue / hub、materialization、compression、framing が集約されている
- `std/` は現在 `FileIO` と `StreamConverters` が中心であり、Pekko の transport / remote stream 系は未着手

## カテゴリ別ギャップ

### コア DSL / 型 ✅ 実装済み 12/14 (86%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SystemMaterializer` | `scaladsl/SystemMaterializer.scala` | 未対応 | std | medium | 共有 materializer の取得 API がない |
| materializer snapshot / diagnostics | `snapshot/MaterializerState.scala` | 未対応 | core | hard | 実行中 graph の introspection API がない |

実装済み代表:
- `Source`, `Flow`, `Sink`, `RunnableGraph`, `BidiFlow`
- `SourceWithContext`, `FlowWithContext`
- `SourceSubFlow`, `FlowSubFlow`, `SourceGroupBySubFlow`
- `KeepLeft`, `KeepRight`, `KeepBoth`, `KeepNone`
- `fromSinkAndSource`, `fromSinkAndSourceMat`, `fromSinkAndSourceCoupled`, `fromSinkAndSourceCoupledMat`

### オペレーター / 変換 DSL ✅ 実装済み 61/74 (82%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FlowWithContextOps` の完全互換 | `scaladsl/FlowWithContextOps.scala` | 部分実装 | core | medium | `map`, `filter`, `map_concat`, `grouped`, `sliding`, `map_async` はあるが、Pekko の演算群すべては未網羅 |
| `statefulMap` / `statefulMapConcat` の契約 parity | `scaladsl/Flow.scala`, `scaladsl/StatefulMapConcatAccumulator.scala` | 部分実装 | core | medium | API はあるが Pekko 契約との差分詰めが必要 |
| `wireTap` / `alsoTo` / `divertTo` の完全互換 | `scaladsl/Flow.scala` | 部分実装 | core | medium | API はあるが termination / backpressure 契約の精査が必要 |
| `FlowMonitor` の公開面 parity | `scaladsl/Flow.scala`, `FlowMonitor.scala` | 部分実装 | core | medium | `FlowMonitorImpl` / `FlowMonitorState` はあるが公開診断 API は薄い |

実装済み代表:
- `map`, `map_async`, `map_async_partitioned`
- `filter`, `filter_not`, `map_concat`, `map_option`
- `recover`, `recover_with_retries`, `flat_map_concat`, `flat_map_merge`
- `grouped`, `sliding`, `prefix_and_tail`
- `merge`, `merge_latest`, `merge_preferred`, `merge_prioritized`, `merge_sorted`
- `zip`, `zip_all`, `zip_latest`, `zip_with`, `zip_with_index`, `concat`, `prepend`
- `buffer`, `throttle`, `debounce`, `expand`, `conflate`, `take_within`

### マテリアライゼーション / ライフサイクル ✅ 実装済み 10/14 (71%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SystemMaterializer` | `SystemMaterializer.scala` | 未対応 | std | medium | actor system extension としての共有 materializer がない |
| `ActorMaterializerSettings` 相当の広い設定面 | `ActorMaterializer.scala`, `ActorMaterializerSettings.scala` | 部分実装 | core / std | medium | `ActorMaterializerConfig` はあるが Pekko の設定群全体には未達 |
| `NeverMaterializedException` など例外種別の互換 | `NeverMaterializedException.scala`, `AbruptTerminationException.scala` | 部分実装 | core | easy | `StreamError` へ統合されており、JVM 例外面は未追従 |
| materializer lifecycle diagnostics | `snapshot/*` | 未対応 | core | hard | 実行時可視化 API 不足 |

実装済み:
- `Materializer` trait
- `ActorMaterializer`
- `watch_termination_mat`
- `UniqueKillSwitch`, `SharedKillSwitch`, `KillSwitches`
- `RestartSource`, `RestartFlow`, `RestartSink`
- `fromSinkAndSourceCoupled*` の coupled termination

### Graph / Shape / Stage API ✅ 実装済み 10/17 (59%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GraphStageLogic` 公開 authoring API の parity | `stage/GraphStage.scala` | 部分実装 | core | hard | `GraphStage`, `GraphStageLogic`, `TimerGraphStageLogic`, `AsyncCallback` はあるが Pekko の low-level 口を完全再現していない |
| `SubSinkInlet` / `SubSourceOutlet` | `stage/GraphStage.scala` | 未対応 | core | medium | substream の低レベル authoring API がない |
| `GraphStageLogicWithLogging` / `TimerGraphStageLogicWithLogging` | `stage/GraphStage.scala` | 未対応 | core | medium | logging 付き stage logic 専用抽象がない |
| `GraphDSL` 記法互換 (`~>`, `<~`, port combinator) | `scaladsl/Graph.scala` | 部分実装 | core | hard | `GraphDslBuilder` はあるが Pekko DSL の記法互換までは未達 |
| `AmorphousShape` | `Shape.scala` | 未対応 | core | medium | 固定 shape 群は厚いが、動的 shape はない |
| `Attributes` の完全互換 | `Attributes.scala` | 部分実装 | core | medium | `InputBuffer`, `LogLevels` 等はあるが、Pekko 属性群の網羅は未完 |

実装済み:
- `Inlet`, `Outlet`, `SourceShape`, `SinkShape`, `FlowShape`, `BidiShape`
- `UniformFanInShape`, `UniformFanOutShape`
- `FanInShape2` から `FanInShape22`
- `GraphDsl`, `GraphDslBuilder`
- `GraphStage`, `GraphStageLogic`, `TimerGraphStageLogic`, `AsyncCallback`

### Queue / Hub / Actor 連携 ✅ 実装済み 9/11 (82%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Sink.actorRefWithAck` の API 分離 | `scaladsl/Sink.scala` | 部分実装 | core | easy | `actor_ref_with_backpressure` で近いが Pekko の名前・契約分離とは異なる |
| legacy actor subscriber / publisher 系 | `impl/*`, `scaladsl/Source.scala`, `scaladsl/Sink.scala` | n/a | n/a | n/a | JVM / Akka 由来の互換層であり、現状の fraktor 方針では優先度低い |

実装済み:
- `Source.actorRef`
- `Source.actorRefWithBackpressure`
- `Sink.actorRef`
- `Sink.actorRefWithBackpressure`
- `BoundedSourceQueue`, `SourceQueue`, `SourceQueueWithComplete`, `SinkQueue`
- `MergeHub`, `BroadcastHub`, `PartitionHub`

### フレーミング / 圧縮 / JSON ✅ 実装済み 7/9 (78%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `JsonFraming` の契約 parity | `scaladsl/JsonFraming.scala` | 部分実装 | core | medium | object / array scanner はあるが strictness や corner case 比較は未完 |
| `Compression` の stage / facade 完全互換 | `scaladsl/Compression.scala` | 部分実装 | core | easy | `Compression` facade と `Flow` の圧縮・解凍はあるが、全 overload・設定面は未網羅 |

実装済み:
- `Framing::delimiter`
- `Framing::length_field`
- `Framing::simple_framing_protocol`
- `JsonFraming::object_scanner`
- `JsonFraming::array_scanner`
- `Compression`
- `Flow::gzip`, `deflate`, `inflate`, `gzip_decompress`

### std / IO / 外部連携 ✅ 実装済み 3/8 (38%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Tcp` | `scaladsl/Tcp.scala` | 未対応 | std | hard | `tokio::net` / backpressure / connection materialization を含む設計が必要 |
| `TLS` | `scaladsl/TLS.scala` | 未対応 | std | hard | `rustls` 等との bridge 設計が必要 |
| `StreamRefs` | `scaladsl/StreamRefs.scala` | 未対応 | std | hard | remote / serialization / actor 連携を跨ぐ |
| Java `InputStream` / `OutputStream` 直結 | `scaladsl/StreamConverters.scala`, `javadsl/*` | n/a | n/a | n/a | Rust では `Read` / `Write` に読み替えるべき |
| `JavaFlowSupport` | `scaladsl/JavaFlowSupport.scala` | n/a | n/a | n/a | JVM 相互運用固有 |

実装済み:
- `FileIO::from_path`, `from_path_with_options`, `to_path`, `to_path_with_options`, `to_path_with_position`
- `StreamConverters::from_reader`
- `StreamConverters::to_writer`

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `Attributes` の不足 helper と属性網羅の補強
  実装先層: core
- `FlowWithContext` / `SourceWithContext` の軽量 helper 追加
  実装先層: core
- Graph / Shape helper の ergonomic 改善
  実装先層: core

### Phase 2: easy（単純な新規実装）
- `Sink.actorRefWithAck` 相当の API 分離
  実装先層: core
- `Compression` / `JsonFraming` の不足 overload 補完
  実装先層: core
- `FileIO` / `StreamConverters` の小さなオプション差分補完
  実装先層: std

### Phase 3: medium（中程度の実装工数）
- `FlowWithContextOps` / `SourceWithContext` 演算の拡充
  実装先層: core
- `GraphStage` authoring API の補完 (`SubSinkInlet`, `SubSourceOutlet`, logging 変種)
  実装先層: core
- `SystemMaterializer` 相当の共有 materializer API
  実装先層: std

### Phase 4: hard（アーキテクチャ変更を伴う）
- `GraphDSL` 記法互換 (`~>`, `<~`, port combinator)
  実装先層: core
- `Tcp`
  実装先層: std
- `TLS`
  実装先層: std
- `StreamRefs`
  実装先層: std
- materializer snapshot / diagnostics
  実装先層: core

### 対象外（n/a）
- `javadsl/*`
- `JavaFlowSupport`
- JVM 例外型そのものの互換
- legacy actor publisher / subscriber 互換層

## まとめ

- 現状の `stream` は、Pekko と比較しても **コア DSL と主要オペレーター群はかなり高い水準で揃っている**。
- 直近の修正で `fromSinkAndSourceCoupled*`、`FileIO`、`StreamConverters::from_reader` の互換性ギャップがかなり埋まり、日常的な stream 演算で困る差は減った。
- すぐ価値が出る未実装は、`FlowWithContextOps` 拡充、`Attributes` 補強、`Sink.actorRefWithAck` API 分離あたりである。
- 実用上の大きなギャップは依然として **GraphStage authoring API**, **GraphDSL 記法互換**, **Tcp / TLS / StreamRefs** であり、ここが Pekko 互換の最終ボトルネックである。
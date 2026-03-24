# stream モジュール ギャップ分析

TCP, TLSがらみはスコープアウトしてください。remoteモジュールとの兼ね合いを検討してから対応します。

> [!IMPORTANT]
> このドキュメントで最優先の指示は `async()` である。
> `async()` は YAGNI の対象外であり、Gate 0 の blocker として扱う。
> `async_boundary()` は公開 API として育てない。公開 API から廃止し、最終的に `async()` を Pekko 互換の意味で置き換える。
> `async()` が実現するまで Gate 0 は完了にしてはならない。
> Gate 0 が未完了の間は、GraphDSL 拡張、materialization API 拡張、実行主体に影響する stage 追加を完了扱いにしてはならない。

生成日: 2026-03-24

対象:
- fraktor-rs: `modules/stream/src/`
- Pekko 参照: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/`
- Pekko stream-typed: `references/pekko/stream-typed/src/main/scala/org/apache/pekko/stream/typed/scaladsl/`

計数ポリシー:
- Pekko 側は `scaladsl/*`、`stage/*`、`snapshot/*`、`stream-typed/scaladsl/*` の top-level public 型を母集団にする
- `javadsl/*` は Scala DSL の重複 wrapper が多いため、カバレッジ母集団から除外し、必要なものだけ備考で補足する
- `impl/*` は内部実装なので母集団から除外する
- 例外型（`NeverMaterializedException` 等）は母集団に含め、fraktor-rs 側で `StreamError` へ統合されている場合は部分実装として扱う
- fraktor-rs 側は `modules/stream/src/core` と `modules/stream/src/std` の純粋な `pub` 型を母集団にする
- カバレッジは「型単位」の概算であり、最終判断はカテゴリ別ギャップを優先する

## 運用指針（Context Rot 対策）

- このドキュメントを `stream` の async 方針に関する **単一の真実源** とする
- 今回の運用ルールは **Gate 0 必達、他フェーズは任意** とする
- Gate 0 が未完了の間は、他フェーズの進捗があっても `stream` async 方針は未完了扱いとする
- `async_boundary()` を残す方向の更新、`async()` を backlog 扱いに戻す更新、YAGNI を再適用する更新を禁止する
- このドキュメントを更新する場合は、先頭と末尾の `IMPORTANT` ブロックも同時に確認する

## 用語固定

- `Gate 0`: `async()` 実現まで完了にできない最優先ゲート
- `async()`: Pekko 互換の非同期境界。graph の island 分割と実行主体分離を伴う公開 API
- `async_boundary()`: 現状の fraktor-rs 実装名。単一 interpreter 内の buffer / backpressure 境界であり、将来は公開 API から廃止する対象
- `async island`: 非同期境界で分割された graph 実行単位。island ごとに独立した mailbox/buffer と実行主体を持つ前提
- `完了`: Gate 0 の条件を満たした状態。文書化だけでは完了とみなさない

## 非目標

- `async_boundary()` を Pekko 互換 API として延命しない
- `async()` を名前だけ先に公開しない
- Gate 0 未完了のまま GraphDSL / materialization / 実行モデル拡張を「完了」にしない
- JVM 固有 API との 1:1 互換を優先して Gate 0 を後回しにしない

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 169（stream: 161 + stream-typed: 8） |
| fraktor-rs 公開型数 | 137（core: 135, std: 2） |
| カバレッジ（型単位） | 137/169 (81%) |
| ギャップ数 | 25（core: 16, std: 9） |

一言評価:
- `Source` / `Flow` / `Sink`、shape 群、GraphDSL 基礎、queue / hub、framing / compression まで含め、`stream` のコア DSL はかなり厚い
- `ask`, `ask_with_status`, `ask_with_context`, `ask_with_status_and_context` のアクター連携オペレーターも実装済み
- `ActorSource`, `ActorSink` のアクター統合もあり、Pekko stream-typed 相当の主要機能はカバー
- 残る大きな差は「GraphStage authoring API の低レベル互換」「GraphDSL の記法互換」「Tcp / TLS / StreamRefs などの外部接続基盤」「PubSub」「SubFlow の API 不足」に集中している

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 13 API ファミリ | 11 API ファミリ | 85% |
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
| `SystemMaterializer` | `SystemMaterializer.scala` | 未対応 | std | medium | 共有 materializer の取得 API がない |
| materializer snapshot / diagnostics | `snapshot/MaterializerState.scala` | 未対応 | core | hard | 実行中 graph の introspection API がない |

実装済み代表:
- `Source`, `Flow`, `Sink`, `RunnableGraph`, `BidiFlow`
- `SourceWithContext`, `FlowWithContext`
- `SourceSubFlow`, `FlowSubFlow`, `SourceGroupBySubFlow`, `FlowGroupBySubFlow`
- `KeepLeft`, `KeepRight`, `KeepBoth`, `KeepNone`
- `fromSinkAndSource`, `fromSinkAndSourceMat`, `fromSinkAndSourceCoupled`, `fromSinkAndSourceCoupledMat`

### オペレーター / 変換 DSL ✅ 実装済み 62/76 (82%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FlowWithContextOps` の完全互換 | `scaladsl/FlowWithContextOps.scala` | 部分実装 | core | medium | `map`, `filter`, `map_concat`, `grouped`, `sliding`, `map_async` はあるが、Pekko の演算群すべては未網羅 |
| `statefulMap` / `statefulMapConcat` の契約 parity | `scaladsl/Flow.scala` | 部分実装 | core | medium | API はあるが Pekko 契約との差分詰めが必要 |
| `wireTap` / `alsoTo` / `divertTo` の完全互換 | `scaladsl/Flow.scala` | 部分実装 | core | medium | API はあるが termination / backpressure 契約の精査が必要 |
| `FlowMonitor` の公開面 parity | `FlowMonitor.scala` | 部分実装 | core | medium | `FlowMonitorImpl` / `FlowMonitorState` はあるが公開診断 API は薄い |
| `Flow.fold_while` | `scaladsl/Flow.scala` | 未対応 | core | easy | `Sink.fold_while` はあるが `Flow` 側のオペレーターがない |
| `BidiFlow.joinMat` | `scaladsl/BidiFlow.scala` | 未対応 | core | easy | `BidiFlow.join` はあるが Mat 合成版がない |

実装済み代表:
- `map`, `map_async`, `map_async_partitioned`, `map_async_partitioned_unordered`
- `filter`, `filter_not`, `map_concat`, `map_option`, `collect`, `collect_type`, `collect_first`, `collect_while`
- `recover`, `recover_with_retries`, `on_error_complete`, `on_error_continue`, `map_error`
- `flat_map_concat`, `flat_map_merge`, `flat_map_prefix`, `flat_map_prefix_mat`, `switch_map`
- `grouped`, `grouped_within`, `grouped_weighted`, `grouped_weighted_within`, `grouped_adjacent_by`, `grouped_adjacent_by_weighted`, `sliding`
- `merge`, `merge_all`, `merge_latest`, `merge_preferred`, `merge_prioritized`, `merge_sorted`, `merge_prioritized_n`
- `zip`, `zip_all`, `zip_latest`, `zip_with`, `zip_with_index`, `zip_n`, `zip_with_n`
- `concat`, `concat_lazy`, `concat_all_lazy`, `prepend`, `prepend_lazy`, `or_else`, `interleave`, `interleave_all`, `interleave_mat`
- `buffer`, `throttle`, `debounce`, `expand`, `extrapolate`, `conflate`, `conflate_with_seed`, `batch`, `batch_weighted`
- `scan`, `scan_async`, `fold`, `fold_async`, `reduce`
- `take`, `drop`, `take_while`, `take_until`, `take_within`, `drop_while`, `drop_within`, `drop_repeated`, `limit`, `limit_weighted`
- `initial_delay`, `initial_timeout`, `completion_timeout`, `idle_timeout`, `backpressure_timeout`, `keep_alive`
- `delay`, `delay_with`
- `log`, `log_with_marker`, `monitor`, `monitor_mat`, `watch_termination`, `watch_termination_mat`
- `also_to`, `also_to_mat`, `also_to_all`, `divert_to`, `divert_to_mat`, `wire_tap`, `wire_tap_mat`
- `do_on_first`, `do_on_cancel`
- `ask`, `ask_with_status`, `ask_with_context`, `ask_with_status_and_context`
- `aggregate_with_boundary`, `prefix_and_tail`
- `intersperse`, `detach`
- `group_by`, `split_when`, `split_after`, `merge_substreams`, `merge_substreams_with_parallelism`, `concat_substreams`
- `via`, `via_mat`, `to`, `to_mat`, `pre_materialize`, `materialize_into_source`

### SubFlow API ✅ 実装済み 3/5 (60%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SubFlow.to` | `scaladsl/SubFlow.scala` | 未対応 | core | easy | SubFlow に直接 Sink を接続するメソッドがない |
| GroupBySubFlow のオペレーター拡充 | `scaladsl/SubFlow.scala` (FlowOps 継承) | 部分実装 | core | medium | `FlowGroupBySubFlow`/`SourceGroupBySubFlow` は `merge_substreams` のみ。parallelism 制御・変換オペレーターなし |

実装済み:
- `FlowSubFlow` / `SourceSubFlow`: `merge_substreams`, `merge_substreams_with_parallelism`, `concat_substreams`, `map`, `filter`, `drop`, `take`, `drop_while`, `take_while`
- `FlowGroupBySubFlow` / `SourceGroupBySubFlow`: `merge_substreams`

### マテリアライゼーション / ライフサイクル ✅ 実装済み 10/14 (71%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SystemMaterializer` | `SystemMaterializer.scala` | 未対応 | std | medium | actor system extension としての共有 materializer がない |
| `ActorMaterializerSettings` 相当の広い設定面 | `ActorMaterializer.scala` | 部分実装 | core / std | medium | `ActorMaterializerConfig` はあるが Pekko の設定群全体には未達 |
| `NeverMaterializedException` など例外種別の互換 | `NeverMaterializedException.scala` | 部分実装 | core | easy | `StreamError` へ統合されており、JVM 例外面は未追従 |
| materializer lifecycle diagnostics | `snapshot/*` | 未対応 | core | hard | 実行時可視化 API 不足 |

実装済み:
- `Materializer` trait, `ActorMaterializer`, `ActorMaterializerConfig`, `Materialized<Mat>`
- `watch_termination`, `watch_termination_mat`
- `UniqueKillSwitch`, `SharedKillSwitch`, `KillSwitches`
- `RestartSource`, `RestartFlow`, `RestartSink`, `RetryFlow`
- `fromSinkAndSourceCoupled*` の coupled termination

### Graph / Shape / Stage API ✅ 実装済み 10/17 (59%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GraphStageLogic` 公開 authoring API の parity | `stage/GraphStage.scala` | 部分実装 | core | hard | `GraphStage`, `GraphStageLogic`, `TimerGraphStageLogic`, `AsyncCallback` はあるが Pekko の low-level 口を完全再現していない |
| `SubSinkInlet` / `SubSourceOutlet` | `stage/GraphStage.scala` | 未対応 | core | medium | substream の低レベル authoring API がない |
| `GraphStageLogicWithLogging` / `TimerGraphStageLogicWithLogging` | `stage/GraphStage.scala` | 未対応 | core | easy | logging 付き stage logic 専用抽象がない |
| `GraphDSL` 記法互換 (`~>`, `<~`, port combinator) | `scaladsl/Graph.scala` | 部分実装 | core | hard | `GraphDslBuilder` はあるが Pekko DSL の記法互換までは未達 |
| `AmorphousShape` | `Shape.scala` | 未対応 | core | medium | 固定 shape 群は厚いが、動的 shape はない |
| `Attributes` の完全互換 | `Attributes.scala` | 部分実装 | core | medium | `InputBuffer`, `LogLevels` 等はあるが、Pekko 属性群の網羅は未完 |
| `FanOutShape3`〜`FanOutShape22` | `FanOutShape.scala` | 未対応 | core | easy | `FanOutShape2` はあるが `FanOutShape3`〜`FanOutShape22` がない（`FanInShape` は 2〜22 まで実装済み） |

実装済み:
- `Inlet`, `Outlet`, `PortId`, `SourceShape`, `SinkShape`, `FlowShape`, `BidiShape`, `StreamShape`
- `UniformFanInShape`, `UniformFanOutShape`, `FanOutShape2`
- `FanInShape2` から `FanInShape22`
- `GraphDsl`, `GraphDslBuilder`, `StreamGraph`, `GraphInterpreter`
- `GraphStage`, `GraphStageLogic`, `TimerGraphStageLogic`, `AsyncCallback`, `StageContext`, `StageKind`

### Queue / Hub / Actor 連携 ✅ 実装済み 10/13 (77%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `PubSub.source` / `PubSub.sink` | `stream-typed/scaladsl/PubSub.scala` | 未対応 | core | medium | Topic ベースの pub/sub ストリームがない |
| `Sink.actorRefWithAck` の API 分離 | `scaladsl/Sink.scala` | 部分実装 | core | easy | `actor_ref_with_backpressure` で近いが Pekko の名前・契約分離とは異なる |
| legacy actor subscriber / publisher 系 | `impl/*` | n/a | n/a | n/a | JVM / Akka 由来の互換層であり、現状の fraktor 方針では優先度低い |

実装済み:
- `ActorSource::actor_ref`, `ActorSource::actor_ref_with_backpressure`
- `ActorSink::actor_ref`, `ActorSink::actor_ref_with_backpressure`
- `ActorSourceRef<T>`
- `Flow::ask`, `Flow::ask_with_status`, `Flow::ask_with_context`, `Flow::ask_with_status_and_context`
- `BoundedSourceQueue`, `SourceQueue`, `SourceQueueWithComplete`, `SinkQueue`
- `MergeHub`, `BroadcastHub`, `PartitionHub`

### RestartSettings ✅ 実装済み 5/7 (71%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `restartOn` predicate | `RestartSettings.scala` | 未対応 | core | easy | エラー種別によるリスタート可否判定がない |
| `LogSettings` | `RestartSettings.scala` | 未対応 | core | easy | リスタート時のログレベル制御がない |

実装済み:
- `RestartSettings`: `min_backoff_ticks`, `max_backoff_ticks`, `random_factor_permille`, `max_restarts`, `max_restarts_within_ticks`
- fraktor-rs 独自: `complete_on_max_restarts`, `jitter_seed`（Pekko にはない機能）

### フレーミング / 圧縮 / JSON ✅ 実装済み 7/9 (78%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `JsonFraming` の契約 parity | `scaladsl/JsonFraming.scala` | 部分実装 | core | medium | object / array scanner はあるが strictness や corner case 比較は未完 |
| `Compression` の stage / facade 完全互換 | `scaladsl/Compression.scala` | 部分実装 | core | easy | `Compression` facade と `Flow` の圧縮・解凍はあるが、全 overload・設定面は未網羅 |

実装済み:
- `Framing::delimiter`, `Framing::length_field`, `Framing::simple_framing_protocol`
- `JsonFraming::object_scanner`, `JsonFraming::array_scanner`
- `Compression`
- `Flow::gzip`, `deflate`, `inflate`, `gzip_decompress`

### std / IO / 外部連携 ✅ 実装済み 3/8 (38%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Tcp` | `scaladsl/Tcp.scala` | 未対応 | std | hard | `tokio::net` / backpressure / connection materialization を含む設計が必要 |
| `TLS` | `scaladsl/TLS.scala` | 未対応 | std | hard | `rustls` 等との bridge 設計が必要 |
| `StreamRefs` (`SourceRef`, `SinkRef`) | `scaladsl/StreamRefs.scala` | 未対応 | std | hard | remote / serialization / actor 連携を跨ぐ |
| Java `InputStream` / `OutputStream` 直結 | `scaladsl/StreamConverters.scala` | n/a | n/a | n/a | Rust では `Read` / `Write` に読み替え済み |
| `JavaFlowSupport` | `scaladsl/JavaFlowSupport.scala` | n/a | n/a | n/a | JVM 相互運用固有 |

実装済み:
- `FileIO::from_path`, `from_path_with_options`, `to_path`, `to_path_with_options`, `to_path_with_position`
- `StreamConverters::from_reader`, `StreamConverters::to_writer`

### テスティング ✅ 実装済み 3/3 (100%)

実装済み:
- `TestSourceProbe<T>`, `TestSinkProbe<T>`, `StreamFuzzRunner`

## 実装優先度の提案

### Gate 0: MUST（完了ゲート、YAGNI 適用外）

- `async()` は YAGNI の対象外とする
  実装先層: core / std
  理由: 追加機能ではなく実行モデルの土台であり、後付けすると materializer / GraphDSL / preMaterialize へ大きく波及する
- `async()` と `async_boundary()` の非互換を明文化する
  実装先層: core
- 現状の `async_boundary()` は **単一 interpreter 内の buffer / backpressure 境界**であり、Pekko の async island ではないと資料・API 方針に固定する
  実装先層: core
- `async_boundary()` は将来 API として育てない。**公開 API から廃止し、最終的に `async()` を Pekko 互換の意味で置き換える**
  実装先層: core
- `1 materialized graph = 1 interpreter` / `1 materializer = 1 drive actor` の現状制約を、解消すべき blocker として固定する
  実装先層: core / std
- この Gate は **`async()` が実現するまで完了にしてはならない**
  実装先層: core / std
- この Gate が未完了の間は、少なくとも以下を完了扱いにしない
  実装先層: core / std
  対象: GraphDSL 拡張、materialization API 拡張、実行主体に影響する stage 追加、`async_boundary()` 前提の公開 API 整備
- 公開 `async()` の解禁条件を先に定義する
  実装先層: core / std
  条件: graph の island 分割、island 間 mailbox/buffer、island ごとの実行主体、dispatcher 指定方針、`async_boundary()` の公開廃止

### Phase 1: trivial / easy（既存組み合わせまたは単純追加で即実装可能）

- `Flow.fold_while` — `Sink.fold_while` と同等のロジックを Flow 側に追加
  実装先層: core
- `BidiFlow.join_mat` — `BidiFlow.join` の Mat 合成版
  実装先層: core
- `SubFlow.to` — SubFlow に直接 Sink を接続するメソッド
  実装先層: core
- `GraphStageLogicWithLogging` / `TimerGraphStageLogicWithLogging` — logging trait 追加
  実装先層: core
- `FanOutShape3`〜`FanOutShape22` — FanInShape と同様のバリエーション生成
  実装先層: core
- `RestartSettings.restart_on` — エラー種別判定 predicate
  実装先層: core
- `RestartSettings.log_settings` — リスタート時ログレベル制御
  実装先層: core
- `Sink.actorRefWithAck` の API 分離 — Pekko 契約との名前・意味合わせ
  実装先層: core
- `Compression` の不足 overload 補完
  実装先層: core
- `Attributes` の不足 helper と属性網羅の補強
  実装先層: core

### Phase 2: medium（中程度の実装工数）

- `FlowWithContextOps` / `SourceWithContext` 演算の拡充
  実装先層: core
- `GraphStage` authoring API の補完 (`SubSinkInlet`, `SubSourceOutlet`)
  実装先層: core
- `AmorphousShape` — 動的 shape
  実装先層: core
- `GroupBySubFlow` のオペレーター拡充（parallelism 制御、変換オペレーター）
  実装先層: core
- `PubSub.source` / `PubSub.sink` — Topic ベース pub/sub ストリーム
  実装先層: core
- `SystemMaterializer` 相当の共有 materializer API
  実装先層: std
- `FlowMonitor` の公開診断 API 強化
  実装先層: core
- `JsonFraming` の契約 parity（strictness、corner case）
  実装先層: core
- `statefulMap` / `statefulMapConcat` の Pekko 契約 parity
  実装先層: core
- `wireTap` / `alsoTo` / `divertTo` の termination / backpressure 契約精査
  実装先層: core
- `ActorMaterializerSettings` の設定面拡充
  実装先層: core / std

### Phase 3: hard（アーキテクチャ変更を伴う）

- `GraphDSL` 記法互換 (`~>`, `<~`, port combinator)
  実装先層: core（Rust マクロまたは trait operator で実現する設計が必要）
- `GraphStageLogic` 公開 authoring API の完全 parity
  実装先層: core
- `Tcp` — `tokio::net` ベースの TCP ストリーム
  実装先層: std（`bind`, `outgoing_connection`, `ServerBinding`, `IncomingConnection`, `OutgoingConnection`）
- `TLS` — `rustls` ベースの TLS ストリーム
  実装先層: std
- `StreamRefs` (`SourceRef`, `SinkRef`) — リモートストリーム参照
  実装先層: std（remote / serialization / actor 連携を跨ぐ）
- materializer snapshot / diagnostics — 実行時可視化 API
  実装先層: core

### 対象外（n/a）

- `javadsl/*` — JVM 相互運用固有
- `JavaFlowSupport` — JVM 相互運用固有
- JVM 例外型そのものの互換（`StreamTimeoutException` 等の JVM 例外階層）
- legacy actor publisher / subscriber 互換層（`impl/ActorPublisher`, `impl/ActorSubscriber`）
- `completionStage` / `fromJavaStream` 等の JVM 固有ファクトリメソッド
- `fromProcessor` / `fromProcessorMat` / `toProcessor` — Reactive Streams Processor インターフェースは JVM 固有

## まとめ

- 現状の `stream` は、Pekko と比較しても **コア DSL と主要オペレーター群はかなり高い水準で揃っている**（型カバレッジ 81%、オペレーターカバレッジ 82%）
- `ask`, `ask_with_status`, `ask_with_context`, `ask_with_status_and_context` のアクター連携オペレーター、`ActorSource`, `ActorSink` の typed アクター統合も実装済みであり、Pekko stream-typed 相当の主要機能はカバーされている
- ただし `async()` は **後でやる hard task ではなく Gate 0 の blocker** である。`async()` が実現するまで Gate 0 は完了にしてはならない
- そのため、方針は **`async_boundary()` を残すことではなく、廃止して `async()` を Pekko 互換で出すこと** である。`async_boundary()` を暫定互換 API として温存しない
- この項目では YAGNI を適用しない。`async()` は「今は要らない機能」ではなく、後続設計を拘束する基盤課題である
- **すぐ価値が出る未実装**（Phase 1）: `Flow.fold_while`, `BidiFlow.join_mat`, `SubFlow.to`, `FanOutShape3-22`, `RestartSettings.restart_on` — いずれも既存パターンの拡張で即座に実装可能
- **実用上の主要ギャップ**（Phase 2-3）: `SubSinkInlet`/`SubSourceOutlet` の低レベル stage authoring、`PubSub` ストリーム、`SystemMaterializer`、`GraphDSL` 記法互換、そして `async()` を成立させる island 分割
- **最終ボトルネック**（Phase 3）: `Tcp`, `TLS`, `StreamRefs` の外部接続基盤。これらは Pekko の production-grade ストリーミングにおける差別化要素であり、fraktor-rs の本格利用には不可欠だが、アーキテクチャ設計を伴う

> [!IMPORTANT]
> 再確認:
> `async()` は Gate 0 の blocker であり、未実現のまま完了扱いにしてはならない。
> `async_boundary()` は暫定互換 API として温存しない。公開 API から廃止し、`async()` を Pekko 互換で実装する。
> この項目には YAGNI を適用しない。

## 更新チェックリスト

- この更新で **Gate 0 必達、他フェーズ任意** の運用ルールを壊していないか
- `async()` を backlog や任意タスクに格下げしていないか
- `async_boundary()` を延命する文言を入れていないか
- Gate 0 の完了条件を弱めていないか
- 先頭と末尾の `IMPORTANT` ブロックが本文と矛盾していないか
- async 方針の正本がこのドキュメントであることを維持しているか

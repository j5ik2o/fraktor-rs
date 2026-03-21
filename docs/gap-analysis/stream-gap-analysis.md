# stream モジュール ギャップ分析

生成日: 2026-03-22
対象:
- fraktor-rs: `modules/stream/src/`
- Pekko 参照: `references/pekko/stream/src/main/scala/`

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（機械抽出, Scala 宣言ベース） | 681 |
| fraktor-rs 公開型数（機械抽出, Rust `pub` 型ベース） | 207 |
| Pekko 公開メソッド数（機械抽出） | 4647 |
| fraktor-rs 公開メソッド数（機械抽出） | 1161 |
| 代表 API セットでのカバレッジ | かなり高い |
| 主な実装ギャップ | `Source.actorRef*`, GraphDSL 完全互換, `StreamConverters`, `Tcp`, `TLS` |

**一言評価**: `Source` / `Flow` / `Sink` の基本 DSL、主要オペレーター、*Mat バリアント、KillSwitch、Hub、Json/Framing、FileIO までかなり埋まっている。現時点の大きな差は「Pekko らしい GraphDSL の宣言的配線」と「Actor/Tcp/StreamConverters の JVM/ActorSystem 前提 API」に寄っている。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 主要 DSL の大半 | 充実 | 高い |
| core / typed ラッパー | 該当なし | 該当なし | n/a |
| std / アダプタ | FileIO, std source | 限定的 | 低め |

**補足**:
- `modules/stream` には `core/typed` サブ層は存在しない。
- `std/` は `file_io` と `source::create` に留まっており、Pekko の `StreamConverters` / `Tcp` / `TLS` 相当は未提供。

## カテゴリ別ギャップ

### コア DSL / 型　✅ 実装済み 10/11 (91%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GraphDSL` の完全互換 Builder + implicit operator DSL | `scaladsl/Graph.scala` | 部分実装 | core | hard | `GraphDslBuilder` はあるが、`~>` / `<~` を含む Pekko 相当 DSL までは未到達 |

実装済み:
- `Graph`, `RunnableGraph`, `Source`, `Flow`, `Sink`
- `BidiFlow`
- `SourceWithContext`, `FlowWithContext`
- `SubFlow` 相当の `FlowSubFlow` / `SourceSubFlow`
- `KeepLeft`, `KeepRight`, `KeepBoth`, `KeepNone`

### Source ファクトリ / 入口 API　✅ 実装済み 38/40 (95%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.actorRef` | `scaladsl/Source.scala:L678` | 未対応 | core | medium | actor モジュールとの配線が必要 |
| `Source.actorRefWithBackpressure` | `scaladsl/Source.scala:L715` | 未対応 | core | medium | backpressure 付きアクター source |

実装済み:
- `empty`, `single`, `from_option`, `from_array`, `from_iterator`, `repeat`, `cycle`, `range`
- `future`, `completion_stage`, `lazy_*`, `unfold*`, `tick`, `never`, `failed`, `maybe`
- `queue`, `queue_with_overflow`, `queue_with_overflow_and_max_concurrent_offers`, `queue_unbounded`
- `from_graph`, `from_materializer`, `from_publisher`
- `combine_mat`, `merge_prioritized_n`, `zip_n`, `zip_with_n`

### Sink ファクトリ / 入口 API　✅ 実装済み 34/34 (100%)

ギャップなし。

実装済み:
- `ignore`, `foreach`, `foreach_async`, `cancelled`, `none`, `never`, `on_complete`
- `head`, `head_option`, `last`, `last_option`, `take_last`, `collect`, `seq`, `count`
- `fold`, `fold_while`, `fold_async`, `reduce`
- `queue`, `from_graph`, `from_materializer`, `source`, `as_publisher`
- Actor 連携は `ActorSink::actor_ref` / `actor_ref_with_backpressure` で別名実装
- `combine`, `combine_mat`

### Flow ファクトリ / 構築 API　✅ 実装済み 10/11 (91%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `toProcessor` | `scaladsl/Flow.scala` | 未対応 | core | n/a | Reactive Streams `Processor` は JVM/RS 前提が強い |

実装済み:
- `new`, `from_function`, `from_graph`, `from_materializer`
- `from_sink_and_source`, `from_sink_and_source_mat`
- `from_sink_and_source_coupled`, `from_sink_and_source_coupled_mat`
- `lazy_flow`, `lazy_future_flow`, `optional_via`

### 合成オペレーター / *Mat バリアント　✅ 実装済み 24/24 (100%)

ギャップなし。

実装済み:
- `via_mat`, `to_mat`
- `zip_mat`, `zip_all_mat`, `zip_with_mat`, `zip_latest_mat`, `zip_latest_with_mat`
- `merge_latest_mat`, `merge_preferred_mat`, `merge_prioritized_mat`, `merge_sorted_mat`
- `concat_mat`, `concat_lazy_mat`, `prepend_mat`, `prepend_lazy_mat`, `or_else_mat`
- `also_to_mat`, `wire_tap_mat`, `divert_to_mat`
- `watch_termination_mat`, `monitor_mat`

### 変換 / フィルタ / バッチ / 時間系　✅ 実装済み 32/32 (100%)

ギャップなし。

代表例:
- `map`, `map_concat`, `map_option`, `map_async`, `map_async_partitioned`
- `filter`, `filter_not`, `take`, `drop`, `take_while`, `drop_while`
- `scan`, `fold`, `reduce`
- `grouped`, `grouped_within`, `grouped_weighted_within`, `sliding`
- `conflate`, `conflate_with_seed`, `batch`, `batch_weighted`
- `switch_map`, `expand`, `extrapolate`, `prefix_and_tail`
- `initial_timeout`, `completion_timeout`, `idle_timeout`, `backpressure_timeout`
- `keep_alive`, `delay`, `delay_with`, `debounce`, `sample`

### ファンイン / ファンアウト / Shape 群　✅ 実装済み 20/22 (91%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `AmorphousShape` | `stream/Shape.scala` 系 | 未対応 | core | medium | 汎用 shape 抽象 |
| `GraphDSL` 依存の高度な shape wiring パターン | `scaladsl/Graph.scala` | 部分実装 | core | hard | 基本 shape は揃ったが配線 DSL が弱い |

実装済み:
- `SourceShape`, `SinkShape`, `FlowShape`, `BidiShape`, `ClosedShape`, `StreamShape`
- `UniformFanInShape`, `UniformFanOutShape`
- `FanInShape2`〜`FanInShape22`, `FanOutShape2`
- `Broadcast`, `Partition`, `Balance`
- `MergeLatest`, `MergePreferred`, `MergePrioritized`, `MergeSorted`, `ZipLatest`

### ライフサイクル / 監視 / Hub / Queue　✅ 実装済み 13/13 (100%)

ギャップなし。

実装済み:
- `KillSwitches`, `UniqueKillSwitch`, `SharedKillSwitch`
- `RestartSource`, `RestartFlow`, `RestartSink`, `RestartSettings`
- `watch_termination`, `monitor_mat`, `FlowMonitor`
- `MergeHub`, `BroadcastHub`, `PartitionHub`
- `BoundedSourceQueue`, `SourceQueue`, `SourceQueueWithComplete`, `SinkQueue`

### フレーミング / 圧縮 / JSON　✅ 実装済み 6/6 (100%)

ギャップなし。

実装済み:
- `Framing.delimiter`
- `Framing.length_field`
- `Framing.simple_framing_protocol`
- `JsonFraming.object_scanner`
- `JsonFraming.array_scanner`
- `Compression` 相当

### std / IO / ランタイム統合　✅ 実装済み 2/5 (40%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `StreamConverters` | `scaladsl/StreamConverters.scala` | 未対応 | std | medium | Rust の `Read` / `Write` / iterator 橋渡し API |
| `Tcp` | `scaladsl/Tcp.scala` | 未対応 | std | hard | `tokio::net` か別 transport 層の設計が必要 |
| `TLS` | `stream/io/TLS` 系 | 未対応 | std | hard | `rustls` 等との統合が必要 |

実装済み:
- `FileIO::from_path`
- `FileIO::to_path`

### n/a（Rust / no_std / JVM 差分）　n/a 4 件

- `toProcessor`: Reactive Streams `Processor` 前提
- Java DSL 固有の `CompletionStage` / `function.Function` 群
- `StreamConverters` の一部 Java Stream / InputStream / OutputStream 直結 API
- JVM / Akka Runtime 前提の TCP/TLS 詳細 API

## 実装優先度の提案

### Phase 1: trivial
- `GraphDslBuilder` の現状整理と docs 明文化
- `watch_termination` / `monitor` まわりの API 名と docs の Pekko 対応表追加

### Phase 2: easy
- `Source.actorRef`
- `Source.actorRefWithBackpressure`
- `StreamConverters` の最小 subset（`Read` / `Write` ベース）

### Phase 3: medium
- `GraphDSL.Builder.add(...)` の型安全な取り込み拡張
- `~>` / `<~` 相当の Rust 風 wiring API
- `StreamConverters` の queue / channel / iterator 変換拡充

### Phase 4: hard
- Pekko 相当の GraphDSL 全面互換
- `Tcp` / `TLS` 層の標準実装
- ランタイム / scheduler / backpressure を跨ぐ高度な IO 連携

### 対象外（n/a）
- Java DSL 専用 API
- Reactive Streams `Processor` 直結
- JVM に強く依存する transport API

## まとめ

- 全体として、**Pekko Streams の日常利用で使う主要 DSL はかなり埋まっている**。
- 即効性が高いのは `Source.actorRef*` と `StreamConverters` の最小実装。
- 実用上の大きな差は **GraphDSL 完全互換** と **Tcp/TLS の標準提供**。
- 逆に、`Flow` / `Source` / `Sink` の基本オペレーター、*Mat バリアント、Json/Framing、KillSwitch、Hub、Queue は十分強い。

# stream モジュール ギャップ分析

更新日: 2026-03-25

対象:
- fraktor-rs: `modules/stream/src/`
- Pekko 参照: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/`
- Pekko stream-typed: `references/pekko/stream-typed/src/main/scala/org/apache/pekko/stream/typed/scaladsl/`

計数ポリシー:
- Pekko 公開型数は `scaladsl/*`, `stage/*`, `snapshot/*`, `stream-typed/scaladsl/*` の top-level public 型を機械抽出した値を使う
- fraktor-rs 公開型数は `modules/stream/src/core` と `modules/stream/src/std` の `pub struct|trait|enum|type` の機械抽出値を使う
- ただし fraktor-rs は `1 file = 1 type` のため型数が細かく割れる。実用カバレッジは「API ファミリ正規化」で評価する
- `javadsl/*`, Java 相互運用専用 overload, JVM 固有 API は母集団から除外する

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（機械抽出） | 179 |
| fraktor-rs 公開型数（機械抽出） | 247（core: 243, std: 4） |
| カバレッジ（API ファミリ正規化） | 85/104 (82%) |
| ギャップ数 | 19（core: 16, std: 3） |

一言評価:
- `Source` / `Flow` / `Sink`、shape 群、GraphDSL 基礎、KillSwitch、Restart、Hub、`FileIO`、`StreamConverters`、`TopicPubSub` まで揃っており、表面 API はかなり厚い
- 古い分析で未対応だった `r#async()`、multi-island materialization、`SystemMaterializer`、`TopicPubSub`、`BidiFlow::join_mat` はすでに実装済み
- 現在の主ギャップは「low-level stage authoring parity」「Reactive Streams bridge の placeholder」「一部 operator の契約差分」「Tcp/TLS/StreamRefs 不在」に集約される

解消済みの重要差分:
- `Graph.async` 相当: `Flow::r#async` は `modules/stream/src/core/stage/flow.rs:680`、`Source::r#async` は `modules/stream/src/core/stage/source.rs:1384`
- multi-island 実行: `ActorMaterializer::materialize` は island 分割を持つ (`modules/stream/src/core/mat/actor_materializer.rs:162`)
- `SystemMaterializer`: `modules/stream/src/std/system_materializer.rs:17`
- `PubSub`: `TopicPubSub::source` / `sink` (`modules/stream/src/core/stage/topic_pub_sub.rs:136`, `modules/stream/src/core/stage/topic_pub_sub.rs:169`)

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 79 | 64 | 81% |
| core / typed ラッパー | 0 | 0 | n/a |
| std / アダプタ | 25 | 21 | 84% |

補足:
- `modules/stream` に `core/typed` サブ層は存在しない
- `stream-typed` 相当は `TopicPubSub` など actor typed 連携で `core/` に直接載っている
- `std/` は `FileIO`, `StreamConverters`, `SystemMaterializer` まではあるが、transport / remote stream 系は未着手

## カテゴリ別ギャップ

### コア DSL / 型 ✅ 実装済み 12/15 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FlowWithContext` / `SourceWithContext` の完全 parity | `FlowWithContext.scala:64`, `FlowWithContext.scala:130`, `SourceWithContext.scala:53`, `SourceWithContext.scala:117` | 部分実装 | core | medium | `map`, `filter`, `via`, `also_to_context`, `wire_tap_context` はあるが `unsafeDataVia` / `unsafeOptionalDataVia` 系がない |
| `BidiFlow.atopMat` | `BidiFlow.scala:71` | 未対応 | core | easy | fraktor-rs は `atop`, `join`, `join_mat` まで (`modules/stream/src/core/stage/bidi_flow.rs:89`) |
| `BidiFlow.mapMaterializedValue` | `BidiFlow.scala:181` | 未対応 | core | easy | `Source` / `Flow` / `Sink` にはあるが `BidiFlow` にはない |

### オペレーター / 変換 DSL ✅ 実装済み 26/31 (84%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Flow.contramap` | `Flow.scala:204` | 部分実装 | core | easy | `modules/stream/src/core/stage/flow.rs:1610` は入力変換をせず `self` を返す |
| `Flow.doOnCancel` | `Flow.scala:1623` | 部分実装 | core | medium | `modules/stream/src/core/stage/flow.rs:1637` は placeholder |
| `groupedAdjacentBy` / `groupedAdjacentByWeighted` | `Flow.scala:1866`, `Flow.scala:1887` | 部分実装 | core | medium | `modules/stream/src/core/stage/flow.rs:1739`, `modules/stream/src/core/stage/flow.rs:1751` は key/weight を使わず `grouped(size)` に寄せている |
| `keepAlive` | `Flow.scala:3080` | 部分実装 | core | medium | `modules/stream/src/core/stage/flow.rs:2454` は idle 監視ではなく `intersperse` ベース |
| `MergeSequence` / `concatAllLazy` | `Graph.scala:1487`, `Flow.scala:3808` | 部分実装 | core | medium | `modules/stream/src/core/stage/flow.rs:2466` は `merge` 委譲、`modules/stream/src/core/stage/flow.rs:2475` は `concat` 委譲 |

### SubFlow ✅ 実装済み 5/6 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GroupBySubFlow` の merge/concat parity | `SubFlow.scala:51`, `SubFlow.scala:61`, `SubFlow.scala:72` | 部分実装 | core | easy | `FlowGroupBySubFlow` / `SourceGroupBySubFlow` は `merge_substreams` のみ。`merge_substreams_with_parallelism` と `concat_substreams` がない (`modules/stream/src/core/stage/flow_group_by_sub_flow.rs:20`, `modules/stream/src/core/stage/source_group_by_sub_flow.rs:21`) |

### マテリアライゼーション / Reactive Streams bridge ✅ 実装済み 9/12 (75%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `MaterializerState.streamSnapshots` | `MaterializerState.scala:45`, `MaterializerState.scala:57` | 部分実装 | core / std | medium | fraktor-rs は `ActorMaterializer::snapshot()` と `MaterializerSnapshot` まで (`modules/stream/src/core/mat/actor_materializer.rs:117`)。running stream / interpreter dump はない |
| `Source.asSubscriber` / `Sink.fromSubscriber` / `Sink.asPublisher` | `Source.scala:637`, `Sink.scala:182`, `Sink.scala:310` | 部分実装 | core | medium | `modules/stream/src/core/stage/source.rs:178` は `Sink::ignore()`、`modules/stream/src/core/stage/sink.rs:136` / `199` は placeholder |
| `Sink.source` / `Sink.fromMaterializer` / `Sink.futureSink` | `Sink.scala:170`, `Sink.scala:326`, `Sink.scala:733` | 部分実装 | core | medium | `modules/stream/src/core/stage/sink.rs:128`, `191`, `141` は `ignore()` / `Source::empty()` ベースの placeholder |

### Graph / Shape / Stage API ✅ 実装済み 12/16 (75%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SubSinkInlet` / `SubSourceOutlet` | `GraphStage.scala:1451`, `GraphStage.scala:1532` | 未対応 | core | hard | substream を low-level stage authoring で組み立てる API がない |
| `GraphStageLogicWithLogging` / `TimerGraphStageLogicWithLogging` | `GraphStage.scala:1855`, `GraphStage.scala:1858` | 未対応 | core | easy | `TimerGraphStageLogic` はあるが logging mixin 相当がない |
| `AmorphousShape` | `Shape.scala:285` | 未対応 | core | medium | fraktor-rs は固定 shape 群中心 (`modules/stream/src/core/shape.rs:1`) |
| GraphDSL の記法互換 (`~>`, `<~`, `<~>`) | `Graph.scala` | 部分実装 | core | medium | `GraphDslBuilder::connect` はあり `~>` 相当コメントもある (`modules/stream/src/core/graph/graph_dsl_builder.rs:237`) が、Pekko の演算子 DSL そのものはない |

### Queue / Hub / Actor 連携 ✅ 実装済み 13/13 (100%)

ギャップなし。

実装済み代表:
- `ActorSource::actor_ref`, `ActorSource::actor_ref_with_backpressure`
- `ActorSink::actor_ref`, `actor_ref_with_result`, `actor_ref_with_backpressure`
- `TopicPubSub::source`, `TopicPubSub::sink`
- `MergeHub`, `BroadcastHub`, `PartitionHub`
- `ActorSourceRef`, `BoundedSourceQueue`, `SourceQueueWithComplete`, `SinkQueue`

### std / IO / transport ✅ 実装済み 5/8 (63%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `StreamRefs` (`sourceRef`, `sinkRef`) | `StreamRefs.scala:24` | 未対応 | std | hard | `ActorSourceRef` は queue handle であり remote stream ref ではない (`modules/stream/src/core/actor_source_ref.rs:13`) |
| `Tcp` | `Tcp.scala:47` | 未対応 | std | hard | `tokio::net` と materialization / backpressure 契約の統合が必要 |
| `TLS` | `TLS.scala:62` | 未対応 | std | hard | `rustls` 等との adapter 設計が必要 |

### テスティング ✅ 実装済み 3/3 (100%)

ギャップなし。

実装済み:
- `TestSourceProbe`
- `TestSinkProbe`
- `StreamFuzzRunner`

## 実装優先度の提案

### Phase 1: trivial / easy

- `BidiFlow.atop_mat` と `BidiFlow.map_materialized_value`
  実装先層: core
- `FlowGroupBySubFlow` / `SourceGroupBySubFlow` に `merge_substreams_with_parallelism` と `concat_substreams` を追加
  実装先層: core
- `Flow.contramap` を no-op ではなく `Flow::from_function(...).via_mat(..., KeepRight)` 相当に置き換える
  実装先層: core
- GraphDSL の operator sugar を `PortOps` / `ReversePortOps` の薄い拡張で補う
  実装先層: core

### Phase 2: medium

- `groupedAdjacentBy` / `groupedAdjacentByWeighted` を key 境界と重み計算つきにする
  実装先層: core
- `keep_alive` を idle timeout 起点の注入 stage に置き換える
  実装先層: core
- `MergeSequence` と `concatAllLazy` を alias ではなく専用ロジックにする
  実装先層: core
- Reactive Streams bridge (`Source.asSubscriber`, `Sink.fromSubscriber`, `Sink.asPublisher`) を placeholder から実装へ置き換える
  実装先層: core / std
- `MaterializerState.streamSnapshots` 相当の実行中 graph 可視化 API を追加する
  実装先層: core / std
- `FlowWithContext` / `SourceWithContext` の `unsafeDataVia` 系を追加する
  実装先層: core

### Phase 3: hard

- `SubSinkInlet` / `SubSourceOutlet` を含む low-level GraphStage authoring parity
  実装先層: core
- `StreamRefs`
  実装先層: std
- `Tcp`
  実装先層: std
- `TLS`
  実装先層: std

### 対象外（n/a）

- `javadsl/*`
- `CompletionStage` 専用 overload
- `JavaFlowSupport`
- その他 JVM 固有の Java 相互運用 API

## まとめ

- 全体として、`stream` は「DSL を触る」段階までは十分に強い。`async()`、multi-island materialization、`SystemMaterializer`、`TopicPubSub` が入ったことで、古いレポートより明確に前進している
- いま不足しているのは、表面 API 数よりも「互換シムを本物にする」段階である。特に `Flow.contramap`、`groupedAdjacentBy*`、Reactive Streams bridge は存在していても契約が Pekko と一致していない
- すぐ価値が高いのは Phase 1 の 4 件。いずれも公開面の整合性を上げつつ、アーキテクチャ破壊を伴わない
- 実用上の大物ギャップは Phase 3 の 4 件。`SubSinkInlet` / `SubSourceOutlet`、`StreamRefs`、`Tcp`、`TLS` が埋まると、Pekko Streams 互換の下限がもう一段上がる

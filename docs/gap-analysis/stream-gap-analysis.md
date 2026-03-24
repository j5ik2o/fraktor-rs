# stream モジュール ギャップ分析

生成日: 2026-03-24

対象:

- fraktor-rs: `modules/stream/src/`
- Pekko 参照: `references/pekko/stream/src/main/scala/org/apache/pekko/stream/`

比較スコープ:

- Pekko 側は `org/apache/pekko/stream/*.scala` と `scaladsl/*.scala` のトップレベル公開 API を対象とする
- `javadsl`、`impl`、`stage`、`snapshot` は母集団から除外する
- `stream` モジュールには `core/typed` が存在しないため、typed ラッパー層は `n/a` とする
- サマリーの数値は **型単位**、カテゴリ別ギャップは **型 + 主要 DSL メソッド単位** で数える

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 57 |
| fraktor-rs 公開型数 | 48（core: 46, std: 2） |
| カバレッジ（型単位） | 48/57 (84%) |
| ギャップ項目数 | 26（core: 22, std: 4） |

短評:

- `Source` / `Flow` / `Sink`、shape 群、KillSwitch、Hub、Queue、Framing、Compression など、主要な骨格はすでに揃っている
- 主要ギャップは「型そのものの不在」よりも、**Pekko と同名だが契約が一致していない DSL**、および **GraphDSL / async() / remote stream refs / preMaterialize 系** に集中している
- `std` 層は `FileIO` と `StreamConverters` はあるが、`StreamRefs` 系とネットワーク越し streaming は未着手

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 51 | 46 | 90% |
| core / typed ラッパー | 0 | 0 | n/a |
| std / アダプタ | 6 | 2 | 33% |

補足:

- `modules/stream/src/core/typed/` は存在しない
- `std` 側で直接対応しているのは `FileIO` と `StreamConverters` のみ
- `StreamRefs` / `SourceRef` / `SinkRef` / `StreamRefSettings` は未実装のため、Pekko の remote streaming 系は未カバー

## カテゴリ別ギャップ

### DSL コア型 ✅ 実装済み 9/11 (82%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Graph[S, M]` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/Graph.scala:28` | 未対応 | core | medium | `modules/stream/src/core/graph.rs` は `StreamGraph` と `GraphStage` を公開しているが、`Graph<S, M>` に相当する共通抽象はない |
| `SubFlow[Out, Mat, F, C]` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/SubFlow.scala:30` | 部分実装 | core | medium | `modules/stream/src/core/stage/source_sub_flow.rs:9` と `modules/stream/src/core/stage/flow_sub_flow.rs:9` に分割されており、Pekko の単一 `SubFlow` 契約ではない |

### Shape / lifecycle 型 ✅ 実装済み 13/15 (87%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FanInShape` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/FanInShape.scala:35` | 未対応 | core | medium | fraktor は `modules/stream/src/core/shape/uniform_fan_in_shape.rs:10` と `modules/stream/src/core/shape/fan_in_shape2.rs:12` などの具体 shape はあるが、抽象基底はない |
| `FanOutShape` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/FanOutShape.scala:35` | 未対応 | core | medium | fraktor は `modules/stream/src/core/shape/uniform_fan_out_shape.rs:10` と `modules/stream/src/core/shape/fan_out_shape2.rs:15` を持つが、抽象基底はない |

### Queue / remote 型 ✅ 実装済み 7/12 (58%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SinkQueueWithCancel[T]` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Queue.scala:125` | 部分実装 | core | easy | `modules/stream/src/core/sink_queue.rs:15` は `cancel()` を持つが、`pull(): Future[Option[T]]` ではなく同期 `Option<T>` で契約が異なる |
| `StreamRefs` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/StreamRefs.scala:133` | 未対応 | std | hard | remote streaming 用の resolver / serialization surface がない |
| `SourceRef[T]` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/StreamRefs.scala:89` | 未対応 | std | hard | `ActorSourceRef` はローカル queue handle であり、network boundary を越える `SourceRef` ではない |
| `SinkRef[In]` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/StreamRefs.scala:55` | 未対応 | std | hard | dual 側の remote sink reference がない |
| `StreamRefSettings` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/StreamRefSettings.scala:49` | 未対応 | std | medium | buffer / redelivery / subscription timeout などの設定面が未実装 |

### 変換 / 集約オペレーター ✅ 実装済み 10/12 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `groupedAdjacentBy` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:1866` | 部分実装 | core | trivial | `modules/stream/src/core/stage/flow.rs:1600` は `grouped(size)` に委譲しており、adjacent key 境界を見ていない |
| `groupedAdjacentByWeighted` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:1887` | 部分実装 | core | trivial | `modules/stream/src/core/stage/flow.rs:1612` は key と weight の両方を無視して `grouped(size)` に委譲している |

### 結合 / 分岐オペレーター ✅ 実装済み 10/12 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `mergeSequence` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Graph.scala:1487` | 部分実装 | core | medium | `modules/stream/src/core/stage/flow.rs:2326` は `merge(fan_in)` に委譲しており、sequence 番号による整列 merge ではない |
| `alsoToAll` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:3996` | 部分実装 | core | trivial | `modules/stream/src/core/stage/flow.rs:3023` は sink 数を数えて `self` を返すだけで、fan-out を作らない |

### マテリアライズ / 監視 ✅ 実装済み 3/8 (38%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Source.preMaterialize` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala:107` | 部分実装 | core | medium | `modules/stream/src/core/stage/source.rs:2643` は `(Self, mat.clone())` を返すだけで、eager materialization ではない |
| `Flow.preMaterialize` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:169` | 未対応 | core | medium | `Flow` には対応 API が存在しない |
| `Sink.preMaterialize` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Sink.scala:77` | 部分実装 | core | medium | `modules/stream/src/core/stage/sink.rs:187` は `(self, StreamCompletion::new())` を返すだけで、実行済み sink placeholder ではない |
| `Source.materializeIntoSource` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Source.scala:122` | 未対応 | core | medium | `Flow.materialize_into_source` はあるが `Source` 側の対応がない |
| `watchTermination` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala:4536` | 部分実装 | core | medium | `modules/stream/src/core/stage/flow.rs:3125` と `modules/stream/src/core/stage/source.rs:735` は `watch_termination_mat` のみで、`Future[Done]` を渡す Pekko 契約とは異なる |

### 非同期境界 / 実行モデル ✅ 実装済み 1/5 (20%)

実装済み代表:

- `modules/stream/src/core/stage/flow.rs:568` の `Flow::async_boundary`
- `modules/stream/src/core/stage/source.rs:1315` の `Source::async_boundary`
- `modules/stream/src/core/stage/flow/logic/async_boundary_logic.rs:5` のとおり、これは単一 interpreter 内の buffer / backpressure 境界である

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Graph.async` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/Graph.scala:54` | 未対応 | core | medium | 公開 API としての `async()` はない。現状は `async_boundary()` のみ |
| `Graph.async(dispatcher)` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/Graph.scala:61` | 未対応 | std | hard | dispatcher / 実行コンテキスト指定機構がない |
| `Graph.async(dispatcher, inputBufferSize)` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/Graph.scala:70` | 未対応 | std | hard | `Attributes::input_buffer` はあるが、dispatcher 分離つき async island には使われていない |
| 非同期 island ごとの独立実行 | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/Graph.scala:54` | 部分実装 | core | hard | `modules/stream/src/core/mat/actor_materializer.rs:115` は materializer ごとに 1 個の `StreamDriveActor`、`modules/stream/src/core/lifecycle/stream.rs:11` は graph ごとに 1 個の `GraphInterpreter` であり、`async_boundary` を挟んでも境界ごとに別 actor / 別 interpreter へ分離されない |

### Context wrappers ✅ 実装済み 6/10 (60%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FlowWithContext.mapMaterializedValue` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/FlowWithContext.scala:174` | 未対応 | core | trivial | `modules/stream/src/core/stage/flow_with_context.rs` には surface がない |
| `SourceWithContext.mapMaterializedValue` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/SourceWithContext.scala:149` | 未対応 | core | trivial | `modules/stream/src/core/stage/source_with_context.rs` に未実装 |
| `SourceWithContext.toMat` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/SourceWithContext.scala:163` | 未対応 | core | easy | `SourceWithContext` は `as_source` まではあるが、専用 `to_mat` surface がない |
| `SourceWithContext.runWith` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/SourceWithContext.scala:185` | 未対応 | core | easy | `SourceWithContext` の terminal 実行 API がない |

### GraphDSL / hub / std adapters ✅ 実装済み 8/12 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GraphDSL.Builder.materializedValue` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Graph.scala:1681` | 未対応 | core | easy | `modules/stream/src/core/graph/graph_dsl_builder.rs` は `add_*` / `connect` は持つが materialized value outlet は持たない |
| Port combinators `~>`, `<~`, `<~>` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Graph.scala:1827` | 未対応 | core | medium | fraktor は `modules/stream/src/core/graph/graph_dsl_builder.rs:242` の `connect` / `connect_via` で代替しており、DSL 記法はない |
| `MergeHub.sourceWithDraining` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Hub.scala:95` | 未対応 | core | easy | fraktor は `modules/stream/src/core/hub/merge_hub.rs:85` の `draining_control()` と `modules/stream/src/core/hub/merge_hub.rs:98` の `source()` が分離している |
| `PartitionHub.statefulSink` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Hub.scala:942` | 未対応 | core | medium | `modules/stream/src/core/hub/partition_hub.rs:148` は `sink_for` のみで、consumer state を持つ routing sink factory はない |

## 実装優先度の提案

### Phase 1: trivial（既存実装の置き換え・薄い surface 追加）

- `groupedAdjacentBy` を key 境界付き実装に置き換える（実装先: `core`）
- `groupedAdjacentByWeighted` を key + weight 対応にする（実装先: `core`）
- `alsoToAll` の no-op を fan-out 実装へ置き換える（実装先: `core`）
- `FlowWithContext.mapMaterializedValue` を追加する（実装先: `core`）
- `SourceWithContext.mapMaterializedValue` を追加する（実装先: `core`）

### Phase 2: easy（API surface の補完）

- `SourceWithContext.toMat` / `runWith` を追加する（実装先: `core`）
- `GraphDslBuilder.materializedValue` を追加する（実装先: `core`）
- `MergeHub.sourceWithDraining` 相当を追加する（実装先: `core`）
- `SinkQueue` を `SinkQueueWithCancel` に寄せるか、命名を契約に合わせて整理する（実装先: `core`）
- `async_boundary()` を Pekko の `async()` と混同しないよう、公開ドキュメントと命名方針を明確化する（実装先: `core`）

### Phase 3: medium（セマンティクス差分の解消）

- `Source.pre_materialize` / `Sink.pre_materialize` を eager materialization に寄せる（実装先: `core`）
- `Flow.pre_materialize` を追加する（実装先: `core`）
- `Source.materializeIntoSource` を追加する（実装先: `core`）
- `watchTermination` を `Future[Done]` 相当の契約で整理する（実装先: `core`）
- `mergeSequence` を sequence-aware merge にする（実装先: `core`）
- GraphDSL port combinator を導入するか、現行 `connect` ベースを正規 API として割り切るか判断する（実装先: `core`）
- `async()` 導入に向けて、1 materialized graph を複数 async island に分割できる内部構造へ拡張する（実装先: `core`）

### Phase 4: hard（アーキテクチャ拡張）

- `Graph[S, M]` の共通抽象を導入する（実装先: `core`）
- `SubFlow` を unified abstraction として再設計する（実装先: `core`）
- `StreamRefs` / `SourceRef` / `SinkRef` / `StreamRefSettings` を追加する（実装先: `std`）
- network boundary をまたぐ back-pressured streaming の serialization / lifecycle 契約を整える（実装先: `std`）
- async island ごとの dispatcher / drive actor 割当を `ActorMaterializer` に導入し、そこで初めて公開 `async()` を出す（実装先: `std`）

### 対象外（n/a）

- `javadsl` の重複 surface
- `impl` / `stage` 配下の内部実装詳細
- JVM 固有の `Config` / Java `CompletionStage` そのものへの 1:1 追従

## まとめ

- 全体として、`stream` モジュールは **Pekko Streams の主要 DSL をかなり広くカバーしているが、GraphDSL の表現力、`async()` の実行モデル、preMaterialize/watchTermination 契約、remote stream refs が薄い**。
- 即座に価値を出せるのは `groupedAdjacentBy*`、`alsoToAll`、`*WithContext.mapMaterializedValue` のような **既存実装の差し替えで済むギャップ**。
- 実用上の主要ギャップは、`Graph` / `SubFlow` の抽象、`async()` を成立させる island 分割 + 実行主体分離、`preMaterialize` 系の本当の eager materialization、そして `StreamRefs` 系の未実装である。

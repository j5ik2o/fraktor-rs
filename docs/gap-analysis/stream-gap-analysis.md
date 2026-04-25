# stream モジュール ギャップ分析

更新日: 2026-04-25 (13th edition / GraphInterpreter failure-restart 分割反映)

## 比較スコープ定義

この調査は、Apache Pekko の `stream` / `stream-typed` をそのまま全量移植する調査ではなく、fraktor-rs の Rust API として再現対象にする範囲を固定して比較する。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| Stream core DSL | `modules/stream-core/src/core/dsl/` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/` |
| Stage / graph / materialization | `modules/stream-core/src/core/stage/`, `modules/stream-core/src/core/impl/`, `modules/stream-core/src/core/materializer/` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/`, `references/pekko/stream/src/main/scala/org/apache/pekko/stream/stage/` |
| Attributes / Shape / Snapshot | `modules/stream-core/src/core/attribute/`, `modules/stream-core/src/core/shape/`, `modules/stream-core/src/core/snapshot/` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/` |
| Stream typed actor interop | `modules/stream-core/src/core/dsl/actor_*.rs`, `modules/stream-core/src/core/dsl/topic_pub_sub.rs` | `references/pekko/stream-typed/src/main/scala/org/apache/pekko/stream/typed/scaladsl/` |
| std adaptor / IO | `modules/stream-adaptor-std/src/std/` | Pekko の FileIO / StreamConverters / Tcp / TLS 相当 |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| `javadsl` API | Java 固有の overload / builder 体系であり、Rust API の再現対象ではない |
| Scala implicit / symbolic syntax / `~>` そのもの | Scala 言語機能であり、Rust では明示的 builder API に置換する |
| Reactive Streams TCK / stream-testkit / テスト専用 API | 利用者向け runtime API ではない |
| HOCON / JVM dispatcher / MaterializerConfigurator | JVM/Typesafe Config 固有 |
| Java `Publisher` / `Subscriber` / `CompletionStage` ブリッジ | JVM interop 固有。Rust では `Stream` / `Sink` / async primitive で代替する |
| deprecated API | 互換性維持目的の API は正式リリース前の fraktor-rs では追わない |

### 判定方針

`references/pekko` から機械的に抽出した public 型・メソッド数は、比較の参考情報に留める。Scala/Java/JVM 固有 API を分母に入れると Rust 側では再現不能なギャップが混ざるため、このレポートでは上記固定スコープのみをギャップ判定の対象にする。

API 面の残ギャップが小さいため、後半で内部モジュール構造のギャップも分析する。

## エグゼクティブサマリー

stream の固定スコープにおける API カバレッジは高い。過去レポートで中程度以上のギャップとして挙がっていた `conflate`、`expand`、`intersperse`、`orElse`、timeout 系、`mapAsyncPartitioned`、`RetryFlow.withBackoffAndContext`、`MergeSequence`、`ZipLatest`、`GraphStageWithMaterializedValue` 相当は現在の実装で確認できる。

2026-04-24 の core DSL fake-impl 排除バッチで、`Flow::contramap` / `Flow::dimap` / `Flow::do_on_cancel`、`Sink::from_materializer`、`Sink::source` は no-op / placeholder ではなく実データパスを持つ実装になった。JVM 固有の `CompletionStage` / `Publisher` / `Subscriber` 互換 alias は固定スコープ外として削除した。

2026-04-25 の GraphInterpreter 構造分割バッチで、plan compile、edge buffer dispatch、snapshot 生成、failure disposition は `GraphInterpreter` 本体から internal module へ切り出された。これにより Pekko の interpreter / connection / snapshot 責務分離意図を、fraktor-rs の tick-based core に合わせて再表現した。

2026-04-25 の GraphInterpreter failure/restart 分割バッチで、restart window tick、restart waiting 判定、failure action 適用、downstream failure propagation、source / flow / sink failure handler は `graph_interpreter/failure_restart.rs` へ切り出された。これにより drive loop 本体と failure / restart policy の変更理由を分離した。

2026-04-25 の DefaultOperatorCatalog 構造分割バッチで、operator contract / coverage は `default_operator_catalog_source` / `default_operator_catalog_transform` / `default_operator_catalog_substream` / `default_operator_catalog_timing` / `default_operator_catalog_fan_in` / `default_operator_catalog_fan_out` / `default_operator_catalog_failure` / `default_operator_catalog_hub` / `default_operator_catalog_kill_switch` のカテゴリ別 internal module へ切り出された。これにより operator 追加時の競合とレビュー負荷を下げつつ、既存の `OperatorCatalog` public 境界は維持した。

残っている主要ギャップは、単純な operator 追加ではなく、remote StreamRef、Tcp/TLS などの integration-level 機能に集中している。

| 指標 | 現在値 |
|------|--------|
| fraktor-rs public type 数 | 209 (core 202 / std 7) |
| fraktor-rs public method 数 | 1470 (shape / DSL boilerplate 含む) |
| 固定スコープ API カバレッジ推定 | 約 98% |
| Hard gap | 3 |
| Medium gap | 0 |
| Easy gap | 0 |
| Trivial gap | 1 |

## レイヤー別カバレッジ

| レイヤー | 判定 | 根拠 |
|----------|------|------|
| Stream core DSL | 約 97% | Source / Flow / Sink / RunnableGraph の主要 operator はほぼ実装済み |
| Graph / materialization | 約 96% | Graph stage / materialized value / public GraphDSL facade は実装済み。GraphInterpreter 内部も plan / connection / snapshot / failure-restart support へ分割済み |
| Stream typed interop | 約 93% | ActorSource / ActorSink / ask / PubSub は実装済み。`ActorSink::actor_ref_with_backpressure_any_ack` も追加済み |
| std adaptor / IO | 約 75-80% | FileIO / StreamConverters 系はあるが Tcp / TLS が未実装 |
| StreamRef / distributed stream | 部分対応 | local handoff の `StreamRefs` / `SourceRef` / `SinkRef` / protocol / settings は実装済み。remote resolver / serializer は未実装 |

## API ギャップ一覧

### Hard

| ギャップ | Pekko 側 | fraktor-rs 現状 | 対応方針 |
|----------|----------|-----------------|----------|
| StreamRef remote integration | `StreamRefResolver`, remote StreamRef protocol transport, serializer | local handoff の `StreamRefs` / `SourceRef` / `SinkRef` / protocol は実装済み。remote resolver / serializer は未実装 | resolver / serializer / remote transport 連携を local handoff とは分けて実装する |
| Tcp stream | `Tcp`, `Tcp.IncomingConnection`, `Tcp.OutgoingConnection`, TCP command/event | std adaptor に TCP 接続 API がない | `stream-adaptor-std/src/std/io/tcp/` を新設し、tokio TCP を Stream DSL に接続する |
| TLS stream | `TLS`, `SslTlsOptions`, TLS session handling | TLS API がない | TCP と独立した `std/io/tls/` と options 型を追加する |

### Medium

| ギャップ | Pekko 側 | fraktor-rs 現状 | 対応方針 |
|----------|----------|-----------------|----------|
| 該当なし | - | Public GraphDSL facade、SubSinkInlet / SubSourceOutlet、StreamRef attributes / exceptions は実装済み | 新規 Medium gap は現時点でなし |

### Easy

| ギャップ | Pekko 側 | fraktor-rs 現状 | 対応方針 |
|----------|----------|-----------------|----------|
| 該当なし | - | typed ActorSink no-ack 相当、`Flow::watch_termination` convenience、system-level stream snapshot helper は実装済み | 新規 Easy gap は現時点でなし |

### Trivial

| ギャップ | Pekko 側 | fraktor-rs 現状 | 対応方針 |
|----------|----------|-----------------|----------|
| Tcp error aliases | `StreamTcpException` | TCP stream API が未実装のため専用 error 型も未実装。StreamRef exception 相当は `StreamError` variants として実装済み | TCP stream API 実装時に Rust の error enum / marker 型として追加する |

## 実装済みと判定した主な過去ギャップ

| 項目 | 判定 | fraktor-rs 根拠 |
|------|------|-----------------|
| `conflate` / `conflateWithSeed` | 実装済み | `Flow::conflate`, `Flow::conflate_with_seed`, `Source` 側対応あり |
| `expand` / `extrapolate` | 実装済み | `Flow::expand`, `Flow::extrapolate`, `Source` 側対応あり |
| `intersperse` | 実装済み | `Flow::intersperse`, `Source::intersperse` |
| `orElse` | 実装済み | `Flow::or_else`, `Source::or_else` |
| timeout 系 | 実装済み | `initial_timeout`, `completion_timeout`, `idle_timeout`, `backpressure_timeout` |
| `mapAsyncPartitioned` | 実装済み | `Flow::map_async_partitioned` |
| `RetryFlow.withBackoffAndContext` | 実装済み | `RetryFlow::with_backoff_and_context` |
| `MergeSequence` | 実装済み | `Flow::merge_sequence` |
| `ZipLatest` | 実装済み | `zip_latest`, `zip_latest_with`, `zip_latest_mat`, `zip_latest_with_mat` |
| `GraphStageWithMaterializedValue` 相当 | 実装済み | `GraphStage<In, Out, Mat>` と `GraphStageLogic<In, Out, Mat>` が `Mat` を型パラメータ化 |
| `Flow.contramap` / `Flow.dimap` | 実装済み | `Flow::contramap`, `Flow::dimap` が入力側 map を元 Flow の前段に合成し、元 Flow の materialized value を保持 |
| `Flow.doOnCancel` | 実装済み | `DoOnCancelLogic` が downstream cancel で callback を一度実行 |
| `Sink.fromMaterializer` | 実装済み | `Sink::from_materializer(factory)` が stream 開始時に factory Sink を生成して実 Sink として処理 |
| `Sink.source` | 実装済み | `Sink::source` が queue backed live `Source` を materialize し、入力要素を Source 側へ流す |
| StreamRef local handoff | 実装済み | `StreamRefs::{source_ref,sink_ref}`, `SourceRef`, `SinkRef`, local protocol / handoff / settings |
| Public GraphDSL facade | 実装済み | `core/dsl/graph_dsl.rs`, `core/dsl/graph_dsl_builder.rs`, `tests/graph_dsl_public.rs` |
| StageActorRef 相当 | 実装済み | `StageActor`, `StageActorEnvelope`, `StageActorReceive`, `StageContext::get_stage_actor`, `tests/stage_actor_public.rs` |
| SubSinkInlet / SubSourceOutlet | 実装済み | `SubSinkInlet`, `SubSourceOutlet`, handler trait 群、`tests/substream_ports_public.rs` |
| StreamRef attributes / exceptions | 実装済み | `Attributes::stream_ref_*`, `StreamError::{StreamRefSubscriptionTimeout, RemoteStreamRefActorTerminated, InvalidSequenceNumber, InvalidPartnerActor}` |
| typed ActorSink backpressure no-ack 相当 | 実装済み | `ActorSink::actor_ref_with_backpressure_any_ack` |
| `Flow::watch_termination` convenience | 実装済み | `Flow::watch_termination`, `Source::watch_termination` |
| system-level stream snapshot helper | 実装済み | `SystemMaterializer::stream_snapshots` |
| GraphInterpreter support 分割 | 実装済み | `CompiledGraphPlan`, `BufferedEdge`, `GraphConnections`, `InterpreterSnapshotBuilder`, `FailureDisposition`, `graph_interpreter/failure_restart.rs` |
| DefaultOperatorCatalog カテゴリ分割 | 実装済み | `default_operator_catalog_{source,transform,substream,timing,fan_in,fan_out,failure,hub,kill_switch}.rs` |

## JVM / Scala 固有として除外した項目

次の項目は raw extraction では API ギャップに見えるが、固定スコープでは parity 分母に入れない。

| 項目 | 判定理由 |
|------|----------|
| Java DSL overload 群 | Rust では overload が存在しないため、同一シグネチャ数の再現はしない |
| `~>` / implicit conversion | Scala DSL 構文。Rust では builder method に置換する |
| `Publisher` / `Subscriber` bridge | JVM Reactive Streams interop。Rust では async `Stream` / `Sink` が対応境界 |
| HOCON based settings factory | JVM 設定基盤依存 |
| TestKit / TCK | runtime API ではなく検証資材 |

## 内部モジュール構造ギャップ

API カバレッジが高いため、残りは内部構造の差分が重要になる。

| 構造ギャップ | 現状 | リスク | 推奨対応 |
|--------------|------|--------|----------|
| GraphDSL の public / internal 境界 | public wrapper は `core/dsl/graph_dsl*.rs`、内部 wiring は `core/impl/graph_dsl_builder.rs` に分離済み | 今後 builder API を拡張すると public wrapper と内部 graph 変換の責務が混ざるリスクがある | public wrapper は DSL 操作だけを公開し、接続検証と実行 graph 変換は `impl` に残す |
| StreamRef remote package 境界 | local handoff は `core/stream_ref/` と `impl/streamref/{protocol,handoff,source_logic,sink_logic}` に分離済み | remote resolver / serializer を追加すると local handoff と remote transport が混ざるリスクがある | remote 側は `resolver` / `serialization` / `transport adapter` を local handoff から分けて追加する |
| std IO adaptor に tcp/tls 境界がない | FileIO / StreamConverters と同じ層に TCP/TLS が未配置 | IO 機能追加時に std adaptor の責務が曖昧になる | `std/io/tcp/` と `std/io/tls/` を明示的に分離する |
| `graph_interpreter.rs` が大きい | plan compile、edge buffer dispatch、snapshot support、failure disposition、failure / restart policy は internal module へ分割済み。drive state machine 本体はまだ集中している | scheduling / demand の変更影響は引き続き広い | drive state machine は demand / scheduling を壊さない単位で別バッチに分割する |
| `default_operator_catalog.rs` が肥大化傾向 | contract / coverage はカテゴリ別 internal module へ分割済み。catalog 本体は dispatcher と結合 coverage に限定済み | operator 追加時にカテゴリ境界が崩れるリスクがある | 新規 operator は対応カテゴリ module へ追加し、coverage / lookup 一致テストを維持する |
| stage actor / substream authoring の配置 | `stage_actor`、`sub_sink_inlet`、`sub_source_outlet` と handler trait 群は `core/stage/` に分離済み | handler 追加時に stage logic 本体へ責務が戻るリスクがある | stage authoring API は `core/stage/` に閉じ、runtime 接続だけを interpreter 側へ渡す |

## 実装優先順位

### Phase 1: 小さく閉じる差分

1. 完了: `Flow::watch_termination` convenience を追加する
2. 完了: typed `ActorSink` の backpressure no-ack overload 相当を追加する
3. 完了: `MaterializerState` / `SystemMaterializer` の system-level snapshot helper を追加する
4. 残: TCP 用 error 型の骨格を追加する

### Phase 2: public authoring API

1. 完了: Public GraphDSL facade を Rust builder API として追加する
2. 完了: `SubSinkInlet` / `SubSourceOutlet` 相当を stage authoring API として追加する
3. 完了: `StreamRefSettings` / StreamRef attributes / StreamRef exception 群を先行追加する

### Phase 3: integration-level 機能

1. `StreamRefResolver` を実装する
2. StreamRef serialization / remote transport 連携を実装する
3. TCP stream API を std adaptor に追加する
4. TLS stream API を std adaptor に追加する
5. 完了: `StageActorRef` 相当を actor / stream 境界 API として追加する

## 結論

stream モジュールは、固定スコープの API parity ではほぼ到達済みと見てよい。現在の主要ギャップは operator の数ではなく、remote StreamRef、Tcp/TLS など、分散・IO 境界機能に集中している。

したがって次の調査・実装は、raw API 数を追うよりも、上記の integration-level 機能と内部モジュール境界を順に固めるのが妥当である。

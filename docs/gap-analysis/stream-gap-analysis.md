# stream モジュール ギャップ分析

更新日: 2026-04-24 (9th edition / 固定スコープ版)

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

残っている主要ギャップは、単純な operator 追加ではなく、分散・IO・public graph authoring・stage 内部 actor などの integration-level 機能に集中している。

| 指標 | 現在値 |
|------|--------|
| fraktor-rs public type 数 | 209 (core 202 / std 7) |
| fraktor-rs public method 数 | 1470 (shape / DSL boilerplate 含む) |
| 固定スコープ API カバレッジ推定 | 約 96% |
| Hard gap | 4 |
| Medium gap | 3 |
| Easy gap | 3 |
| Trivial gap | 2 |

## レイヤー別カバレッジ

| レイヤー | 判定 | 根拠 |
|----------|------|------|
| Stream core DSL | 約 97% | Source / Flow / Sink / RunnableGraph の主要 operator はほぼ実装済み |
| Graph / materialization | 約 93% | Graph stage / materialized value は実装済みだが、public GraphDSL facade が不足 |
| Stream typed interop | 約 90% | ActorSource / ActorSink / ask / PubSub は実装済み。一部 overload 相当が不足 |
| std adaptor / IO | 約 75-80% | FileIO / StreamConverters 系はあるが Tcp / TLS が未実装 |
| StreamRef / distributed stream | 低 | runtime ファイルはあるが public SourceRef / SinkRef / resolver / serializer が未実装 |

## API ギャップ一覧

### Hard

| ギャップ | Pekko 側 | fraktor-rs 現状 | 対応方針 |
|----------|----------|-----------------|----------|
| StreamRef | `StreamRefs`, `SourceRef`, `SinkRef`, `StreamRefResolver`, StreamRef protocol | `impl/streamref/stream_ref_runtime.rs` はあるが public API と protocol がない | `SourceRef` / `SinkRef` / resolver / protocol / serializer を分けて実装する |
| Tcp stream | `Tcp`, `Tcp.IncomingConnection`, `Tcp.OutgoingConnection`, TCP command/event | std adaptor に TCP 接続 API がない | `stream-adaptor-std/src/std/io/tcp/` を新設し、tokio TCP を Stream DSL に接続する |
| TLS stream | `TLS`, `SslTlsOptions`, TLS session handling | TLS API がない | TCP と独立した `std/io/tls/` と options 型を追加する |
| StageActorRef | `StageActorRef`, stage 内 actor integration | stage から actor を安全に扱う public API がない | stage logic と actor mailbox 境界を明確化して追加する |

### Medium

| ギャップ | Pekko 側 | fraktor-rs 現状 | 対応方針 |
|----------|----------|-----------------|----------|
| Public GraphDSL facade | `GraphDSL.create`, builder API, `~>` 接続 DSL | `GraphDsl` / `GraphDslBuilder` は `pub(crate)` の内部実装のみ | Scala の `~>` は除外し、Rust の明示的 public builder として公開する |
| SubSinkInlet / SubSourceOutlet | `SubSinkInlet`, `SubSourceOutlet` | substream 用 inlet/outlet authoring API がない | stage authoring 用の substream port API として追加する |
| StreamRef settings / attributes / exceptions | `StreamRefSettings`, `StreamRefAttributes`, StreamRef exception 群 | StreamRef 関連型がない | StreamRef 本体の前提型として先に追加する |

### Easy

| ギャップ | Pekko 側 | fraktor-rs 現状 | 対応方針 |
|----------|----------|-----------------|----------|
| typed ActorSink backpressure no-ack overload | `ActorSink.actorRefWithBackpressure` の ack 固定なし overload | ack message 指定版はある。no-ack 版はテスト TODO が残る | 既存 ActorSink に overload 相当の Rust API を追加する |
| `Flow::watch_termination` convenience | `watchTermination` | `watch_termination_mat` はあるが、plain convenience が見当たらない | materialized value を固定する薄い wrapper を追加する |
| system-level stream snapshot helper | `MaterializerState.streamSnapshots(system)` | `SystemMaterializer` と `MaterializerState::stream_snapshots(&ActorMaterializer)` はある | `SystemMaterializer` 経由の convenience helper を追加する |

### Trivial

| ギャップ | Pekko 側 | fraktor-rs 現状 | 対応方針 |
|----------|----------|-----------------|----------|
| Tcp / StreamRef error aliases | `StreamTcpException`, StreamRef exception classes | 汎用 `StreamError::Timeout` などはあるが専用 error 型がない | Rust の error enum / marker 型として追加する |
| StreamRef module exports | `org.apache.pekko.stream.StreamRefs` | public re-export がない | StreamRef 実装時に `core/dsl` または `core/stream_ref` で export する |

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
| GraphDSL の public / internal 境界 | builder machinery は `impl` 配下にあり `pub(crate)` | public DSL を出す時に内部 wiring と利用者 API が混ざりやすい | public wrapper を `core/dsl/graph_dsl*.rs` に置き、接続検証と実行 graph 変換は `impl` に残す |
| StreamRef package が空に近い | `impl/streamref/stream_ref_runtime.rs` は実質プレースホルダー | 後から resolver / protocol / serializer を足すと責務が集中する | `core/stream_ref/` と `impl/streamref/{protocol,resolver,serialization,runtime}` に分ける |
| std IO adaptor に tcp/tls 境界がない | FileIO / StreamConverters と同じ層に TCP/TLS が未配置 | IO 機能追加時に std adaptor の責務が曖昧になる | `std/io/tcp/` と `std/io/tls/` を明示的に分離する |
| `graph_interpreter.rs` が大きい | 約 1600 行で interpreter kernel が集中 | boundary dispatch / snapshot / scheduling の変更影響が広い | state machine、boundary dispatch、snapshot support を小さく分割する |
| `default_operator_catalog.rs` が肥大化傾向 | operator 登録が 1 ファイルに集約 | operator 追加時の競合とレビュー負荷が増える | transform / timing / aggregation / fan-in / fan-out / actor など登録カテゴリで分割する |
| stage actor / substream authoring の配置が未定 | StageActorRef / SubSinkInlet / SubSourceOutlet が未実装 | 実装時に stage logic 本体へ責務が流入しやすい | `stage_actor` と `substream_port` 相当の独立モジュールを先に切る |

## 実装優先順位

### Phase 1: 小さく閉じる差分

1. `Flow::watch_termination` convenience を追加する
2. typed `ActorSink` の backpressure no-ack overload 相当を追加する
3. `MaterializerState` の system-level snapshot helper を追加する
4. TCP / StreamRef 用 error 型の骨格を追加する

### Phase 2: public authoring API

1. Public GraphDSL facade を Rust builder API として追加する
2. `SubSinkInlet` / `SubSourceOutlet` 相当を stage authoring API として追加する
3. `StreamRefSettings` / `StreamRefAttributes` / StreamRef exception 群を先行追加する

### Phase 3: integration-level 機能

1. `SourceRef` / `SinkRef` / `StreamRefs` factories / `StreamRefResolver` を実装する
2. StreamRef serialization / remote transport 連携を実装する
3. TCP stream API を std adaptor に追加する
4. TLS stream API を std adaptor に追加する
5. `StageActorRef` を actor / stream 境界 API として追加する

## 結論

stream モジュールは、固定スコープの API parity ではほぼ到達済みと見てよい。現在の主要ギャップは operator の数ではなく、StreamRef、Tcp/TLS、Public GraphDSL、StageActorRef、SubSinkInlet/SubSourceOutlet など、分散・IO・graph authoring の境界機能に集中している。

したがって次の調査・実装は、raw API 数を追うよりも、上記の integration-level 機能と内部モジュール境界を順に固めるのが妥当である。

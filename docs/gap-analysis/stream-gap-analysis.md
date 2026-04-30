# stream モジュール ギャップ分析

更新日: 2026-04-30 (15th edition / 現行ツリー再検証)

## 比較スコープ定義

この調査は Apache Pekko の `stream` / `stream-typed` を全量移植する調査ではない。Rust で再現可能な fraktor-rs の stream runtime 契約だけを parity 対象に固定し、Scala / Java / JVM 固有の表層 API は分母から除外する。

### 対象に含めるもの

| 領域 | fraktor-rs 側 | Pekko 側 |
|------|---------------|----------|
| Stream core DSL | `modules/stream-core/src/core/dsl/` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/` |
| Graph / stage / materialization | `modules/stream-core/src/core/stage/`, `modules/stream-core/src/core/materialization/`, `modules/stream-core/src/core/impl/` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/`, `references/pekko/stream/src/main/scala/org/apache/pekko/stream/stage/`, `references/pekko/stream/src/main/scala/org/apache/pekko/stream/impl/` |
| Attributes / shape / snapshot | `modules/stream-core/src/core/attributes/`, `modules/stream-core/src/core/shape/`, `modules/stream-core/src/core/snapshot/` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/` |
| typed actor interop | `modules/stream-core/src/core/dsl/actor_*.rs`, `modules/stream-core/src/core/dsl/topic_pub_sub.rs` | `references/pekko/stream-typed/src/main/scala/org/apache/pekko/stream/typed/scaladsl/` |
| Framing / compression | `modules/stream-core/src/core/dsl/framing.rs`, `modules/stream-core/src/core/dsl/json_framing.rs`, `modules/stream-core/src/core/impl/io/compression.rs` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Framing.scala`, `Compression.scala` |
| StreamRef | `modules/stream-core/src/core/stream_ref/`, `modules/stream-core/src/core/impl/streamref/`, `modules/stream-core/src/core/dsl/stream_refs.rs` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/StreamRefs.scala`, `references/pekko/stream/src/main/scala/org/apache/pekko/stream/impl/streamref/`, `references/pekko/stream/src/main/scala/org/apache/pekko/stream/serialization/StreamRefSerializer.scala`, `references/pekko/stream/src/main/protobuf/StreamRefMessages.proto` |
| std adaptor / IO | `modules/stream-adaptor-std/src/std/io/`, `modules/stream-adaptor-std/src/std/materializer/` | Pekko の `FileIO`, `StreamConverters`, `Tcp`, `TLS` 相当 |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| `javadsl` API / Java overload / `CompletionStage` | Java 相互運用専用であり、Rust API の再現対象ではない |
| Scala implicit / package ops / symbolic GraphDSL syntax | Scala 言語機能。Rust では明示的 builder API に置換する |
| Reactive Streams `Publisher` / `Subscriber` ブリッジ | JVM interop 固有。Rust では stream / sink / async primitive で代替する |
| `stream-testkit`, `stream-tests`, `stream-tests-tck`, `stream-typed-tests` | runtime API ではなく検証資材。ユーザーが testkit 調査を明示した場合だけ別スコープ |
| HOCON / JVM dispatcher / `MaterializerConfigurator` | JVM / Typesafe Config 固有 |
| deprecated API | 正式リリース前の fraktor-rs では互換維持目的 API を追わない |

### 現行パス確認

| 種別 | 確認結果 |
|------|----------|
| core crate | `modules/stream-core/src/lib.rs` が `pub mod core;` を公開し、`#![cfg_attr(not(test), no_std)]` と `#![deny(cfg_std_forbid)]` を持つ |
| core root | `modules/stream-core/src/core/` が存在し、DSL / impl / stage / materialization / stream_ref / shape などに分割されている |
| std crate | `modules/stream-adaptor-std/src/lib.rs` が `pub mod std;` を公開する |
| std root | `modules/stream-adaptor-std/src/std/` が存在し、`io/` と `materializer/` を公開する |
| Pekko stream | `references/pekko/stream/src/main/` が存在する |
| Pekko stream-typed | `references/pekko/stream-typed/src/main/` が存在する |

### 判定方針

`references/pekko` から機械的に抽出した raw 公開型数・メソッド数は参考値に留める。Scala / Java / JVM 固有 API を分母に入れると Rust では再現不能なギャップが混ざるため、このレポートでは上記固定スコープだけを parity 分母にする。

API 面の hard / medium ギャップは 5 件以下であり、主要 operator / stage authoring / typed interop の致命的欠落もない。そのため、API ギャップ一覧に加えて内部モジュール構造ギャップも分析する。

## サマリー

stream の固定スコープにおける API parity は高い。残ギャップは単純な operator 追加ではなく、remote StreamRef と TCP / TLS の integration-level 機能に集中している。

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 50 |
| fraktor-rs 固定スコープ対応概念 | 46 |
| 固定スコープ概念カバレッジ | 46/50 (92%) |
| fraktor-rs raw public type declarations | 347 (core: 340 / std: 7) |
| fraktor-rs raw public method declarations | 1898 (core: 1881 / std: 17) |
| Pekko raw public type declarations | 689 (stream + stream-typed `src/main/scala` 参考値) |
| Pekko raw `scaladsl` public method candidates | 978 (参考値) |
| hard / medium / easy / trivial gap | 3 / 0 / 0 / 1 |

raw declaration count は API parity の分母ではない。shape boilerplate、internal helper の public surface、Rust の 1 file 1 type 方針、Scala / Java / JVM 固有 API を含むため、Pekko と fraktor-rs の raw 数は直接比較しない。

スタブ確認では、`todo!()` / `unimplemented!()` / `panic!("not implemented")` は stream core / std adaptor から検出されなかった。`Sink::combine` の rustdoc に古い `stub implementation` 表記が残っているが、実装は `Sink::combine_mat` 経由で broadcast fan-out し、テストも存在するため API ギャップには分類しない。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core | `Source` / `Flow` / `Sink` / `BidiFlow` / `RunnableGraph`、主要 operator、GraphDSL、stage authoring、shape、attributes、materialization、supervision、restart、kill switch、queue、hub、snapshot、StreamRef local handoff、framing / compression | `modules/stream-core/src/core/` に集約。compression は `compression` feature 下で `no_std + alloc` として公開される | 高。主要 API は対応済み |
| typed surface | `ActorSource`, `ActorSink`, `ActorFlow.ask*`, `PubSub` | 専用 `typed/` root はなく、`core/dsl/` 内で actor-core typed API と連携する | 高。typed interop は core DSL に吸収済み |
| std / adaptor | `FileIO`, `StreamConverters`, `SystemMaterializer`, TCP, TLS | `FileIO` / `StreamConverters` / `SystemMaterializer` は存在。TCP / TLS と TCP error contract は未配置 | 部分対応 |

## カテゴリ別ギャップ

ギャップ表には未対応・部分実装・n/a のみを列挙する。実装済み項目はカテゴリヘッダーの件数に含める。

### Core DSL / operator semantics 実装済み 18/18 (100%)

未対応ギャップなし。

実装済みと判定した代表項目は、`map`, `filter`, `collect`, `statefulMap`, `statefulMapConcat`, `conflate` / `conflateWithSeed`, `expand` / `extrapolate`, `intersperse`, `orElse`, timeout 系, `mapAsync`, `mapAsyncPartitioned`, `RetryFlow.withBackoffAndContext`, `MergeSequence`, `ZipLatest`, `Sink.combine`, `watchTermination`, `fromMaterializer`, `contramap`, `dimap`, `doOnCancel`, `Sink.source`, `FlowWithContext` / `SourceWithContext` の partitioned async 系である。

### Graph / stage / materialization 実装済み 8/8 (100%)

未対応ギャップなし。

根拠は `GraphDsl`, `GraphDslBuilder`, `GraphStage<In, Out, Mat>`, `GraphStageLogic<In, Out, Mat>`, `AsyncCallback`, `TimerGraphStageLogic`, `StageActor`, `SubSinkInlet`, `SubSourceOutlet`, `SystemMaterializer::stream_snapshots` である。

### Typed actor interop 実装済み 4/4 (100%)

未対応ギャップなし。

`ActorSource::actor_ref`, `ActorSource::actor_ref_with_backpressure`, `ActorSink::actor_ref_with_backpressure_any_ack`, `Flow::ask*`, `Flow::ask_with_context*`, `TopicPubSub::source` / `TopicPubSub::sink` を確認済み。

### Framing / compression / byte stream utilities 実装済み 8/8 (100%)

未対応ギャップなし。

`Framing::delimiter`, `Framing::length_field`, `Framing::simple_framing_protocol`, `JsonFraming::object_scanner`, `JsonFraming::array_scanner`, `Flow::gzip`, `Flow::gzip_decompress`, `Flow::deflate`, `Flow::inflate` を確認済み。compression 系 API は `compression` feature 下で公開されるため、標準ビルドで常時公開される API とは分けて扱う。

### StreamRef / distributed stream 実装済み 6/7 (86%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `StreamRefResolver` + remote serializer / transport | `StreamRefs.scala:133`, `StreamRefs.scala:146`, `impl/streamref/StreamRefResolverImpl.scala:26`, `serialization/StreamRefSerializer.scala:24`, `StreamRefMessages.proto:37` | local handoff の `StreamRefs`, `SourceRef`, `SinkRef`, protocol, settings, attributes, exception variants は実装済み。remote resolver / serializer / actor transport 連携は未実装 | core + std / remote adapter | hard | local handoff と remote actor ref 解決を混ぜない境界設計が必要 |

### std adaptor / IO 実装済み 2/5 (40%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Tcp`, `Tcp.IncomingConnection`, `Tcp.OutgoingConnection` | `scaladsl/Tcp.scala:47`, `scaladsl/Tcp.scala:67`, `scaladsl/Tcp.scala:86`, `scaladsl/Tcp.scala:139`, `scaladsl/Tcp.scala:216`, `impl/io/TcpStages.scala:227` | 未対応 | std | hard | tokio TCP を Stream DSL と materialization に接続する adapter が必要 |
| `TLS`, `TLSPlacebo`, `SslTlsOptions`, `TLSProtocol`, `TLSActor`, `TlsModule` | `scaladsl/TLS.scala:62`, `scaladsl/TLS.scala:102`, `SslTlsOptions.scala:29`, `SslTlsOptions.scala:152`, `impl/io/TLSActor.scala:41`, `impl/io/TlsModule.scala:32` | 未対応 | std | hard | TLS option / protocol messages / byte stream wrapping の境界が必要 |
| `StreamTcpException`, `BindFailedException`, `ConnectionException` | `StreamTcpException.scala:18`, `StreamTcpException.scala:20`, `StreamTcpException.scala:22` | 未対応 | std | trivial | TCP 実装時に Rust の error enum / marker 型として追加する |

`FileIO` と `StreamConverters` は `modules/stream-adaptor-std/src/std/io/` で対応済みと判定する。Java Stream collector / Java Stream bridge は Java interop 専用のため n/a に分類する。

### JVM / Scala / Java 固有 API n/a

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `javadsl.*`, Java overload, Java `CompletionStage` bridge | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/javadsl/` | n/a | - | n/a | Rust API では overload / Java wrapper を再現しない |
| `JavaFlowSupport`, `StreamConverters.javaCollector`, `asJavaStream`, `fromJavaStream` | `scaladsl/JavaFlowSupport.scala`, `scaladsl/StreamConverters.scala:118`, `scaladsl/StreamConverters.scala:187`, `scaladsl/StreamConverters.scala:225` | n/a | - | n/a | JVM / Java Stream interop 専用 |
| Scala implicit / symbolic GraphDSL syntax | `scaladsl/Graph.scala`, package ops | n/a | - | n/a | Rust では明示的 builder API として再表現する |
| Reactive Streams TCK / test probe API | `stream-testkit`, `stream-tests`, `stream-tests-tck` | n/a | - | n/a | runtime API ではない |
| HOCON / dispatcher / materializer configurator | Pekko stream config provider | n/a | - | n/a | JVM 設定ロード方式に依存する |

## 内部モジュール構造ギャップ

API ギャップが少ないため、残りは内部構造と integration 境界が主なボトルネックになる。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| StreamRef remote 境界が未配置 | `impl/streamref/StreamRefResolverImpl.scala`, `impl/streamref/StreamRefsMaster.scala`, `serialization/StreamRefSerializer.scala`, `StreamRefMessages.proto` | `core/stream_ref/` と `core/impl/streamref/` は local handoff に閉じている | resolver / serialization / remote transport adapter を local handoff から分ける | hard | high | remote を local handoff に直結すると core/std/remote の依存方向が崩れる |
| std IO の tcp/tls サブモジュールが未配置 | `impl/io/TcpStages.scala`, `impl/io/TLSActor.scala`, `impl/io/TlsModule.scala` | `std/io/` は `FileIO` / `StreamConverters` 中心。`core/impl/io/` は compression のみ | `stream-adaptor-std/src/std/io/tcp/` と `stream-adaptor-std/src/std/io/tls/` を分ける | hard | high | TCP と TLS を同一モジュールへ詰めると options / protocol / connection lifecycle が混ざる |
| GraphInterpreter drive state machine が中心に残る | `impl/fusing/GraphInterpreter.scala`, `impl/fusing/ActorGraphInterpreter.scala` | plan compile、edge buffer、snapshot、failure / restart は分割済み。drive loop 本体は `core/impl/interpreter/graph_interpreter.rs` に残る | demand / scheduling を壊さない単位で後続分割する | medium | medium | 現時点では API ギャップではないが、今後の interpreter 変更の衝突点になる |

## 実装優先度

この節は、上記のカテゴリ別ギャップに列挙した項目だけを parity ギャップ解消順に再配置する。

### Phase 1: trivial / easy

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `StreamTcpException` / `BindFailedException` / `ConnectionException` 相当の TCP error 型 | std | 既存 error enum / marker 型の追加で閉じる。ただし TCP API と命名を合わせる必要がある |

### Phase 2: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| 該当なし | - | 現時点の未実装は trivial か hard に二極化している |

### Phase 3: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `StreamRefResolver` + remote serializer / transport | core + std / remote adapter | resolver / serializer / actor transport 連携を伴う |
| `Tcp`, `IncomingConnection`, `OutgoingConnection` | std | tokio TCP、connection lifecycle、materialized binding / connection 型が必要 |
| `TLS`, `TLSPlacebo`, `SslTlsOptions`, `TLSProtocol`, `TLSActor`, `TlsModule` | std | TLS option、protocol message、byte stream wrapping、TCP 連携が必要 |

### 対象外: n/a

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| Java DSL / Java Stream / Reactive Streams TCK / Scala syntax sugar / HOCON 固有 API | - | JVM / Java / Scala 固有であり、固定スコープの parity 分母に含めない |

## まとめ

stream モジュールは固定スコープの API parity では高い水準に到達している。主要 operator、GraphDSL / stage authoring、typed actor interop、framing、compression、FileIO / StreamConverters は実装済みと判定できる。

低コストで進められる残差は TCP error 型だけである。主要ギャップは `StreamRefResolver` + remote serializer / transport、TCP stream、TLS stream の 3 点で、いずれも std / remote 連携を伴う integration-level 実装である。

次のボトルネックは API surface の追加数ではなく、remote StreamRef と std IO adapter の境界設計である。実装時は local handoff、remote resolver、serialization、transport adapter を分け、core/std/remote の依存方向を崩さないことが重要になる。

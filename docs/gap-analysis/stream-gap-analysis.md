# stream モジュール ギャップ分析

更新日: 2026-04-25 (14th edition / 固定スコープ再分類)

## 比較スコープ定義

この調査は、Apache Pekko の `stream` / `stream-typed` を全量移植する調査ではない。Rust で再現可能な fraktor-rs の stream runtime 契約だけを parity 対象に固定し、Scala / Java / JVM 固有の表層 API は分母から除外する。

### 対象に含めるもの

| 領域 | fraktor-rs 側 | Pekko 側 |
|------|---------------|----------|
| Stream core DSL | `modules/stream-core/src/core/dsl/` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/` |
| Graph / stage / materialization | `modules/stream-core/src/core/stage/`, `modules/stream-core/src/core/materialization/`, `modules/stream-core/src/core/impl/` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/`, `references/pekko/stream/src/main/scala/org/apache/pekko/stream/stage/`, `references/pekko/stream/src/main/scala/org/apache/pekko/stream/impl/` |
| Attributes / shape / snapshot | `modules/stream-core/src/core/attributes/`, `modules/stream-core/src/core/shape/`, `modules/stream-core/src/core/snapshot/` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/` |
| typed actor interop | `modules/stream-core/src/core/dsl/actor_*.rs`, `modules/stream-core/src/core/dsl/topic_pub_sub.rs` | `references/pekko/stream-typed/src/main/scala/org/apache/pekko/stream/typed/scaladsl/` |
| StreamRef | `modules/stream-core/src/core/stream_ref/`, `modules/stream-core/src/core/impl/streamref/`, `modules/stream-core/src/core/dsl/stream_refs.rs` | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/StreamRefs.scala`, `references/pekko/stream/src/main/scala/org/apache/pekko/stream/impl/streamref/` |
| std adaptor / IO | `modules/stream-adaptor-std/src/std/io/`, `modules/stream-adaptor-std/src/std/materializer/` | Pekko の FileIO / StreamConverters / Tcp / TLS 相当 |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| `javadsl` API / Java overload / `CompletionStage` | Java 相互運用専用であり、Rust API の再現対象ではない |
| Scala implicit / package ops / symbolic syntax / `~>` そのもの | Scala 言語機能。Rust では明示的 builder API に置換する |
| Reactive Streams `Publisher` / `Subscriber` ブリッジ | JVM interop 固有。Rust では stream / sink / async primitive で代替する |
| `stream-testkit`, `stream-tests`, `stream-tests-tck`, `stream-typed-tests` | runtime API ではなく検証資材。ユーザーが testkit 調査を明示した場合だけ別スコープ |
| HOCON / JVM dispatcher / MaterializerConfigurator | JVM / Typesafe Config 固有 |
| deprecated API | 正式リリース前の fraktor-rs では互換維持目的 API を追わない |

### 判定方針

`references/pekko` から機械的に抽出した raw 公開型数・メソッド数は参考値に留める。Scala / Java / JVM 固有 API を分母に入れると Rust では再現不能なギャップが混ざるため、このレポートでは上記固定スコープだけを parity 分母にする。

API 面の hard / medium ギャップは 5 件以下であり、主要 operator / stage authoring / typed interop の致命的欠落もない。そのため、API ギャップ一覧に加えて内部モジュール構造ギャップも分析する。

## サマリー

stream の固定スコープにおける API parity はほぼ到達済み。残ギャップは単純な operator 追加ではなく、remote StreamRef と Tcp / TLS の integration-level 機能に集中している。

| 指標 | 値 |
|------|-----|
| fraktor-rs raw public type declarations | 229 (core 222 / std 7) |
| fraktor-rs raw public method declarations | 1535 (core 1520 / std 15) |
| 固定スコープ API カバレッジ推定 | 約 98% |
| Hard gap | 3 |
| Medium gap | 0 |
| Easy gap | 0 |
| Trivial gap | 1 |

raw declaration count は API parity の分母ではない。shape boilerplate、internal helper の public surface、Rust の型分割方針を含むため、Pekko の raw Scala / Java API 数とは直接比較しない。

スタブ確認では、`todo!()` / `unimplemented!()` / `panic!("not implemented")` は stream core / std adaptor から検出されなかった。`Sink::combine` の rustdoc に古い `stub implementation` 表記が残っているが、実装は `Sink::combine_mat` 経由で broadcast fan-out し、テストも存在するため API ギャップには分類しない。

## 層別カバレッジ

| 層 | 固定スコープ判定 | fraktor-rs 現状 | 残ギャップ |
|----|------------------|-----------------|------------|
| core DSL / operator semantics | 高 | Source / Flow / Sink / BidiFlow / RunnableGraph の主要 operator は実装済み。`conflate`, `expand`, `intersperse`, `orElse`, timeout, `mapAsyncPartitioned`, `RetryFlow.withBackoffAndContext`, `MergeSequence`, `ZipLatest` も確認済み | なし |
| graph / stage / materialization | 高 | `GraphDSL`, `GraphDslBuilder`, `GraphStage<In, Out, Mat>`, `GraphStageLogic<In, Out, Mat>`, stage actor, substream authoring, snapshot helper が存在する | なし |
| typed actor interop | 高 | `ActorSource`, `ActorSink`, ask / pubsub、backpressure no-ack 相当を実装済み | なし |
| StreamRef / distributed stream | 部分対応 | local handoff の `StreamRefs`, `SourceRef`, `SinkRef`, settings, attributes, exception variants は実装済み | remote resolver / serializer / transport integration |
| std adaptor / IO | 部分対応 | FileIO / StreamConverters / SystemMaterializer helper は実装済み | Tcp / TLS |

## カテゴリ別ギャップ

ギャップ表には未対応・部分実装・n/a のみを列挙する。実装済み項目はカテゴリヘッダーの件数に含める。

### Core DSL / operator semantics 実装済み 18/18 (100%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| 該当なし | - | 実装済み | core | - | 既知の主要 operator ギャップは残っていない |

実装済みと判定した代表項目は、`conflate` / `conflateWithSeed`, `expand` / `extrapolate`, `intersperse`, `orElse`, timeout 系, `mapAsyncPartitioned`, `RetryFlow.withBackoffAndContext`, `MergeSequence`, `ZipLatest`, `Sink.combine`, `watchTermination`, `fromMaterializer`, `contramap`, `dimap`, `doOnCancel`, `Sink.source`, `FlowWithContext` / `SourceWithContext` の partitioned async 系である。

### Graph / stage / materialization 実装済み 8/8 (100%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| 該当なし | - | 実装済み | core | - | public authoring API と materialization helper の主要ギャップは残っていない |

根拠は `GraphDsl`, `GraphDslBuilder`, `GraphStage<In, Out, Mat>`, `GraphStageLogic<In, Out, Mat>`, `StageActor`, `SubSinkInlet`, `SubSourceOutlet`, `SystemMaterializer::stream_snapshots` である。

### Typed actor interop 実装済み 4/4 (100%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| 該当なし | - | 実装済み | core | - | actor interop の既知ギャップは残っていない |

`ActorSink::actor_ref_with_backpressure_any_ack`、`ActorSource`、ask pattern、topic pub/sub 相当を確認済み。

### StreamRef / distributed stream 部分対応 6/7 (約 86%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `StreamRefResolver` + remote serializer / transport | `StreamRefs.scala:133`, `impl/streamref/StreamRefResolverImpl.scala:26`, `StreamRefMessages.proto:20` | local handoff の `StreamRefs`, `SourceRef`, `SinkRef`, protocol, settings, attributes, exception variants まで実装済み。remote resolver / serializer / actor transport 連携は未実装 | core + std / remote adapter | hard | local handoff と remote actor ref 解決を混ぜない境界設計が必要 |

### std adaptor / IO 部分対応 2/5 (40%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Tcp`, `Tcp.IncomingConnection`, `Tcp.OutgoingConnection` | `scaladsl/Tcp.scala:47`, `impl/io/TcpStages.scala:53` | 未対応 | std | hard | tokio TCP を Stream DSL と materialization に接続する adapter が必要 |
| `TLS`, `SslTlsOptions`, `TLSProtocol`, `TLSPlacebo` | `scaladsl/TLS.scala:62`, `SslTlsOptions.scala:29`, `SslTlsOptions.scala:152`, `impl/io/TLSActor.scala:41` | 未対応 | std | hard | TLS option / protocol messages / byte stream wrapping の境界が必要 |
| `StreamTcpException`, `BindFailedException`, `ConnectionException` | `StreamTcpException.scala:18` | 未対応 | std | trivial | TCP 実装時に Rust の error enum / marker 型として追加する |

### JVM / Scala / Java 固有 API n/a

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `javadsl.*`, Java overload, Java `CompletionStage` bridge | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/javadsl/` | n/a | - | n/a | Rust API では overload / Java wrapper を再現しない |
| Scala implicit / symbolic GraphDSL syntax | `scaladsl` / package ops | n/a | - | n/a | Rust では明示的 builder API として再表現する |
| testkit / TCK / tests | `stream-testkit`, `stream-tests`, `stream-tests-tck` | n/a | - | n/a | runtime API ではない |

## 内部モジュール構造ギャップ

API ギャップが少ないため、残りは内部構造と integration 境界が主なボトルネックになる。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| StreamRef remote 境界が未配置 | `impl/streamref/StreamRefResolverImpl.scala`, `impl/streamref/StreamRefsMaster.scala`, `StreamRefMessages.proto` | `core/stream_ref/` と `core/impl/streamref/` は local handoff に閉じている | resolver / serialization / remote transport adapter を local handoff から分ける | hard | high | remote を local handoff に直結すると core/std/remote の依存方向が崩れる |
| std IO の tcp/tls サブモジュールが未配置 | `impl/io/TcpStages.scala`, `impl/io/TLSActor.scala`, `impl/io/TlsModule.scala` | `std/io/` は FileIO / StreamConverters 中心。`core/impl/io/` は compression のみ | `stream-adaptor-std/src/std/io/tcp/` と `stream-adaptor-std/src/std/io/tls/` を分ける | hard | high | TCP と TLS を同一モジュールへ詰めると options / protocol / connection lifecycle が混ざる |
| GraphInterpreter drive state machine がまだ中心に残る | `impl/fusing/GraphInterpreter.scala`, `ActorGraphInterpreter.scala` | plan compile、edge buffer、snapshot、failure / restart は分割済み。drive loop 本体は `graph_interpreter.rs` に残る | demand / scheduling を壊さない単位で後続分割する | medium | medium | 現時点では API ギャップではないが、今後の interpreter 変更の衝突点になる |
| DefaultOperatorCatalog のカテゴリ境界維持 | Pekko は fusing / impl に operator 実装責務を分散 | `default_operator_catalog_{source,transform,substream,timing,fan_in,fan_out,failure,hub,kill_switch}.rs` へ分割済み | 新規 operator は対応カテゴリへ追加し、catalog 本体は dispatcher に留める | trivial | medium | 既に改善済み。今後の追加時に境界を崩さないことが重要 |
| stage authoring API と runtime 接続の境界維持 | `stage/GraphStage.scala`, `stage/GraphStageLogic.scala` | `core/stage/` に public authoring API、`core/impl/interpreter/` に runtime 接続を配置 | handler / substream / stage actor は `core/stage/` に閉じる | trivial | low | 既に改善済み。runtime 都合を public stage API に漏らさない |

## 実装優先度

この節は、上記のカテゴリ別ギャップに列挙した項目だけを parity ギャップ解消順に再配置する。

### Phase 1: trivial / easy

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `StreamTcpException` / TCP error 型 | std | 既存 error enum / marker 型の追加で閉じる。ただし TCP API と命名を合わせる必要がある |

### Phase 2: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| 該当なし | - | 現時点の未実装は trivial か hard に二極化している |

### Phase 3: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| StreamRef remote integration | core + std / remote adapter | resolver / serializer / actor transport 連携を伴う |
| TCP stream API | std | tokio TCP、connection lifecycle、materialized binding / connection 型が必要 |
| TLS stream API | std | TLS option、protocol message、byte stream wrapping、TCP 連携が必要 |

## まとめ

stream モジュールは固定スコープの API parity ではほぼ到達済みで、過去の「API ギャップ大」は raw extraction を分母にした場合の見え方だった。Rust で再現可能な stream runtime 契約に絞ると、主要 operator、GraphDSL / stage authoring、typed actor interop は実装済みと判定できる。

低コストで進められる残差は TCP error 型だけである。主要ギャップは StreamRef remote integration、TCP stream、TLS stream の 3 点で、いずれも std / remote 連携を伴う integration-level 実装である。

次のボトルネックは API surface の追加数ではなく、remote StreamRef と std IO adapter の境界設計である。実装時は local handoff、remote resolver、serialization、transport adapter を分け、core/std/remote の依存方向を崩さないことが重要になる。

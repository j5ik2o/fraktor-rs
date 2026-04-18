# stream モジュール ギャップ分析

更新日: 2026-04-18 (7th edition)

## 前提と集計範囲

- **比較対象**:
  - fraktor-rs 側:
    - `modules/stream-core/src/core/` (untyped kernel 相当。typed サブ層なし)
    - `modules/stream-adaptor-std/src/std/` (tokio/std 依存アダプタ)
  - Pekko 側:
    - `references/pekko/stream/src/main/scala/org/apache/pekko/stream/` (公開契約)
    - `.../scaladsl/` (Scala DSL 本体)
    - `.../stage/` (Stage authoring)
    - `.../snapshot/` / `.../serialization/`
    - `src/main/boilerplate/` (FanInShape/FanOutShape の生成ソース)
- **除外**: `javadsl/`, `impl/` 配下の内部実装, `private[...]` 修飾付き, `stream-testkit/`, `stream-typed/`
- fraktor-rs 側は **`core/typed` 層が存在しない**。typed ラッパーは `0/0` として扱う。
- オペレーターは型ではなくメソッドで比較する (fraktor-rs は `impl Flow` / `impl Source` / `impl Sink` に集約しているため)。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開 API 数 (13 カテゴリ合計) | 約 267 件 |
| fraktor-rs 公開型数 | 151 (core: 146 / std: 5) |
| fraktor-rs 公開メソッド数 | 671 (core: 661 / std: 10) |
| カテゴリ単位の推定カバレッジ | 約 **72%** |
| ギャップ数 (medium+) | 36 (core: 30 / std: 6) |
| ギャップ数 (hard) | 6 |

**要約**: 基本 DSL (Source/Flow/Sink と主要オペレーター) はほぼ完成。重点的なギャップは以下 6 領域。

1. **StreamRef (SinkRef/SourceRef)** — 分散 stream 参照 API が未実装 (remote-core との連携必須)
2. **Tcp / TLS** — tokio ネットワークソース/フロー/シンクが adaptor-std に未配備
3. **Graph DSL の明示化** — `GraphDSL.create` / `~>` 相当の配線 DSL が未整備 (Flow メソッドに圧縮)
4. **MaterializerState スナップショット** — 実行中 interpreter の可視化 API が未実装
5. **Stage Authoring の補助型** — `SubSinkInlet/SubSourceOutlet`, 標準 Handler (EagerTerminate*/IgnoreTerminate*), `StageActorRef` が欠損
6. **Pekko 固有オペレーター** — `conflate` / `expand` / `extrapolate` / `intersperse` / `alsoTo` / `orElse` / `switchMap` / `mergeLatest` / `*_timeout` / `keepAlive` 等が未実装

## 層別カバレッジ

| 層 | Pekko 対応数 | fraktor-rs 実装数 | カバレッジ |
|----|-------------|-------------------|-----------|
| core / untyped kernel | 約 247 | 146 型 + 661 メソッド | 約 75% |
| core / typed ラッパー | 該当層なし (Pekko 側も `stream-typed` は別モジュール) | 0 | 0/0 |
| std / アダプタ | 約 20 (IO/Tcp/TLS/Snapshot) | 5 型 / 10 メソッド | 約 35% |

## カテゴリ別ギャップ

各カテゴリ見出しに **実装済み / Pekko 総数 (カバレッジ%)** を付記。ギャップ (未対応・部分実装・n/a) のみ表に列挙する。

### 1. 型・トレイト　✅ 実装済み 15/22 (68%)

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `FlowOpsMat` | `scaladsl/Flow.scala:4106` | 別経路で実装 | core | n/a | Rust では `impl Flow` のメソッド内で `Mat` 合成を扱い trait は不要 |
| `FlowWithContextOps` | `scaladsl/FlowWithContextOps.scala:1` | 別経路で実装 | core | n/a | Rust では `FlowWithContext` / `SourceWithContext` のメソッドで直接提供 |
| `SinkRef[In]` | `stream/StreamRefs.scala:55` | 未対応 | core + std | hard | 分散 stream 参照。remote-core のシリアライザと連携必須 |
| `SourceRef[T]` | `stream/StreamRefs.scala:89` | 未対応 | core + std | hard | 同上 |
| `StreamRefResolver` | `stream/StreamRefs.scala:133` | 未対応 | core | medium | StreamRef の文字列シリアライズ |
| `SourceQueueWithComplete[T]` | `scaladsl/Queue.scala:63` | 型エイリアスあり | core | trivial | 完了通知付き SourceQueue。契約の仕上げが必要 |
| `SinkQueueWithCancel[T]` | `scaladsl/Queue.scala:136` | 未対応 | core | easy | キャンセル付き SinkQueue |

### 2. オペレーター　✅ 実装済み 70/85+ (82%)

fraktor-rs は 661 メソッドを実装済みで、Pekko の 85+ 主要オペレーターのほとんどをカバー。下記は Pekko 固有の未実装オペレーター。

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `conflate` | `FlowOps.scala` | 未対応 | core | medium | 需要不足時に要素を畳み込むバックプレッシャ回避。Graph stage 実装が必要 |
| `conflateWithSeed` | `FlowOps.scala` | 未対応 | core | medium | 初期値付き conflate |
| `expand` | `FlowOps.scala` | 未対応 | core | medium | 需要超過時にイテレータで補完 |
| `extrapolate` | `FlowOps.scala` | 未対応 | core | medium | expand の静的初期値版 |
| `intersperse` | `FlowOps.scala` | 未対応 | core | easy | 要素間に区切り要素を挿入 |
| `alsoTo` | `FlowOps.scala` | 未対応 | core | easy | Sink に分岐しつつ本流にも流す |
| `alsoToAll` | `FlowOps.scala` | 未対応 | core | easy | N Sink 全てに分岐 |
| `divertTo` | `FlowOps.scala` | 未対応 | core | easy | 条件付き Sink 分岐 |
| `orElse` | `FlowOps.scala` | 未対応 | core | easy | 先行 Source が空なら代替 Source |
| `switchMap` | `scaladsl/Flow.scala:1440` | 未対応 | core | medium | 新要素到着で前のサブストリームをキャンセル |
| `mergeLatest` | `scaladsl/MergeLatest.scala` | 未対応 | core | medium | 全入力の最新値をリスト出力 |
| `mergePreferred` | `scaladsl/Graph.scala:120` | 未対応 | core | medium | 優先ポート付きマージ |
| `mapAsyncPartitioned` | `scaladsl/Flow.scala:975` | 未対応 | core | medium | パーティション別並列実行 |
| `aggregateWithBoundary` | `FlowOps.scala` | 未対応 | core | medium | 境界述語でバッチ集約 |
| `backpressureTimeout` | `FlowOps.scala` | 未対応 | core | easy | 下流需要不足のタイムアウト |
| `completionTimeout` | `FlowOps.scala` | 未対応 | core | easy | 全体完了のタイムアウト |
| `idleTimeout` | `FlowOps.scala` | 未対応 | core | easy | アイドル時間でタイムアウト |
| `initialTimeout` | `FlowOps.scala` | 未対応 | core | easy | 初回要素までのタイムアウト |
| `keepAlive` | `FlowOps.scala` | 未対応 | core | easy | アイドル時に代替要素を注入 |

### 3. マテリアライゼーション　✅ 実装済み 12/12 (100%)

ギャップなし。fraktor-rs 側は `MatCombine` (enum) / `KeepLeft/Right/Both/None` (struct) / `MatCombineRule` (trait) で Pekko の `Keep` object を代替。`ActorMaterializer` / `ActorMaterializerConfig` / `RunnableGraph` / `Materialized` / `StreamCompletion` を備え、追加で `SubscriptionTimeoutConfig` / `MaterializerLifecycleState` を提供している。

### 4. Graph DSL　✅ 実装済み 12/18 (67%)

fraktor-rs は GraphDSL を独立 API として持たず、配線を `Flow::merge` / `Flow::broadcast` / `Flow::balance` / `Source::combine` / `Source::zip_n` 等のメソッドに統合している。
このため型・メソッド単位では大半をカバーしているが、**任意の有向グラフを構築する DSL (GraphDSL.create + `~>`) は未整備**。

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `GraphDSL.create` | `scaladsl/Graph.scala:1590` | 部分実装 | core | hard | `GraphDSLBuilder` (impl 内) はあるが公開 DSL にならず。型安全な配線 API が未整備 |
| `GraphDSL.Implicits` / `~>` 演算子 | `scaladsl/Graph.scala:1721` | 未対応 | core | hard | Rust では演算子オーバーロード制約あり。代替 API (`.wire_to(port)` 等) の設計が必要 |
| `MergePreferred[In]` | `scaladsl/Graph.scala:120` | 未対応 | core | medium | 優先ポート付きマージ stage |
| `MergeSequence[A]` | `scaladsl/Graph.scala:1200` | 未対応 | core | medium | シーケンス番号順マージ |
| `Unzip[A,B]` | `scaladsl/Graph.scala:950` | 未対応 | core | easy | ペアを 2 出力に分解 (Flow::unzip で代替あるが型不明瞭) |
| `ZipLatest[A,B]` / `ZipLatestWith` | `scaladsl/Graph.scala:820` | 未対応 | core | medium | 最新値での Zip |

### 5. ライフサイクル　✅ 実装済み 10/14 (71%)

fraktor-rs は `KillSwitches` / `UniqueKillSwitch` / `SharedKillSwitch` / `KillSwitch` trait / `RestartSource/Flow/Sink` / `RestartConfig` (Pekko の RestartSettings 相当) / `MergeHub` / `BroadcastHub` / `PartitionHub` / `DrainingControl` を実装済み。以下はまだ不足。

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `KillableGraphStageLogic` | `KillSwitch.scala:65` | 未対応 | core | medium | KillSwitch 連携ステージ基底。GraphStage 側と同時設計 |
| `watchTermination` メソッド | Flow / Source | 部分実装 | core | easy | `watch_termination_mat` はあるが契約 (Future<Done>) の統一が必要 |
| `SharedKillSwitch.flow[T]` / `flow[Bidi]` | `KillSwitch.scala:220` | 部分実装 | core | easy | fraktor-rs は `KillSwitches::shared` だが bidi 版が未確認 |
| `RestartSettings.withMaxBackoff` 詳細 API | `RestartSettings.scala` | 部分実装 | core | trivial | `RestartConfig` の builder メソッド差分 |

### 6. エラー処理　✅ 実装済み 9/16 (56%)

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `AbruptStreamTerminationException` | `ActorMaterializer.scala:50` | 未対応 | core | easy | `StreamDslError` に variant 追加 |
| `AbruptStageTerminationException` | `ActorMaterializer.scala:65` | 未対応 | core | easy | 同上 |
| `StreamLimitReachedException` | `StreamLimitReachedException.scala:1` | 未対応 | core | easy | `limit` / `limitWeighted` 超過用 |
| `StreamTimeoutException` | `StreamTimeoutException.scala:1` | 未対応 | core | easy | `*_timeout` オペレーター導入時に必要 |
| `NeverMaterializedException` | `NeverMaterializedException.scala:1` | 未対応 | core | easy | マテリアライズされなかった Mat 値アクセス用 |
| `WatchedActorTerminatedException` | `WatchedActorTerminatedException.scala:1` | 未対応 | core | easy | ActorRef watch 終了用 |
| `TooManySubstreamsOpenException` | `TooManySubstreamsOpenException.scala:1` | 未対応 | core | easy | `groupBy` 上限超過 |
| `Supervision.Directive` | `Supervision.scala:22` | 別名で実装済み | core | n/a | `SupervisionStrategy` enum (Stop/Resume/Restart) で代替 |
| `RetryFlow.withBackoffAndContext` | `RetryFlow.scala:80` | 未対応 | core | medium | コンテキスト付きバックオフ再試行 |

### 7. Stage Authoring　✅ 実装済み 9/18 (50%)

fraktor-rs は `GraphStage` / `GraphStageLogic` / `StageContext` / `StreamStage` / `StageKind` / `TimerGraphStageLogic` / `AsyncCallback` / `DemandTracker` / `StreamBufferConfig` を実装済み。Pekko の Handler 系と Sub 系、StageActorRef 連携、標準ロガーが未整備。

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `GraphStageWithMaterializedValue[S,M]` | `stage/GraphStage.scala:52` | 未対応 | core | medium | Mat 値付きステージ。materializer 拡張が必要 |
| `InHandler` (trait) | `stage/GraphStage.scala:1870` | 部分実装 | core | easy | Rust 側は関数ポインタで扱っているが trait に統一する必要 |
| `OutHandler` (trait) | `stage/GraphStage.scala:1895` | 部分実装 | core | easy | 同上 |
| `StageActorRef` | `stage/GraphStage.scala:241` | 未対応 | core | hard | ステージ内 actor ref。actor-core との双方向連携が必要 |
| `SubSinkInlet[T]` | `stage/GraphStage.scala:1451` | 未対応 | core | medium | ネストグラフ入力 |
| `SubSourceOutlet[T]` | `stage/GraphStage.scala:1532` | 未対応 | core | medium | ネストグラフ出力 |
| `StageLogging` | `stage/StageLogging.scala:32` | 未対応 | core | easy | Logger mixin |
| `EagerTerminateInput` | `stage/GraphStage.scala:1880` | 未対応 | core | trivial | 標準 InHandler |
| `IgnoreTerminateInput` | `stage/GraphStage.scala:1883` | 未対応 | core | trivial | 標準 InHandler |
| `TotallyIgnorantInput` | `stage/GraphStage.scala:1886` | 未対応 | core | trivial | 標準 InHandler |
| `EagerTerminateOutput` | `stage/GraphStage.scala:1910` | 未対応 | core | trivial | 標準 OutHandler |
| `IgnoreTerminateOutput` | `stage/GraphStage.scala:1913` | 未対応 | core | trivial | 標準 OutHandler |
| `MaterializerLoggingProvider` | `MaterializerLoggingProvider.scala:1` | 未対応 | core | easy | Materializer のロガー抽象 |

### 8. Attributes　✅ 実装済み 10/16 (63%)

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `Attributes.Name` | `Attributes.scala:330` | 部分実装 | core | trivial | `Flow::named` はあるが Attribute としての公開型が不明瞭 |
| `Attributes.MandatoryAttribute` | `Attributes.scala:325` | 未対応 | core | easy | 必須属性のマーカー trait |
| `Attributes.NestedMaterializationCancellationPolicy` | `Attributes.scala:520` | 未対応 | core | medium | ネストマテリアライズのキャンセルポリシー |
| `Attributes.SourceLocation` | `Attributes.scala:340` | 未対応 | core | easy | デバッグ用ソース位置 |
| `ActorAttributes.SupervisionStrategy` | `Attributes.scala:760` | 別名で実装済み | core | n/a | fraktor-rs は `SupervisionStrategy` enum で代替 (attribute 化はしていない) |
| `StreamSubscriptionTimeoutSettings` | `ActorMaterializer.scala:90` | 部分実装 | core | trivial | `SubscriptionTimeoutConfig` で近似。API 名前合わせ |
| `StreamSubscriptionTimeoutTerminationMode` | `ActorMaterializer.scala:110` | 別名で実装済み | core | n/a | `SubscriptionTimeoutMode` enum で代替 |

### 9. Shape　✅ 実装済み 14/16 (88%)

fraktor-rs は FanInShape2〜22 (20 種) / UniformFanInShape / UniformFanOutShape / BidiShape / FanOutShape2 を備える。ギャップ:

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `FanInShape1[-T0,+O]` | `FanInShapeN.scala.template` | 未対応 | core | trivial | 実質 FlowShape と同等だが契約整合のため |
| `FanOutShape3〜22` | `FanOutShapeN.scala.template` | 未対応 | core | easy | FanOutShape2 のパターンを 3〜22 まで拡張 (ボイラープレート) |

### 10. IO　✅ 実装済み 15/20 (75%)

fraktor-rs 側:
- `stream-core`: `IOResult`, `Compression`, `Source::from_path`, `Sink::to_path`, `Source::from_input_stream`, `Sink::into_output_stream` 等
- `stream-adaptor-std`: `FileIO` (from_path / from_path_with_options / to_path / to_path_with_options / to_path_with_position), `StreamConverters` (from_reader / to_writer), `SourceFactory`

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `StreamConverters.fromInputStream` | `scaladsl/StreamConverters.scala:45` | 部分実装 | std | trivial | `from_reader` はあるが `InputStream` と契約が違う (API 名寄せ) |
| `StreamConverters.asOutputStream` | `scaladsl/StreamConverters.scala:70` | 未対応 | std | easy | OutputStream として書き込む Sink |
| `StreamConverters.fromOutputStream` | `scaladsl/StreamConverters.scala:100` | 未対応 | std | easy | OutputStream からの Source |
| `StreamConverters.asInputStream` | `scaladsl/StreamConverters.scala:120` | 未対応 | std | easy | InputStream として読む Sink |
| `StreamConverters.javaCollector` | `scaladsl/StreamConverters.scala:145` | n/a | — | n/a | Java Collector 互換 (JVM 固有) |
| `StreamConverters.asJavaStream` | `scaladsl/StreamConverters.scala:180` | n/a | — | n/a | Java Stream 変換 (JVM 固有) |
| `StreamConverters.fromJavaStream` | `scaladsl/StreamConverters.scala:200` | n/a | — | n/a | Java Stream 取込 (JVM 固有) |

### 11. Snapshot / Serialization　✅ 実装済み 2/10 (20%)

fraktor-rs は `MaterializerSnapshot` 型が存在するのみ。Pekko の `MaterializerState` API 群による実行中 interpreter の状態取得は未整備。

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `MaterializerState.streamSnapshots(system)` | `snapshot/MaterializerState.scala:45` | 未対応 | core + std | hard | ActorSystem から全ストリームのスナップショットを採集。actor-core との連携必須 |
| `MaterializerState.streamSnapshots(mat)` | `snapshot/MaterializerState.scala:55` | 未対応 | core | medium | 単一 materializer 版 |
| `StreamSnapshot` | `snapshot/MaterializerState.scala:93` | 未対応 | core | easy | スナップショット基底 |
| `InterpreterSnapshot` | `snapshot/MaterializerState.scala:113` | 未対応 | core | easy | interpreter 状態 |
| `RunningInterpreter` | `snapshot/MaterializerState.scala:129` | 未対応 | core | medium | 実行中 interpreter のランタイム情報 (stages/ports/handlers) |
| `UninitializedInterpreter` | `snapshot/MaterializerState.scala:123` | 未対応 | core | easy | 未初期化 interpreter |
| `LogicSnapshot` | `snapshot/MaterializerState.scala:156` | 未対応 | core | easy | ステージロジック単位のスナップショット |
| `ConnectionSnapshot` | `snapshot/MaterializerState.scala:162` | 未対応 | core | easy | ポート接続状態 |
| `StreamRefSerializer` | `serialization/StreamRefSerializer.scala:1` | 部分実装 | core + std | medium | 内部実装はあるが公開 API 化されていない。StreamRef 実装と同時に必要 |

### 12. Testing / Probes　✅ 実装済み 3/2 (100%)

Pekko 本体 (non-testkit) には Testing API はほぼない (StreamRefs ファクトリと StreamRefResolver のみ)。
fraktor-rs は独自に `TestSinkProbe` / `TestSourceProbe` / `StreamFuzzRunner` を core に持っており、Pekko より充実している。
ギャップなし (むしろ Pekko の `stream-testkit` モジュール分まで含んでいる)。

### 13. その他　✅ 実装済み 14/18 (78%)

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `Tcp` (object) | `scaladsl/Tcp.scala:1` | 未対応 | std | hard | TCP bind/outgoing/connection の Source/Flow/Sink。tokio::net 依存 |
| `TLS` (object) | `scaladsl/TLS.scala:1` | 未対応 | std | hard | TLS セッション管理。rustls/native-tls 依存 |
| `CoupledTerminationFlow` | `scaladsl/CoupledTerminationFlow.scala:1` | 未対応 | core | easy | 入出力の終了を連動 |
| `DelayOverflowStrategy` | `OverflowStrategy.scala:28` | 部分実装 | core | trivial | `OverflowStrategy` に variant 追加のみ |
| `BufferOverflowException` | `OverflowStrategy.scala:34` | 未対応 | core | trivial | `StreamDslError` に variant 追加 |
| `SubscriptionWithCancelException` | `SubscriptionWithCancelException.scala:1` | 未対応 | core | trivial | 同上 |
| `StreamDetachedException` | `StreamDetachedException.scala:1` | 未対応 | core | trivial | 同上 |
| `FramingException` | `scaladsl/Framing.scala:159` | 部分実装 | core | trivial | `StreamDslError::Framing(...)` 等がある可能性。命名整合 |
| `StreamTcpException` | `StreamTcpException.scala:1` | 未対応 | std | trivial | Tcp 実装時に同時追加 |
| `TargetRefNotInitializedException` | `stream/StreamRefs.scala:100` | 未対応 | core | trivial | SinkRef 実装時に同時追加 |
| `StreamRefSubscriptionTimeoutException` | `stream/StreamRefs.scala:108` | 未対応 | core | trivial | 同上 |

## 内部モジュール構造ギャップ

**今回は API ギャップが支配的なため省略**。判定理由:
- カテゴリ単位の平均カバレッジ 72% (閾値 80% 未満)
- hard ギャップが 6 件 (閾値 5 件超過): StreamRef × 2, Tcp, TLS, GraphDSL core, MaterializerState スナップショット, StageActorRef
- Stage Authoring / Snapshot の基盤カテゴリが 50% 以下 (致命的カテゴリで 2 件以上欠落)

構造比較は StreamRef / Tcp / MaterializerState / Stage Authoring が parity 達成した後の後続フェーズで実施する。

## 実装優先度

分類ルール:
- Phase 1: trivial / easy。既存設計の範囲で API surface や placeholder を埋められるもの
- Phase 2: medium。追加ロジックは要るが、既存の core / std 境界の中で閉じるもの
- Phase 3: hard。新規基盤やアーキテクチャ変更を要するもの

### Phase 1 (trivial / easy) — 既存設計の拡張

**Stage Authoring 補助 (core)**:
- `EagerTerminateInput` / `IgnoreTerminateInput` / `TotallyIgnorantInput` (trivial)
- `EagerTerminateOutput` / `IgnoreTerminateOutput` (trivial)
- `InHandler` / `OutHandler` を trait 化 (easy)
- `StageLogging` / `MaterializerLoggingProvider` (easy)

**オペレーター (core)**:
- `intersperse` / `alsoTo` / `alsoToAll` / `divertTo` / `orElse` (easy)
- `backpressureTimeout` / `completionTimeout` / `idleTimeout` / `initialTimeout` / `keepAlive` (easy、`StreamTimeoutException` と同時導入)

**Shape (core)**:
- `FanInShape1` (trivial)
- `FanOutShape3〜22` ボイラープレート拡張 (easy)

**エラー型 (core)**:
- `AbruptStreamTerminationException` / `AbruptStageTerminationException` / `StreamLimitReachedException` / `StreamTimeoutException` / `NeverMaterializedException` / `WatchedActorTerminatedException` / `TooManySubstreamsOpenException` を `StreamDslError` に variant として追加 (easy)
- `BufferOverflowException` / `SubscriptionWithCancelException` / `StreamDetachedException` / `FramingException` の命名整合 (trivial)

**Attributes (core)**:
- `Attributes.Name` を公開型として整理 (trivial)
- `Attributes.MandatoryAttribute` trait (easy)
- `Attributes.SourceLocation` (easy)

**IO (std)**:
- `StreamConverters.fromInputStream` の命名整合 (trivial)
- `StreamConverters.asOutputStream` / `fromOutputStream` / `asInputStream` (easy)

**Snapshot (core)**:
- `StreamSnapshot` / `InterpreterSnapshot` / `UninitializedInterpreter` / `LogicSnapshot` / `ConnectionSnapshot` の型骨格 (easy)

**ライフサイクル (core)**:
- `SharedKillSwitch.flow` の bidi 版 (easy)
- `RestartSettings` の builder API 差分 (trivial)
- `watchTermination` 契約の Future<Done> 整合 (easy)

**型・トレイト (core)**:
- `SourceQueueWithComplete[T]` の契約仕上げ (trivial)
- `SinkQueueWithCancel[T]` (easy)

**その他 (core)**:
- `CoupledTerminationFlow` (easy)
- `DelayOverflowStrategy` variant 追加 (trivial)

### Phase 2 (medium) — 新規ロジック・既存境界内

**オペレーター (core)**:
- `conflate` / `conflateWithSeed` (medium)
- `expand` / `extrapolate` (medium)
- `switchMap` (medium)
- `mergeLatest` (medium)
- `mergePreferred` (medium)
- `mapAsyncPartitioned` (medium)
- `aggregateWithBoundary` (medium)

**Graph DSL (core)**:
- `MergeSequence` (medium)
- `Unzip` (easy だが GraphDSL 整備と同時対応が望ましく Phase 2 に配置)
- `ZipLatest` / `ZipLatestWith` (medium)

**Stage Authoring (core)**:
- `GraphStageWithMaterializedValue[S,M]` (medium)
- `SubSinkInlet[T]` / `SubSourceOutlet[T]` (medium)

**Snapshot (core)**:
- `MaterializerState.streamSnapshots(mat)` 単一 materializer 版 (medium)
- `RunningInterpreter` の詳細ランタイム情報 (medium)
- `StreamRefSerializer` の公開 API 化 (medium)

**エラー処理 (core)**:
- `RetryFlow.withBackoffAndContext` (medium)

**Attributes (core)**:
- `Attributes.NestedMaterializationCancellationPolicy` (medium)

**ライフサイクル (core)**:
- `KillableGraphStageLogic` (medium、Stage Authoring と同時設計)

**StreamRef 基盤 (core)**:
- `StreamRefResolver` (medium) — シリアライザ層だけ先行

### Phase 3 (hard) — 新基盤・アーキテクチャ変更

- **`SinkRef[In]` / `SourceRef[T]`** (core + std, hard) — 分散 stream 参照。remote-core のワイヤープロトコルとシリアライザに依存。`StreamRefSerializer` の公開化と bootstrapping が必要
- **`Tcp` (std, hard)** — tokio::net 依存。`Tcp.bind` / `outgoingConnection` / `IncomingConnection` / `OutgoingConnection` の Source/Flow/Sink
- **`TLS` (std, hard)** — rustls / native-tls 依存。Tcp と連携した SessionBidiFlow
- **`GraphDSL.create` / `~>` 相当の公開配線 DSL (core, hard)** — 型安全かつ Rust イディオム準拠の配線 API 設計。Pekko の implicit + case class + SyntaxBuilder 依存を Rust trait + builder に落とし込む必要
- **`MaterializerState.streamSnapshots(system)` (core + std, hard)** — ActorSystem 全体のストリーム採集。actor-core の extension 機構と interpreter 側の snapshot 同期が必要
- **`StageActorRef` (core, hard)** — ステージ内 actor ref。actor-core の ActorCell / TypedActorRef と Stage Authoring の双方向連携

### Phase 対象外 (n/a)

- `FlowOps` / `FlowOpsMat` / `FlowWithContextOps` (Scala trait 階層固有、Rust は `impl Flow` で等価)
- `ActorAttributes.SupervisionStrategy` (`SupervisionStrategy` enum で代替済み)
- `StreamSubscriptionTimeoutTerminationMode` (`SubscriptionTimeoutMode` enum で代替済み)
- `Supervision.Directive` (`SupervisionStrategy` enum で代替済み)
- `StreamConverters.javaCollector` / `asJavaStream` / `fromJavaStream` (JVM 固有)

## まとめ

- **全体カバレッジ 約 72%**。Source / Flow / Sink の基本 DSL と主要オペレーター (map / filter / scan / fold / merge / zip / concat / flatMap / groupBy / restart / retry) はほぼ実装済み。fraktor-rs 側の公開メソッド数は 671 件で Pekko のオペレーター総数 (約 140) を大きく上回り、実用的な API surface は既に充実している。
- **parity を低コストで前進できる領域 (Phase 1〜2)**: Pekko 固有オペレーター (`conflate` / `expand` / `intersperse` / `alsoTo` / `orElse` / `switchMap` / `*_timeout` / `keepAlive`) の追加、Stage Authoring 標準 Handler (`EagerTerminate*` / `IgnoreTerminate*`)、Shape のボイラープレート拡張 (`FanOutShape3〜22`)、エラー型の `StreamDslError` variant 充実、`SubSinkInlet` / `SubSourceOutlet` による Stage authoring の底上げ。ここで 15〜20% のカバレッジ上昇が見込める。
- **parity 上の主要ギャップ (Phase 3)**: StreamRef (SinkRef/SourceRef) による分散 stream 参照、Tcp/TLS ネットワーク IO、明示的 GraphDSL、MaterializerState 実行中スナップショット API、StageActorRef による actor-stream 連携の 5 本柱。いずれも remote-core / actor-core / std アダプタとの協調設計が必要で、fraktor-rs の他モジュール parity と並走する形で進める必要がある。
- **次のボトルネックは内部構造ではなく API ギャップ**。カバレッジが 80% を超えるまでは、公開 API 追加と Stage Authoring 基盤の強化を優先する。内部 `impl/fusing` / `impl/interpreter` / `impl/materialization` の責務境界レビューは Phase 3 完了後に実施することを推奨する。

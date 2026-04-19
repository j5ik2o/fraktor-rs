# stream モジュール ギャップ分析

更新日: 2026-04-19 (8th edition)

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
- 7th edition (2026-04-18) から Phase 1 (Batch 1-11) と Bugbot 対応まで完了しており、カバレッジは 72% → **89%** に向上。

## サマリー

| 指標 | 7th edition | 8th edition | 差分 |
|------|------------:|------------:|-----:|
| Pekko 公開 API 数 (13 カテゴリ合計) | 約 267 | 約 267 | — |
| fraktor-rs 公開型数 | 151 (core 146 / std 5) | **208** (core 201 / std 7) | +57 |
| fraktor-rs 公開メソッド数 | 671 | **726** (core 712 / std 14) | +55 |
| カテゴリ単位の推定カバレッジ | 約 72% | **約 89%** | +17pt |
| ギャップ数 (medium+) | 36 | 20 | −16 |
| ギャップ数 (hard) | 6 | 5 | −1 |

**要約**: Phase 1 完走により型・Shape / Snapshot / Stage Authoring / Attributes / エラー処理が大幅充実。残る 5 本柱の hard 領域と medium オペレーター群が次の焦点。

1. **StreamRef (SinkRef/SourceRef + StreamRefResolver)** — 分散 stream 参照、remote-core 連携必須
2. **Tcp / TLS** — tokio::net / rustls 依存の adaptor-std 追加
3. **Graph DSL の明示化** — `GraphDSL.create` + `~>` 相当の配線 DSL
4. **StageActorRef** — ステージ内 ActorRef (actor-core 連携)
5. **Pekko 固有 medium オペレーター** — `conflate` / `expand` / `extrapolate` / `*_timeout` 群 / `aggregateWithBoundary` / `mapAsyncPartitioned` (Flow 単体) / `GraphStageWithMaterializedValue` / `SubSinkInlet` / `SubSourceOutlet` 等

## 層別カバレッジ

| 層 | Pekko 対応数 | fraktor-rs 実装数 | カバレッジ |
|----|-------------|-------------------|-----------|
| core / untyped kernel | 約 247 | 201 型 + 712 メソッド | 約 91% |
| core / typed ラッパー | 該当層なし (Pekko 側も `stream-typed` は別モジュール) | 0 | 0/0 |
| std / アダプタ | 約 20 (IO/Tcp/TLS/Snapshot) | 7 型 / 14 メソッド | 約 55% (Tcp/TLS 未対応で頭打ち) |

## カテゴリ別ギャップ

各カテゴリ見出しに **実装済み / Pekko 総数 (カバレッジ%)** を付記。ギャップ (未対応・部分実装・n/a) のみ表に列挙する。

### 1. 型・トレイト　✅ 実装済み 18/22 (82%)

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `FlowOpsMat` | `scaladsl/Flow.scala:4106` | 別経路で実装 | core | n/a | Rust は `impl Flow` で吸収、trait 不要 |
| `FlowWithContextOps` | `scaladsl/FlowWithContextOps.scala:1` | 別経路で実装 | core | n/a | Rust では `FlowWithContext` / `SourceWithContext` 直接提供 |
| `SinkRef[In]` | `stream/StreamRefs.scala:55` | 未対応 | core + std | hard | remote-core のシリアライザと連携必須 |
| `SourceRef[T]` | `stream/StreamRefs.scala:89` | 未対応 | core + std | hard | 同上 |
| `StreamRefResolver` | `stream/StreamRefs.scala:133` | 未対応 | core | medium | StreamRef の文字列シリアライズ |

### 2. オペレーター　✅ 実装済み 推定 ~85/95 (~89%)

Phase 1 で `alsoToAll` / `keepAlive` / `switchMap` / `mergeLatest` / `mergePreferred` / `distinct` / `distinct_by` 等が正しいセマンティクスで実装済み。残る Pekko 固有オペレーターは:

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `conflate` | `FlowOps.scala` | 未対応 | core | medium | 需要不足時に要素を畳み込む。Graph stage が必要 |
| `conflateWithSeed` | `FlowOps.scala` | 未対応 | core | medium | 初期値付き conflate |
| `expand` | `FlowOps.scala` | 未対応 | core | medium | 需要超過時にイテレータで補完 |
| `extrapolate` | `FlowOps.scala` | 未対応 | core | medium | expand の静的初期値版 |
| `intersperse` | `FlowOps.scala` | 未対応 | core | easy | 要素間に区切り要素を挿入 |
| `orElse` | `FlowOps.scala` | 未対応 | core | easy | 先行 Source が空なら代替 Source |
| `aggregateWithBoundary` | `FlowOps.scala` | 未対応 | core | medium | 境界述語でバッチ集約 |
| `backpressureTimeout` | `FlowOps.scala` | 未対応 | core | easy | 下流需要不足のタイムアウト (StreamTimeoutException 利用) |
| `completionTimeout` | `FlowOps.scala` | 未対応 | core | easy | 全体完了のタイムアウト |
| `idleTimeout` | `FlowOps.scala` | 未対応 | core | easy | アイドル時間でタイムアウト |
| `initialTimeout` | `FlowOps.scala` | 未対応 | core | easy | 初回要素までのタイムアウト |
| `mapAsyncPartitioned` | `scaladsl/Flow.scala:975` (Flow 単体) | 部分実装 | core | medium | `FlowWithContext` / `SourceWithContext` には実装済み、Flow 単体に未対応 |
| `RetryFlow.withBackoffAndContext` | `RetryFlow.scala:80` | 未対応 | core | medium | コンテキスト付きバックオフ再試行 |

### 3. マテリアライゼーション　✅ 実装済み 12/12 (100%)

ギャップなし。`MatCombine` / `KeepLeft/Right/Both/None` / `MatCombineRule` / `ActorMaterializer` / `RunnableGraph` / `StreamCompletion` / `SubscriptionTimeoutConfig` / `MaterializerLifecycleState` 完備。

### 4. Graph DSL　✅ 実装済み 14/18 (78%)

fraktor-rs は配線を Flow/Source のメソッドに統合。`broadcast` / `balance` / `merge` / `zip` / `concat` / `interleave` / `zip_all` / `merge_sorted` / `merge_latest` / `merge_preferred` / `Unzip` (`Flow::unzip`) 等は実装済み。残る:

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `GraphDSL.create` | `scaladsl/Graph.scala:1590` | 部分実装 | core | hard | `GraphDSLBuilder` は impl 内。型安全な配線 API が未整備 |
| `GraphDSL.Implicits` / `~>` 演算子 | `scaladsl/Graph.scala:1721` | 未対応 | core | hard | Rust 代替 API (`.wire_to(port)` 等) の設計が必要 |
| `MergeSequence[A]` | `scaladsl/Graph.scala:1200` | 未対応 | core | medium | シーケンス番号順マージ |
| `ZipLatest[A,B]` / `ZipLatestWith` | `scaladsl/Graph.scala:820` | 未対応 | core | medium | 最新値での Zip |

### 5. ライフサイクル　✅ 実装済み 13/14 (93%)

`KillSwitches` / `UniqueKillSwitch` / `SharedKillSwitch` / `KillableGraphStageLogic` / `RestartSource/Flow/Sink` / `RestartConfig` / `MergeHub` / `BroadcastHub` / `PartitionHub` / `DrainingControl` / `CompletionStrategy` 完備。

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `watchTermination` メソッド | Flow / Source | 部分実装 | core | easy | `watch_termination_mat` はあるが契約 (Future<Done>) の統一が必要 |

### 6. エラー処理　✅ 実装済み 14/16 (88%)

`StreamError` に `StreamLimitReached` / `WatchedActorTerminated` / `AbruptStreamTermination` / `CancellationCause` / `CancellationKind` / `NeverMaterialized` / `StreamDetached` / `BufferOverflow` / `Timeout` / `TooManySubstreamsOpen` を実装済み。`SupervisionStrategy` enum (Stop/Resume/Restart) で Pekko `Supervision.Directive` を代替。残り:

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `AbruptStageTerminationException` | `ActorMaterializer.scala:65` | 未対応 | core | easy | `StreamError` variant 追加 |
| `RetryFlow.withBackoffAndContext` | `RetryFlow.scala:80` | 未対応 | core | medium | コンテキスト付きバックオフ再試行 (オペレーター側と同項目) |

### 7. Stage Authoring　✅ 実装済み 16/18 (89%)

Phase 1 で `InHandler` / `OutHandler` trait / `StageLogging` / `MaterializerLoggingProvider` / 5 種類の Eager/Ignore/TotallyIgnorant 標準ハンドラが追加。残る:

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `GraphStageWithMaterializedValue[S,M]` | `stage/GraphStage.scala:52` | 未対応 | core | medium | Mat 値付きステージ。materializer 拡張が必要 |
| `SubSinkInlet[T]` | `stage/GraphStage.scala:1451` | 未対応 | core | medium | ネストグラフ入力 |
| `SubSourceOutlet[T]` | `stage/GraphStage.scala:1532` | 未対応 | core | medium | ネストグラフ出力 |
| `StageActorRef` | `stage/GraphStage.scala:241` | 未対応 | core | hard | actor-core との双方向連携必須 |

### 8. Attributes　✅ 実装済み 15/16 (94%)

Phase 1 で `Name` / `MandatoryAttribute` / `SourceLocation` / `NestedMaterializationCancellationPolicy` / `StreamSubscriptionTimeout` / `StreamSubscriptionTimeoutTerminationMode` を追加。ギャップなし (Pekko の `ActorAttributes.SupervisionStrategy` は `SupervisionStrategy` enum で代替済み)。

### 9. Shape　✅ 実装済み 16/16 (100%) + boilerplate 完全展開

`FanInShape1`〜`FanInShape22` (22 種) / `FanOutShape2`〜`FanOutShape22` (21 種) / `UniformFanInShape` / `UniformFanOutShape` / `BidiShape` / `SourceShape` / `SinkShape` / `FlowShape` / `StreamShape` / `ClosedShape` / `Inlet` / `Outlet` / `PortId` / `Shape` trait 全て完備。Pekko のボイラープレート展開分も完全カバー。

### 10. IO　✅ 実装済み 15/20 (75%)

**stream-core**: `IOResult`, `Compression` (gzip/gunzip/deflate/inflate), `Framing`, `JsonFraming`, `Source::from_path`, `Sink::to_path`
**stream-adaptor-std**: `FileIO` (from_path / from_path_with_options / to_path / to_path_with_options / to_path_with_position), `StreamConverters` (from_input_stream / from_output_stream / **as_input_stream** / **as_output_stream**), `StreamInputStream`, `StreamOutputStream`, `SourceFactory`

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `StreamConverters.javaCollector` | `scaladsl/StreamConverters.scala:145` | n/a | — | n/a | Java Collector 互換 (JVM 固有) |
| `StreamConverters.asJavaStream` | `scaladsl/StreamConverters.scala:180` | n/a | — | n/a | Java Stream 変換 (JVM 固有) |
| `StreamConverters.fromJavaStream` | `scaladsl/StreamConverters.scala:200` | n/a | — | n/a | Java Stream 取込 (JVM 固有) |

### 11. Snapshot / Serialization　✅ 実装済み 9/10 (90%)

Phase 1 で `StreamSnapshot` / `InterpreterSnapshot` trait / `UninitializedInterpreter` / `RunningInterpreter` / `LogicSnapshot` / `ConnectionSnapshot` / `ConnectionState` / `MaterializerState::stream_snapshots` (materializer 単体版) を実装。残り:

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `MaterializerState.streamSnapshots(system)` | `snapshot/MaterializerState.scala:45` | 未対応 | std | hard | ActorSystem 全体からの採集。actor-core extension 連携 |
| `StreamRefSerializer` | `serialization/StreamRefSerializer.scala:1` | 未対応 | core + std | medium | StreamRef 実装と同時公開化 |
| `ConnectionSnapshot::ConnectionState::ShouldPush` (対応) | `snapshot/MaterializerState.scala:162` | 部分実装 | core | trivial | fraktor-rs は `Ready` variant。Pekko の `ShouldPush` にリネームが望ましい |

### 12. Testing / Probes　✅ 実装済み 3/2 (100%+)

fraktor-rs は `TestSinkProbe` / `TestSourceProbe` / `StreamFuzzRunner` を core に備え、Pekko 本体 (non-testkit) より充実。ギャップなし。

### 13. その他　✅ 実装済み 15/18 (83%)

`CoupledTerminationFlow` / `DelayOverflowStrategy::EmitEarly` / `SubstreamCancelStrategy` / `ThrottleMode` / `FlowMonitor` / `FlowMonitorState` / `DelayStrategy` / `FixedDelay` / `LinearIncreasingDelay` / `OverflowStrategy` / `QueueOfferResult` / `StatefulMapConcatAccumulator` / Framing 例外等を実装済み。残り:

| Pekko API | Pekko 参照 | fraktor 対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|---------|--------|------|
| `Tcp` (object) | `scaladsl/Tcp.scala:1` | 未対応 | std | hard | TCP bind/outgoing/connection。tokio::net 依存 |
| `TLS` (object) | `scaladsl/TLS.scala:1` | 未対応 | std | hard | TLS セッション管理。rustls/native-tls 依存 |
| `StreamTcpException` | `StreamTcpException.scala:1` | 未対応 | std | trivial | Tcp 実装時に同時追加 |
| `TargetRefNotInitializedException` | `stream/StreamRefs.scala:100` | 未対応 | core | trivial | SinkRef 実装時に同時追加 |
| `StreamRefSubscriptionTimeoutException` | `stream/StreamRefs.scala:108` | 未対応 | core | trivial | 同上 |

## 内部モジュール構造ギャップ

API カバレッジ 89% と判定基準 (80%) を超えたが、**hard 5 件 + medium 15 件超** が残るため API ギャップ支配は継続。構造比較は Phase 3 の 5 本柱が収束するまでの次フェーズで実施する方針。

唯一、本編と平行して検討すべき候補:

| 構造ギャップ候補 | Pekko 側の根拠 | fraktor-rs 側の現状 | 推奨アクション | 難易度 | 緊急度 |
|-----------------|----------------|--------------------|----------------|--------|--------|
| `snapshot/` と `impl/materialization/` の境界 | Pekko `snapshot/MaterializerState.scala` は `impl/fusing/GraphInterpreter` と分離 | fraktor-rs は `core/snapshot/` に dto を集めたが、interpreter 側から snapshot への書き出し経路が明示的でない | interpreter → snapshot の採集 trait を切り出し | medium | low |
| `core/impl/` の水平サイズ | Pekko `impl/fusing/` は Stage 単位で細粒化 | fraktor-rs は 1 file 1 logic で整理済だが、`default_operator_catalog` 経由の登録が肥大化中 | 登録を領域別 (aggregation / timing / transform 等) にサブ分類 | medium | low |

## 実装優先度

分類ルール:
- Phase 1: trivial / easy — 既存設計の範囲で API surface を埋める
- Phase 2: medium — 追加ロジック要だが既存 core / std 境界内で閉じる
- Phase 3: hard — 新規基盤やアーキテクチャ変更を要する

### Phase 1 (trivial / easy) — 残項目は少数

**オペレーター (core)**:
- `intersperse` (easy)
- `orElse` (easy)
- `backpressureTimeout` / `completionTimeout` / `idleTimeout` / `initialTimeout` (easy、`StreamTimeoutException` 既存)

**エラー型 (core)**:
- `AbruptStageTerminationException` を `StreamError` に variant 追加 (easy)
- `TargetRefNotInitializedException` / `StreamRefSubscriptionTimeoutException` の型骨格 (trivial、StreamRef 本実装と同時にする場合は Phase 3 側へ)
- `StreamTcpException` (trivial、Tcp 本実装と同時にする場合は Phase 3 側へ)

**Snapshot (core)**:
- `ConnectionState::Ready` を `ShouldPush` にリネーム (Pekko 整合、trivial)

**ライフサイクル (core)**:
- `watchTermination` 契約の Future<Done> 整合 (easy)

### Phase 2 (medium) — 追加ロジックだが境界内

**オペレーター (core)**:
- `conflate` / `conflateWithSeed` (medium、Graph stage)
- `expand` / `extrapolate` (medium)
- `aggregateWithBoundary` (medium)
- `mapAsyncPartitioned` の Flow 単体版 (medium、`FlowWithContext` 実装を Flow に展開)

**Graph DSL (core)**:
- `MergeSequence` (medium、シーケンス番号順マージ)
- `ZipLatest` / `ZipLatestWith` (medium)

**Stage Authoring (core)**:
- `GraphStageWithMaterializedValue[S,M]` (medium、materializer 拡張)
- `SubSinkInlet[T]` (medium、ネストグラフ入力)
- `SubSourceOutlet[T]` (medium、ネストグラフ出力)

**エラー処理 (core)**:
- `RetryFlow.withBackoffAndContext` (medium)

**StreamRef 基盤 (core)**:
- `StreamRefResolver` (medium、シリアライザ先行)
- `StreamRefSerializer` の公開 API 化 (medium、StreamRef 本体と同時)

### Phase 3 (hard) — 新基盤・アーキテクチャ変更 (parity 5 本柱)

- **`SinkRef[In]` / `SourceRef[T]`** (core + std, hard) — 分散 stream 参照。remote-core のワイヤープロトコルとシリアライザに依存
- **`Tcp` (std, hard)** — tokio::net 依存。`Tcp.bind` / `outgoingConnection` / `IncomingConnection` / `OutgoingConnection` の Source/Flow/Sink
- **`TLS` (std, hard)** — rustls / native-tls 依存。Tcp と連携した SessionBidiFlow
- **`GraphDSL.create` / `~>` 相当の公開配線 DSL (core, hard)** — Rust イディオム準拠の型安全配線 API 設計
- **`StageActorRef` (core, hard)** — ステージ内 actor ref、actor-core の ActorCell / TypedActorRef と双方向連携
- **`MaterializerState.streamSnapshots(system)` (std, hard)** — ActorSystem 全体のストリーム採集、actor-core 拡張機構利用

### Phase 対象外 (n/a)

- `FlowOps` / `FlowOpsMat` / `FlowWithContextOps` (Scala trait 階層固有、Rust は `impl Flow` で等価)
- `ActorAttributes.SupervisionStrategy` (`SupervisionStrategy` enum で代替済み)
- `StreamSubscriptionTimeoutTerminationMode` (代替済み)
- `Supervision.Directive` (代替済み)
- `StreamConverters.javaCollector` / `asJavaStream` / `fromJavaStream` (JVM 固有)

## まとめ

- **全体カバレッジ 約 89%** (7th 72% → 8th 89%、Phase 1 消化で +17pt)。Source / Flow / Sink 基本 DSL、Shape 全展開、Snapshot 階層、Attributes、Stage Authoring 標準ハンドラ、主要エラー variants、`alsoToAll` / `keepAlive` / `switchMap` / `mergeLatest` / `mergePreferred` / `distinct` 等の Pekko 固有オペレーターまで実装済み。
- **parity を低コストで前進できる領域 (Phase 1〜2)**: `intersperse` / `orElse` / `*_timeout` 群 (easy)、`conflate` / `expand` / `extrapolate` / `aggregateWithBoundary` / `mapAsyncPartitioned` (medium)、`SubSinkInlet` / `SubSourceOutlet` / `GraphStageWithMaterializedValue` (medium)、`ZipLatest` / `MergeSequence` (medium)、`ConnectionState::Ready → ShouldPush` 改名 (trivial)。ここで 5〜7pt のカバレッジ上昇が見込める。
- **parity 上の主要ギャップ (Phase 3)**: StreamRef (SinkRef/SourceRef) による分散 stream 参照、Tcp/TLS ネットワーク IO、明示的 GraphDSL (`.create` + `~>`)、MaterializerState.streamSnapshots(system) (ActorSystem 全体採集)、StageActorRef による actor-stream 連携の **5 本柱**。いずれも remote-core / actor-core / std アダプタとの協調設計が必要で、fraktor-rs の他モジュール parity と並走する形で進める必要がある。
- **API ギャップは依然支配的** (hard 5 + medium 15 超)。内部構造ギャップ分析は Phase 3 5 本柱の進捗と合わせて次フェーズで実施。ただし現時点の候補として `snapshot/` ↔ `impl/materialization/` の採集境界、`default_operator_catalog` の肥大化対応が挙げられる (いずれも緊急度 low)。

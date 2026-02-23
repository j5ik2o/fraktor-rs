# streams モジュール ギャップ分析

> 分析日: 2026-02-24
> 対象: `modules/streams/src/` vs `references/pekko/stream/src/`
> 前回分析: 2026-02-22（スタブ実装の評価を更新）

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | ~70（Shape, Graph stage, IO, Remote 等含む） |
| fraktor-rs 公開型数 | 58 |
| カバレッジ（型単位） | ~83%（直接対応する型ベース） |
| Flow オペレーター数（Pekko FlowOps） | ~90 |
| Flow オペレーター数（fraktor-rs） | ~145（Pekko 超） |
| Source オペレーター数（fraktor-rs） | ~116 |
| Sink ファクトリ数（fraktor-rs） | ~39 |
| 未実装ギャップ数 | 23 |

### 設計上の差異

- **Source オペレーター**: Pekko では `Source` が `FlowOps` を継承し全オペレーターを持つ。fraktor-rs では `Source` に厳選された ~116 メソッドのみ配置し、その他は `source.via(flow)` で合成する設計。これはギャップではなく意図的な設計判断。
- **実行モデル**: fraktor-rs は tick ベースの同期実行モデルを採用。Pekko の `FiniteDuration` パラメータは `ticks: u64` で代替。
- **エラーモデル**: Pekko の `Throwable` ベースは Rust の `Result<T, StreamError>` ベースに適応済み。

---

## カテゴリ別ギャップ

### 1. 変換オペレーター（FlowOps）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `debounce(duration)` | FlowOps | 未対応 | medium | タイマー統合が必要 |
| `sample(interval)` / `sample(n)` | FlowOps | 未対応 | medium | タイマー統合 or カウントベース |
| `distinct` | FlowOps | `drop_repeated`（連続のみ） | easy | HashSet ベースで全体重複排除。`drop_repeated` は連続重複のみ |
| `distinctBy(f)` | FlowOps | 未対応 | easy | `distinct` の変換関数版 |

### 2. Source ファクトリ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Source.fromGraph(g)` | Source.scala | 未対応 | easy | Graph → Source 変換 |
| `Source.fromMaterializer(f)` | Source.scala | 未対応 | medium | Materializer 依存の遅延生成 |
| `Source.preMaterialize()` | Source.scala | Sink のみ実装 | easy | `Sink.pre_materialize` と同パターン |

### 3. Flow ファクトリ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Flow.fromGraph(g)` | Flow.scala | 未対応 | easy | Graph → Flow 変換 |
| `Flow.fromMaterializer(f)` | Flow.scala | 未対応 | medium | Materializer 依存の遅延生成 |
| `Flow.fromProcessor(f)` | Flow.scala | 未対応 | n/a | Reactive Streams Processor。JVM 固有 |

### 4. Sink ファクトリ / メソッド

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Sink.fromGraph(g)` | Sink.scala | 未対応 | easy | Graph → Sink 変換 |
| `Sink.queue()` | Sink.scala | 未対応 | medium | Pull 型 Sink（SinkQueueWithCancel） |
| `Sink.contramap(f)` | Sink.scala | Flow のみ | trivial | `Sink` の入力型変換。`Flow.contramap` は実装済み |
| `Sink.asSubscriber()` | Sink.scala | 未対応 | easy | Reactive Streams Subscriber としての Sink |

### 5. 属性・メタデータシステム

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Attributes` 型 | Attributes.scala | 未対応 | hard | 汎用 Attributes 型の設計が必要 |
| `withAttributes(attr)` | Graph.scala | 未対応 | hard | Attributes 基盤に依存 |
| `addAttributes(attr)` | Graph.scala | 未対応 | hard | 同上 |
| `named(name)` | Graph.scala | 未対応 | medium | デバッグ用ステージ命名。Attributes 基盤に依存 |
| `ThrottleMode` (Shaping/Enforcing) | ThrottleMode.scala | 未対応 | easy | throttle の動作モード切替 |

### 6. KillSwitch 拡張

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `KillSwitches.singleBidi()` | KillSwitches.scala | 未対応 | easy | BidiFlow 用の UniqueKillSwitch |

### 7. Stream IO

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Framing.delimiter()` | Framing.scala | 未対応 | medium | バイトストリームのフレーム分割 |
| `Framing.lengthField()` | Framing.scala | 未対応 | medium | 長さフィールドベースのフレーミング |
| `Compression.gzip/gunzip` | Compression.scala | 未対応 | n/a | std 依存、no_std 環境では利用不可 |
| `Compression.deflate/inflate` | Compression.scala | 未対応 | n/a | 同上 |
| `Tcp.bind/outgoing` | Tcp.scala | 未対応 | n/a | JVM 固有のネットワーク IO |

### 8. リモートストリーミング

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `SourceRef[T]` | StreamRefs.scala | 未対応 | hard | リモート Source 参照 |
| `SinkRef[T]` | StreamRefs.scala | 未対応 | hard | リモート Sink 参照。cluster モジュール依存 |

### 9. コンテキスト伝播

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `FlowWithContext[Ctx, In, Out]` | FlowWithContext.scala | 部分実装 | medium | `as_flow_with_context()` で変換可能だが、専用型としての完全な API は未提供 |
| `SourceWithContext[Ctx, Out]` | SourceWithContext.scala | 部分実装 | medium | `as_source_with_context()` で変換可能 |

---

## 実装済み（Pekko に対してカバー済みの主要 API）

### コア型

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `Source[+Out, +Mat]` | `Source<Out, Mat>` | 完全 |
| `Flow[-In, +Out, +Mat]` | `Flow<In, Out, Mat>` | 完全 |
| `Sink[-In, +Mat]` | `Sink<In, Mat>` | 完全 |
| `BidiFlow[-I1,+O1,-I2,+O2,+Mat]` | `BidiFlow<InTop,OutTop,InBottom,OutBottom,Mat>` | 完全 |
| `RunnableGraph[+Mat]` | `RunnableGraph<Mat>` | 完全 |
| `SubFlow[+Out,+Mat,+F[+_],C]` | `SourceSubFlow<Out,Mat>`, `FlowSubFlow<In,Out,Mat>` | 別設計 |

### シェイプ型

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `Shape` / `Inlet[T]` / `Outlet[T]` | `Shape`, `Inlet<T>`, `Outlet<T>` | 完全 |
| `SourceShape` / `FlowShape` / `SinkShape` | 同名型 | 完全 |
| `BidiShape` / `ClosedShape` | 同名型 | 完全 |
| `PortId` | `PortId` | 完全 |

### カテゴリ別カバー状況

| カテゴリ | カバー状況 |
|----------|-----------|
| Source ファクトリ（30+） | ほぼ完全（empty, single, repeat, cycle, tick, unfold, future, queue, actor_ref 等） |
| Sink ファクトリ（30+） | ほぼ完全（foreach, fold, reduce, head, last, seq, count, exists, forall, pre_materialize 等） |
| 基本変換（map, filter, take, drop 等） | 完全 |
| 状態付き変換（scan, fold, reduce, statefulMap） | 完全 |
| 非同期変換（mapAsync, mapAsyncUnordered, mapAsyncPartitioned） | 完全 |
| タイミング（throttle, delay, keepAlive, timeout 系 4 種） | 完全 |
| 結合（merge, zip, concat, interleave 全変種） | 完全 |
| 分配（broadcast, balance, partition, unzip） | 完全 |
| エラー処理（recover, recoverWith, onError*, mapError） | 完全 |
| サブストリーム（groupBy, splitWhen, splitAfter） | 完全 |
| ライフサイクル（watch_termination_mat, monitor, KillSwitch） | 完全 |
| 再起動（restart_*_with_backoff, on_failures_with_backoff） | 完全 |
| Hub（BroadcastHub, MergeHub, PartitionHub, DrainingControl） | 完全 |
| 副作用（also_to, wire_tap, divert_to） | 完全（Flow のみ） |
| Ask パターン | 完全（ask, ask_with_status, ask_with_context, ask_with_status_and_context） |
| マテリアライゼーション（Keep*, MatCombineRule, mapMaterializedValue） | 完全 |
| Graph DSL（GraphDsl, GraphStage, GraphStageLogic） | 完全 |
| テスティング（TestSourceProbe, TestSinkProbe, StreamFuzzRunner） | 完全 |

### スタブ実装の改善状況（前回分析からの変更）

前回分析時にスタブだった以下のオペレーターは、その後の改善状況を確認する必要がある：

| オペレーター | 前回状態 | 備考 |
|-------------|---------|------|
| `conflate` / `conflate_with_seed` | スタブ | 同期モデルではレート差なし。実装あり |
| `expand` / `extrapolate` | スタブ | 同期モデルでは不要。Flow に存在 |
| `watch_termination` | **改善済み** | `watch_termination_mat()` として Source/Flow 両方に実装（PR #132, #133） |
| `merge_preferred` / `merge_prioritized` / `merge_sorted` | スタブ | 優先度ロジック未実装、merge に委譲 |
| `zip_latest` | スタブ | zip_all に委譲、Latest 保持なし |

### fraktor-rs 独自の追加機能

| 機能 | 備考 |
|------|------|
| `do_on_cancel`, `do_on_first` | Pekko にない副作用フック |
| `interleave_all` | 複数ストリームの一括インターリーブ |
| `merge_prioritized_n` | N 入力の優先度付きマージ |
| `grouped_weighted`, `grouped_weighted_within` | 重み付きグルーピング |
| `on_failures_with_backoff` | Source/Flow 両方に実装 |
| `StreamFuzzRunner` | ストリームのファズテスト |
| `OperatorCatalog/Contract/Coverage` | オペレーター互換性追跡システム |
| `DemandTracker` | 明示的なデマンド管理型 |
| `StreamState` (enum) | ストリームライフサイクル状態の型安全な表現 |
| `DriveOutcome` | tick 駆動のステップ実行結果 |
| `StreamCompletion<T>` / `Completion<T>` | ポーリングベースの完了監視 |
| `contramap` / `dimap` (Flow) | 関手・双関手操作 |
| `take_until` | 述語ベースの終了 |
| `fold_while` (Sink) | 条件付き fold |
| `exists` / `forall` (Sink) | 述語 Sink |

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- `Sink.contramap(f)` — Flow.contramap と同じパターンで入力型変換
- `ThrottleMode` — throttle の Shaping/Enforcing モード切替

### Phase 2: easy（単純な新規実装）

- `distinct` / `distinctBy` — HashSet ベースの全体重複排除
- `Source.fromGraph(g)` — Graph → Source 変換ファクトリ
- `Flow.fromGraph(g)` — Graph → Flow 変換ファクトリ
- `Sink.fromGraph(g)` — Graph → Sink 変換ファクトリ
- `Source.preMaterialize()` — Sink.pre_materialize と同パターン
- `KillSwitches.singleBidi()` — BidiFlow 用 UniqueKillSwitch
- `Sink.asSubscriber()` — Reactive Streams Subscriber

### Phase 3: medium（中程度の実装工数）

- `debounce(duration)` — タイマーステージとの統合
- `sample(interval)` — 定期サンプリング
- `named(name)` — ステージ命名（Attributes 基盤の簡易版でも可）
- `Source.fromMaterializer(f)` / `Flow.fromMaterializer(f)` — Materializer 遅延ファクトリ
- `Sink.queue()` — Pull 型 SinkQueue
- `Framing.delimiter()` / `Framing.lengthField()` — バイトストリームフレーミング
- `FlowWithContext` / `SourceWithContext` の完全な API 化

### Phase 4: hard（アーキテクチャ変更を伴う）

- `Attributes` 汎用システム（withAttributes, addAttributes, named の基盤）
- `SourceRef` / `SinkRef`（リモートストリーミング、cluster モジュール依存）

### 対象外（n/a）

- `Flow.fromProcessor()` — JVM Reactive Streams Processor 固有
- `Compression.gzip/deflate` — JVM IO 固有、no_std 制約
- `Tcp.bind/outgoing` — JVM ネットワーク IO 固有
- `TLS` — JVM セキュリティ基盤固有
- `AmorphousShape` — 動的ポート数（設計対象外）

---

## 総評

fraktor-rs の streams モジュールは **Pekko Streams の FlowOps オペレーターをほぼ網羅**しており、一部では Pekko を超える独自機能（ファズテスト、オペレーター互換性追跡、明示的デマンド管理）を持つ。型数ベースで ~83%、オペレーターベースでは Flow が ~95% 以上のカバレッジ。

主要なギャップは以下の 3 領域に集中：

1. **時間系オペレーター**（debounce, sample）— タイマー統合が必要
2. **ファクトリメソッド**（fromGraph, fromMaterializer）— Graph 基盤との接続
3. **インフラストラクチャ**（Attributes, Framing, StreamRefs）— 基盤レイヤーの設計が必要

現在の利用状況で必要性が低い場合は YAGNI 原則に従い、Phase 1-2 の trivial/easy 項目のみを優先的に実装するのが妥当。

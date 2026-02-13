# ギャップ分析: pekko-stream-must-compat-fraktor-streams-rs

## 前提
- 本分析は `.kiro/specs/pekko-stream-must-compat-fraktor-streams-rs/requirements.md` を対象に、既存実装 (`modules/streams`) と Pekko 参照実装 (`references/pekko`) の差分を整理したもの。
- `spec.json` 上で要件承認は未完了（`approvals.requirements.approved: false`）のため、ここでの内容は設計フェーズの意思決定材料として扱う。
- 本プロジェクト方針として、互換MUST達成のための破壊的変更は許容（後方互換性は非必須）。

## 1. 現状調査（既存アセット）

### 1.1 既存 Streams 実装の構造
- 公開 DSL は `Source/Flow/Sink` を中心に存在し、`group_by`/`split_when`/`split_after`/`flat_map_concat`/`flat_map_merge`/`merge_substreams`/`concat_substreams`/Hub/KillSwitch/Restart/Supervision が定義済み。
- ただし Substream 系 API は `SubFlow` ではなく、`Source<(Key, Out), _>` と `Source<Vec<Out>, _>`（Flow も同様）で表現される。
  - `modules/streams/src/core/source.rs:260`
  - `modules/streams/src/core/source.rs:277`
  - `modules/streams/src/core/source.rs:292`
  - `modules/streams/src/core/flow.rs:200`
  - `modules/streams/src/core/flow.rs:217`
  - `modules/streams/src/core/flow.rs:232`
- Substream 再統合は `Vec<Out>` の平坦化として実装。
  - `modules/streams/src/core/flow.rs:656`
  - `modules/streams/src/core/flow.rs:663`
- `flat_map_concat`/`flat_map_merge` は内側 Source を `collect_values()` で eager 収集する実装。
  - `modules/streams/src/core/flow.rs:969`
  - `modules/streams/src/core/flow.rs:938`
- `async_boundary` は現状 no-op（入力値をそのまま返す）。
  - `modules/streams/src/core/flow.rs:1069`
- Graph 実行系は単一の `GraphInterpreter` で駆動し、実行計画は 1 Source + 1 Sink を前提。
  - `modules/streams/src/core/graph_interpreter.rs:187`
  - `modules/streams/src/core/graph_interpreter.rs:197`
  - `modules/streams/src/core/stream_graph.rs:152`
- Hub は `VecDeque` ベースの最小実装（動的 materialization 順序や consumer-aware backpressure 契約は未実装）。
  - `modules/streams/src/core/merge_hub.rs:11`
  - `modules/streams/src/core/broadcast_hub.rs:11`
  - `modules/streams/src/core/partition_hub.rs:10`
- `SharedKillSwitch` は `flow()` 差し込み API ではなく、ハンドル由来 state 共有モデル。
  - `modules/streams/src/core/shared_kill_switch.rs:13`
  - `modules/streams/src/core/stream_handle_generic.rs:61`
- Restart/Backoff は固定 tick ベースの簡易実装で、指数バックオフや `maxRestartsWithin` 相当は未実装。
  - `modules/streams/src/core.rs:257`
  - `modules/streams/src/core.rs:274`
  - `modules/streams/src/core/graph_interpreter.rs:585`

### 1.2 参照仕様（Pekko）で要求される意味論
- Substream は `SubFlow/SubSource` として表現され、`groupBy/splitWhen/splitAfter` は新しい substream を生成。
  - `references/pekko/docs/src/main/paradox/stream/stream-substream.md:18`
  - `references/pekko/docs/src/main/paradox/stream/stream-substream.md:40`
  - `references/pekko/docs/src/main/paradox/stream/stream-substream.md:92`
- `mergeSubstreams/concatSubstreams` は `SubFlow` 上の操作で、並列度制限やデッドロック注意点がある。
  - `references/pekko/docs/src/main/paradox/stream/stream-substream.md:62`
  - `references/pekko/docs/src/main/paradox/stream/stream-substream.md:72`
- `flatMapConcat` は「1 substream を完了してから次へ」、`flatMapMerge` は `breadth` 到達時に上流を backpressure。
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/flatMapConcat.md:15`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/flatMapMerge.md:14`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/flatMapMerge.md:41`
- KillSwitch は `FlowShape` に差し込む制御点で、`SharedKillSwitch.flow()` を materialize 前に複数回利用可能。初回 `shutdown/abort` 以降は無視。
  - `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md:65`
  - `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md:66`
  - `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md:87`
- Hub は動的 fan-in/fan-out で、`MergeHub` は Source 起動後に producer 接続、`BroadcastHub/PartitionHub` は subscriber/consumer 不在時に upstream backpressure。
  - `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md:102`
  - `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md:105`
  - `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md:132`
  - `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md:211`
- 非同期境界は fused 実行を分割し、内部バッファと windowed/batching backpressure を伴う。
  - `references/pekko/docs/src/main/paradox/stream/stream-flows-and-basics.md:286`
  - `references/pekko/docs/src/main/paradox/stream/stream-flows-and-basics.md:294`
  - `references/pekko/docs/src/main/paradox/stream/stream-rate.md:56`
- Restart.withBackoff は fail/complete の両方で再起動し、指数バックオフと backpressure 契約を持つ。
  - `references/pekko/docs/src/main/paradox/stream/operators/RestartSource/withBackoff.md:13`
  - `references/pekko/docs/src/main/paradox/stream/operators/RestartSource/withBackoff.md:38`
  - `references/pekko/docs/src/main/paradox/stream/operators/RestartFlow/withBackoff.md:13`
  - `references/pekko/docs/src/main/paradox/stream/operators/RestartSink/withBackoff.md:13`
- `splitWhen/splitAfter` は supervision の `restart` を `resume` 同等として扱う。
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/splitWhen.md:12`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/splitAfter.md:12`

### 1.3 プロジェクト制約
- `fraktor-streams-rs` は `#![cfg_attr(not(test), no_std)]` を維持しており、core no_std 制約は満たしている。
  - `modules/streams/src/lib.rs:55`
- `core`/`std` 分離、Dylint ルール群、`Less is more`/`YAGNI`、破壊的変更許容の方針に従う必要がある。

## 2. 要件-資産マップ（Requirement-to-Asset Map）

| 要件 | 既存アセット | 充足状況 | ギャップ種別 | 根拠 |
|---|---|---|---|---|
| 要件1: 公開API互換性 | `Source/Flow/Sink` に主要メソッドは存在 | Partial | Missing | 演算子は存在するが SubFlow 契約不一致、`assert!` panic ベース入力検証、互換範囲の識別情報が不足 |
| 要件2: Substream意味論 | `group_by/split_*/merge_substreams/concat_substreams` | Missing | Missing | `SubFlow` 不在、`(Key,Out)`/`Vec<Out>` 化により substream lifecycle/backpressure 契約を表現できない |
| 要件3: flatMap意味論 | `flat_map_concat`, `flat_map_merge` | Partial | Missing | eager 収集で streaming 性が不足、`breadth` 到達時の upstream 抑止が未実装（待機キューへ無制限蓄積） |
| 要件4: Dynamic Hub | `MergeHub`, `BroadcastHub`, `PartitionHub` | Missing | Missing | materialization 順序保証・動的接続時契約・consumer 不在時 backpressure 契約が未実装 |
| 要件5: KillSwitch | `UniqueKillSwitch`, `SharedKillSwitch`, `Stream` 駆動 | Partial | Missing | 初回発火後の無視契約未実装（状態上書き可能）、`SharedKillSwitch.flow()` に相当する公開 API 不在 |
| 要件6: Restart/Backoff/Supervision | RestartBackoff + supervision | Partial | Missing | 指数バックオフ/時間窓/完了起因再起動未実装、split 系で restart=resume 同等契約が未担保 |
| 要件7: 非同期境界 | `async_boundary` ステージ定義あり | Missing | Missing | 実体は no-op で実行アイランド分離なし、境界バッファ起因 backpressure 契約なし |
| 要件8: 互換検証 + no_std | no_std 設定、単体テスト群 | Partial | Missing | no_std は維持、ただし emits/backpressures/completes/fails の互換行動を網羅する検証マトリクスが未整備 |
| 要件9: 破壊的変更許容 | 方針として適用可能 | Partial | Constraint | 実装上は阻害要因なし。CI/テスト完了を仕様上のゲートとして運用整備が必要 |

## 3. 主要ギャップ（実装観点）

1. **SubFlow モデル欠落が最上流の差分**
   - Pekko 互換MUSTの中核である substream 系演算子を、現実装は値変換で近似しており、substream 単位の完了・背圧・取消しを扱えない。

2. **Interpreter が「単一線形グラフ」前提**
   - `into_plan` / `compile_plan` の 1 Source + 1 Sink 制約により、Pekko 的な動的接続・複数 materialization 連携の表現が難しい。

3. **非同期境界・再起動・Hub が簡略化されすぎている**
   - `async_boundary` no-op、RestartBackoff 簡易版、Hub 最小キュー版のため、MUST 意味論へ到達できない。

4. **テストが単一パス回帰に偏っている**
   - `*_keeps_single_path_behavior` 系テストは存在するが、互換意味論（特に backpressure 契約）を保証する仕様テストが不足。

## 4. 実装アプローチ候補

### Option A: 現行モデルを拡張し続ける（既存 API 温存）
- **概要**: `Vec`/`(Key,Value)` モデルを維持したまま、個別に契約を追加実装。
- **利点**:
  - 既存コード差分が小さい。
  - 段階的に修正しやすい。
- **欠点**:
  - SubFlow 契約を型で表現できず、Pekko MUST への収束が難しい。
  - 例外的分岐が増え、YAGNI/保守性に反する。

### Option B: SubFlow 中心へ再設計（破壊的変更前提）
- **概要**: `SubFlow`/`SubSource` 相当型を導入し、group/split/merge/concat/flatMap の意味論を再定義。Interpreter を substream lifecycle 前提で再編。
- **利点**:
  - Pekko 互換MUSTに最短距離。
  - 仕様と型が整合し、将来の互換検証を自動化しやすい。
- **欠点**:
  - 変更範囲が大きく、初期工数が重い。
  - 既存 API は破壊的に変わる（ただし本仕様では許容）。

### Option C: ハイブリッド（内部再設計 + 一時適応層）
- **概要**: 内部を Option B 方向で作り直しつつ、外部 API だけを短期互換層でつなぐ。
- **利点**:
  - 移行中も段階的に動かせる。
  - リスク分散しやすい。
- **欠点**:
  - 一時コードの寿命管理が必要。
  - 「いずれ捨てる層」の分だけ実装量が増える。

## 5. 工数・リスク

- **Option A**: Effort **L** / Risk **High**  
  SubFlow 不在の根本問題を回避できず、後半で破綻しやすい。

- **Option B**: Effort **XL** / Risk **Medium**  
  初期コストは大きいが、MUST達成ルートが最も明確。

- **Option C**: Effort **XL** / Risk **Medium-High**  
  技術的には安全だが、短命コード管理に失敗すると負債化しやすい。

## 6. Research Needed（設計フェーズで要調査）

1. SubFlow の最小 API 面（materialized value 非関与を含む）を Rust 型でどう表現するか。
2. `flatMapMerge` の `breadth` 到達時の demand 制御を現 Interpreter へどう組み込むか。
3. Hub の materialization 順序保証と consumer 不在時 backpressure を no_std でどう実現するか。
4. `SharedKillSwitch.flow()` 相当 API と `shutdown/abort` 初回優先契約の実装モデル。
5. Restart の指数バックオフ（`min/max/randomFactor/maxRestartsWithin`）を core でどこまで再現するか。
6. `async_boundary` の実行アイランド分割（ActorMaterializer/StreamDriveActor との責務分担）。
7. 互換検証スイート（emits/backpressures/completes/fails）を requirements と 1:1 に対応付けるテスト構成。

## 7. 設計フェーズへの提案

- **推奨**: Option B を第一候補とし、必要最小限で Option C の段階導入要素を取り込む。  
  理由: 本仕様は「Pekko互換MUST」が最優先で、後方互換性は非要件のため。

- **設計時に確定すべき決定事項**:
  1. SubFlow 導入に伴う公開 API の破壊的変更範囲
  2. Interpreter の再設計方針（単一ループ継続 or 境界分割実行）
  3. Hub/KillSwitch/Restart の意味論テストの最小セット
  4. core no_std 維持のための依存・同期戦略

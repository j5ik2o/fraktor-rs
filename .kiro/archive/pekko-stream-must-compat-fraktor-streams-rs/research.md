# 調査・設計判断ログ

## Summary
- **Feature**: `pekko-stream-must-compat-fraktor-streams-rs`
- **Discovery Scope**: Complex Integration（既存 `modules/streams` 拡張・再設計）
- **Key Findings**:
  - 既存 `group_by/split_*` は `SubFlow` ではなく `Tuple/Vec` 近似で、Pekko MUST の substream 契約を満たせない。
  - `flat_map_concat/flat_map_merge` は eager 収集実装で、`breadth` 到達時の上流 backpressure 契約が不足。
  - Hub/KillSwitch/AsyncBoundary/Restart は API 名称は近いが意味論が簡略化されており、互換MUST達成には中核再設計が必要。

## Research Log

### Substream 契約の差分
- **Context**: 要件2（Substream意味論互換）を満たすため、現実装と Pekko の契約差分を確認。
- **Sources Consulted**:
  - `modules/streams/src/core/source.rs`
  - `modules/streams/src/core/flow.rs`
  - `references/pekko/docs/src/main/paradox/stream/stream-substream.md`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/groupBy.md`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/splitWhen.md`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/splitAfter.md`
- **Findings**:
  - 現実装は `group_by -> Source<(Key,Out)>`、`split_* -> Source<Vec<Out>>`。
  - Pekko は `SubFlow/SubSource` で substream 自体を第一級として扱う。
  - `mergeSubstreams/concatSubstreams` も Pekko では `SubFlow` 上操作であり、現在の単純 flatten では意味論を表現しきれない。
- **Implications**:
  - `SubFlow` 相当型の導入と substream lifecycle の明示が必須。
  - API 層だけでなく interpreter の実行モデルも変更が必要。

### flatMap 系の意味論
- **Context**: 要件3（flatMap意味論互換）の実現可能性確認。
- **Sources Consulted**:
  - `modules/streams/src/core/flow.rs`
  - `modules/streams/src/core/source.rs`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/flatMapConcat.md`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/flatMapMerge.md`
- **Findings**:
  - 現実装は内側 Source を `collect_values()` で全件収集してから処理している。
  - `flatMapConcat` の「前 substream 完了後に次開始」は近似できるが streaming 特性を失う。
  - `flatMapMerge` の `breadth` 到達時に上流 demand を止める契約は未実装（待機キュー蓄積で代替）。
- **Implications**:
  - 内側 substream を逐次/並行に実行する scheduler 的構造を導入し、demand と結合すべき。

### Dynamic Hub の意味論
- **Context**: 要件4（MergeHub/BroadcastHub/PartitionHub）の契約確認。
- **Sources Consulted**:
  - `modules/streams/src/core/merge_hub.rs`
  - `modules/streams/src/core/broadcast_hub.rs`
  - `modules/streams/src/core/partition_hub.rs`
  - `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md`
- **Findings**:
  - 既存 Hub は固定 queue の最小構造で、materialization 順序や dynamic consumer 契約が弱い。
  - Pekko は「先に受信側 materialize」「consumer 不在時の upstream backpressure」「動的接続」を明示。
- **Implications**:
  - Hub を「単なる queue 共有」から「接続管理 + demand 伝播」モデルへ拡張する必要がある。

### KillSwitch 契約
- **Context**: 要件5（Unique/Shared KillSwitch）で API 互換を確認。
- **Sources Consulted**:
  - `modules/streams/src/core/shared_kill_switch.rs`
  - `modules/streams/src/core/unique_kill_switch.rs`
  - `modules/streams/src/core/stream_handle_generic.rs`
  - `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md`
- **Findings**:
  - 現実装は state 共有はあるが `SharedKillSwitch.flow()` 相当の差し込み API がない。
  - `shutdown/abort` 後の後続呼び出し無視契約が明示的に保証されていない（状態上書き可能）。
- **Implications**:
  - `flow()` 提供と「最初の制御を優先する idempotent 遷移」を導入する必要がある。

### Async Boundary / Fusing
- **Context**: 要件7（非同期境界契約）で動作差を調査。
- **Sources Consulted**:
  - `modules/streams/src/core/flow.rs`
  - `references/pekko/docs/src/main/paradox/stream/stream-flows-and-basics.md`
  - `references/pekko/docs/src/main/paradox/stream/stream-rate.md`
- **Findings**:
  - `async_boundary` は現在 no-op。
  - Pekko は fused 実行がデフォルトで、`async` により実行アイランドを分離し、内部バッファ経由で backpressure 伝播。
- **Implications**:
  - 境界ごとに独立実行区間と境界バッファを持つ実行計画へ変更が必要。

### Restart/Backoff/Supervision
- **Context**: 要件6の再起動契約（失敗/完了起因、backpressure、予算超過終端）を検証。
- **Sources Consulted**:
  - `modules/streams/src/core.rs`
  - `modules/streams/src/core/graph_interpreter.rs`
  - `references/pekko/docs/src/main/paradox/stream/operators/RestartSource/withBackoff.md`
  - `references/pekko/docs/src/main/paradox/stream/operators/RestartFlow/withBackoff.md`
  - `references/pekko/docs/src/main/paradox/stream/operators/RestartSink/withBackoff.md`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/splitWhen.md`
  - `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/splitAfter.md`
- **Findings**:
  - 現行 RestartBackoff は固定 tick カウンタ中心で、指数バックオフ・時間窓の表現がない。
  - split 系の `restart == resume` 特則を仕様として保持していない。
- **Implications**:
  - Backoff 状態機械の再設計と、stage 種別ごとの supervision 例外規則を導入すべき。

### 既存テスト資産
- **Context**: 要件8（互換検証）の実現可能性判断。
- **Sources Consulted**:
  - `modules/streams/src/core/source/tests.rs`
  - `modules/streams/src/core/flow/tests.rs`
  - `modules/streams/src/core/graph_interpreter/tests.rs`
  - `modules/streams/src/core/*hub*/tests.rs`
- **Findings**:
  - `*_keeps_single_path_behavior` の回帰テストは豊富。
  - ただし emits/backpressures/completes/fails を要件単位で横断保証する互換スイートは不足。
- **Implications**:
  - 互換MUST向けの仕様テスト群（要件IDベース）を新設し、CI ゲート化する必要がある。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: 既存モデル拡張 | `Tuple/Vec` 近似のまま段階修正 | 差分が小さい | SubFlow 契約を型で表現できず収束リスク高 | 短期延命向け |
| B: SubFlow 中心再設計 | `SubFlow` 導入 + interpreter 再編 | 互換MUSTへの最短経路、型と意味論が一致 | 初期変更量が大きい | **採用** |
| C: ハイブリッド | 内部再設計 + 一時適応層 | 移行分割がしやすい | 一時コードが負債化しやすい | B の補助策として限定採用 |

## Design Decisions

### Decision: SubFlow 中心モデルを採用する
- **Context**: 要件2/3/7の中核は substream lifecycle と backpressure 契約。
- **Alternatives Considered**:
  1. A: 既存 `Tuple/Vec` モデル拡張
  2. B: `SubFlow` 中心再設計
- **Selected Approach**: B を採用し、`group_by/split_*` の戻り値を `SubFlow` 系へ再定義する。
- **Rationale**: 互換MUST達成が目的であり、後方互換性は非要件。
- **Trade-offs**: 破壊的変更は増えるが、互換性と保守性が改善する。
- **Follow-up**: `SubFlow` API と merge/concat 契約の詳細設計。

### Decision: Interpreter を境界分割前提に再編する
- **Context**: `async_boundary` no-op では要件7を満たせない。
- **Alternatives Considered**:
  1. 単一 interpreter にフラグ追加
  2. 実行アイランド + 境界バッファ化
- **Selected Approach**: 実行アイランド化し、境界で明示バッファと demand 伝播を管理。
- **Rationale**: Pekko の fused/async モデルと整合する。
- **Trade-offs**: 実装複雑度は上がる。
- **Follow-up**: バッファサイズ既定値と no_std でのメモリ戦略を決定。

### Decision: Hub を接続管理モデルへ移行する
- **Context**: 要件4で materialization 順序・consumer 不在時の契約が必要。
- **Alternatives Considered**:
  1. 既存 queue 共有のままルール追加
  2. 接続レジストリ + demand-aware ルーティング
- **Selected Approach**: 接続レジストリを持つ demand-aware モデルへ移行。
- **Rationale**: 動的 fan-in/fan-out 契約の再現に必要。
- **Trade-offs**: 状態管理が増加。
- **Follow-up**: MergeHub/BroadcastHub/PartitionHub で共通化できる最小抽象を切り出す。

### Decision: SharedKillSwitch に flow 差し込み API を追加する
- **Context**: 要件5.4が Pekko の `SharedKillSwitch.flow()` 契約を要求。
- **Alternatives Considered**:
  1. ハンドル経由のみ維持
  2. `flow()` を追加し DSL へ直接差し込み可能にする
- **Selected Approach**: `flow()` を追加し、初回発火優先（後続無視）を状態遷移で保証。
- **Rationale**: 互換APIと制御予測性を同時に満たす。
- **Trade-offs**: 既存 API の一部は置換対象。
- **Follow-up**: Unique/Shared の責務境界を設計で固定。

### Decision: 互換MUST検証を要件IDベースで新設する
- **Context**: 要件8.1/8.4は再現可能な互換検証を要求。
- **Alternatives Considered**:
  1. 既存単体テストに追記
  2. 互換仕様テスト群を独立
- **Selected Approach**: 独立した仕様テスト群を作り、要件IDと 1:1 対応させる。
- **Rationale**: 退行箇所の特定性が高くなる。
- **Trade-offs**: 初期テスト実装コストが増える。
- **Follow-up**: `emits/backpressures/completes/fails` を統一 DSL で記述する。

### Decision: 設計フェーズで Option B を正式確定する
- **Context**: `-y` 指定で設計フェーズへ進み、実装判断を固定する必要がある。
- **Alternatives Considered**:
  1. Option B を正式採用し互換層は導入しない
  2. Option C を標準化し一時適応層を常設する
- **Selected Approach**: Option B を正式確定し、必要時のみ短命の移行補助を局所導入する。
- **Rationale**: 要件9の「不要な互換コードを残さない」制約と最も整合する。
- **Trade-offs**: 移行時のAPI破壊影響は増える。
- **Follow-up**: `design.md` の要件トレーサビリティ（1.1〜9.4）を実装タスクの唯一の正本として扱う。

### Decision: Hub の consumer 不在ポリシーを backpressure に固定する
- **Context**: 要件4.2/4.5に対して「backpressure または失敗」の分岐が残ると実装解釈が割れる。
- **Alternatives Considered**:
  1. Hub 種別ごとに backpressure へ固定
  2. Hub 種別ごとに失敗へ固定
  3. 実装者判断で選択可能にする
- **Selected Approach**: Merge/Broadcast/Partition いずれも consumer 不在時は backpressure を返す方針へ固定。
- **Rationale**: Pekko `stream-dynamic` の説明と `Hub.scala` の `buffer full -> backpressure` 契約に整合し、要件4.2/4.5を満たしやすい。
- **Trade-offs**: 利用者が drop 挙動を求める場合は別途明示的 overflow 戦略が必要。
- **Follow-up**: `HubStep` に `Backpressure` を必須状態として保持し、テストで consumer 不在時の挙動を固定する。

### Decision: MUST 範囲の引数は検証済み型で受ける
- **Context**: 要件1.2（不正引数の明示失敗）と API シグネチャが不整合だった。
- **Alternatives Considered**:
  1. `usize` のまま実行時チェック
  2. `NonZeroUsize` 相当の値オブジェクトを導入
- **Selected Approach**: `SubstreamLimit`、`MergeParallelism`、`MergeBreadth`、`HubBufferSize` を導入し、構築時に `CompatError` を返す。
- **Rationale**: Pekko `FlattenMerge` の `breadth >= 1`、Hub の `bufferSize > 0` 契約を Rust 側で型化できる。
- **Trade-offs**: DSL 入口に変換ステップが増える。
- **Follow-up**: 変換ヘルパとエラーメッセージを互換テストで固定する。

### Decision: RestartSettings と SupervisionDecider を分離して明文化する
- **Context**: 要件6.1〜6.6に対し、backoff 設定と supervision 判定の責務が曖昧だった。
- **Alternatives Considered**:
  1. RestartCoordinator に全判定を集約
  2. Backoff 設定と supervision 判定を分離
- **Selected Approach**: `BackoffPolicy`（Pekko `RestartSettings` 相当）と `SupervisionDecider` を分離。
- **Rationale**: Pekko `RestartSettings(min/max/random/maxRestarts/maxRestartsWithin)` と split系 `restart == resume` 特則を明示的に反映できる。
- **Trade-offs**: コンポーネント数は増える。
- **Follow-up**: `splitWhen/splitAfter` での supervision 判定テストを独立ケース化する。

### Decision: SubFlow API を Pekko 命名と同じ意味論へ分離する
- **Context**: `merge_substreams(parallelism)` は Pekko の `mergeSubstreams` と `mergeSubstreamsWithParallelism` を混在させていた。
- **Alternatives Considered**:
  1. 既存 `merge_substreams(parallelism)` を維持
  2. `merge_substreams()` と `merge_substreams_with_parallelism()` を分離
- **Selected Approach**: Option 2 を採用し、`concat_substreams()` は `parallelism=1` 同等として扱う。
- **Rationale**: Pekko `SubFlow` のAPI構造と一致し、要件1.1/2.5の実装解釈を統一できる。
- **Trade-offs**: API数は増えるが、意味論の曖昧さは減る。
- **Follow-up**: タスク分解時に旧 `merge_substreams(parallelism)` 想定実装を全削除する。

### Decision: max_restarts 到達時の終端を「互換MUST既定=Complete」で固定する
- **Context**: `max_restarts` 到達時の終端が complete/fail 両論併記で未確定だった。
- **Alternatives Considered**:
  1. 常に `Complete` で固定
  2. `Complete/Fail` を設定可能にし、互換MUSTでは `Complete` を既定
- **Selected Approach**: Option 2 を採用し、`BackoffPolicy.on_max_restarts` を導入。MUSTプロファイルは `Complete` 固定。
- **Rationale**: Pekko `RestartSource/Flow/Sink.withBackoff` の既定終端（上限到達時 complete）と一致しつつ、拡張余地を残せる。
- **Trade-offs**: 拡張時の仕様逸脱リスクがあるため、MUST/非MUST境界の明示が必要。
- **Follow-up**: 互換テストで「MUSTプロファイルは complete 終端」を固定し、Fail 終端は別プロファイルに隔離する。

## Risks & Mitigations
- SubFlow 導入で API 破壊範囲が広がる  
  - 互換MUST外の旧 API は段階的に削除し、設計段階で移行マップを固定する。
- Interpreter 再編で性能退行が出る可能性  
  - 境界バッファの既定値・上限を明示し、ベンチ観測ポイントを先に決める。
- Hub の動的接続ロジックでデッドロックが発生する可能性  
  - materialization 順序と backpressure 遷移を状態機械として定義し、テストで網羅する。
- no_std でメモリ使用量が増える可能性  
  - `alloc` 利用箇所を明示し、容量境界をコンフィグ化する。

## References
- `references/pekko/docs/src/main/paradox/stream/stream-substream.md`
- `references/pekko/docs/src/main/paradox/stream/stream-dynamic.md`
- `references/pekko/docs/src/main/paradox/stream/stream-flows-and-basics.md`
- `references/pekko/docs/src/main/paradox/stream/stream-rate.md`
- `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/groupBy.md`
- `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/splitWhen.md`
- `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/splitAfter.md`
- `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/flatMapConcat.md`
- `references/pekko/docs/src/main/paradox/stream/operators/Source-or-Flow/flatMapMerge.md`
- `references/pekko/docs/src/main/paradox/stream/operators/RestartSource/withBackoff.md`
- `references/pekko/docs/src/main/paradox/stream/operators/RestartFlow/withBackoff.md`
- `references/pekko/docs/src/main/paradox/stream/operators/RestartSink/withBackoff.md`
- `modules/streams/src/core/source.rs`
- `modules/streams/src/core/flow.rs`
- `modules/streams/src/core/graph_interpreter.rs`
- `modules/streams/src/core/merge_hub.rs`
- `modules/streams/src/core/broadcast_hub.rs`
- `modules/streams/src/core/partition_hub.rs`
- `modules/streams/src/core/shared_kill_switch.rs`
- `modules/streams/src/core/unique_kill_switch.rs`

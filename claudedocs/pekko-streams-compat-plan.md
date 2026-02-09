# Pekko互換 Streams 強化計画（互換性MUST優先）

## 概要
この計画は、現行 `modules/streams` を「線形DSL中心の最小実装」から、Pekko Streams互換に必要な中核機能へ段階的に引き上げるための実装仕様である。  
方針は「Pekko互換に必要な要素のみを順次実装する」。過剰な独自拡張は行わない。

## 目的と完了条件
1. `Source -> Flow* -> Sink` の線形制約を解消し、Pekkoの `Graph/Shape` モデルに追従する。
2. fan-in/fan-out/junction、substream、障害回復、動的制御をPekko準拠で提供する。
3. 互換優先で API と実行セマンティクスを固定し、回帰を防ぐテスト基盤を整備する。
4. 各フェーズ完了時に対象範囲テストを全パスさせ、最終的に `./scripts/ci-check.sh all` をパスさせる。

## 現状差分（実装根拠）
1. `modules/streams/src/core/stream_graph.rs` の `into_plan()` は線形前提。
2. `modules/streams/src/core/stream_shape.rs` は 1入力1出力固定。
3. `modules/streams/src/core/graph_interpreter.rs` は単一source/flows/sinkの実行器。
4. `modules/streams/src/core/stage_kind.rs` は最小演算子集合。
5. Pekko側参照は `references/pekko/docs/src/main/paradox/stream/*.md` と `general/stream/stream-design.md`。

## スコープ
1. 対象IN: `modules/streams` の core DSL・実行器・演算子・テスト。
2. 対象IN: Pekkoドキュメントにある中核互換機能。
3. 対象OUT: 独自最適化のみを目的とする新機能。
4. 対象OUT: Pekko互換に無関係なUI/運用都合の拡張。

## 公開API/型の変更方針（決定事項）
1. `StreamShape<In, Out>` は将来的に汎用 `Shape` 系へ移行する。
2. `Source/Flow/Sink` は `from_graph` 系コンストラクションを正式サポートする。
3. `GraphDSL` 相当の構築APIを追加する。
4. junction 型として最低限 `Broadcast`, `Merge`, `Zip`, `Concat`, `Balance` を追加する。
5. flatten系として `flat_map_merge` を追加する。
6. substream系として `group_by`, `split_when`, `split_after`, `merge_substreams` 系を追加する。
7. 障害処理として `recover`, `recover_with_retries`, `RestartSource/Flow/Sink(with backoff)`, supervision設定を追加する。
8. 動的制御として `UniqueKillSwitch`, `SharedKillSwitch` を追加する。
9. テスト用に `TestSource`, `TestSink` probe API を追加する。
10. 命名は既存ルールに従い曖昧サフィックス禁止。`Service/Manager/Engine/Runtime` を新規導入しない。

## 実装フェーズ

## Phase 1: グラフ基盤の互換化（最優先）
1. `Shape` 抽象を導入し、`SourceShape`, `SinkShape`, `FlowShape`, `BidiShape` の骨格を整備する。
2. 既存 `StreamGraph` を「ステージ列」ではなく「ノード＋ポート＋エッジ」モデルに置換する。
3. `StreamPlan` を線形専用から、グラフ実行に必要な中間表現へ刷新する。
4. 実行器をポート駆動に再設計し、複数in/out、junction、閉路検出、materialization境界を扱えるようにする。
5. `Graph` は不変blueprint、materialization時に実体化、という契約を明文化する。
6. `Source/Flow/Sink` の既存線形APIは互換アダプタとして維持し、内部でグラフ基盤を使う。

## Phase 2: 中核演算子と合成性（Pekko互換コア）
1. fan-out: `Broadcast`, `Balance` を追加する。
2. fan-in: `Merge`, `Zip`, `Concat` を追加する。
3. flow演算子: `flat_map_merge`, `buffer(overflow strategy)`, `async boundary` を追加する。
4. substream: `group_by`, `split_when`, `split_after`, `merge_substreams`, `concat_substreams` を追加する。
5. GraphDSL相当で partial graph を構築可能にする。
6. `BidiFlow` の最小構成を追加し、IO系将来拡張の足場を整える。

## Phase 3: 障害モデル互換（運用必須）
1. `recover` と `recover_with_retries` を追加する。
2. `RestartSource/RestartFlow/RestartSink` の backoff 再起動を追加する。
3. supervision 方針 `Stop/Resume/Restart` を導入する。
4. 演算子ごとに supervision 対応有無を明示し、非対応は失敗させる。
5. Failure と Error（データとしてのエラー）を設計上明確化する。

## Phase 4: 動的制御とテスト基盤
1. `UniqueKillSwitch` と `SharedKillSwitch` を追加する。
2. `MergeHub/BroadcastHub/PartitionHub` は段階導入し、まず `MergeHub` と `BroadcastHub` を優先する。
3. `TestSource` / `TestSink` probe を導入し、需要制御・失敗注入・完了検証を可能にする。
4. 並行経路の不具合検出のため、fuzzing相当のテスト実行モードを追加する。

## テスト計画（必須シナリオ）
1. 線形パイプラインが従来どおり動作する回帰テスト。
2. Broadcast→Merge、Zip、Concat の配線・順序・backpressure検証。
3. `flat_map_concat` と `flat_map_merge` の順序差異検証。
4. `group_by` サブストリームの作成・再結合・上限制御・デッドロック検証。
5. `recover` と `recover_with_retries` の境界条件検証。
6. Restart with backoff の再起動回数・待機時間・停止条件検証。
7. supervision `Stop/Resume/Restart` の状態遷移検証。
8. KillSwitch の `shutdown/abort` が upstream/downstream に与える影響検証。
9. TestKit probe による demand 駆動の厳密検証。
10. `no_std` 前提の core が維持されることのビルド検証。
11. 最終で `./scripts/ci-check.sh all` を実行し全パス確認。

## 受け入れ基準
1. Pekko参照ドキュメントの中核機能に対し、同等の概念対応が説明可能である。
2. 線形専用実装の制約が除去され、graph/junction/substream が実用レベルで動作する。
3. 障害回復と動的制御が API とテストで保証される。
4. 既存機能回帰がなく、CI 全体がグリーンである。
5. 命名・型配置・lint ルール違反がない。

## リスクと対策
1. リスク: 実行器刷新で既存動作が壊れる。
2. 対策: Phase 1で回帰テストを先に固定し、互換アダプタを残す。
3. リスク: substream と supervision の組み合わせで複雑化。
4. 対策: Phase 2 と Phase 3 を分離し、段階的に失敗モードを固定。
5. リスク: API拡張が肥大化。
6. 対策: Pekko互換MUSTのみ採用し、独自拡張は保留。

## 前提とデフォルト（明示）
1. 互換目標は「Pekko互換がMUST」であり、それ以上の独自目標は置かない。
2. 後方互換性は不要とし、最適設計を優先する。
3. 実装順は Phase 1 → Phase 2 → Phase 3 → Phase 4 を固定する。
4. 途中コミットは意味単位で分割し、各単位でテストを通す。
5. 既存ルール群と dylint 8種は常時準拠とする。

## 実行タスクリスト（進捗管理用）
### Phase 1: グラフ基盤の互換化
- [x] P1-01 `Shape` 抽象を導入する。
- [x] P1-02 `SourceShape` / `SinkShape` / `FlowShape` / `BidiShape` の骨格を実装する。
- [x] P1-03 `StreamGraph` をノード＋ポート＋エッジ表現へ置換する。
- [x] P1-04 `StreamPlan` をグラフ実行向け中間表現に刷新する。
- [x] P1-05 実行器をポート駆動モデルへ再設計する。
- [x] P1-06 junction・複数in/out・閉路検出を扱えるようにする。
- [x] P1-07 `Graph` の不変blueprint契約とmaterialization境界を明文化する。
- [x] P1-08 既存線形APIを互換アダプタとして維持する。
- [x] P1-09 Phase 1対象テストを追加し全パスさせる。

### Phase 2: 中核演算子と合成性
- [x] P2-01 `Broadcast` を追加する。
- [x] P2-02 `Balance` を追加する。
- [x] P2-03 `Merge` を追加する。
- [x] P2-04 `Zip` を追加する。
- [x] P2-05 `Concat` を追加する。
- [ ] P2-06 `flat_map_merge` を追加する。
- [ ] P2-07 `buffer` と overflow strategy を追加する。
- [ ] P2-08 `async boundary` を追加する。
- [ ] P2-09 `group_by` / `split_when` / `split_after` を追加する。
- [ ] P2-10 `merge_substreams` / `concat_substreams` を追加する。
- [ ] P2-11 GraphDSL相当のpartial graph構築APIを追加する。
- [ ] P2-12 `BidiFlow` の最小構成を追加する。
- [ ] P2-13 Phase 2対象テストを追加し全パスさせる。

### Phase 3: 障害モデル互換
- [ ] P3-01 `recover` を追加する。
- [ ] P3-02 `recover_with_retries` を追加する。
- [ ] P3-03 `RestartSource` with backoff を追加する。
- [ ] P3-04 `RestartFlow` with backoff を追加する。
- [ ] P3-05 `RestartSink` with backoff を追加する。
- [ ] P3-06 supervision `Stop/Resume/Restart` を導入する。
- [ ] P3-07 演算子ごとの supervision 対応可否を仕様化する。
- [ ] P3-08 Failure と Error のセマンティクスを整理し反映する。
- [ ] P3-09 Phase 3対象テストを追加し全パスさせる。

### Phase 4: 動的制御とテスト基盤
- [ ] P4-01 `UniqueKillSwitch` を追加する。
- [ ] P4-02 `SharedKillSwitch` を追加する。
- [ ] P4-03 `MergeHub` を追加する。
- [ ] P4-04 `BroadcastHub` を追加する。
- [ ] P4-05 `PartitionHub` の導入方針を確定し実装する。
- [ ] P4-06 `TestSource` probe を追加する。
- [ ] P4-07 `TestSink` probe を追加する。
- [ ] P4-08 需要制御・失敗注入・完了検証のテストヘルパーを整備する。
- [ ] P4-09 fuzzing相当のテスト実行モードを追加する。
- [ ] P4-10 Phase 4対象テストを追加し全パスさせる。

### 最終確認
- [ ] F-01 受け入れ基準5項目をすべて満たすことを確認する。
- [ ] F-02 命名・型配置・lint違反がないことを確認する。
- [ ] F-03 `no_std` 前提が維持されることを確認する。
- [ ] F-04 `./scripts/ci-check.sh all` を実行し全パスを確認する。

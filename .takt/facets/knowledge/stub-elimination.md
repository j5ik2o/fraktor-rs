# スタブ撲滅 知識ベース

## 実行モデルの制約

fraktor-rsはtickベースの同期実行モデル。Pekkoの`FiniteDuration`パラメータはtick数(`ticks: usize`)で代替する。
実時間タイマーは使えないが、tick数でのタイミング制御は可能。

## スタブ履歴（初期抽出25件）

この一覧は初期抽出時点のスタブ履歴であり、各バッチで解消済みに変わる。計画時は必ず実コードを読み直すこと。

### グループA: バッファリング・レート制御（5件）
tickベース同期モデルでの再設計が必要。

1. `conflate` — 解消済み。`conflate_with_seed_definition` と `ConflateWithSeedLogic` 経由で seed/aggregate を保持して処理する。
2. `conflate_with_seed` — 解消済み。`ConflateWithSeedLogic` が seed と aggregate を適用する。
3. `expand` — 解消済み。`ExpandLogic` が最後の入力値から iterator を生成し、idle tick 時の extrapolate を扱う。
4. `extrapolate` — 解消済み。`expand` と同じ `ExpandLogic` 経由で扱う。
5. `grouped_within` — 解消済み。`grouped_within_definition` と `GroupedWithinLogic` が size 到達または tick 超過でグループを出力する。

### グループB: Fan-In バリエーション（6件）
merge/zip系のセマンティクス強化。

6. `merge_preferred` — 解消済み。`merge_preferred_definition` と `MergePreferredLogic` が preferred edge を優先して出力する。
7. `merge_prioritized` — 解消済み。`merge_prioritized_definition` と `MergePrioritizedLogic` が重み配列に基づいて出力 edge を選択する。
8. `merge_sorted` — 解消済み。`merge_sorted_definition` と `MergeSortedLogic` が各入力の先頭要素を比較して整列順を維持する。
9. `merge_latest` — 解消済み。`merge_latest_definition` と `MergeLatestLogic` が各入力の最新値を保持し、更新時に最新値集合を出力する。
10. `or_else` — 解消済み。`or_else_definition` と `OrElseSourceLogic` がプライマリ完了後にセカンダリ source へ切り替える。
11. `zip_latest` — 解消済み。`zip_latest` は `merge_latest` 系の最新値キャッシュ実装を利用する。

### グループC: タイムアウト系（4件）
全てtickベースで実装可能。

12. `backpressure_timeout` — 解消済み。`BackpressureTimeoutLogic` が pull 間 tick を監視し、超過時に timeout error を返す。
13. `completion_timeout` — 解消済み。`CompletionTimeoutLogic` が開始からの tick を監視し、未完了なら timeout error を返す。
14. `idle_timeout` — 解消済み。`IdleTimeoutLogic` が要素間 idle tick を監視し、超過時に timeout error を返す。
15. `initial_timeout` — 解消済み。`InitialTimeoutLogic` が最初の要素到着までの tick を監視し、超過時に timeout error を返す。

### グループD: Lazy評価（3件）
遅延評価パターン。

16. `concat_lazy` — 解消済み。`ConcatSourceLogic` がプライマリ完了後にセカンダリ `Source` を materialize して連結する。
17. `concat_all_lazy` — 解消済み。`IntoIterator<Item = Source<_, _>>` を受け、空列を構築エラーにし、各セカンダリを指定順に `concat_lazy` で遅延連結する。
18. `prepend_lazy` — 解消済み。`prepend_lazy_definition` が `ConcatSourceLogic` 経由で prepend 側 Source を遅延 materialize する。

### グループE: その他オペレーター（7件）

19. `prefix_and_tail` — 解消済み。`PrefixAndTailLogic` が prefix と tail source を分離して返す。
20. `switch_map` — 解消済み。`SwitchMapLogic` が新要素到着時に前の内側 source を破棄して切り替える。
21. `keep_alive` — 解消済み。`KeepAliveLogic` が idle tick 超過時に injected element を出力する。
22. `limit` — 解消済み。`limit_weighted(max, |_| 1)` 相当の専用 stage で max 超過時に `StreamError::StreamLimitReached` を返す。
23. `limit_weighted` — 解消済み。重み関数で remaining budget を減算し、超過時に `StreamError::StreamLimitReached` を返す。
24. `batch_weighted` — 解消済み。`limit_weighted` 系と同じ重み計算方針で `batch_weighted` DSL が weight 関数を受け取る。
25. `watch_termination` — 解消済み。plain 版は materialized value を保持して stage を挿入し、`watch_termination_mat` は `StreamCompletion<()>` を combine rule で合成する。

### 「ほぼ完全実装」（対象外）
- `drop_repeated` — stateful_mapで実装済み、ほぼ完全。
- `grouped_adjacent_by` — キーベース隣接グルーピング、ほぼ完全。
- `wire_tap` — map内callback、機能的に実装済み。
- `also_to` — also_to_matに委譲済み、実装完了。
- `log` — wire_tap委譲、stdロガー未対応だが設計上妥当。
- `monitor` — カウンタ付きmap、簡易だが妥当。
- `flat_map_prefix` — prefix処理あり、機能的に近い。

## Pekko参照パス

| 対象 | パス |
|------|------|
| FlowOps | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/Flow.scala` |

## 既存パターン参考

- `stateful_map` / `stateful_map_concat` — 状態付き変換のパターン
- `take_within` — tickベースタイミングのパターン
- `flat_map_merge` / `flat_map_concat` — サブストリーム処理のパターン
- `also_to_mat` / `wire_tap_mat` — Mat合成のパターン

## 保守注意

本リストは手書きの既知スタブ集であり、以降の実装バッチによって解消済みの項目を含む可能性がある。

- plan ステップで本ファイルを参照する際は、該当 API の **実際の実装を必ず読み直し**、記述が古い場合は本ファイルを同一 PR 内で更新すること。
- 記述更新の判定は `pekko-porting-plan.md` の「既知スタブとの突合（必須）」節のファクトチェック手順に従う。
- 新しいスタブを発見した場合は、グループ分類 / 現状 / Pekko セマンティクス / 参照パス を同じ粒度で追記すること。

# スタブ撲滅 知識ベース

## 実行モデルの制約

fraktor-rsはtickベースの同期実行モデル。Pekkoの`FiniteDuration`パラメータはtick数(`ticks: usize`)で代替する。
実時間タイマーは使えないが、tick数でのタイミング制御は可能。

## スタブ一覧（全25件）

### グループA: バッファリング・レート制御（5件）
tickベース同期モデルでの再設計が必要。

1. `conflate` — 現状: `self.map(|v| v)` (no-op)。Pekko: 下流が遅い時に上流要素を集約。tick同期では下流遅延なし → 要素をドロップせず全通過でよいが、集約ロジック(aggregate fn)は受け取って適用すべき。
2. `conflate_with_seed` — 現状: `self.map(seed)`。Pekko: seed + aggregate。同上。
3. `expand` — 現状: `self` (no-op)。Pekko: 上流が遅い時に最後の値を繰り返し展開。tick同期では「上流が遅い」がないため、extrapolate的な動作。stateful_mapで前回値を保持し、上流Noneならextrapolation。
4. `extrapolate` — 現状: `self` (no-op)。Pekko: expandと同様。上の方針と同じ。
5. `grouped_within` — 現状: `self.grouped(size)` (ticksパラメータ無視)。tick数で区切り判定を追加。stateful_mapでtickカウンタを持ち、size到達またはticks超過でグループを出力。

### グループB: Fan-In バリエーション（6件）
merge/zip系のセマンティクス強化。

6. `merge_preferred` — 現状: `self.merge(fan_in)`。Pekko: preferredポートを優先取得。実装: preferred側を先にpollし、要素があればそちらを出力。
7. `merge_prioritized` — 現状: `self.merge(fan_in)`。Pekko: 重み付き優先度マージ。実装: 重み配列を受け取り、比率に応じてポートを選択。
8. `merge_sorted` — 現状: `self.merge(fan_in)`。Pekko: Ordに基づくソート済みマージ。実装: 各ポートの先頭要素を比較し最小を出力(min-heap or 単純比較)。Tは`Ord`バウンド。
9. `merge_latest` — 現状: `self.merge(fan_in)`。Pekko: 各入力の最新値を保持し、どれか更新されるたびにVec全体を出力。実装: Vec<Option<T>>を保持するstateful処理。
10. `or_else` — 現状: `self.prepend(fan_in)`。Pekko: プライマリが空完了した場合にセカンダリに切替。実装: プライマリ完了後にセカンダリソースをフォールバック。
11. `zip_latest` — 現状: `self.zip_all(fan_in, fill)`。Pekko: 各入力の最新値を保持し、いずれか更新時にペアを出力。実装: 最新値キャッシュ付きzip。

### グループC: タイムアウト系（4件）
全てtickベースで実装可能。

12. `backpressure_timeout` — 現状: `self.take_within(ticks)`。Pekko: バックプレッシャーがticks以上続いたらエラー。実装: pull間のtickカウントを監視し、超過でStreamError。
13. `completion_timeout` — 現状: `self.take_within(ticks)`。Pekko: ストリーム全体がticks内に完了しなければエラー。実装: 開始からの合計tickカウント監視。
14. `idle_timeout` — 現状: `self.take_within(ticks)`。Pekko: 要素間の無通信がticks超過でエラー。実装: 最後の要素からのtickカウント監視。
15. `initial_timeout` — 現状: `self.take_within(ticks)`。Pekko: 最初の要素がticks内に来なければエラー。実装: 開始からfirst elementまでのtickカウント監視。

### グループD: Lazy評価（3件）
遅延評価パターン。

16. `concat_lazy` — 現状: `self.concat(fan_in)`。Pekko: セカンダリソースの作成を遅延。実装: クロージャを受け取り、プライマリ完了時にセカンダリを生成してconcat。
17. `concat_all_lazy` — 現状: `self.concat(fan_in)`。同上、複数ソース版。
18. `prepend_lazy` — 現状: `self.prepend(fan_in)`。Pekko: prepend対象の生成を遅延。

### グループE: その他オペレーター（7件）

19. `prefix_and_tail` — 現状: `self.grouped(size)`。Pekko: 先頭N要素(prefix)と残り(tail)のSource/Flowを返す。実装: take(n)で先頭を集め、残りはdrop(n)相当のFlowとしてペアで返す。
20. `switch_map` — 現状: `self.flat_map_merge(1, func)`。Pekko: 新要素が来たら前のサブストリームをキャンセルして新サブストリームに切替。breadth=1のflat_map_mergeは近いが、前のキャンセルが不足。
21. `keep_alive` — 現状: `self.intersperse(...)`。Pekko: 無通信がinterval超過したらinject要素を注入。tick版: 無要素tickカウントがinterval超過で注入。
22. `limit` — 現状: `self.take(max)`。Pekko: max超過時に`StreamLimitReachedException`をthrow。実装: カウントしてmax超過でStreamError。
23. `limit_weighted` — 現状: `self.take(max_weight)`。Pekko: 重み関数で重み合計がmax超過でエラー。
24. `batch_weighted` — 現状: `self.batch(size)`。Pekko: 重み関数で重み合計がmax_weight未満の間バッチ蓄積。実装: stateful_mapで重み管理付きバッチ。
25. `watch_termination` — 現状: `self` (no-op)。Pekko: ストリーム終了時にFutureを完了。tick版: Completion<()>をMat値として返し、完了時にReadyにする。

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
| FlowOps | `references/pekko/stream/src/main/scala/org/apache/pekko/stream/scaladsl/FlowOps.scala` |

## 既存パターン参考

- `stateful_map` / `stateful_map_concat` — 状態付き変換のパターン
- `take_within` — tickベースタイミングのパターン
- `flat_map_merge` / `flat_map_concat` — サブストリーム処理のパターン
- `also_to_mat` / `wire_tap_mat` — Mat合成のパターン

# SmallestMailbox ルーティングの Pekko 互換化 TODO

## 背景

`SmallestMailboxRoutingLogic::select_observed`（kernel 層）および
`pool_router::select_smallest_mailbox_index`（typed DSL 層）は、
Pekko の `SmallestMailboxRoutingLogic` を簡略化した実装になっている。

CodeRabbit レビューで「空メールボックスが複数ある場合に最小 index のルーティーへ偏る」
という指摘を受けた。早期 break 自体は `mailbox_len < best_observed_len`（厳密 less-than）
の都合で動作に影響しないため、本質は**タイブレーク機構の欠如**と
**Pekko 互換に必要な状態情報の不足**である。

参照ファイル:

- `modules/actor-core/src/core/kernel/routing/smallest_mailbox_routing_logic.rs`
- `modules/actor-core/src/core/typed/dsl/routing/pool_router.rs`
- 参照実装: `references/pekko/actor/src/main/scala/org/apache/pekko/routing/SmallestMailbox.scala`

## 現状と Pekko の差分

| 観点 | Pekko | fraktor-rs |
|------|-------|------------|
| score=0 判定 | `!isSuspended && !isProcessingMessage && !hasMessages` | `mailbox_len == 0` のみ |
| 処理中追跡 | `ActorCell.currentMessage != null` | なし |
| サスペンド追跡 | `mailbox.isSuspended` | なし |
| タイブレーク | 処理中を score=1 でペナルティ化（idle 優先） | なし（最小 index 勝ち） |
| 2 パス探索 | あり（deep=false → deep=true） | なし |

fraktor-rs は mailbox 長のみを見る簡略版であり、Pekko 互換ではない。
現時点ではこの簡略実装を許容し、Pekko 互換化は将来対応とする。

## 将来対応方針

### フェーズ 1: 状態追跡の追加

- `ActorCell` に「処理中」フラグを追加する（受信ループ開始/終了で更新）
- `Mailbox` の `isSuspended` に相当する状態を公開する
- `no_std` 制約下でのアトミック操作・`SpinSyncMutex` 活用を確認する

### フェーズ 2: スコアリング実装

- Pekko の `selectNext` に相当する再帰/ループを Rust で実装
- score 定義:
  - `isSuspended` → `u64::MAX - 1`
  - `isProcessingMessage` → `+1`
  - `!hasMessages` → `+0`
  - `hasMessages && !deep` → `u64::MAX - 3`
  - `hasMessages && deep` → `numberOfMessages`
- `newScore == 0` での早期 return は Pekko 準拠で残す
- 2 パス探索（deep=false → deep=true）を実装

### フェーズ 3: typed DSL 層の統合

- `pool_router::select_smallest_mailbox_index` を kernel 実装に委譲する
- `dispatch_counts` フォールバックは Pekko に存在しないため、
  観測不能ルーティーの扱いを再検討する（`NoRoutee` 相当 or ランダム）

## 今すぐの対応（このドキュメントに合わせて実施）

- `SmallestMailboxRoutingLogic::select_observed` の rustdoc に
  「Pekko の `isSuspended`/`isProcessingMessage` 判定を省略した簡略実装。
  空メールボックスルーティーが複数ある場合、最小 index 側に偏る」旨を明記する
- `pool_router::select_smallest_mailbox_index` にも同等の注記を追加する
- 本 TODO ドキュメントへのリンクをコメントで残す

## 検証（将来対応時）

- `./scripts/ci-check.sh ai dylint -m actor-core`
- `./scripts/ci-check.sh ai test -m actor-core`
- Pekko の `SmallestMailboxSpec` を参考にしたテストケースの追加:
  - 全ルーティーが空 → ラウンドロビン相当で分散されること
  - 1 つが処理中、1 つが idle で両方空 → idle が選ばれること
  - サスペンド中ルーティーは最後の選択肢となること

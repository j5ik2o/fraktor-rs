# Affinity Executor 再計画メモ

## 対象
- `modules/actor-adaptor-std/src/std/dispatch/dispatcher/affinity_executor.rs`
- `modules/actor-adaptor-std/src/std/dispatch/dispatcher/affinity_executor_factory.rs`
- `modules/actor-adaptor-std/src/std/dispatch/dispatcher.rs`

## 現状
- Pekko 互換レビューで、以前の `AffinityExecutor` 案は mailbox stickiness を持たない別物であり、`AffinityPool` 対応としては不正確と判断された
- 現行の `Executor` trait は `Box<dyn FnOnce()>` しか受け取らず、同じ mailbox を同じ worker に固定するための識別子を運べない
- そのため、`review-fix` では **実装完了扱いを撤回** し、Phase 3 の残件として再計画する

## 次バッチで詰める論点
1. `Executor` submission に mailbox / queue identity を渡す seam をどこで持つか
2. Pekko `queueSelector.getQueue(command, parallelism)` 相当を fraktor-rs でどう表現するか
3. 公開 API に出さず internal 実装に閉じるのか、あるいは設定面まで公開するのか
4. stickiness と shutdown 契約を固定するテストをどの層に置くか

## 今回の扱い
- `AffinityPool` executor は **未実装**
- public re-export は追加しない
- parity 完了主張には含めない

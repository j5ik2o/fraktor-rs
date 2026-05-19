# テスト作成計画

## タスク
classic kernel routing の `ConsistentHashingRoutingLogic` と `SmallestMailboxRoutingLogic` に対する先行テストを追加する。

## 対象
- `modules/actor-core/src/core/kernel/routing/consistent_hashing_routing_logic.rs`
- `modules/actor-core/src/core/kernel/routing/smallest_mailbox_routing_logic.rs`

## 方針
1. 既存の `random_routing_logic` / `round_robin_routing_logic` と同じ `{type}/tests.rs` パターンでテストを置く
2. `ConsistentHashingRoutingLogic` は安定選択と routee 並び替え耐性を確認する
3. `SmallestMailboxRoutingLogic` は mailbox 観測に基づく選択と未観測 routee の低優先扱いを確認する
4. 実装前ステップでも未実装型への参照が検出できるよう、既存の compiled な routing logic テスト面に型契約テストを追加する

## 変更予定
| 種別 | ファイル |
|------|---------|
| 作成 | `modules/actor-core/src/core/kernel/routing/consistent_hashing_routing_logic/tests.rs` |
| 作成 | `modules/actor-core/src/core/kernel/routing/smallest_mailbox_routing_logic/tests.rs` |
| 変更 | `modules/actor-core/src/core/kernel/routing/routing_logic/tests.rs` |
| 作成 | `.takt/runs/20260416-023446-pekko-phase-kernel-typed/reports/test-scope.md` |
| 作成 | `.takt/runs/20260416-023446-pekko-phase-kernel-typed/reports/test-decisions.md` |

## 検証
- `cargo test` は未実装型参照により失敗する想定
- 失敗形を確認するため、`./scripts/ci-check.sh ai test -m actor-core -- --no-run` ではなく対象 crate の `cargo test --no-run` を直接使う

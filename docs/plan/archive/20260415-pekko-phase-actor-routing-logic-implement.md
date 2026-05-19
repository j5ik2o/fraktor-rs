# 実装計画

## 対象
- classic kernel routing の `ConsistentHashingRoutingLogic`
- classic kernel routing の `SmallestMailboxRoutingLogic`

## 方針
1. `modules/actor-core/src/core/kernel/routing/` に公開 `RoutingLogic` 実装を追加する
2. `routing.rs` で module 配線と再公開を行う
3. 先行追加済みの `routing_logic/tests.rs` を満たす最小実装に限定する
4. mailbox 長は既存 runtime の観測値だけを使い、追加の dispatch fallback は入れない

## 変更予定
| 種別 | ファイル |
|------|---------|
| 作成 | `modules/actor-core/src/core/kernel/routing/consistent_hashing_routing_logic.rs` |
| 作成 | `modules/actor-core/src/core/kernel/routing/smallest_mailbox_routing_logic.rs` |
| 変更 | `modules/actor-core/src/core/kernel/routing.rs` |
| 変更 | `.takt/runs/20260415-063831-pekko-phase-actor/reports/coder-scope.md` |
| 変更 | `.takt/runs/20260415-063831-pekko-phase-actor/reports/coder-decisions.md` |

## 検証
- `./scripts/ci-check.sh ai dylint -m actor-core`
- classic kernel routing の対象テスト

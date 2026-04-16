# pekko-phase-kernel-typed implement 計画

## 概要
`00-plan.md` に従い、今回の implement ステップでは classic kernel routing の未実装 2 件に限定して着手する。対象は `ConsistentHashingRoutingLogic` と `SmallestMailboxRoutingLogic`、およびそれらを `routing.rs` に接続する module wiring である。

## 実装対象
- `ConsistentHashingRoutingLogic`
- `SmallestMailboxRoutingLogic`
- `modules/actor-core/src/core/kernel/routing.rs` の module wiring
- 先行追加済み routing logic テストの成立

## 実装方針
- kernel 層に閉じた実装にする
- `RoutingLogic::select(&self, ...)` 契約を守り、`&mut self` を導入しない
- consistent hash は無状態で評価できる rendezvous hashing で表現する
- smallest mailbox は既存の `ActorRef -> system_state -> cell -> mailbox().user_len()` 観測経路を使う
- public API の見た目合わせではなく、Pekko の契約意図を Rust の既存パターンに翻訳する

## 変更対象
- `modules/actor-core/src/core/kernel/routing/consistent_hashing_routing_logic.rs`
- `modules/actor-core/src/core/kernel/routing/smallest_mailbox_routing_logic.rs`
- `modules/actor-core/src/core/kernel/routing.rs`

## このステップで扱わない項目
- `LoggingFilter / LoggingFilterWithMarker`
- classic `RouterConfig / Pool / Group / CustomRouterConfig`
- typed 側の routing 再利用整理

## Why

Phase III により stash 起点の `prepend_via_drain_and_requeue` は production から外れたが、`Mailbox::prepend_user_messages(...)` 自体は依然として non-deque queue に対する drain-and-requeue fallback を内包している。Phase IV の outer lock reduction を lock 削減だけに集中させるために、まず prepend contract 自体を deque-only に硬化し、fallback 依存を別 change として切り離す必要がある。

## What Changes

- `Mailbox::prepend_user_messages(...)` を廃止し、deque-capable queue を事前に解決した caller だけが使える crate-private な deque 専用 prepend API に置き換える
- `prepend_via_drain_and_requeue` を削除し、旧 fallback 前提の code/test を新 contract に合わせて整理する
- `Mailbox::prepend_user_messages(...)` の caller inventory を整理し、production caller が deque contract を満たしていることを固定する
- stash / persistence / showcase の既存 caller がこの prepend contract の上で引き続き動作することを test で固定する
- **BREAKING**: actor-core 内部で generic `prepend_user_messages(...)` に依存していた caller は、deque-capable queue を事前に解決しない限り新しい prepend API を呼べなくなる

## Capabilities

### New Capabilities
- `mailbox-prepend-deque-contract`: `Mailbox::prepend_user_messages(...)` が deque-capable queue 専用契約となり、fallback を許さない capability

### Modified Capabilities
- なし

## Impact

- 影響コード:
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`
  - `modules/actor-core/src/core/kernel/actor/actor_cell.rs`
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/` 配下の crate-private error / helper
  - 関連 test 一式
- API 影響:
  - public API 変更は原則として伴わない
  - `Mailbox` の crate-private prepend API が generic fallback つき呼び出しから deque 専用 API へ変わる
- 非対象:
  - outer lock 自体の削減
  - close correctness の再設計
  - shared queue / BalancingDispatcher の close semantics
  - queue 実装の置き換え

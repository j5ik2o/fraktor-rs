## Why

Mailbox の scheduling はすでに atomic 化されているが、標準の user message queue は hot path の enqueue/dequeue を shared lock と `put_lock` 経由で直列化している。次の性能改善対象として、既存の mailbox drain / close / overflow semantics を維持したまま、通常 FIFO user queue を mailbox-owned な lock-free MPSC queue に置き換える。

## What Changes

- 通常 unbounded mailbox 用に lock-free MPSC-backed FIFO user queue を追加する。
- 通常 `enqueue_envelope` path では user queue mutation を `Mailbox::put_lock` から外し、queue 側の atomic close protocol で close 後 enqueue を拒否する。
- public な `MessageQueue` contract と `Envelope` payload contract は変更しない。
- bounded / priority / control-aware / deque-prepend 対応 queue は初回スライスの対象外にする。
- bounded / deque / priority など lock-backed queue では、既存の `Mailbox::put_lock` による close 直列化を維持する。
- FIFO preservation、exact-once dequeue、cleanup/drop safety、producer/consumer race を検証する並行テストを追加する。
- unsafe queue primitive の検証として `miri` と `loom` のタスクを追加する。

## Capabilities

### New Capabilities

- `lock-free-mailbox-user-queue`: 通常 mailbox user queue の lock-free MPSC behavior、safety boundary、既存 mailbox semantics との互換を規定する。

### Modified Capabilities

- `mailbox-runnable-drain`: 既存の `Mailbox::run` drain behavior を維持したまま、標準の通常 user queue が lock-free MPSC 実装で backing され得ることを明確化する。
- `mailbox-close-semantics`: 通常 lock-free user queue では `put_lock` ではなく queue-local atomic close protocol で close 後 enqueue を拒否することを明確化する。

## Impact

- 影響するコード:
  - `modules/actor-core-kernel/src/dispatch/mailbox/unbounded_message_queue.rs`
  - `modules/actor-core-kernel/src/dispatch/mailbox/mailbox_queue_handles.rs`
  - `modules/actor-core-kernel/src/dispatch/mailbox/mailbox_queue_state.rs`
  - `modules/actor-core-kernel/src/dispatch/mailbox/base.rs`
  - `modules/actor-core-kernel/src/dispatch/mailbox/` 配下の新しい mailbox-local queue primitive module
- 検証:
  - 既存 mailbox tests は引き続き適用する。
  - queue 専用の並行テストを追加する。
  - raw pointer ownership safety を `miri` で検証する。
  - producer/consumer interleaving を可能な範囲で `loom` model test により検証する。
- 依存関係:
  - runtime dependency は追加しない。
  - `loom` は dev-only の検証依存として追加するか、既存 test tooling の範囲に隔離する。

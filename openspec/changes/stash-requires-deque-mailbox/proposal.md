## Why

現状の `stash` 経路は、typed/classic ともに deque-capable mailbox requirement を欠いたまま動作できてしまい、`unstash` が production で `prepend_via_drain_and_requeue` に到達する。これにより silent な性能劣化と Phase IV 以降の前提不成立が残っているため、Phase III で mailbox 契約を明示的に強化する必要がある。

## What Changes

- `stash` を使う actor 向けに、`Props::with_stash_mailbox()` / `TypedProps::with_stash_mailbox()` を追加し、deque mailbox requirement を明示する
- `stash_message_with_limit` / `unstash_message` / `unstash_messages` / `unstash_messages_with_limit` は、deque-capable mailbox でない場合に silent fallback せず deterministic に失敗する
- `bounded + stash` は silent fallback を許さず、既存の mailbox config validation により deterministic failure として扱う
- typed/classic の stash 関連テスト・showcase・サンプル呼び出しを新しい contract に合わせて更新する
- **BREAKING**: `Behaviors::with_stash(...)` や classic stash を使うが mailbox requirement を明示していない caller は、従来の silent fallback ではなく deterministic failure に変わる

## Capabilities

### New Capabilities
- `stash-mailbox-requirement`: stash 利用経路が deque-capable mailbox requirement を明示し、欠落時は deterministic に失敗する contract

### Modified Capabilities
- なし

## Impact

- 影響コード:
  - `modules/actor-core/src/core/kernel/actor/props/base.rs`
  - `modules/actor-core/src/core/typed/props.rs`
  - `modules/actor-core/src/core/kernel/actor/actor_cell.rs`
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` または mailbox capability を問い合わせる補助 API
  - stash 関連 tests / showcases
- API 影響:
  - `Props` / `TypedProps` に stash 用 mailbox convenience API が追加される
  - stash contract を満たさない caller は stash/unstash 時に deterministic failure へ変わる
- 非対象:
  - mailbox prepend 自体の廃止
  - `Behavior` と `Props` の責務再設計
  - outer lock 削減
  - bounded deque mailbox の新規実装

## Context

現状の stash 実装は、stashed message 自体は `ActorCell` の `stashed_messages` に保持し、`unstash_*` 時に `Mailbox::prepend_user_messages(...)` を呼び出して mailbox 先頭へ戻している。この prepend 経路は deque-capable queue なら `enqueue_first` を使うが、non-deque queue では `prepend_via_drain_and_requeue` にフォールバックする。

問題は、`MailboxRequirement::for_stash()` が存在する一方で、typed の `Behaviors::with_stash(...)` / `TypedProps::from_behavior_factory(...)` はその requirement を自動では運ばないことにある。classic でも `Props::with_mailbox_requirement(...)` を呼ばなければ default mailbox のままなので、`unstash_*` は production で non-deque queue に到達し得る。

この phase の目的は mailbox/stash 契約を明示化し、silent fallback を止めることである。逆に、mailbox prepend の撤廃や `Behavior` と `Props` の責務再設計まで広げると、Phase III を超えて別問題になる。

## Goals / Non-Goals

**Goals:**
- stash 利用経路に deque mailbox requirement を明示する API を用意する
- typed/classic の `stash_*` / `unstash_*` が non-deque mailbox に対して silent fallback せず deterministic に失敗するようにする
- `bounded + stash` を silent fallback ではなく deterministic failure として扱う
- deque mailbox requirement を満たした場合の既存 ordering contract を維持する
- `prepend_via_drain_and_requeue` を stash 起点の production 経路から外す

**Non-Goals:**
- mailbox prepend 自体を廃止すること
- `Behavior` と `Props` の責務境界を大きく変更すること
- outer lock 削減や lock 戦略の再設計
- bounded deque mailbox 実装の新設

## Decisions

### 1. Phase III は「明示 API + deterministic failure」を採用する

本 change では自動伝播ではなく、明示 API で stash contract を表現する。

採用方針:
- `Props::with_stash_mailbox()` を追加し、内部で `MailboxRequirement::for_stash()` を適用する
- `TypedProps::with_stash_mailbox()` を追加し、untyped `Props` 側へ同じ requirement を委譲する

この方針を選ぶ理由:
- `Behavior` と spawn config の責務分離を維持できる
- 既存の `MailboxRequirement` / `MailboxConfig` の仕組みを再利用できる
- `Less is more` / YAGNI に沿って最小変更で Phase III を完了できる
- API 名だけで「stash 用 mailbox 契約を付与する」ことが分かる

代替案:
- `Behaviors::with_stash(...)` から requirement を暗黙伝播する
  - 利用者体験は良い
  - ただし `Behavior` と `Props` の境界を崩し、今回は非目標に触れるため採用しない

### 2. deterministic failure は `ActorCell::stash_*` と `unstash_*` の入口で返す

silent fallback を止める判定は `ActorCell::stash_message_with_limit` と `unstash_message` / `unstash_messages` / `unstash_messages_with_limit` に入れる。ここで mailbox が deque-capable prepend を提供できるかを確認し、満たさなければ recoverable な `ActorError` を返す。

採用理由:
- stash contract 違反を `stash` 時点で reject すれば、「stash できたが unstash できない」という袋小路を作らずに済む
- `unstash_*` 側にも同じ check を残すことで、既に stashed state を持つ actor や将来の回帰に対する防御線になる
- `Mailbox::prepend_user_messages(...)` 自体は generic な prepend API として残せる
- `prepend_via_drain_and_requeue` を即削除せずに、stash 起点の production 経路だけを止められる

実装上は `Mailbox` 側に `pub(crate) fn user_queue_is_deque_capable(&self) -> bool` を追加する。`ActorCell` は mailbox 内部実装を直接覗かず、その helper を通して判定する。

補足:
- `ActorContext::stash_with_limit(...)` の `current_message` 前提チェックはそのまま維持し、deque capability check は `ActorCell::stash_message_with_limit(...)` の中で行う
- したがって stash 文脈不正 (`current_message` 不在) は、mailbox capability 違反より先に従来どおり返る
- `unstash_*` は既存の `Ok(0)` semantics を維持するため、stash が空の場合は capability check より先に早期 return する

代替案:
- `Mailbox::prepend_user_messages(...)` 自体を non-deque で常に error にする
  - より単純ではある
  - ただし generic API の意味まで変えてしまうため、この phase では採用しない

### 3. stash contract violation は既存の recoverable error パターンで表現する

error 表現は新しい `ActorError` variant を増やさず、既存の `STASH_OVERFLOW_REASON` と同じ流儀に揃える。

採用方針:
- `STASH_REQUIRES_DEQUE_REASON` 定数を追加する
- `ActorError::recoverable(STASH_REQUIRES_DEQUE_REASON)` を返す
- 既存の判定 helper と同様に、`ActorContext` から参照できる `is_stash_requires_deque_error(...)` を追加する

この方針を選ぶ理由:
- 既存の stash error 設計と一貫する
- supervisor / test / caller 側の影響を最小化できる
- error variant 増設より差分が小さい

### 4. `bounded + stash` は unsupported のまま deterministic failure とする

現行実装では `MailboxRequirement::for_stash()` と bounded mailbox policy は `MailboxConfigError::BoundedWithDeque` で reject される。この phase では bounded deque queue を新設せず、unsupported を明示契約として固定する。

採用理由:
- bounded deque queue 実装は本 phase の非目標
- 既存 validation を活かせば、silent fallback を増やさず deterministic failure にできる

代替案:
- `BoundedDequeMessageQueue` を新設する
  - 将来的にはあり得る
  - 今回は scope overrun のため採用しない

### 5. ordering contract は「requirement を満たした場合」に維持する

この change 後も、deque mailbox requirement を満たした stash actor では、既存の ordering contract を保持する。

保持対象:
- `typed_behaviors_unstash_replays_before_already_queued_messages`
- `unstash_messages_are_replayed_before_existing_mailbox_messages`

つまり、Phase III の変更は fallback 経路の排除であり、deque prepend 経路の意味は変えない。

## Risks / Trade-offs

- **[Risk]** stash caller に明示 API の追従が必要になる
  - **Mitigation:** `Props` / `TypedProps` に対称な convenience API を追加し、`Behaviors::with_stash(...)` の rustdoc に「この helper は mailbox を設定しないため `TypedProps::with_stash_mailbox()` を併用すること」を明記し、tests / showcases / examples を先に更新する
- **[Risk]** requirement 欠落は compile-time ではなく runtime で検出される
  - **Mitigation:** `unstash_*` で deterministic failure にし、silent fallback を残さない
- **[Risk]** `prepend_via_drain_and_requeue` 自体は残るため、他経路の production reachability は即ゼロにならない
  - **Mitigation:** 本 phase の完了条件を「stash 起点の production 経路から外す」に限定し、削除判断は Phase IV に送る
- **[Risk]** caller 更新漏れが tests 以外に残る可能性がある
  - **Mitigation:** stash 関連の search ベース棚卸しと showcase 更新をタスクに含める

## Migration Plan

1. `Props` / `TypedProps` に stash 用 convenience API を追加する
2. `ActorCell::stash_*` / `unstash_*` に deque capability check を追加し、違反時は deterministic failure にする
3. typed/classic の stash テスト・showcase・サンプル呼び出し、および `Behaviors::with_stash(...)` の rustdoc を新 contract に更新する
4. `bounded + stash` の validation contract を tests で固定する
5. OpenSpec validate と stash 関連 test を通し、Phase IV の前提として扱える状態にする

rollback はシンプルで、new helper と `unstash_*` validation を戻せば従来 fallback に戻る。ただし本プロジェクトは pre-release であり、rollback より contract 固定を優先する。

## Open Questions

- 現時点ではなし

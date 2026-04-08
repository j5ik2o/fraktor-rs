## ADDED Requirements

### Requirement: Stash usage SHALL be backed by deque-capable mailbox or by an in-Behavior replay path

`actor-core` の stash 機構 (`Behaviors::with_stash` および `ActorCell::stash_message_with_limit` 経路) は、以下のいずれかを **満たさなければならない**（MUST）:

- **Path 1**: stash を使う actor が必ず **deque-capable mailbox** (`MailboxRequirement::for_stash() = requires_deque()` 強制) で spawn される。`unstash` は `Mailbox::prepend_user_messages` を経由するが、その内部は deque 専用 path (`UnboundedDequeMessageQueue::enqueue_first` 等の atomic prepend) のみを使う
- **Path 2**: stash の replay (`unstash`) が `Mailbox::prepend_user_messages` を経由せず、Behavior interpreter / dispatcher loop の中で直接 stashed messages を replay する

どちらの path を選ぶかは本 change の Phase 1 (探索) ではなく、Phase 1 完了後の合意で決定される。本 spec は **どちらの path でも満たすべき不変条件** だけを記述する。

注: Phase 1 の現時点の recommend は **Path 1** (`Props` 側の explicit opt-in) だが、本 spec 自体はその決定に依存しない。

#### Scenario: stash backing is deterministic (no silent fallback)
- **WHEN** stash 機能を使うコード (typed `Behaviors::with_stash` または classic `cell.stash_message_with_limit`) を実行する
- **THEN** Phase 2 完了後は `Mailbox::prepend_via_drain_and_requeue` が **実行されない** (Path 1 では deque 経路だけが取られ、Path 2 では mailbox prepend 自体を経由しないため)
- **AND** silent な性能劣化 (drain_and_requeue の暗黙 fallback) は発生しない

### Requirement: Existing stash tests SHALL continue to pass after Phase 2

`actor-core` の既存 stash 関連 test は、Phase 2 で選ばれた option を実装した後も **すべて pass しなければならない**（MUST）:

- `typed_behaviors_stash_buffered_messages_across_transition` (`modules/actor-core/src/core/typed/tests.rs`)
- `typed_behaviors_with_stash_limits_capacity` (同上)
- `typed_behaviors_with_stash_keeps_adapter_payload_after_unstash` (同上)
- `unstash_messages_are_replayed_before_existing_mailbox_messages` (`modules/actor-core/src/core/kernel/actor/actor_cell/tests.rs`)
- その他 stash 関連 test 全般

これらの test の期待値 (assert) は維持される。test の構築方法 (例: Props 構築の API、mailbox 構築方法) は **option 次第で書き換わる可能性がある** が、observable behavior (stashed → unstash 後の処理順) は不変である。

#### Scenario: All existing stash tests pass after Phase 2
- **WHEN** Phase 2 実装後に `cargo test -p fraktor-actor-core-rs --lib` を実行する
- **THEN** stash 関連の全 test が pass する
- **AND** test の assertion (受信 message の順序、count 等) は Phase 1 以前と同じ

### Requirement: `bounded + stash` combination SHALL be handled deterministically

現状の mailbox factory では `MailboxRequirement::for_stash()` と bounded mailbox policy は両立せず、`MailboxConfigError::BoundedWithDeque` で reject される。Phase 2 ではこの組み合わせを **明示的に定義** しなければならない（MUST）。

許容されるのは次のどちらかのみ:

- **Path A**: `bounded + stash` を正式サポートし、deque-capable bounded mailbox 実装を導入する
- **Path B**: `bounded + stash` は unsupported とし、spawn/configuration 時に deterministic に失敗させる

どちらにしても、silent fallback や暗黙の degrade は許されない（MUST NOT）。

#### Scenario: bounded stash never degrades silently
- **WHEN** user が bounded mailbox と stash を同時に要求する
- **THEN** system は deque-capable bounded mailbox を構成する、または deterministic な configuration failure を返す
- **AND** `prepend_via_drain_and_requeue` に暗黙 fallback しない

### Requirement: Stash → unstash ordering SHALL be preserved

Phase 2 の実装では、現状の **「stashed messages are processed before existing pending mailbox messages」** という order semantic を **維持しなければならない**（MUST）。この ordering は既存の classic / typed テストが依存している観測可能な契約であり、explore change の段階で緩めてはならない。

#### Scenario: Stashed messages observed before pending messages
- **WHEN** actor が stash → 新着 message を受信 → unstash を順次行う
- **THEN** unstashed messages が新着 message より **前に** 処理される
- **AND** `typed_behaviors_unstash_replays_before_already_queued_messages` と `unstash_messages_are_replayed_before_existing_mailbox_messages` の観測結果は維持される

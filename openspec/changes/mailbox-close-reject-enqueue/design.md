## Context

`Mailbox` の close 状態は現在 `MailboxScheduleState` に保持されている一方で、user message の queue mutation は `user_queue_lock` により直列化されている。この 2 つが別々に動いているため、close と mutation の間に TOCTOU がある。

現状の `enqueue_envelope`:

```rust
pub fn enqueue_envelope(&self, envelope: Envelope) -> Result<(), SendError> {
    if self.is_suspended() {
        return Err(SendError::suspended(envelope.into_payload()));
    }

    let enqueue_result = {
        let _guard = self.user_queue_lock.lock();
        self.user.enqueue(envelope)
    };
    // ...
}
```

現状の `become_closed_and_clean_up`:

```rust
pub fn become_closed_and_clean_up(&self) {
    self.state.close();
    if matches!(self.cleanup_policy, MailboxCleanupPolicy::DrainToDeadLetters) {
        let _guard = self.user_queue_lock.lock();
        while let Some(envelope) = self.user.dequeue() {
            // dead letter
        }
    }
    self.user.clean_up();
}
```

`state.close()` が `user_queue_lock` の外で実行されるため、producer が fast path を通過した後に cleanup が close を完了し、その後 producer が lock を取って enqueue できてしまう。

同じ問題は `prepend_user_messages` にもある:

```rust
pub(crate) fn prepend_user_messages(&self, messages: &VecDeque<AnyMessage>) -> Result<(), SendError> {
    let Some(first_message) = messages.front().cloned() else {
        return Ok(());
    };

    if self.is_suspended() {
        return Err(SendError::suspended(first_message));
    }

    let _guard = self.user_queue_lock.lock();
    // ...
}
```

`ActorCell::unstash_*` はこの経路を production で使うため、`enqueue_envelope` だけを直しても close correctness は成立しない。

## Goals / Non-Goals

**Goals**

- cleanup と mailbox-owned user queue mutation を `user_queue_lock` で直列化する
- close 後の `enqueue_envelope` / `enqueue_user` を `SendError::Closed` で拒否する
- close 後の `prepend_user_messages` / unstash を `SendError::Closed` で拒否する
- fast path と lock 取得の間に cleanup が勝った場合でも phantom enqueue を防ぐ

**Non-Goals**

- `enqueue_system` の close semantics を定義し直すこと
- `MailboxScheduleState::close()` を close+suspend に変えること
- `MessageQueue` に queue-level close を導入すること
- `BalancingDispatcher` の shared queue 経路を直すこと
- `user_queue_lock` を撤去すること

## Decisions

### 1. close correctness の authoritative boundary は `user_queue_lock` に置く

この change では、既存の `user_queue_lock` を **mailbox-owned user queue mutation の authoritative serialization boundary** として採用する。

理由:

- 既に `enqueue_envelope`, `prepend_user_messages`, `dequeue`, cleanup がこの lock と関係している
- queue-level close を導入すると `BalancingDispatcher` の shared queue と衝突する
- 現行アーキテクチャの責務境界を保ったまま race を閉じられる

### 2. `become_closed_and_clean_up` は lock を取ってから close する

`become_closed_and_clean_up` は次の順序にする:

1. `user_queue_lock` を取得
2. `state.close()`
3. 必要なら user queue を drain
4. `self.user.clean_up()`
5. cleanup 後の `user_len` を **lock 区間内で snapshot として取得**
6. lock 解放
7. `publish_metrics_with_user_len(user_len_snapshot)` を lock 解放後に呼ぶ

これにより、「cleanup が close を宣言した mailbox に対して、同じ lock を通る user mutation が後から成功する」ことを防ぐ。

`MailboxCleanupPolicy::LeaveSharedQueue` の sharing mailbox でも、この lock 順序自体は維持する。`clean_up()` が no-op でも、direct mailbox API (`enqueue_user`, `prepend_user_messages`) との直列化に意味があるためである。したがって、cleanup policy の分岐に関わらず「lock → close → queue cleanup」の順序を取る。

`publish_metrics_with_user_len` 自体は instrumentation を触るため lock 解放後に呼ぶ。一方で `number_of_messages()` の読み取りは cleanup と同じ lock 区間内で snapshot を取る。これにより non-shared queue では cleanup 直後の値を安定して publish できる。sharing mailbox では shared queue が dispatcher 経路から別途変化し得るため、この値はあくまで cleanup 時点の snapshot として扱う。

### 3. `enqueue_envelope` は fast path + lock 内再 check の 2 段構えにする

最終形は次の意図を持つ:

```rust
pub fn enqueue_envelope(&self, envelope: Envelope) -> Result<(), SendError> {
    if self.is_closed() {
        return Err(SendError::closed(envelope.into_payload()));
    }
    if self.is_suspended() {
        return Err(SendError::suspended(envelope.into_payload()));
    }

    let enqueue_result = {
        let _guard = self.user_queue_lock.lock();
        if self.is_closed() {
            return Err(SendError::closed(envelope.into_payload()));
        }
        self.user.enqueue(envelope)
    };
    // ...
}
```

外側の check は hot path の早期 reject、内側の check は close race を閉じるための authoritative check である。

この change では **内側で再 check するのは `is_closed()` のみ** とする。`is_suspended()` の TOCTOU も理論上は存在するが、それは本 change の対象外であり、close correctness に集中する。

### 4. `prepend_user_messages` も同じ lock-based close check に揃える

`prepend_user_messages` は production reachable な user message mutation path なので、`enqueue_envelope` と同じ原則で直す。

意図する形:

```rust
pub(crate) fn prepend_user_messages(&self, messages: &VecDeque<AnyMessage>) -> Result<(), SendError> {
    let Some(first_message) = messages.front().cloned() else {
        return Ok(());
    };

    if self.is_closed() {
        return Err(SendError::closed(first_message));
    }
    if self.is_suspended() {
        return Err(SendError::suspended(first_message));
    }

    let _guard = self.user_queue_lock.lock();
    if self.is_closed() {
        return Err(SendError::closed(first_message.clone()));
    }

    // 既存の capacity check / prepend / drain-and-requeue
}
```

### 5. `BalancingDispatcher` は別問題として分離する

`BalancingDispatcher::dispatch` は `Mailbox::enqueue_envelope` を通らず、shared queue に直接 `enqueue` する。

```rust
fn dispatch(&mut self, receiver: &ArcShared<ActorCell>, envelope: Envelope) -> Result<Vec<ArcShared<Mailbox>>, SendError> {
    self.shared_queue.enqueue(envelope)?;
    // ...
}
```

したがって、本 change で確立する close correctness は **mailbox 自身が所有する user queue mutation path** に限る。shared queue と team membership を伴う close semantics は dispatcher-level の別 change で扱う。

### 6. 回帰テストは直列ケースに加えて in-flight case を含める

直列テストだけでは、`is_closed()` を lock 外で 1 回足しただけの不十分な実装でも green になる。したがって、本 change では次の 2 種類の test を要求する。

1. **直列テスト**
   - close 後の `enqueue_envelope`
   - close 後の `enqueue_user`
   - close 後の `prepend_user_messages`
2. **並行回帰テスト**
   - fast path 通過後に cleanup が close を完了する interleave で `enqueue_user` が `Closed` になる
   - 同じ interleave で `prepend_user_messages` が `Closed` になる

並行回帰テストは deterministic に書く。本 change では **`#[cfg(test)]` の pre-lock hook を `base.rs` に追加する案を推奨**する。理由:

- `enqueue_envelope` の race を再現するには「fast path 通過後、lock 取得前」で thread を停止できる必要がある
- 既存の `mailbox_prepend_user_messages_blocks_concurrent_enqueue_until_prepend_finishes` は prepend 途中の block を検証する雛形として有用だが、enqueue 側の fast path 通過点を deterministic に固定するには不十分
- test-only hook なら production API を汚さずに interleave を固定できる

### 7. `clean_up()` を lock 内に移す前に queue 実装を監査する

`self.user.clean_up()` を `user_queue_lock` 区間内に移す以上、対象 `MessageQueue` 実装が `Mailbox` 側へ再入せず、`user_queue_lock` を再取得しないことを確認する必要がある。

この監査には少なくとも以下を含める:

- `UnboundedMessageQueue`
- `BoundedMessageQueue`
- `BoundedPriorityMessageQueue`
- `BoundedStablePriorityMessageQueue`
- `UnboundedPriorityMessageQueue`
- `UnboundedStablePriorityMessageQueue`
- `UnboundedDequeMessageQueue`
- `UnboundedControlAwareMessageQueue`
- `BalancingDispatcher` の `SharedMessageQueueBox`
- `base/tests.rs` の `ScriptedMessageQueue`

確認対象は「queue 内部 lock を使うこと」ではなく、「`Mailbox` 側の `user_queue_lock` と再入関係を作らないこと」である。

`SharedMessageQueueBox` は dispatcher shared queue 経路そのものの close semantics の対象ではないが、sharing mailbox では `Mailbox::user` に格納される concrete `MessageQueue` 実装である。そのため、本 change の `self.user.clean_up()` を lock 内へ移す影響確認対象には含める。

## Risks / Trade-offs

- `user_queue_lock` への依存を明示化するため、将来の outer lock 撤廃 change では再設計が必要
- `prepend_user_messages` にも修正が及ぶため、proposal 名より scope はやや広い
- hot path に lock 内の追加 check が入るが、既存 lock 段数は増えない

## Migration Plan

本 change は 1 PR で完結させる。

1. `base.rs` の close 順序を修正
2. `enqueue_envelope` の lock 内再 check を追加
3. `prepend_user_messages` の lock 内再 check を追加
4. 直列テストと並行回帰テストを追加
5. `cargo test` / `./scripts/ci-check.sh ai all` / `openspec validate --strict`

## Open Questions

- `is_suspended()` の lock 内再 check も同時に入れるかどうか
  - 本 change では見送る
  - close correctness を超える scope になるため

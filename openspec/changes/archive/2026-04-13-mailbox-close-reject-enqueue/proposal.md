## Why

`Mailbox::become_closed_and_clean_up` と user message の mutation 経路 (`enqueue_envelope`, `enqueue_user`, `prepend_user_messages`) は、現状では **close 状態を一貫して直列化していない**。そのため、cleanup が勝った後に in-flight producer が phantom enqueue する race が残っている。

現状コードの問題は次のタイムラインで表せる:

```text
T0  Producer:  is_closed() をまだ見ない / is_suspended() だけ確認
T1  Cleanup:   state.close()                      (user_queue_lock の外)
T2  Cleanup:   user_queue_lock を取得
T3  Cleanup:   user queue を drain / clean_up
T4  Cleanup:   user_queue_lock を解放
T5  Producer:  user_queue_lock を取得
T6  Producer:  user.enqueue(...)                 ← drain 済み queue に phantom enqueue
```

`is_closed()` を lock の外で 1 回足すだけではこの race は閉じない。producer が `is_closed() == false` を観測した後で cleanup が close を完了し、その後に producer が lock を取得して enqueue できてしまうためである。

さらに、問題は `enqueue_envelope` だけではない。`ActorCell::unstash_*` から到達する `prepend_user_messages` も close を見ずに `user_queue_lock` を取って mutation するため、close 後の unstash でも同じ phantom enqueue が起こり得る。

### 修正方針

本 change は **B 案 (lock-based 再 check)** を採用する。すなわち:

1. `Mailbox` が既に持っている `user_queue_lock` を、**close と user queue mutation の直列化境界** として明示的に使う
2. `become_closed_and_clean_up` は `user_queue_lock` を取得してから `state.close()` し、同じ lock 区間で drain / clean_up を完了する
3. `enqueue_envelope` と `prepend_user_messages` は、fast path の事前 check に加えて、**`user_queue_lock` 取得後に `is_closed()` を再 check** する

これにより、cleanup が lock を先に取って close した場合、待機していた producer / unstash 側は lock 取得後の再 check で `SendError::Closed` を返し、queue mutation に到達できなくなる。

## What Changes

### 変更対象

#### `Mailbox::become_closed_and_clean_up` の close 順序を修正

`modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` の `become_closed_and_clean_up` を次の方針で書き換える:

- `cleanup_policy` に関わらず `user_queue_lock` を **先に取得**
- その lock 区間の中で `state.close()` を実行
- `DrainToDeadLetters` の場合は同じ lock 区間で `user.dequeue()` による drain を実行
- `self.user.clean_up()` も同じ lock 区間で実行
- lock 解放後に `publish_metrics_with_user_len(...)`

意図は「cleanup が close を宣言した後に、同じ mailbox の user queue mutation が割り込めない」ことを保証する点にある。

#### `Mailbox::enqueue_envelope` に lock 内の `is_closed()` 再 check を追加

`enqueue_envelope` は次の 2 段構えにする:

1. **fast path**:
   - `is_closed()` を確認し、closed なら即 `SendError::Closed`
   - `is_suspended()` を確認し、suspended なら即 `SendError::Suspended`
2. **authoritative check under lock**:
   - `user_queue_lock` 取得後に `is_closed()` を再 check
   - closed なら `SendError::Closed`
   - そうでなければ `self.user.enqueue(envelope)`

これにより、cleanup が fast path と lock 取得の間に close を完了した場合でも、queue mutation 前に reject できる。

#### `Mailbox::prepend_user_messages` に lock 内の `is_closed()` 再 check を追加

`prepend_user_messages` も同じ lock-based パターンに揃える:

1. 空 batch なら `Ok(())`
2. fast path で `is_closed()` / `is_suspended()` を確認
3. `user_queue_lock` 取得後に `is_closed()` を再 check
4. その後に既存の capacity check と prepend / drain-and-requeue 経路を実行

この修正により、close 後の unstash / prepend が phantom enqueue しないことを保証する。

### 追加 test

`modules/actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs` に次のテストを追加する:

- `mailbox_enqueue_envelope_returns_closed_after_mailbox_close`
- `mailbox_enqueue_user_returns_closed_after_mailbox_close`
- `mailbox_prepend_user_messages_returns_closed_after_mailbox_close`
- `mailbox_is_closed_after_mailbox_close`
- `cleanup_close_wins_against_inflight_enqueue`
- `cleanup_close_wins_against_inflight_prepend`

後者 2 つは、**fast path 通過後に cleanup が close を完了する interleave** を再現し、lock 内の再 check が無ければ phantom enqueue していたケースが `SendError::Closed` で止まることを verify する。必要なら `#[cfg(test)]` の helper / hook を追加して deterministic に再現する。

## 触らない範囲 (Non-Goals)

- `enqueue_system` の close semantics
- `MailboxScheduleState::close()` 自体の意味変更 (`close = suspend` への変更など)
- `MessageQueue` trait への `close()` / `is_closed()` 追加
- `SharedMessageQueue` や `BalancingDispatcher::dispatch` の shared queue 経路
- `user_queue_lock` の撤廃や `put_lock` への限定化
- lock-free queue 化や `QueueStateHandle` の内側二重ロック整理

特に `BalancingDispatcher` は `Mailbox::enqueue_envelope` を経由せず shared queue へ直接 enqueue するため、本 change の修正対象外である。

## Capabilities

### Added Capabilities

- **`mailbox-close-semantics`**:
  - mailbox が closed になった後、mailbox 自身が所有する user queue mutation (`enqueue_envelope`, `enqueue_user`, `prepend_user_messages`) は `SendError::Closed` で拒否される
  - cleanup と user queue mutation の直列化が `user_queue_lock` により保証される

### Modified Capabilities

- なし

## Impact

### 影響コード

- `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`
- `modules/actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs`

### 観測可能な挙動の変化

- close 後の `enqueue_envelope` / `enqueue_user` は `SendError::Closed` を返す
- close 後の `prepend_user_messages` も `SendError::Closed` を返す
- fast path 通過後に cleanup が close を完了した場合でも、lock 内再 check により mutation は拒否される
- mailbox が理論上 `closed` かつ `suspended` の両方を満たす場合、user message enqueue 系は `Suspended` ではなく `Closed` を優先して返す

### 影響 caller

- `ActorRef::tell` 系の通常 user message path
- `ActorCell::unstash_message` / `unstash_messages` / `unstash_messages_with_limit`
- mailbox を直接使うテストや内部 helper

既存の `SendError::Closed` と `ActorError::from_send_error` の経路はそのまま使えるため、新しい error variant は不要である。

### BalancingDispatcher について

本 change は shared queue 経路を直さない。`BalancingDispatcher` の close semantics は別 change で dispatcher-level に扱う必要がある。

### 性能インパクト

- hot path に `is_closed()` の fast path check と lock 内の再 check が入る
- ただし再 check は既に取得している `user_queue_lock` 区間内で行うだけであり、追加コストは小さい
- correctness 優先の変更であり、hot path の lock 段数自体は増減しない

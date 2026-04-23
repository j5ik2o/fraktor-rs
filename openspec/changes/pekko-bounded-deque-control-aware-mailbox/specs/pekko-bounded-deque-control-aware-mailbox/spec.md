## ADDED Requirements

### Requirement: `BoundedDequeMessageQueue` は deque semantics を保ちつつ capacity を強制する

fraktor-rs は `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_deque_message_queue.rs` に `BoundedDequeMessageQueue` を提供し、以下の契約をすべて満たさなければならない (MUST):

- [`MessageQueue`](crate::core::kernel::dispatch::mailbox::message_queue::MessageQueue) trait を実装し、`enqueue` / `dequeue` / `number_of_messages` / `clean_up` を提供する。
- [`DequeMessageQueue`](crate::core::kernel::dispatch::mailbox::deque_message_queue::DequeMessageQueue) trait を実装し、`enqueue_first(envelope)` で **front 挿入**を提供する。
- `new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy)` で構築され、内部に `SharedLock<VecDeque<Envelope>>` を保持する。
- `enqueue` (back 挿入) は `overflow` に従って振舞う (Pekko `Mailbox.scala:844` `BoundedDequeBasedMailbox` 相当):
  - `Grow`: capacity を無視して push_back し、`Ok(EnqueueOutcome::Accepted)` を返す。
  - `DropNewest`: `len >= capacity` なら到着 envelope を drop し `Ok(EnqueueOutcome::Rejected(envelope))` を返す。それ以外は push_back し `Accepted`。
  - `DropOldest`: `len >= capacity` なら front を evict してから push_back し `Ok(EnqueueOutcome::Evicted(evicted))` を返す。それ以外は push_back し `Accepted`。
- `enqueue_first` (front 挿入) も同様に `overflow` に従う (Pekko には明示規定がないが、`enqueue` と同一戦略を適用する):
  - `Grow`: capacity 無視で push_front し `Ok(())`。
  - `DropNewest`: `len >= capacity` なら `Err(SendError::Full(envelope))`。それ以外は push_front し `Ok(())`。
  - `DropOldest`: `len >= capacity` なら **back** を evict してから push_front し `Ok(())` (DequeMessageQueue trait 契約により evicted 情報は返さない)。
- `dequeue` は front を `pop_front` する (Pekko の FIFO / deque semantics)。
- `as_deque(&self) -> Option<&dyn DequeMessageQueue>` は `Some(self)` を返し、stash 層が front 挿入を実行できる。

`BoundedDequeMailboxType` は `create(&self) -> Box<dyn MessageQueue>` で新しい `BoundedDequeMessageQueue` を生成する factory でなければならない (MUST)。`new(capacity, overflow)` を持ち、`MailboxType` trait を実装する。

#### Scenario: Grow strategy で capacity を超えた enqueue も受理する

- **GIVEN** `BoundedDequeMessageQueue::new(NonZeroUsize::new(2).unwrap(), MailboxOverflowStrategy::Grow)`
- **WHEN** envelope を 3 回 `enqueue` する
- **THEN** 3 回とも `Ok(EnqueueOutcome::Accepted)` が返る
- **AND** `number_of_messages()` が 3 を返す

#### Scenario: DropNewest strategy で capacity 超過時は到着 envelope を拒否する

- **GIVEN** `BoundedDequeMessageQueue::new(capacity=2, DropNewest)` に envelope A, B が enqueue 済
- **WHEN** envelope C を `enqueue(C)` する
- **THEN** `Ok(EnqueueOutcome::Rejected(C))` が返る
- **AND** `number_of_messages()` が 2 のまま (A, B 保持)
- **AND** `dequeue` を 2 回呼ぶと A, B の順で取り出せる

#### Scenario: DropOldest strategy で capacity 超過時は front を evict する

- **GIVEN** `BoundedDequeMessageQueue::new(capacity=2, DropOldest)` に envelope A, B が enqueue 済
- **WHEN** envelope C を `enqueue(C)` する
- **THEN** `Ok(EnqueueOutcome::Evicted(A))` が返る (A が evict される)
- **AND** `number_of_messages()` が 2 (B, C 保持)
- **AND** `dequeue` を 2 回呼ぶと B, C の順で取り出せる

#### Scenario: enqueue_first (front 挿入) が capacity と overflow に従う

- **GIVEN** `BoundedDequeMessageQueue::new(capacity=1, DropNewest)` に envelope A が enqueue 済
- **WHEN** envelope B を `enqueue_first(B)` する
- **THEN** `Err(SendError::Full(B))` が返る (DropNewest は到着拒否)
- **AND** `number_of_messages()` が 1 のまま (A 保持)

#### Scenario: DropOldest 下の enqueue_first は back を evict する

- **GIVEN** `BoundedDequeMessageQueue::new(capacity=2, DropOldest)` に envelope A, B が enqueue 済 (front→back = A, B)
- **WHEN** envelope C を `enqueue_first(C)` する
- **THEN** `Ok(())` が返る
- **AND** `number_of_messages()` が 2
- **AND** `dequeue` を 2 回呼ぶと C, A の順で取り出せる (B は evict された)

#### Scenario: clean_up で全 envelope を破棄する

- **GIVEN** `BoundedDequeMessageQueue` に複数 envelope が enqueue 済
- **WHEN** `clean_up()` を呼ぶ
- **THEN** `number_of_messages()` が 0 を返す
- **AND** 後続 `dequeue` が `None` を返す

---

### Requirement: `BoundedControlAwareMessageQueue` は control 優先を保ちつつ合計 capacity を強制する

fraktor-rs は `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_control_aware_message_queue.rs` に `BoundedControlAwareMessageQueue` を提供し、以下の契約をすべて満たさなければならない (MUST):

- [`MessageQueue`](crate::core::kernel::dispatch::mailbox::message_queue::MessageQueue) trait を実装する (deque trait は実装しない)。
- `new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy)` で構築され、内部に `SharedLock<Inner>` (control_queue + normal_queue の 2 本 VecDeque) を保持する。
- `number_of_messages()` は control + normal の合計長を返す。
- `enqueue(envelope)` の挙動:
  - `envelope.payload().is_control()` が true の場合は control_queue に、false の場合は normal_queue に入れる (Pekko `Mailbox.scala:931` `BoundedControlAwareMailbox` 相当)。
  - 容量判定は `control_queue.len() + normal_queue.len() >= capacity` で行う。
  - overflow 戦略に従う:
    - `Grow`: capacity 無視、対応 queue に push_back、`Accepted`。
    - `DropNewest`: capacity 超過なら `Rejected(envelope)`。
    - `DropOldest`: capacity 超過時は **normal_queue の front** を evict してから対応 queue に push_back、`Evicted(evicted_from_normal)`。normal_queue が空かつ capacity 超過の場合は control drop を避けるため `Rejected(envelope)` を返す (Decision 3)。
- `dequeue()` は control_queue.pop_front() を優先し、control が空なら normal_queue.pop_front() を返す (Pekko と同じ順序契約)。
- `clean_up()` は両 queue を clear する。

`BoundedControlAwareMailboxType` は `create(&self) -> Box<dyn MessageQueue>` で新しい `BoundedControlAwareMessageQueue` を生成する factory でなければならない (MUST)。`new(capacity, overflow)` を持ち、`MailboxType` trait を実装する。

#### Scenario: control envelope が normal より先に dequeue される

- **GIVEN** `BoundedControlAwareMessageQueue::new(capacity=10, Grow)` で envelope normal_A, normal_B が enqueue 済 (いずれも非 control)
- **WHEN** control envelope control_X (`AnyMessage::control(...)`) を enqueue、続けて dequeue を繰り返す
- **THEN** dequeue 順序は control_X, normal_A, normal_B

#### Scenario: DropOldest は normal queue の front を優先 evict する

- **GIVEN** `BoundedControlAwareMessageQueue::new(capacity=3, DropOldest)` に normal_A, normal_B, control_X が enqueue 済 (合計 3)
- **WHEN** normal_C を enqueue する
- **THEN** `Ok(EnqueueOutcome::Evicted(normal_A))` が返る (normal_A が evict された)
- **AND** `number_of_messages()` が 3 (normal_B, normal_C, control_X)
- **AND** dequeue 順序は control_X, normal_B, normal_C

#### Scenario: DropOldest 下で normal queue が空なら control envelope を Reject する

- **GIVEN** `BoundedControlAwareMessageQueue::new(capacity=2, DropOldest)` に control_X, control_Y が enqueue 済 (normal_queue 空)
- **WHEN** control_Z を enqueue する
- **THEN** `Ok(EnqueueOutcome::Rejected(control_Z))` が返る (control drop を避けるため)
- **AND** `number_of_messages()` が 2 (control_X, control_Y 保持)

#### Scenario: DropNewest で capacity 超過時は到着 envelope を拒否する

- **GIVEN** `BoundedControlAwareMessageQueue::new(capacity=2, DropNewest)` に normal_A, control_X が enqueue 済
- **WHEN** normal_B を enqueue する
- **THEN** `Ok(EnqueueOutcome::Rejected(normal_B))`
- **AND** `number_of_messages()` が 2
- **AND** dequeue 順序は control_X, normal_A

#### Scenario: Grow strategy で capacity を超えた enqueue も受理する

- **GIVEN** `BoundedControlAwareMessageQueue::new(capacity=2, Grow)` に control_X, normal_A が enqueue 済
- **WHEN** 追加で normal_B, normal_C, control_Y を enqueue する (合計 5)
- **THEN** 3 回とも `Ok(EnqueueOutcome::Accepted)` が返る
- **AND** `number_of_messages()` が 5
- **AND** dequeue 順序は control_X, control_Y, normal_A, normal_B, normal_C

---

### Requirement: `create_message_queue_from_config` は bounded + deque / bounded + control_aware を正しく dispatch する

`modules/actor-core/src/core/kernel/dispatch/mailbox/mailboxes.rs::create_message_queue_from_config` は、`MailboxConfig::requirement()` と `policy()` の組合せに応じて以下を返さなければならない (MUST):

- `needs_deque()` が true かつ `policy.capacity()` が `Bounded { capacity }` の場合: `BoundedDequeMessageQueue::new(capacity, policy.overflow())` を返す。
- `needs_deque()` が true かつ `policy.capacity()` が `Unbounded` の場合: 既存 `UnboundedDequeMessageQueue` を返す (挙動不変)。
- `needs_control_aware()` が true かつ `policy.capacity()` が `Bounded { capacity }` の場合: `BoundedControlAwareMessageQueue::new(capacity, policy.overflow())` を返す。
- `needs_control_aware()` が true かつ `policy.capacity()` が `Unbounded` の場合: 既存 `UnboundedControlAwareMessageQueue` を返す (挙動不変)。
- 他の組合せ (priority / plain bounded / plain unbounded) は現行挙動を維持する。

`MailboxConfig::validate()` は `bounded + deque` を拒否してはならない (MUST NOT)。従来 `Err(MailboxConfigError::BoundedWithDeque)` を返していたケースは `Ok(())` を返さなければならない (MUST)。`MailboxConfigError::BoundedWithDeque` variant は削除される (BREAKING)。

#### Scenario: bounded + deque 構成の validate は成功する

- **GIVEN** `MailboxConfig` with `policy = Bounded { capacity: 16, overflow: DropOldest }` and `requirement.needs_deque() == true`
- **WHEN** `config.validate()` を呼ぶ
- **THEN** `Ok(())` が返る (従来は `Err(BoundedWithDeque)`)
- **AND** `create_message_queue_from_config(&config)` が `BoundedDequeMessageQueue` を生成する

#### Scenario: bounded + control_aware 構成は BoundedControlAwareMessageQueue を返す

- **GIVEN** `MailboxConfig` with `policy = Bounded { capacity: 8, overflow: DropNewest }` and `requirement.needs_control_aware() == true`
- **WHEN** `create_message_queue_from_config(&config)` を呼ぶ
- **THEN** `Ok(BoundedControlAwareMessageQueue)` 相当の `Box<dyn MessageQueue>` が返る (従来は silently Unbounded fallback)
- **AND** 10 個 enqueue すると 8 件目以降が `EnqueueOutcome::Rejected` を返す (bounded が実効)

#### Scenario: unbounded + deque 構成の挙動は変化しない (regression)

- **GIVEN** `MailboxConfig` with `policy = Unbounded` and `requirement.needs_deque() == true`
- **WHEN** `create_message_queue_from_config(&config)` を呼ぶ
- **THEN** `UnboundedDequeMessageQueue` が生成される (本 change で挙動不変)

#### Scenario: unbounded + control_aware 構成の挙動は変化しない (regression)

- **GIVEN** `MailboxConfig` with `policy = Unbounded` and `requirement.needs_control_aware() == true`
- **WHEN** `create_message_queue_from_config(&config)` を呼ぶ
- **THEN** `UnboundedControlAwareMessageQueue` が生成される (本 change で挙動不変)

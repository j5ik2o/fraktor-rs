## ADDED Requirements
### Requirement: Dual Queue Async Mailbox
The actor runtime MUST provide a mailbox struct exposing separate system and user queues implemented with `AsyncMpscQueue<PriorityEnvelope<AnyOwnedMessage>, _>` and a blocking-aware backend that honours Drop/Grow/Block overflow policies described in `docs/mailbox-spec.md`.

#### Scenario: System queue retains priority
- **GIVEN** a mailbox created via `MailboxOptions` with both queues empty
- **WHEN** `enqueue_system(envelope)` is invoked
- **THEN** the envelope is routed to the system queue via the producer handle and becomes observable by `dequeue()` before any user-queue messages.

### Requirement: Mailbox Options and Hooks
The mailbox MUST store `MailboxOptions`, a `MailboxSignalHandle`, optional `MailboxMetricsHook`, optional `SchedulerNotifyHook`, and optional `MailboxEventPublisher`, and use them when processing enqueue/dequeue operations and reporting backpressure or growth events.

#### Scenario: Block policy triggers wait future
- **GIVEN** `MailboxOptions` configured with `MailboxPolicy::Block`
- **WHEN** the mailbox receives messages beyond capacity via `enqueue_user`
- **THEN** the producer registers with the signal handle and the returned Future remains pending until `dequeue()` frees capacity, after which instrumentation hooks are notified.

### Requirement: Public Mailbox API
The mailbox MUST expose `enqueue_system`, `enqueue_user`, `dequeue`, and `drain_blocking` (or equivalent) methods that interact with queue producer/consumer traits (`try_send_mailbox`, `try_dequeue_mailbox`, `offer_blocking`, `poll_blocking`) so that Phase 3 tasks (T016–T019) can integrate Dispatcher/MessageInvoker with no hidden queues.

#### Scenario: Dequeue falls back to user queue
- **GIVEN** a mailbox whose system queue is empty and user queue contains an envelope
- **WHEN** `dequeue()` is called
- **THEN** it returns the next user envelope by consulting the user queue consumer after confirming the system queue is empty.

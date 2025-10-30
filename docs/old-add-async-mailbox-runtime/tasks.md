# Tasks: add-async-mailbox-runtime

## 1. Shared Mailbox Backend
- [ ] Implement AsyncMpscQueue-based blocking backend in utils-core (MailboxQueueBackend traits, offer_blocking/poll_blocking futures).
- [ ] Provide PriorityEnvelope, MailboxOptions, and signal/metric hook abstractions.

## 2. Actor Core Mailbox Integration
- [ ] Create actor-core mailbox struct with system/user queues, options, signal, hooks, and event publisher wiring.
- [ ] Expose enqueue/dequeue/drain APIs mapped to producer/consumer traits and blocking futures.
- [ ] Emit metrics and EventStream notifications for drop/grow/backpressure scenarios.

## 3. Tests & Documentation
- [ ] Add unit/integration tests covering system-priority dequeue fallback, block policy futures, and hook invocation.
- [ ] Update quickstart/research notes if necessary to describe new mailbox behavior.

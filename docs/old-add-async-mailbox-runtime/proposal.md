# Change Proposal: Async Mailbox Runtime Integration

## Summary
- Implement the Phase 3 mailbox described in `specs/001-add-actor-runtime/tasks.md`, aligning with `docs/mailbox-spec.md`.
- Provide system/user priority queues backed by `AsyncMpscQueue` + blocking backend to honor Drop/Grow/Block overflow policies.
- Expose mailbox options, signals, metrics/scheduler hooks, and event publisher as per the runtime requirements.

## Motivation
- The current actor runtime lacks a concrete mailbox that satisfies the OpenSpec mailbox capability and Phase 3 tasks.
- We must unblock US1 by delivering enqueue/dequeue APIs with backpressure handling, throughput limits, and instrumentation hooks.

## Goals
- Define a mailbox struct with `system_queue`, `user_queue`, `options`, `signal`, optional metrics/scheduler hooks, and optional event publisher.
- Support system priority dispatch, user queue fallback, and blocking futures for `MailboxPolicy::Block`.
- Surface mailbox events (drop/grow/warning) to EventStream and metrics hooks.

## Non-Goals
- Full implementation of Dispatcher and MessageInvoker (handled in adjacent tasks).
- Remote/cluster mailbox variants.

## Plan
1. Extend utils-core mailbox abstractions (AsyncMpscQueue + blocking adapter) to satisfy docs/mailbox-spec.md requirements.
2. Implement `PriorityEnvelope<AnyOwnedMessage>` and `MailboxOptions` if not already present; wire them into actor-core mailbox struct.
3. Provide `enqueue_system`, `enqueue_user`, `dequeue`, and blocking drain helpers using queue producer/consumer traits.
4. Integrate metrics/scheduler hooks and event publisher triggers based on queue outcomes.
5. Update tests/docs to reflect new mailbox behavior.

## Impact
- Enables completion of Phase 3 tasks (T016–T019) by delivering required mailbox capability.
- Introduces new public API for mailbox hooks and options.

## Risks
- Concurrency bugs in blocking futures; mitigated by reusing WaitQueue-backed backends.
- Potential API churn if future dispatcher needs additional hooks (address via extensible hook traits).

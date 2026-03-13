## ADDED Requirements

### Requirement: Mailbox construction preserves policy and queue invariants
The actor runtime SHALL construct mailboxes only through paths that keep `MailboxPolicy` and the actual `MessageQueue` behavior consistent, and bounded queue operations MUST not rely on unsynchronized time-of-check/time-of-use windows.

#### Scenario: Registry-backed mailbox keeps resolved policy
- **WHEN** an actor is created from a mailbox selector or mailbox id resolved through the registry
- **THEN** the mailbox SHALL use the resolved mailbox configuration as the single source of truth for queue policy, capacity, overflow behavior, and instrumentation

#### Scenario: Externally supplied queue cannot bypass invariants
- **WHEN** mailbox construction is attempted with a pre-built queue
- **THEN** the runtime SHALL either reject inconsistent policy/queue pairs or keep the constructor internal so inconsistent pairs cannot be created outside the module

#### Scenario: Bounded queue operations are synchronized
- **WHEN** prepend, enqueue, metrics publication, or user length checks run concurrently on a bounded mailbox
- **THEN** the runtime SHALL not observe unsynchronized queue state that can produce false overflow decisions, stale metrics, or data races

### Requirement: Typed actor restarts preserve interceptor correctness
The typed actor runtime SHALL remain restart-safe under supervision, including behaviors created through interceptors or deferred initialization.

#### Scenario: Restarted intercepted behavior is recreated
- **WHEN** a supervised typed actor using an intercepted behavior receives `Started` again after restart
- **THEN** the runtime SHALL recreate or retain the intercepted behavior state without panicking or failing due to one-shot initialization

#### Scenario: Supervisor strategy access is lock-safe
- **WHEN** runtime code reads supervision strategy state during restart or failure handling
- **THEN** it SHALL do so through a lock-safe read path that does not require write-only access for queries

### Requirement: Stash inspection does not execute user callbacks under runtime locks
The typed stash buffer SHALL release actor cell internal locks before running caller-provided predicates, equality checks, or iteration callbacks.

#### Scenario: `contains` evaluates after snapshot
- **WHEN** a caller checks whether a typed message exists in the stash
- **THEN** the stash buffer SHALL snapshot matching typed messages before invoking equality comparison outside the actor cell lock

#### Scenario: `exists` and `foreach` evaluate after snapshot
- **WHEN** a caller supplies a predicate or iteration callback to inspect stashed messages
- **THEN** the stash buffer SHALL invoke that callback only after the lock has been released

### Requirement: Router and registration behavior reflect actual runtime guarantees
The actor runtime SHALL expose routing and registration behavior whose names and effects match the guarantees actually provided.

#### Scenario: Consistent hashing provides stable affinity
- **WHEN** a group router exposes a consistent-hash routing mode and the routee set changes
- **THEN** the selected routee for a key SHALL change only according to a consistent-hashing algorithm rather than a simple `hash % routee_count` remap

#### Scenario: Failed top-level registration rolls back spawned state
- **WHEN** top-level actor registration or receptionist registration fails during spawn
- **THEN** the runtime SHALL roll back any partially spawned state so no orphaned receptionist or top-level registration remains

#### Scenario: Dispatcher selectors resolve the intended blocking dispatcher
- **WHEN** props select a blocking dispatcher through registry-backed configuration
- **THEN** the selector and registry lookup SHALL resolve the same dispatcher id and executor semantics

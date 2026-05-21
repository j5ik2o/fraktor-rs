## Context

`modules/persistence-core-typed` currently exposes the effector-first API: `PersistenceEffector`, `PersistenceEffectorConfig`, `PersistenceEffectorSignal`, `PersistenceEffectorMessageAdapter`, `PersistenceId`, `PersistenceMode`, `SnapshotCriteria`, and `RetentionCriteria`. This is enough to persist event-sourced aggregates through a hidden store actor, but the Phase 1 gap-analysis still lists three low-cost parity gaps:

- typed `Recovery` / typed `SnapshotSelectionCriteria`
- typed `EventAdapter` / `EventSeq` / `SnapshotAdapter`
- `DurableStateSignal` family

The kernel crate already owns classic persistence runtime contracts such as journal event adapters, `EventSeq`, snapshot selection, snapshot stores, and durable state store traits. This change should not redesign those kernel contracts. It should add typed-facing public API wrappers in `persistence-core-typed` so later Phase 2 work can build serializer and durable state behavior contracts on stable names.

## Goals / Non-Goals

**Goals:**

- Add typed recovery selection API without replacing `PersistenceEffector` or adding a new recovery engine.
- Add typed event adapter contracts that can register typed event adaptation with the existing kernel erased event adapter pipeline.
- Add a typed snapshot adapter contract as a public conversion boundary for later serializer / durable state behavior work.
- Add typed durable state signal types for future `DurableStateBehavior` implementation.
- Keep `modules/persistence-core-typed` no_std and allocation-only.
- Keep new public types in one-type-per-file modules and re-export them from `lib.rs`.
- Add focused public-surface and behavior tests for the new contracts.

**Non-Goals:**

- Implement typed `EventSourcedBehavior`, `EffectBuilder`, or `ReplyEffect`.
- Implement typed `DurableStateBehavior` or durable state effects.
- Add persistence serializer registry, `AtomicWrite`, or storage plugin selection.
- Add `persistence-adaptor-std` or filesystem-backed snapshot stores.
- Change kernel journal / snapshot / durable state store protocols.
- Add compatibility aliases for deprecated or intermediate names.

## Decisions

### Decision 1: typed recovery selection is an explicit API, not a rename of `SnapshotCriteria`

`SnapshotCriteria<S, E>` controls when the effector writes snapshots after events are persisted. Pekko typed `Recovery` / `SnapshotSelectionCriteria` controls which persisted state is selected during recovery and which event range is replayed. These are different decisions, so this change will add separate typed recovery selection types instead of overloading `SnapshotCriteria`.

The typed recovery API should translate into the existing kernel `persistent::Recovery`. `PersistenceStoreActor` already implements `Eventsourced`, so implementation should override `Eventsourced::recovery()` and return the config-selected kernel recovery. This keeps recovery execution inside `PersistenceContext::start_recovery()` instead of adding a second recovery path.

Alternative considered: treat current `SnapshotCriteria` as enough and only update the gap document. That would leave the API ambiguous because snapshot write policy and recovery read selection would share one name but not one meaning.

### Decision 2: typed event adapters bridge to the kernel event adapter registry

Kernel event adapters work with erased `Any` payloads because journal storage is classic-runtime oriented. Typed APIs should let users implement event adaptation in terms of typed `E`, then provide conversion points into the kernel event adapter pipeline. Snapshot adaptation is handled separately as a typed conversion contract because there is no kernel snapshot adapter registry yet.

The typed `EventSeq<E>` should represent zero, one, or many typed events, matching kernel `EventSeq` semantics without requiring callers to downcast. The typed event adapter should expose manifest handling and bidirectional event adaptation, then be wrapped into kernel `ReadEventAdapter` / `WriteEventAdapter` implementations when registered on `PersistenceEffectorConfig`.

Alternative considered: re-export kernel `EventSeq`, `ReadEventAdapter`, and `WriteEventAdapter` directly. That keeps the type surface small but forces typed users to write erased payload code and does not close the typed parity gap.

### Decision 3: typed snapshot adapter is a standalone conversion contract in this phase

Kernel snapshot persistence currently stores erased snapshot payloads directly and does not have a dedicated snapshot adapter registry equivalent to journal event adapters. This change will still add a typed `SnapshotAdapter<S>` contract because Phase 1 parity needs the public boundary, but it will not claim runtime snapshot-store integration yet.

The first implementation should provide typed state to snapshot payload conversion, manifest access, and snapshot payload to typed state conversion. Wiring that contract into serializer registry or snapshot store IO is Phase 2 serializer / durable state behavior work.

Alternative considered: delay `SnapshotAdapter` entirely. That would keep Phase 1 smaller but leave one of the three explicit Phase 1 gap rows open.

### Decision 4: durable state signals are separate from `PersistenceEffectorSignal`

`PersistenceEffectorSignal<S, E>` represents event-sourced effector outcomes: recovery, persisted events, persisted snapshot, deleted snapshots, and failure. Durable state behavior has different lifecycle concepts: state recovery completed, recovery failed, durable state persisted, durable state deleted, and persistence failure.

This change will add a distinct `DurableStateSignal<S>` family. It should be public and stable enough for a future durable state behavior wrapper, but it should not imply that durable state behavior itself is implemented in this change.

Alternative considered: extend `PersistenceEffectorSignal` with durable-state variants. That would mix event-sourced and durable-state protocols and make later behavior-specific APIs harder to separate.

### Decision 5: this change only adds core typed contracts

All new types live in `modules/persistence-core-typed/src/` and use `alloc` plus existing core dependencies only. No `std::*`, filesystem, runtime task, or plugin-loading dependency is introduced. Tests should be either sibling unit tests or focused crate tests under `modules/persistence-core-typed/tests/`.

## Risks / Trade-offs

- Typed recovery selection may duplicate existing kernel names -> Use typed names to expose typed crate parity, but translate them into kernel `persistent::Recovery` at the `Eventsourced::recovery()` boundary.
- Adapter wrappers may become too broad -> Keep event adapter integration to kernel event adapter registration, and keep snapshot adapter as a standalone conversion contract until serializer work.
- Durable state signals may pre-decide too much of Phase 3 -> Limit them to signal data and construction / matching tests; do not add durable state behavior or effects.
- Public API naming may conflict with later Pekko direct DSL work -> Use names that match the Phase 1 gap and keep direct `EventSourcedBehavior` / `DurableStateBehavior` outside this change.

## Migration Plan

1. Add typed recovery selection types and config wiring, preserving existing default behavior through `Eventsourced::recovery()`.
2. Add typed event adapter contracts, config registration, kernel adapter wrappers, and public-surface tests.
3. Add typed snapshot adapter contract and public-surface tests without runtime snapshot store integration.
4. Add durable state signal types and public-surface tests.
5. Re-export new public types from `fraktor-persistence-core-typed-rs`.
6. Update `docs/gap-analysis/persistence-gap-analysis.md` Phase 1 statuses.
7. Run targeted `persistence-core-typed` tests, OpenSpec validation, and diff checks.

Rollback is deleting this active change before implementation. After implementation, rollback is a normal revert because this repository is pre-release and no compatibility layer is required.

## Open Questions

No unresolved semantic questions. Exact method names should follow existing `SnapshotCriteria`, `PersistenceEffectorConfig`, and kernel adapter naming during implementation.

## 1. Baseline and API Boundary

- [x] 1.1 Confirm current `modules/persistence-core-typed` public exports, sibling test layout, and no_std constraints before editing.
- [x] 1.2 Confirm kernel recovery, snapshot selection, event adapter, event sequence, and snapshot adapter contracts that typed wrappers should reuse instead of duplicating.
- [x] 1.3 Run the current targeted typed persistence tests to capture the baseline behavior.

## 2. Typed Recovery Selection API

- [x] 2.1 Add one-type-per-file public typed recovery selection types for `Recovery` and `SnapshotSelectionCriteria`.
- [x] 2.2 Wire recovery selection into `PersistenceEffectorConfig` with default behavior unchanged.
- [x] 2.3 Override `Eventsourced::recovery()` in `PersistenceStoreActor` so typed recovery selection translates into kernel `persistent::Recovery`.
- [x] 2.4 Add focused tests for default recovery, snapshot-disabled recovery, sequence-bound snapshot selection, timestamp-bound snapshot selection, and replay limit propagation.

## 3. Typed Event and Snapshot Adapter API

- [x] 3.1 Add typed `EventSeq<E>` with empty, single, multiple, length, and into-events behavior.
- [x] 3.2 Add typed `EventAdapter<E>` contract for manifest, to-journal, and from-journal adaptation.
- [x] 3.3 Add typed `SnapshotAdapter<S>` contract for manifest, to-snapshot, and from-snapshot adaptation.
- [x] 3.4 Add config registration and kernel adapter wrappers needed to connect typed event adapters to the existing kernel erased event adapter pipeline.
- [x] 3.5 Keep typed snapshot adapter as a standalone conversion contract; do not add snapshot-store runtime integration in this change.
- [x] 3.6 Add tests covering one-to-many event read adaptation, manifest propagation, event adapter registration, and snapshot round trip.

## 4. Typed Durable State Signals

- [x] 4.1 Add `DurableStateSignal<S>` as a separate public signal family from `PersistenceEffectorSignal<S, E>`.
- [x] 4.2 Include recovery completed, recovery failed, state persisted, state deleted, and persistence failed variants with appropriate error payloads.
- [x] 4.3 Add public-surface tests proving durable state signals can be wrapped by user private messages without importing internal store protocol.

## 5. Re-exports, Docs, and Gap Status

- [x] 5.1 Re-export all Phase 1 parity types from `modules/persistence-core-typed/src/lib.rs`.
- [x] 5.2 Keep new public types in one-type-per-file modules and add sibling unit tests where behavior is local to the type.
- [x] 5.3 Update `docs/gap-analysis/persistence-gap-analysis.md` so Phase 1 marks the three implemented items as done or clearly explains any intentional non-goal.

## 6. Verification

- [x] 6.1 Run targeted tests for `modules/persistence-core-typed`.
- [x] 6.2 Run targeted no_std / dylint checks required by the touched persistence crate.
- [x] 6.3 Run `mise exec -- openspec validate add-persistence-typed-phase1-parity --strict`.
- [x] 6.4 Run `git diff --check`.

## Why

fraktor-rs persistence currently stores `PersistentRepr` and `Snapshot` payloads as erased `dyn Any` values, which is sufficient for in-memory execution but does not define the Pekko-style serialization boundary needed by durable journal and snapshot store implementations.
Issue #529 tracks this gap as G-020; closing it requires a first-class persistence serialization layer that delegates domain payload encoding to the existing actor serialization registry.

## What Changes

- Add Pekko-compatible persistence serialization concepts for `PersistentRepr`, `AtomicWrite`, and snapshot payload wrappers.
- **BREAKING**: Change journal write contracts from raw `Vec<PersistentRepr>` batches to explicit `AtomicWrite` units so all-or-none write boundaries are represented in the public persistence API.
- Register persistence serializers automatically when the persistence extension is installed, using the actor serialization extension as the nested payload serializer registry.
- Keep byte-level compatibility with Pekko protobuf formats out of scope; this change targets responsibility/API compatibility under fraktor-rs no_std constraints.
- Keep disk-backed journal and local snapshot store implementations out of scope; future std adapters can use the serializer contracts introduced here.

## Capabilities

### New Capabilities
- `persistence-serialization`: Pekko-style persistence serialization contracts for journal records, atomic writes, and snapshot payload wrappers.

### Modified Capabilities

## Impact

- Affected crates: `modules/persistence-core-kernel`, `modules/actor-core-kernel`.
- Affected APIs: `Journal`, `JournalMessage::WriteMessages`, `InMemoryJournal`, persistence extension installation, serialization registry runtime registration surface.
- Affected tests: persistence journal actor tests, in-memory journal tests, serializer round-trip tests, no_std/check and relevant dylints.
- References: Pekko `Persistent.scala`, `MessageSerializer.scala`, `SnapshotSerializer.scala`, `AsyncWriteJournal.scala`, `LeveldbStore.scala`, and `LocalSnapshotStore.scala`.

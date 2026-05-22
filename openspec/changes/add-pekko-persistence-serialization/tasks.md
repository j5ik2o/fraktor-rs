## 1. Atomic Write Contract

- [ ] 1.1 Add `AtomicWrite` as a public persistence type with non-empty and single-persistence-id construction checks.
- [ ] 1.2 Add focused unit tests for valid creation, empty payload rejection, mixed persistence id rejection, and sequence accessors.
- [ ] 1.3 Update `Journal` to accept `&[AtomicWrite]` for write operations.
- [ ] 1.4 Update `JournalMessage::WriteMessages`, `JournalActor`, and journal response paths to iterate atomic write payloads for per-event responses.
- [ ] 1.5 Update `InMemoryJournal` and persistence tests/examples to use `AtomicWrite`.

## 2. Persistence Serializer Types

- [ ] 2.1 Add a `serialization` module under `persistence-core-kernel` with public `MessageSerializer` and `SnapshotSerializer` exports.
- [ ] 2.2 Add any required persistence serialization wrapper type for snapshot data without conflicting with the existing `snapshot::Snapshot` container.
- [ ] 2.3 Implement internal no_std encoding/decoding helpers for nested `SerializedMessage` records and persistence metadata fields.
- [ ] 2.4 Implement `MessageSerializer` for `PersistentRepr` and `AtomicWrite`, delegating payload and metadata through `SerializationDelegator` while preserving durable metadata including sender and adapter type id.
- [ ] 2.5 Implement `SnapshotSerializer` for snapshot payload wrappers, delegating data through `SerializationDelegator`.
- [ ] 2.6 Add serializer round-trip tests for `PersistentRepr`, `AtomicWrite`, snapshot data, metadata, sender, adapter type id, runtime event adapter registry behavior, and unregistered payload failure.

## 3. Automatic Registration

- [ ] 3.1 Add persistence serializer ids and a registration helper that registers serializers and type bindings against `SerializationRegistry`.
- [ ] 3.2 Expose the minimal actor serialization extension API needed to register serializer instances at runtime, following existing collision behavior.
- [ ] 3.3 Update `PersistenceExtensionInstaller` to ensure the actor serialization extension exists and register persistence serializers before installing persistence actors.
- [ ] 3.4 Add installer tests for default serialization extension creation and augmentation of an existing custom serialization extension.

## 4. Documentation and Validation

- [ ] 4.1 Update persistence gap analysis to mark `AtomicWrite`, `MessageSerializer`, and `SnapshotSerializer` according to the implemented scope.
- [ ] 4.2 Run targeted persistence-core-kernel tests.
- [ ] 4.3 Run serialization-related actor-core-kernel tests touched by runtime registration changes.
- [ ] 4.4 Run `cargo fmt --check`.
- [ ] 4.5 Run the relevant `./scripts/ci-check.sh ai ...` checks for persistence, no_std, and dylint coverage.

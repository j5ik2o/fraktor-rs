## 1. Atomic Write Contract

- [x] 1.1 Add `AtomicWrite` as a public persistence type with non-empty and single-persistence-id construction checks.
- [x] 1.2 Add focused unit tests for valid creation, empty payload rejection, mixed persistence id rejection, and sequence accessors.
- [x] 1.3 Update `Journal` to accept `&[AtomicWrite]` for write operations.
- [x] 1.4 Update `JournalMessage::WriteMessages`, `JournalActor`, and journal response paths to iterate atomic write payloads for per-event responses.
- [x] 1.5 Update `InMemoryJournal` and persistence tests/examples to use `AtomicWrite`.
- [x] 1.6 Add tests for backends that reject unsupported multi-entry atomic writes without partial persistence.

## 2. Persistence Serializer Types

- [x] 2.1 Add a `serialization` module under `persistence-core-kernel` with public `MessageSerializer` and `SnapshotSerializer` exports.
- [x] 2.2 Add any required persistence serialization wrapper type for snapshot data without conflicting with the existing `snapshot::Snapshot` container.
- [x] 2.3 Implement internal no_std encoding/decoding helpers for nested `SerializedMessage` records and persistence metadata fields.
- [x] 2.4 Implement `MessageSerializer` for `PersistentRepr` and `AtomicWrite`, delegating payload and metadata through `SerializationDelegator` while preserving durable metadata and excluding runtime-only sender, `EventAdapters`, and Rust `TypeId` adapter keys from durable bytes.
- [x] 2.5 Implement `SnapshotSerializer` for snapshot payload wrappers, delegating data through `SerializationDelegator`.
- [x] 2.6 Add serializer round-trip tests for `PersistentRepr`, `AtomicWrite`, snapshot data, metadata, runtime replay context exclusion, unregistered payload failure, and non-manifest-resolvable payload failure.

## 3. Automatic Registration

- [x] 3.1 Add persistence serializer ids and a registration helper that registers serializers and type bindings against `SerializationRegistry`.
- [x] 3.2 Define fail-fast collision handling for occupied persistence serializer ids and conflicting persistence type bindings.
- [x] 3.3 Expose the minimal actor serialization bootstrap API needed to compose persistence serializers with default and custom serialization setup before `SerializationExtension` instantiation.
- [x] 3.4 Update `PersistenceExtensionInstaller` to contribute persistence serializers without installing a default serialization extension that can shadow later custom setup.
- [x] 3.5 Add installer tests for default serialization extension creation, custom setup installed before persistence, custom setup installed after persistence, serializer id collision failure, and persistence type binding collision failure.

## 4. Documentation and Validation

- [x] 4.1 Update persistence gap analysis to mark `AtomicWrite`, `MessageSerializer`, and `SnapshotSerializer` according to the implemented scope.
- [x] 4.2 Run targeted persistence-core-kernel tests.
- [x] 4.3 Run serialization-related actor-core-kernel tests touched by runtime registration changes.
- [x] 4.4 Run `cargo fmt --check`.
- [x] 4.5 Run the relevant `./scripts/ci-check.sh ai ...` checks for persistence, no_std, and dylint coverage.

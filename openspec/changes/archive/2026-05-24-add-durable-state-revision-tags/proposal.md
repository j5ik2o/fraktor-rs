## Why

Issue #521 still has an unresolved durable state write-side gap: loads expose revisions, but updates and deletes cannot validate an expected revision or attach tags for downstream change classification. Without this contract, future typed `DurableStateBehavior` work has no optimistic concurrency boundary to build on.

## What Changes

- **BREAKING**: Change durable state write APIs to carry expected revision information for upsert and delete operations.
- Add tag-aware durable state update metadata so stores can classify changes for future change streams.
- Keep `GetObjectResult` and `DurableStateError::DeleteRevision` as existing prerequisites instead of reintroducing them in this change.
- Keep full typed `DurableStateBehavior` and durable state effect DSL out of scope.

## Capabilities

### New Capabilities

- `persistence-durable-state-store`: durable state store contracts for revision-aware writes, deletes, and tagged change metadata.

### Modified Capabilities

## Impact

- Affected crates: `modules/persistence-core-kernel`, and tests under the same crate.
- Affected APIs: `DurableStateStore`, `DurableStateUpdateStore`, in-memory/test store implementations, durable state errors if an upsert revision mismatch needs a first-class error.
- Affected docs: persistence gap analysis and issue #521 status.
- Compatibility: breaking API change is acceptable because fraktor-rs is pre-release and should not keep legacy value-only write paths.

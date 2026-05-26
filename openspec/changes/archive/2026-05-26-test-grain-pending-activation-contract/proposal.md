## Why

The accepted Grain runtime operational contract says distributed activation must expose pending resolution until placement commands complete. Existing tests exercise the coordinator command flow, but the public `IdentityLookup::resolve` pending behavior is not isolated as its own contract.

## What Changes

- Add focused contract tests for `PartitionIdentityLookup` with distributed activation enabled.
- Verify the first public `resolve` returns `LookupError::Pending` while activation commands are outstanding.
- Verify repeated public `resolve` calls stay pending until command results complete, instead of fabricating a PID or returning a stale cache entry.
- Verify completion through placement command results makes a later public `resolve` return the stored PID.
- Keep this change test-first and narrow; fix only behavior directly exposed by these tests.

## Capabilities

### New Capabilities

- `cluster-grain-runtime-contract-tests`: Test coverage contract for Grain runtime operational scenarios that must be executable in `cluster-core`.

### Modified Capabilities

- None.

## Impact

- Affected code: `modules/cluster-core/src/identity/*_test.rs` and, only if tests expose a defect, minimal `modules/cluster-core/src/identity` / `modules/cluster-core/src/placement` implementation changes.
- Public APIs: none expected.
- Dependencies: none expected.
- Runtime behavior: no intended change beyond correcting any pending-resolution defect found by the tests.

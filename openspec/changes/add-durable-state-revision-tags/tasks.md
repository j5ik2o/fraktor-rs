## 1. Durable State Write Contract

- [x] 1.1 Add focused tests for revision-aware upsert success, upsert revision mismatch, delete success, and delete revision mismatch.
- [x] 1.2 Update `DurableStateStore` to require expected revision parameters for upsert and delete operations.
- [x] 1.3 Add a deterministic upsert revision mismatch error if the existing error surface is not specific enough.
- [x] 1.4 Update all in-repo durable state store test implementations to enforce no-mutation-on-mismatch semantics.

## 2. Tagged Change Metadata

- [x] 2.1 Add a durable state change record type that carries offset, persistence id, revision, tag, and value.
- [x] 2.2 Update `DurableStateUpdateStore::changes` to query by tag and offset instead of persistence id and offset.
- [x] 2.3 Add tests for tagged update lookup, untagged update exclusion, and tag isolation.

## 3. Documentation and Validation

- [x] 3.1 Update persistence gap analysis and issue #521 status notes to reflect the remaining/resolved durable state revision and tag scope.
- [x] 3.2 Run targeted `persistence-core-kernel` state tests.
- [x] 3.3 Run `cargo fmt --check`.
- [x] 3.4 Run relevant `./scripts/ci-check.sh ai ...` checks for persistence-core-kernel and no_std/dylint coverage touched by this change.

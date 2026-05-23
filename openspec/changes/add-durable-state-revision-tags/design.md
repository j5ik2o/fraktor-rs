## Context

`DurableStateStore::get_object` already returns `GetObjectResult<A>` with a revision, and `DurableStateError::DeleteRevision` already represents delete revision mismatch. The remaining gap from issue #521 is that writes are still value-only: `upsert_object(persistence_id, object)` and `delete_object(persistence_id)` cannot enforce optimistic concurrency, and `DurableStateUpdateStore::changes` is keyed by persistence id instead of tags.

The current persistence gap analysis identifies revision/tag-aware durable state updates as the remaining medium-sized durable state store gap.

## Goals / Non-Goals

**Goals:**

- Make durable state upsert and delete revision-aware.
- Preserve all-or-nothing write semantics when the expected revision does not match the stored revision.
- Add tag-aware durable state update metadata so future change streams can query updates by tag.
- Keep the contract in `persistence-core-kernel` and maintain no_std compatibility.

**Non-Goals:**

- Do not implement typed `DurableStateBehavior`.
- Do not add a durable state effect DSL.
- Do not introduce std-backed durable state storage.
- Do not keep legacy value-only write methods as compatibility shims.

## Decisions

1. Use explicit expected revision parameters on write methods.

   `upsert_object` and `delete_object` should require the caller's expected revision. This keeps optimistic concurrency at the durable store boundary instead of leaving it to typed behavior code. The alternative was to keep store writes value-only and enforce revision in future typed APIs, but that would let backend implementations bypass the invariant.

2. Treat revision mismatch as a failed write with no partial mutation.

   On mismatch, the store must leave object value, revision, and change log untouched. This makes retry behavior deterministic and matches the existing `DeleteRevision` error direction. If upsert needs its own first-class error variant, it should be added rather than overloading a generic string error.

3. Attach tags to upserted updates, not to the durable state identity.

   A tag classifies a change event for update-stream queries. It should not become part of the persistence id or object key. Untagged updates remain valid, but they must not appear in a tag query.

4. Replace `changes(persistence_id, offset)` with tag-oriented update lookup.

   The existing method shape reports updates for one persistence id. Pekko's durable state changes are classified by tag, so the contract should move to `changes(tag, offset)` and return change metadata including persistence id, revision, and value. A compact record type is preferable to tuple growth because the offset, persistence id, revision, value, and tag have different meanings.

## Risks / Trade-offs

- Breaking trait implementations could touch several tests at once -> update all local test stores in the same change and avoid compatibility wrappers.
- Adding change metadata can invite over-designed stream APIs -> keep this change to a single-step query contract and defer streaming adapters.
- Revision numbering for missing objects can be ambiguous -> define empty/missing revision as `0`, so first successful upsert expects `0` and produces revision `1`.
- Delete change classification is unclear without a delete tag parameter -> keep delete revision-aware in this change; do not require tagged delete notifications unless a future durable state behavior change needs them.

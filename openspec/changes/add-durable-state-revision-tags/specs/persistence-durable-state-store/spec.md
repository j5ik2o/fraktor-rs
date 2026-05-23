## ADDED Requirements

### Requirement: Revision-aware durable state writes
The durable state store contract SHALL require callers to provide an expected revision for upsert and delete operations. The store MUST compare the expected revision with the currently stored revision before mutating durable state.

#### Scenario: First upsert creates revision one
- **WHEN** a durable state store receives an upsert for a missing persistence id with expected revision `0`
- **THEN** the store persists the value and subsequent load returns revision `1`

#### Scenario: Upsert revision mismatch is rejected
- **WHEN** a durable state store receives an upsert whose expected revision does not match the currently stored revision
- **THEN** the store returns a deterministic revision mismatch error
- **AND** the previous value, previous revision, and update log are not changed

#### Scenario: Delete revision mismatch is rejected
- **WHEN** a durable state store receives a delete whose expected revision does not match the currently stored revision
- **THEN** the store returns a delete revision mismatch error
- **AND** the object remains loadable at the previous revision

#### Scenario: Delete removes matching revision
- **WHEN** a durable state store receives a delete whose expected revision matches the currently stored revision
- **THEN** the store removes the object
- **AND** subsequent load returns an empty result with revision `0`

### Requirement: Tagged durable state update lookup
The durable state update store contract SHALL expose tag-oriented change lookup. Successful tagged upserts MUST record change metadata containing offset, persistence id, new revision, tag, and value.

#### Scenario: Tagged upsert is visible by tag
- **WHEN** a successful upsert stores a value with tag `orders`
- **THEN** querying changes for tag `orders` after the previous offset returns that update with its persistence id, new revision, tag, value, and next offset

#### Scenario: Untagged upsert is not returned by tag query
- **WHEN** a successful upsert stores a value without a tag
- **THEN** querying changes for any tag does not return that untagged update

#### Scenario: Different tag is isolated
- **WHEN** updates are stored under tags `orders` and `payments`
- **THEN** querying changes for tag `orders` returns only `orders` updates

### Requirement: Legacy value-only durable state writes are removed
The durable state public traits MUST NOT expose value-only upsert or delete methods that bypass revision checks.

#### Scenario: Store implementations implement revision-aware signatures
- **WHEN** a durable state store implements the public durable state traits
- **THEN** it implements upsert and delete methods with expected revision parameters
- **AND** no public value-only upsert or delete path remains on those traits

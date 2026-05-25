## 1. Boundary Documentation

- [x] 1.1 Review local, static, and AWS ECS provider behavior against the new provider boundary spec.
- [x] 1.2 Add a focused provider boundary note under `docs/plan/`.
- [x] 1.3 Link the boundary note from the cluster Grain runtime roadmap.

## 2. Contract Coverage

- [x] 2.1 Confirm existing local provider tests cover explicit join, leave, and down membership input.
- [x] 2.2 Confirm existing static provider tests cover configured topology publication without discovery.
- [x] 2.3 Confirm existing std adapter tests cover remoting subscription lifetime and weak provider retention.
- [x] 2.4 Confirm existing AWS ECS provider tests cover startup, explicit down, unsupported join/leave, and shutdown boundary.

## 3. Verification

- [x] 3.1 Run OpenSpec validation for `document-cluster-provider-boundary`.
- [x] 3.2 Run targeted cluster provider tests for `cluster-core` and `cluster-adaptor-std`.
- [x] 3.3 Run formatting checks for touched Markdown and Rust files.

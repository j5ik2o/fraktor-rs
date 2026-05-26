## Context

`cluster-core` already has `FailureDetector`, `FailureDetectorRegistry`, `MembershipCoordinator`, `MembershipEvent::MarkedSuspect`, `CurrentClusterState::unreachable`, and a `DowningProvider` hook used by explicit `down` commands. The provider boundary change fixed the input side: providers produce topology or departure input, while Grain runtime consumes provider-neutral state.

The missing boundary is the step between failure observation and member departure. Today `suspect_timeout` can drive membership transitions, but the decision model is not documented as a standalone contract. Without that boundary, future SBR, reachability matrix, or rebalance work can leak into provider and Grain runtime semantics.

## Goals / Non-Goals

**Goals:**

- Define the minimum failure observation contract for suspect / reachable / unreachable state.
- Define where downing decisions are made and how they become member departure input.
- Keep `cluster-core` as the owner of the decision port and membership state semantics.
- Keep std adapters limited to detector implementations, timers, networking, and runtime execution.
- Map the contract to existing membership / failure detector / downing tests, adding small targeted tests only where a gap is found.

**Non-Goals:**

- Implement Split Brain Resolver behavior.
- Introduce a reachability matrix or full gossip reachability semantics.
- Implement rebalance, remembered entities, in-flight drain, or recovery behavior.
- Add provider-specific failure policy to local / static / AWS ECS providers.
- Define Pekko public API parity for cluster downing.

## Decisions

### Decision 1: failure-downing-minimum is a new capability

The provider boundary spec deliberately leaves downing policy outside its scope. A separate `failure-downing-minimum` capability keeps the next contract focused on failure observation and decision flow without mixing it with provider discovery or Grain placement.

Alternative: add downing requirements to `cluster-provider-boundary`. This would blur provider input with failure policy and make providers appear responsible for downing decisions.

### Decision 2: suspect / unreachable is observation, not departure

Failure detectors and membership coordination can mark a member as suspect or unreachable, but that observation is not the same as a member departure. Departure input begins only when an explicit down command or downing decision removes the authority from active topology.

Alternative: treat suspect timeout as implicit downing everywhere. This is simpler but hides the policy boundary and makes SBR or manual downing hard to introduce later.

### Decision 3: DowningProvider is the decision boundary

`DowningProvider` should remain the core-owned port for downing behavior. The change should evaluate whether the port needs to grow from explicit `down(authority)` into a decision contract that can consume failure observation and return a down / keep / defer decision.

Alternative: make `MembershipCoordinator` own downing decisions directly. This reduces indirection but couples failure detection, policy, and topology mutation too tightly.

### Decision 4: Grain runtime only consumes member departure input

Identity lookup, placement, activation, and PID cache invalidation should continue to observe provider-neutral topology and departure input. They should not inspect phi values, suspect timers, SBR choices, or detector-specific state.

Alternative: let Grain runtime inspect unreachable state directly. This would make placement policy depend on failure detector details and duplicate membership semantics.

## Risks / Trade-offs

- [Risk] The minimal contract may be too weak for future SBR. -> Mitigation: explicitly leave SBR as a future capability and keep this change to port shape and state transitions.
- [Risk] `DowningProvider` changes could be premature. -> Mitigation: start from current tests and add only the smallest API needed to represent decision output.
- [Risk] Suspect timeout behavior may already imply a policy. -> Mitigation: document whether current timeout is the default downing strategy or only a coordinator transition before changing code.
- [Risk] Provider boundary and downing boundary may overlap. -> Mitigation: provider specs own discovery/topology input; this capability owns failure observation and decision semantics only.

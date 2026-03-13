## ADDED Requirements

### Requirement: `Source::create` honors asynchronous producer pacing
The std streams API SHALL allow `Source::create` producers to emit values asynchronously without failing solely because the producer is slower than a tight synchronous polling budget.

#### Scenario: Slow producer does not trip idle budget
- **WHEN** a background producer emits elements with observable delay between `offer()` calls
- **THEN** `Source::create` SHALL continue waiting according to its asynchronous contract instead of permanently failing with `StreamError::WouldBlock`

### Requirement: Source queue backpressure preserves pending work and wake discipline
Source queue implementations SHALL preserve pending offers until they are accepted or terminally rejected, and they MUST emit wake notifications only when state transitions can make progress.

#### Scenario: Backpressure does not silently drop buffered offers
- **WHEN** a source queue is in backpressure mode and multiple offers are pending
- **THEN** the queue SHALL retain or reject each offer explicitly according to the overflow contract rather than silently discarding accepted work

#### Scenario: Poll does not self-wake without state change
- **WHEN** `QueueOfferFuture::poll` is called while no progress is possible
- **THEN** it SHALL not repeatedly wake itself unless a state transition has occurred that can change the poll result

#### Scenario: Progress wakes waiting tasks
- **WHEN** capacity becomes available or terminal state changes the outcome of a pending offer
- **THEN** waiting tasks SHALL receive a wake notification and tests SHALL be able to observe that transition

### Requirement: Async callback and timer outputs survive intermediate apply failure
The graph interpreter SHALL not lose outputs drained from async callbacks or timers when a subsequent stage apply fails with a disposition that continues or completes processing.

#### Scenario: Continue after apply failure retains drained outputs
- **WHEN** async or timer outputs are collected and a later `apply` call fails with a disposition that continues execution
- **THEN** those collected outputs SHALL remain available for later delivery instead of being discarded

#### Scenario: Complete after apply failure retains drained outputs until terminal handling
- **WHEN** async or timer outputs are collected and a later `apply` call transitions the stage toward completion
- **THEN** the runtime SHALL preserve those outputs through terminal handling so they are not irrecoverably lost

### Requirement: Actor-backed source and sink stages match their published contracts
Actor-backed stream stages SHALL implement the delivery, acknowledgement, cancellation, and terminal-state behavior advertised by their public API names.

#### Scenario: `actor_ref` forwards elements to the target actor
- **WHEN** a stage is built with `actor_ref`
- **THEN** delivered elements SHALL reach the target actor rather than being ignored

#### Scenario: `actor_ref_with_backpressure` waits for acknowledgements
- **WHEN** a stage is built with `actor_ref_with_backpressure`
- **THEN** element delivery SHALL honor the advertised acknowledgement protocol before additional elements are considered accepted

#### Scenario: Graceful cancellation closes handles deterministically
- **WHEN** a source queue or actor-backed source is cancelled through its graceful completion path
- **THEN** associated handles and completion watchers SHALL transition to closed or completed state deterministically

## ADDED Requirements

### Requirement: Typed behaviors expose Pekko-style supervision DSL
`Behaviors::supervise` MUST wrap an existing `Behavior` and allow specifying a `SupervisorStrategy` before spawning typed actors.

#### Scenario: Builder syntax mirrors Pekko usage
- **GIVEN** 任意の `Behavior<M, TB>`
- **WHEN** `Behaviors::supervise(behavior).on_failure(strategy)` を呼び出す
- **THEN** 返される `Behavior` は再度 `Behaviors::supervise` でラップしなくても戦略を保持し続ける
- **AND** `SupervisorStrategy` は clone され、`BehaviorRunner` が後続の `Behavior` 遷移を通じて保持できる

### Requirement: Typed runtime honors behavior-level supervisor strategy
`BehaviorRunner` MUST expose the overridden `SupervisorStrategy` through the typed actor interface so that parents in the untyped runtime enforce it.

#### Scenario: Restart directive propagates to parent
- **GIVEN** `Behaviors::supervise(counter).on_failure(custom_strategy)` から作られた typed actor
- **WHEN** `counter` が `ActorError::Recoverable` を返し失敗する
- **THEN** 親の `ActorCell` へ報告される `SupervisorStrategy` が `custom_strategy` となり、`SupervisorDirective::Restart` 決定ロジックにその設定が用いられる

#### Scenario: Stop directive propagates to parent
- **GIVEN** `Behaviors::supervise(worker).on_failure(stop_strategy)` のように停止戦略を設定した typed actor
- **WHEN** `worker` が `ActorError::Fatal` を返す
- **THEN** 親は `SupervisorDirective::Stop` を選択し、子アクターを停止させる

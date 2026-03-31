## ADDED Requirements

### Requirement: typed/internal/ package が新設され内部実装型と scheduler 実装が隔離される
`modules/actor/src/core/typed/internal/` を新設し、Pekko `typed/internal/` に対応する内部実装型を集約しなければならない。移設対象は `BehaviorRunner`・`BehaviorSignalInterceptor`・`TypedActorAdapter`・`ReceiveTimeoutConfig`、および `typed/scheduler/` の内部実装（`SchedulerContext`・`TypedSchedulerGuard`・`TypedSchedulerShared`）である。`internal` module は `pub(crate)` visibility とし、クレート外には公開しない。`typed/scheduler/` ディレクトリは internal への吸収後に削除する。

#### Scenario: internal package 配下に内部実装型が配置される
- **WHEN** `modules/actor/src/core/typed/internal/` の構造を確認する
- **THEN** `behavior_runner.rs`・`behavior_signal_interceptor.rs`・`typed_actor_adapter.rs`・`receive_timeout_config.rs`・`scheduler_context.rs`・`typed_scheduler_guard.rs`・`typed_scheduler_shared.rs` が存在する

#### Scenario: internal module が pub(crate) で宣言される
- **WHEN** `modules/actor/src/core/typed.rs` の mod 宣言を確認する
- **THEN** `pub(crate) mod internal;` が存在し `pub mod internal;` は存在しない

#### Scenario: typed/scheduler/ ディレクトリが削除される
- **WHEN** `modules/actor/src/core/typed/` を確認する
- **THEN** `scheduler/` ディレクトリと `scheduler.rs` は存在しない

#### Scenario: typed root から BehaviorSignalInterceptor が pub use されない
- **WHEN** `modules/actor/src/core/typed.rs` の pub use 宣言を確認する
- **THEN** `BehaviorSignalInterceptor` は root 直下の `pub use` に含まれない

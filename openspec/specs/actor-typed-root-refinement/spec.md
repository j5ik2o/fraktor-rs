## ADDED Requirements

### Requirement: TypedActorRef が typed root へ昇格する
`TypedActorRef` は `crate::core::typed::actor::TypedActorRef` から `crate::core::typed::TypedActorRef`（root level）へ昇格しなければならない。Pekko では `ActorRef[T]` が `org.apache.pekko.actor.typed.ActorRef` として root 公開面に存在することに対応する。

#### Scenario: TypedActorRef が typed root から直接参照できる
- **WHEN** `modules/actor/src/core/typed.rs` の公開面を確認する
- **THEN** `TypedActorRef` が root から直接 `pub use` または root レベルの `actor_ref.rs` として参照できる

### Requirement: typed root 公開面が Pekko typed root 相当の基盤型に限定される
`modules/actor/src/core/typed.rs` の root `pub use` は、Pekko `typed/` root に対応する基盤型（`Behavior`・`BehaviorInterceptor`・`BehaviorSignal`・`TypedProps`・`SpawnProtocol`・`TypedActorSystem`・`ActorRefResolver`・`ActorRefResolverId`・`DispatcherSelector`・`ExtensionSetup`・`MailboxSelector`・`RecipientRef`・`DeathPactException`・`TypedActorRef`）のみでなければならない。

#### Scenario: typed root の pub use が基盤型のみになる
- **WHEN** `modules/actor/src/core/typed.rs` の `pub use` 宣言を確認する
- **THEN** `Behavior`・`BehaviorInterceptor`・`BehaviorSignal`・`TypedProps`・`SpawnProtocol`・`TypedActorSystem`・`ActorRefResolver`・`ActorRefResolverId`・`DispatcherSelector`・`ExtensionSetup`・`MailboxSelector`・`RecipientRef`・`DeathPactException` が含まれる
- **AND** `Behaviors`・`FsmBuilder`・`Routers`・`StashBuffer`・`TimerScheduler`・`TimerKey`・`TypedAskError`・`TypedAskFuture`・`TypedAskResponse`・`StatusReply`・`StatusReplyError`・`Supervise`・`FailureHandler`・`BehaviorSignalInterceptor` は含まれない

#### Scenario: pub mod の宣言が新構造を反映する
- **WHEN** `modules/actor/src/core/typed.rs` の `pub mod` 宣言を確認する
- **THEN** `pub mod dsl;`・`pub mod eventstream;`・`pub(crate) mod internal;` が存在する
- **AND** `mod routing;`・`mod scheduler;`・`mod behaviors;`・`mod fsm_builder;`・`mod stash_buffer;`・`mod timer_scheduler;` 等の旧宣言は存在しない

#### Scenario: std/typed.rs と tests が新 import path を参照する
- **WHEN** `modules/actor/src/std/typed.rs` および actor モジュールの tests を確認する
- **THEN** `crate::core::typed::Behaviors` 等の旧 import path は `crate::core::typed::dsl::Behaviors` 等の新 import path へ更新されている
- **AND** `./scripts/ci-check.sh ai all` が成功する

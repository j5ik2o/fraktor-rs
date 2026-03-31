## ADDED Requirements

### Requirement: typed/dsl/ package が新設され DSL ビルダー群・routing・scheduler API が集約される
`modules/actor/src/core/typed/dsl/` を新設し、Pekko `typed/scaladsl/` に対応する DSL ビルダー群を集約しなければならない。移設対象は `Behaviors`・`FsmBuilder`・`StashBuffer`・`TimerScheduler`・`TimerKey`・`Supervise`・`FailureHandler`・`TypedAsk{Error,Future,Response}`・`StatusReply{,Error}`、および `typed/routing/` 配下の `Routers`・各 `*RouterBuilder`・`Resizer`・`DefaultResizer`、`typed/scheduler/` の公開 API（`TimerScheduler` facade）である。これらは `core/typed/` root からは直接 re-export されてはならない。

#### Scenario: dsl package 配下に DSL ビルダー群が配置される
- **WHEN** `modules/actor/src/core/typed/dsl/` の構造を確認する
- **THEN** `behaviors.rs`・`fsm_builder.rs`・`stash_buffer.rs`・`timer_scheduler.rs`・`timer_key.rs`・`supervise.rs`・`failure_handler.rs`・`typed_ask_error.rs`・`typed_ask_future.rs`・`typed_ask_response.rs`・`status_reply.rs`・`status_reply_error.rs` が存在する

#### Scenario: dsl package 配下に routing builders が配置される
- **WHEN** `modules/actor/src/core/typed/dsl/` の構造を確認する
- **THEN** `routers.rs`・`pool_router_builder.rs`・`group_router_builder.rs`・`balancing_pool_router_builder.rs`・`scatter_gather_first_completed_router_builder.rs`・`tail_chopping_router_builder.rs`・`resizer.rs`・`default_resizer.rs` が存在する

#### Scenario: typed/routing/ ディレクトリが削除される
- **WHEN** `modules/actor/src/core/typed/` を確認する
- **THEN** `routing/` ディレクトリと `routing.rs` は存在しない

#### Scenario: typed root から dsl 型が直接 pub use されない
- **WHEN** `modules/actor/src/core/typed.rs` の `pub use` 宣言を確認する
- **THEN** `Behaviors`・`FsmBuilder`・`Routers`・`StashBuffer`・`TimerScheduler`・`TimerKey`・`Supervise`・`FailureHandler`・`TypedAskError`・`TypedAskFuture`・`TypedAskResponse`・`StatusReply`・`StatusReplyError` は root 直下の `pub use` に含まれない

#### Scenario: dsl パッケージが pub mod として公開される
- **WHEN** `modules/actor/src/core/typed.rs` の mod 宣言を確認する
- **THEN** `pub mod dsl;` が存在する

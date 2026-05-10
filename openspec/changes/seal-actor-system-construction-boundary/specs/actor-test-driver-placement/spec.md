## MODIFIED Requirements

### Requirement: std 依存のテストドライバおよびテストヘルパは actor-adaptor-std 側に配置されなければならない

std 依存のテストドライバおよびテストヘルパは actor-adaptor-std 側に配置されなければならない (MUST)。

`fraktor-actor-*` workspace において、`std::thread` / `std::time::Instant` / tokio 等の std 環境固有機能に依存する
テスト向けの TickDriver 実装および actor system test helper は、no_std クレートである
`fraktor-actor-core-kernel-rs` ではなく `fraktor-actor-adaptor-std-rs` 側に配置されなければならない(MUST)。

`fraktor-actor-core-kernel-rs` 側は以下のみを提供する(MUST):

- no_std で動作する抽象（`TickDriver` trait、`TickFeed`、`SchedulerTickExecutor` 等）
- `#[cfg(test)]` 配下の inline unit tests に必要な crate-private fixture

`fraktor-actor-adaptor-std-rs` 側は以下を提供する(MUST):

- std 環境固有の TickDriver 実装（`StdTickDriver`、`TokioTickDriver`、`TestTickDriver` など）
- std 環境を前提とした test helper（`new_noop_actor_system` / `new_noop_actor_system_with<F>`）

actor-adaptor-std の test helper は actor-core-kernel の private construction seam に依存してはならず(MUST NOT)、
`ActorSystem::create_with_noop_guardian` 経由で bootstrapped system を作らなければならない(MUST)。

#### Scenario: TestTickDriver の公開定義は actor-adaptor-std 側にのみ存在する

- **WHEN** workspace の `modules/actor-*/src/**/*.rs` で `pub struct TestTickDriver` の定義を検査する
- **THEN** `modules/actor-adaptor-std/src/tick_driver/test_tick_driver.rs` にのみ存在する
- **AND** actor-core-kernel 側に公開可視性の `TestTickDriver` 定義は存在しない
- **AND** actor-core-kernel は `TestTickDriver` を公開 re-export しない

#### Scenario: std 依存の actor system test helper の公開 API は actor-adaptor-std 側にのみ存在する

- **WHEN** actor-core-kernel の `impl ActorSystem` を検査する
- **THEN** `pub fn new_empty` / `pub fn new_empty_with` / `pub fn new_noop` / `pub fn new_noop_with` は存在しない
- **AND** std 依存 test helper は `fraktor_actor_adaptor_std_rs::system::new_noop_actor_system` /
  `new_noop_actor_system_with<F>` として提供される

#### Scenario: std test helper は actor-core construction bypass を使わない

- **WHEN** `modules/actor-adaptor-std/src/system` の helper 実装を検査する
- **THEN** helper は `TestTickDriver` と std mailbox clock を設定する
- **AND** `ActorSystem::create_with_noop_guardian` を呼ぶ
- **AND** `ActorSystem::from_state`、`ActorSystem::create_started_from_config`、`SystemStateShared::new(SystemState::new())`
  を呼ばない

#### Scenario: downstream crate は actor-adaptor-std の new noop helper を使う

- **WHEN** `fraktor-actor-*` workspace 内の downstream crate tests が test actor system を必要とする
- **THEN** `fraktor_actor_adaptor_std_rs::system::new_noop_actor_system` または
  `new_noop_actor_system_with` を import する
- **AND** `new_empty_actor_system` を import しない
- **AND** actor-core-kernel の internal constructor を呼ばない

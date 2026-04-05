## ADDED Requirements

### Requirement: tick driver 配線は `core` が所有する
tick driver の構築配線は `core` が所有しなければならない。`TickFeed`、`TickExecutorSignal`、`SchedulerTickExecutor`、`TickDriverHandle`、`TickDriverBundle`、自動ドライバーメタデータを platform adapter が直接組み立ててはならない。

#### Scenario: std adapter が complete bundle を構築しない
- **WHEN** `modules/actor-adaptor/src/std/scheduler/tick.rs` を確認する
- **THEN** `TickDriverBundle::new(...)`、`TickFeed::new(...)`、`SchedulerTickExecutor::new(...)`、`TickDriverHandle::new(...)`、`next_tick_driver_id(...)` の呼び出しは存在しない
- **AND** それらの構築は `core` 側に集約されている

### Requirement: platform adapter は最小 contract のみ実装する
platform adapter は、tick source または executor pump のような最小 contract のみを実装しなければならない。adapter が scheduler の内部構造を知ってはならない。

#### Scenario: Tokio adapter は executor pump だけを提供する
- **WHEN** Tokio ベースの tick driver adapter 実装を確認する
- **THEN** adapter 実装は tick source と executor pump の定義だけを持つ
- **AND** `TickFeed` や `SchedulerTickExecutor` の所有責務を持たない

### Requirement: tick driver のプロビジョニング失敗は panic ではなくエラーで扱われる
tick driver adapter の初期化に必要な platform 前提が不足している場合、失敗は `TickDriverError` として扱われなければならない。adapter 内部で安易に panic してはならない。

#### Scenario: platform 前提不足が `TickDriverError` へ変換される
- **WHEN** tick driver adapter の初期化に必要な runtime context や platform resource が不足している
- **THEN** 失敗は `TickDriverError` として返される
- **AND** adapter 内部の `expect(...)` でプロセスを即座に中断しない

### Requirement: showcase support は core 配線 API を使う
showcase support は、新しい `core` tick driver 配線 API を使わなければならない。showcase 側で `TickFeed`、`SchedulerTickExecutor`、`TickDriverHandle` を直接構築してはならない。

#### Scenario: showcase が独自 wiring を複製しない
- **WHEN** `showcases/std/src/support/tick_driver.rs` を確認する
- **THEN** `TickFeed::new(...)`、`SchedulerTickExecutor::new(...)`、`TickDriverHandle::new(...)` の独自組み立てコードを持たない
- **AND** `core` の tick driver 配線 API を使って tick driver を構成する

### Requirement: デフォルト tick driver 構成の利用体験は維持される
tick driver 配線の責務分離を行っても、`ActorSystem::new()` によるデフォルト起動体験は維持されなければならない。

#### Scenario: tokio-executor 有効時のデフォルト起動は継続する
- **WHEN** `tokio-executor` feature 有効環境で `ActorSystem::new(&props)` を呼び出す
- **THEN** デフォルト tick driver 構成で actor system が起動する
- **AND** 利用者は tick driver 配線の再設計を意識せずに済む

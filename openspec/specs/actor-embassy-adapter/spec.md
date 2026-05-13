# actor-embassy-adapter Specification

## Purpose
TBD - created by archiving change async-first-actor-adapters. Update Purpose after archive.
## Requirements
### Requirement: actor-adaptor-embassy は actor-core-kernel を Embassy task / signal / timer に接続する

`actor-adaptor-embassy` は `actor-core-kernel` の port を Embassy 環境に接続する adapter crate として提供されなければならない (MUST)。Embassy 依存は `actor-adaptor-embassy` に閉じ込め、`actor-core-kernel` は `embassy-*` crate に依存してはならない (MUST NOT)。

初期 scope は actor system の executor、tick driver、mailbox clock injection に限定しなければならない (MUST)。remote transport、stream materialization、persistence adapter は含めてはならない (MUST NOT)。

#### Scenario: actor-core-kernel に Embassy 依存が入らない

- **WHEN** `modules/actor-core-kernel/Cargo.toml` と `modules/actor-core-kernel/src/` を確認する
- **THEN** `embassy-*` crate への dependency は存在しない
- **AND** `use embassy_` で始まる import は存在しない

#### Scenario: Embassy adapter crate が workspace member として存在する

- **WHEN** workspace modules を確認する
- **THEN** `modules/actor-adaptor-embassy` が存在する
- **AND** その crate は `fraktor-actor-core-kernel-rs` に依存する
- **AND** Embassy 依存はその crate 側にのみ置かれる

### Requirement: EmbassyExecutor は bounded ready queue と signal で mailbox drain を駆動する

`EmbassyExecutor` は core の `Executor` trait を実装しなければならない (MUST)。`Executor::execute` は mailbox drain closure を bounded ready queue へ enqueue し、Embassy task を wake する signal を通知しなければならない (MUST)。`execute` は thread blocking や busy wait を行ってはならない (MUST NOT)。

ready queue が満杯の場合、`EmbassyExecutor::execute` は block せず `ExecuteError` を返さなければならない (MUST)。これにより dispatcher は既存の submit failure rollback 経路で mailbox schedule state を戻せる。

#### Scenario: execute は ready queue へ enqueue して signal を通知する

- **GIVEN** `EmbassyExecutor` が構築済みである
- **WHEN** `Executor::execute(task, affinity_key)` が呼ばれる
- **THEN** `task` は bounded ready queue に入る
- **AND** Embassy worker task を起こす signal が通知される
- **AND** `execute` 自体は mailbox drain closure を同期実行しない

#### Scenario: ready queue 満杯時は ExecuteError を返す

- **GIVEN** Embassy ready queue が満杯である
- **WHEN** `EmbassyExecutor::execute(task, affinity_key)` が呼ばれる
- **THEN** `Err(ExecuteError)` が返る
- **AND** caller thread / task は空き待ちで block されない

#### Scenario: Embassy worker task が ready queue を drain する

- **GIVEN** Embassy worker task が起動済みである
- **WHEN** ready queue signal が通知される
- **THEN** worker task は ready queue から task を取り出す
- **AND** 取り出した mailbox drain closure を同期的に実行する
- **AND** closure 内部で `.await` は発生しない

### Requirement: EmbassyTickDriver は embassy-time で scheduler tick を供給する

`actor-adaptor-embassy` は `TickDriver` trait を実装する `EmbassyTickDriver` を提供しなければならない (MUST)。`EmbassyTickDriver` は `embassy-time` を使って tick を生成し、`TickFeedHandle` に tick を供給しなければならない (MUST)。

`EmbassyTickDriver::kind()` は `TickDriverKind::Embassy` を返さなければならない (MUST)。`provision` が返す `TickDriverProvision::auto_metadata` は `AutoProfileKind::Embassy` を含まなければならない (MUST)。

#### Scenario: EmbassyTickDriver は Embassy profile を公開する

- **GIVEN** `EmbassyTickDriver::default()` が生成される
- **WHEN** `kind()` が呼ばれる
- **THEN** `TickDriverKind::Embassy` が返る

#### Scenario: EmbassyTickDriver は embassy-time で tick を供給する

- **GIVEN** `EmbassyTickDriver` が provision 済みである
- **WHEN** configured resolution が経過する
- **THEN** `TickFeedHandle` に tick が enqueue される
- **AND** scheduler tick executor が Embassy task から駆動される

### Requirement: Embassy adapter は mailbox throughput deadline 用 monotonic clock を注入する

`actor-adaptor-embassy` は `embassy-time::Instant` 相当の monotonic time source を actor system config に注入する helper を提供しなければならない (MUST)。この clock は `Mailbox::run` の throughput deadline 判定に使われ、wall-clock に依存してはならない (MUST NOT)。

#### Scenario: Embassy clock helper は mailbox clock を設定する

- **WHEN** Embassy actor system config helper が呼ばれる
- **THEN** config には Embassy monotonic clock 由来の `MailboxClock` が設定される
- **AND** `Mailbox::run(..., Some(deadline))` は Embassy clock で deadline 判定を行う

#### Scenario: clock injection は actor-core-kernel の no_std 境界を保つ

- **WHEN** mailbox clock injection の実装を確認する
- **THEN** Embassy 固有型は `actor-adaptor-embassy` 内に閉じている
- **AND** `actor-core-kernel` から Embassy 固有型は参照されない


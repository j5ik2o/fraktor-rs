# pekko-panic-guard Specification

## Purpose
TBD - created by archiving change 2026-04-20-pekko-panic-guard. Update Purpose after archive.
## Requirements
### Requirement: `InvokeGuard` trait は dyn-compatible で `receive` クロージャを包まなければならない

`modules/actor-core/src/core/kernel/actor/invoke_guard.rs` に定義される `InvokeGuard` trait は、`&mut dyn FnMut() -> Result<(), ActorError>` を受け取って `Result<(), ActorError>` を返す `wrap_receive` メソッドを提供しなければならない（MUST）。trait は `Send + Sync` を要求し、`ArcShared<Box<dyn InvokeGuard>>` として保持できる dyn-compatible 形でなければならない。no_std 互換でなければならず、`std::*` に依存してはならない。

#### Scenario: trait 定義は dyn-compatible

- **WHEN** `InvokeGuard` trait を定義する
- **THEN** `fn wrap_receive(&self, call: &mut dyn FnMut() -> Result<(), ActorError>) -> Result<(), ActorError>;` シグネチャが採用される
- **AND** trait は `Send + Sync` bound を持ち、`ArcShared<Box<dyn InvokeGuard>>` で持ち運べる

#### Scenario: `wrap` は `Self: Sized` 付き default method として提供される

- **WHEN** `InvokeGuard` trait 定義を確認する
- **THEN** trait 本体に `fn wrap<F: FnOnce() -> Result<(), ActorError>>(&self, f: F) -> Result<(), ActorError> where Self: Sized` の default method が存在する
- **AND** default 実装は `wrap_receive` に `FnMut` 経由でクロージャを渡す形で書かれている
- **AND** `Self: Sized` により `dyn InvokeGuard` からは `wrap` が呼べず、trait の dyn-compatibility は保たれる

#### Scenario: 具象型の method resolution で `wrap` が使える

- **GIVEN** `use fraktor_actor_core_rs::core::kernel::actor::invoke_guard::{InvokeGuard, NoopInvokeGuard};` のみ import された呼び出し側
- **WHEN** `NoopInvokeGuard::new().wrap(|| Ok(()))` を呼ぶ
- **THEN** method resolution で trait default method `wrap` が見つかり、追加の拡張 trait import なしに解決される

#### Scenario: core モジュールは std::panic を import しない

- **WHEN** `modules/actor-core/src/core/kernel/actor/invoke_guard*` を grep で検索する
- **THEN** `std::panic` / `std::process` / `catch_unwind` の import は 0 件である
- **AND** `cfg-std-forbid` dylint lint が違反を検出しない

### Requirement: `InvokeGuardFactory` trait は `ArcShared<Box<dyn InvokeGuard>>` を生成しなければならない

`InvokeGuard` 実体を `ActorSystemConfig` から `ActorCell` まで運ぶため、`InvokeGuardFactory` trait を core 側に定義し、factory から `ArcShared<Box<dyn InvokeGuard>>` を得られなければならない（MUST）。

#### Scenario: NoopInvokeGuardFactory が default として提供される

- **WHEN** 利用者が `InvokeGuardFactory` の default 実装を参照する
- **THEN** `NoopInvokeGuardFactory` が存在し、`build()` が `ArcShared::new(Box::new(NoopInvokeGuard))` 相当を返す

#### Scenario: factory は Send + Sync

- **WHEN** `InvokeGuardFactory` trait 定義を確認する
- **THEN** `Send + Sync` bound が課されている
- **AND** `ArcShared<Box<dyn InvokeGuardFactory>>` として持ち運べる

### Requirement: `ActorSystemConfig` は guard factory 設定面を持たなければならない

`ActorSystemConfig` は `InvokeGuardFactory` を設定する builder setter を提供しなければならない（MUST）。未設定時は `NoopInvokeGuardFactory` が default として適用される。

#### Scenario: `with_invoke_guard_factory` setter が存在する

- **WHEN** 利用者が `ActorSystemConfig::default().with_invoke_guard_factory(factory)` を呼ぶ
- **THEN** config に factory が格納される
- **AND** `SystemState::build_from_owned_config(config)` 経路で `SystemState` に受け渡される

#### Scenario: 未設定時は NoopInvokeGuardFactory が default として取得される

- **WHEN** 利用者が `ActorSystemConfig::default()` で `with_invoke_guard_factory` を呼ばずに `ActorSystem::create_with_config` する
- **THEN** 構築された `SystemState` から `invoke_guard_factory()` が `NoopInvokeGuardFactory` 相当を返す
- **AND** panic は従来どおり worker thread まで伝播する

### Requirement: `ActorCell` は config 由来の guard を `MessageInvokerPipeline` に注入しなければならない

全ての `ActorCell` は生成時に `SystemState` から `ArcShared<Box<dyn InvokeGuard>>` を取得し、`MessageInvokerPipeline::new_with_guard(guard)` として pipeline を構築しなければならない（MUST）。`MessageInvokerPipeline::new()` はこの change で廃止される。

#### Scenario: `ActorCell::new` は factory から guard を取得する

- **WHEN** 新規 `ActorCell` を `actor_cell.rs:181` 付近の経路で生成する
- **THEN** `system.invoke_guard_factory().build()` で `ArcShared<Box<dyn InvokeGuard>>` が取得される
- **AND** `MessageInvokerPipeline::new_with_guard(guard)` として pipeline に注入される

#### Scenario: `MessageInvokerPipeline::new()` は廃止される

- **WHEN** workspace 全体で `MessageInvokerPipeline::new(` を grep する
- **THEN** `new_with_guard(` を除き、引数無しの `new()` 呼び出しは 0 件である
- **AND** 既存テスト helper は `ArcShared::new(Box::new(NoopInvokeGuard))` を明示的に渡す形に更新されている

### Requirement: `NoopInvokeGuard` は no_std default として素通しでなければならない

`NoopInvokeGuard` は kernel の default guard として、渡されたクロージャの戻り値をそのまま返さなければならない（MUST）。panic を捕捉してはならず、panic は呼び出し元へ伝播しなければならない。

#### Scenario: 正常戻り値は素通し

- **GIVEN** `NoopInvokeGuard::new()` で構成された guard
- **WHEN** `guard.wrap(|| Ok(()))` を呼ぶ
- **THEN** `Ok(())` が返る
- **WHEN** `guard.wrap(|| Err(ActorError::recoverable("x")))` を呼ぶ
- **THEN** `Err(ActorError::Recoverable(_))` が返る

#### Scenario: panic は捕捉されない

- **GIVEN** `NoopInvokeGuard::new()` で構成された guard
- **WHEN** `std::panic::catch_unwind(AssertUnwindSafe(|| guard.wrap(|| panic!("x"))))` を呼ぶ
- **THEN** `catch_unwind` は `Err(_)` を返す（panic が外側まで伝播した証拠）
- **AND** `NoopInvokeGuard` は panic を `ActorError::Escalate` に変換していない

### Requirement: `PanicInvokeGuard` は panic を `ActorError::Escalate` に変換しなければならない

`modules/actor-adaptor-std/src/std/actor/panic_invoke_guard.rs` に定義される `PanicInvokeGuard` は、渡されたクロージャを `std::panic::catch_unwind(AssertUnwindSafe(..))` で包み、panic を `ActorError::Escalate(ActorErrorReason::new(panic_msg))` に変換しなければならない（MUST）。

#### Scenario: panic は Escalate に変換される

- **GIVEN** `PanicInvokeGuard::new()` で構成された guard
- **WHEN** `guard.wrap(|| panic!("boom"))` を呼ぶ
- **THEN** `Err(ActorError::Escalate(_))` が返る

#### Scenario: panic メッセージは ActorErrorReason に保持される

- **GIVEN** `PanicInvokeGuard::new()` で構成された guard
- **WHEN** `guard.wrap(|| panic!("custom panic detail xyz"))` を呼ぶ
- **THEN** 返り値は `Err(ActorError::Escalate(reason))` であり `reason.as_str().contains("custom panic detail xyz")` が真

#### Scenario: 正常戻り値と `ActorError` は素通し

- **GIVEN** `PanicInvokeGuard::new()` で構成された guard
- **WHEN** `guard.wrap(|| Ok(()))` を呼ぶ
- **THEN** `Ok(())` が返る
- **WHEN** `guard.wrap(|| Err(ActorError::recoverable("planned")))` を呼ぶ
- **THEN** `Err(ActorError::Recoverable(_))` がそのまま返る（Escalate に誤変換されない）
- **WHEN** `guard.wrap(|| Err(ActorError::fatal("planned-fatal")))` を呼ぶ
- **THEN** `Err(ActorError::Fatal(_))` がそのまま返る（Escalate に誤変換されない）

### Requirement: `MessageInvokerPipeline` は `InvokeGuard` で `receive` を包囲しなければならない

`MessageInvokerPipeline` は `ArcShared<Box<dyn InvokeGuard>>` フィールドを保持し、`invoke_user` 内の `actor.receive(ctx, view)` 呼び出しを `self.guard.wrap_receive(&mut || actor.receive(ctx, view))` で包囲しなければならない（MUST）。generic parameter (`G: InvokeGuard`) は導入しない。`MessageInvokerPipeline::new()` は廃止し、`MessageInvokerPipeline::new_with_guard(guard: ArcShared<Box<dyn InvokeGuard>>)` のみを公開する。

#### Scenario: invoke_user は guard.wrap_receive を経由する

- **WHEN** `MessageInvokerPipeline::invoke_user(ctx, view)` が実行される
- **THEN** 内部で `self.guard.wrap_receive(&mut || actor.receive(ctx, view))` が呼ばれる
- **AND** `actor.receive` の戻り値・panic は `wrap_receive` を通じて `Result<(), ActorError>` として返る

#### Scenario: pipeline は generic parameter を持たない

- **WHEN** `MessageInvokerPipeline` の型定義を確認する
- **THEN** `G: InvokeGuard` のような generic parameter は導入されていない
- **AND** `guard` フィールドは `ArcShared<Box<dyn InvokeGuard>>` 型で保持される

#### Scenario: lifecycle hooks は guard で包まない

- **WHEN** `MessageInvokerPipeline` が `pre_start` / `post_stop` / `pre_restart` / `post_restart` を呼ぶ
- **THEN** これらの呼び出しは `guard.wrap_receive` で包まれない（本 change のスコープ外）
- **AND** panic が発生した場合の挙動は現状維持

### Requirement: std adaptor は `PanicInvokeGuardFactory` と install helper を提供しなければならない

`modules/actor-adaptor-std/src/std/actor/` には `PanicInvokeGuardFactory` と `install_panic_invoke_guard(config: ActorSystemConfig) -> ActorSystemConfig` helper が存在しなければならない（MUST）。helper は `config.with_invoke_guard_factory(ArcShared::new(Box::new(PanicInvokeGuardFactory)))` を返す。

#### Scenario: `install_panic_invoke_guard` が factory を config にセットする

- **WHEN** 利用者が `let cfg = install_panic_invoke_guard(ActorSystemConfig::default());` を呼ぶ
- **THEN** `cfg` には `PanicInvokeGuardFactory` が `invoke_guard_factory` として格納されている
- **AND** `ActorSystem::create_with_config(&props, cfg)` で生成した system 配下の全 `ActorCell` は、factory `build()` から得た state-less な `PanicInvokeGuard` 由来の `ArcShared<Box<dyn InvokeGuard>>` を pipeline に注入される（各 cell の guard instance は別だが挙動は同一）

#### Scenario: install helper を使った system で panic が supervisor エスカレーションに変換される

- **GIVEN** `install_panic_invoke_guard` 経由で構築した `ActorSystem`
- **WHEN** 子 actor の `receive` 内で panic が発生する
- **THEN** `MessageInvokerPipeline` は `PanicInvokeGuard::wrap_receive` で `Err(ActorError::Escalate(_))` を得る
- **AND** `handle_failure` 経路で parent の supervisor directive 判定が駆動される

#### Scenario: helper を使わない default は NoopInvokeGuardFactory

- **WHEN** 利用者が `install_panic_invoke_guard` を呼ばずに `ActorSystem::create_with_config` する
- **THEN** `MessageInvokerPipeline` には `NoopInvokeGuardFactory` 由来の `NoopInvokeGuard` が注入される
- **AND** panic の挙動は従来どおり worker thread まで伝播する

### Requirement: std adaptor 公開モジュールには `actor` サブモジュールを含めなければならない

`modules/actor-adaptor-std/src/std.rs` は `dispatch` / `event` / `pattern` / `tick_driver` / `time` に加えて `pub mod actor;` を公開しなければならない（MUST）。`actor` サブモジュールは std 固有 helper である `PanicInvokeGuard` を re-export し、既存 capability `actor-std-adapter-surface` の「std 公開面は adapter と std 固有 helper のみ」の制約を破らない。

#### Scenario: `fraktor_actor_adaptor_std_rs::std::actor::PanicInvokeGuard` が解決できる

- **WHEN** integration test や外部 crate が `use fraktor_actor_adaptor_std_rs::std::actor::PanicInvokeGuard;` する
- **THEN** import が成功する

#### Scenario: 既存の公開サブモジュールは維持される

- **WHEN** `modules/actor-adaptor-std/src/std.rs` を確認する
- **THEN** `dispatch` / `event` / `pattern` / `tick_driver` / `time` は引き続き `pub mod` として公開されている
- **AND** 新規 `pub mod actor;` が追加されている


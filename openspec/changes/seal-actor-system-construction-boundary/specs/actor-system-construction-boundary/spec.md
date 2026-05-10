## ADDED Requirements

### Requirement: public actor system construction は bootstrap 境界を通らなければならない

`ActorSystem` と `TypedActorSystem` の public bootstrap constructor は bootstrap 境界を通らなければならない (MUST)。

`ActorSystem` と `TypedActorSystem` の public bootstrap constructor は、guardian props / no-op guardian / typed guardian /
setup-derived config のいずれかを入力とし、guardian bootstrap、extension installer、actor-ref provider installer、
default serialization extension、root started state を一貫して初期化しなければならない(MUST)。

外部 caller は `SystemStateShared`、`SystemState`、または同等の internal runtime state を渡して `ActorSystem`
を構築できてはならない(MUST NOT)。`#[doc(hidden)] pub`、deprecated alias、feature-gated public method による
迂回も許可しない(MUST NOT)。

`TypedActorSystem::from_untyped(ActorSystem)` は、すでに bootstrapped な untyped handle を typed API で包む
advanced wrapper であり、この requirement が対象にする raw runtime construction API ではない。ただし typed
no-op system の標準入口としては使ってはならない(MUST NOT)。

#### Scenario: external crate は `ActorSystem::from_state` を呼べない

- **WHEN** external fixture crate が `ActorSystem::from_state(SystemStateShared::new(SystemState::new()))` を呼ぶ
- **THEN** fixture は compile に失敗する
- **AND** diagnostic は `from_state` が public API ではないことを示す

#### Scenario: external crate は `ActorSystem::create_started_from_config` を呼べない

- **WHEN** external fixture crate が `ActorSystem::create_started_from_config(ActorSystemConfig::default())` を呼ぶ
- **THEN** fixture は compile に失敗する
- **AND** diagnostic は `create_started_from_config` が public API ではないことを示す

#### Scenario: untyped public constructor は bootstrapped system を返す

- **WHEN** caller が `ActorSystem::create_from_props`、`ActorSystem::create_with_noop_guardian`、または
  `ActorSystem::create_from_props_with_init` を呼ぶ
- **THEN** returned system は user guardian と system guardian を持つ
- **AND** extension installers と provider installer が実行済みである
- **AND** default serialization extension が登録済みである
- **AND** root は started として扱われる

#### Scenario: typed public constructor は typed bootstrap を欠落させない

- **WHEN** caller が `TypedActorSystem::create_from_props`、`TypedActorSystem::create_from_behavior_factory`、
  `TypedActorSystem::create_with_noop_guardian`、または `TypedActorSystem::create_from_props_with_init` を呼ぶ
- **THEN** returned typed system の underlying `ActorSystem` は bootstrapped system である
- **AND** system receptionist が system top-level actor として install 済みである
- **AND** actor-ref resolver と typed event stream facade が利用できる

#### Scenario: typed `create_from_props_with_init` は typed bootstrap と caller callback を両立する

- **WHEN** caller が `TypedActorSystem::create_from_props_with_init` に bootstrap callback を渡す
- **THEN** typed system receptionist は caller callback より先に install される
- **AND** caller callback は kernel bootstrap 中に 1 回だけ実行される
- **AND** callback が成功した場合、returned typed system は root started として扱われる

### Requirement: actor-core-kernel 内部だけが state handle を `ActorSystem` に再ラップできる

actor-core-kernel 内部だけが state handle を `ActorSystem` に再ラップできなければならない (MUST)。

actor-core-kernel は weak upgrade、actor selection、actor cell context creation のために、既存の
`SystemStateShared` を `ActorSystem` handle へ再ラップできなければならない(MUST)。ただし、この helper は
crate-private でなければならず(MUST)、旧 public API 名 `from_state` を使ってはならない(MUST NOT)。

#### Scenario: internal handle reconstruction は actor-core-kernel 内に閉じる

- **WHEN** `modules/actor-core-kernel/src` を検査する
- **THEN** `ActorSystemWeak::upgrade`、actor selection、actor cell context creation は crate-private helper で
  `ActorSystem` handle を再構成できる
- **AND** `modules/actor-core-kernel/src/system/base.rs` に `pub fn from_state` は存在しない
- **AND** `ActorSystem::from_state` という呼び出しは source code に残らない

### Requirement: std test helper は no-op guardian 付き bootstrapped system を作らなければならない

std test helper は no-op guardian 付き bootstrapped system を作らなければならない (MUST)。

`fraktor_actor_adaptor_std_rs::system` は std test helper として `new_noop_actor_system` と
`new_noop_actor_system_with` を提供しなければならない(MUST)。これらの helper は `TestTickDriver` と std
mailbox clock を設定した `ActorSystemConfig` を使い、`ActorSystem::create_with_noop_guardian` 経由で system を
作らなければならない(MUST)。

`new_empty_actor_system` / `new_empty_actor_system_with` は提供してはならない(MUST NOT)。

#### Scenario: std test helper は bootstrap bypass に依存しない

- **WHEN** `modules/actor-adaptor-std/src/system` の helper 実装を確認する
- **THEN** helper は `ActorSystem::create_with_noop_guardian` を呼ぶ
- **AND** `ActorSystem::create_started_from_config` を呼ばない
- **AND** `SystemStateShared::new(SystemState::new())` を直接合成しない

#### Scenario: old empty helper name は re-export されない

- **WHEN** `fraktor_actor_adaptor_std_rs::system` の公開 API を確認する
- **THEN** `new_noop_actor_system` と `new_noop_actor_system_with` は利用できる
- **AND** `new_empty_actor_system` と `new_empty_actor_system_with` は利用できない

### Requirement: tests は invalid actor system shell を fixture として使ってはならない

tests は invalid actor system shell を fixture として使ってはならない (MUST NOT)。

workspace の tests は、`SystemStateShared::new(SystemState::new())` を `ActorSystem` に包んだ invalid shell を fixture
として使ってはならない(MUST NOT)。`ActorSystem` が必要なテストは bootstrapped no-op system を使い、bare
state の契約を検証するテストは `SystemState` / `SystemStateShared` 単位に留めなければならない(MUST)。

削除対象 API の test caller が大量に存在しても、それを理由に compatibility constructor、deprecated alias、
test-only public helper を追加してはならない(MUST NOT)。大量の test rewrite は、設計境界を正すための
必要な移行として扱わなければならない(MUST)。

#### Scenario: downstream tests は `ActorSystem::from_state` に依存しない

- **WHEN** `modules/**/src/**/*tests.rs` と `modules/**/tests/**/*.rs` を検査する
- **THEN** `ActorSystem::from_state` の呼び出しは存在しない
- **AND** `create_started_from_config` の呼び出しは存在しない
- **AND** untyped system handle が必要な tests は `new_noop_actor_system` または `ActorSystem::create_with_noop_guardian`
  を使う
- **AND** typed no-op system が必要な tests は `TypedActorSystem::create_with_noop_guardian` を使う

#### Scenario: 大量の test caller があっても compatibility constructor は復活しない

- **WHEN** 削除対象 API に多数の test caller が存在する
- **THEN** tests は bootstrapped no-op system または lower-level state test へ移行される
- **AND** `ActorSystem::from_state`、`ActorSystem::create_started_from_config`、または同等の test-only public
  constructor は追加されない

#### Scenario: synthetic cell tests は bootstrapped system 上で pid を確保する

- **WHEN** test が `ActorCell::create` と `ActorContext::new` のために synthetic cell を作る
- **THEN** pid は `ActorSystem::allocate_pid` または system state の allocation 経路で確保される
- **AND** no-op guardian と衝突しうる hard-coded `Pid::new(1, 1)` に依存しない

### Requirement: `ActorSystemSetup` conversion は unit test で保証されなければならない

`ActorSystemSetup` conversion は unit test で保証されなければならない (MUST)。

`ActorSystemSetup::into_actor_system_config` は、bootstrap settings と runtime settings を
`ActorSystemConfig` に正しく転写しなければならない(MUST)。この契約は integration test の副作用ではなく、
`ActorSystemSetup` の unit test で直接検証されなければならない(MUST)。

#### Scenario: setup conversion は bootstrap settings を保持する

- **GIVEN** `ActorSystemSetup` に system name、remoting config、start time が設定されている
- **WHEN** `into_actor_system_config` を呼ぶ
- **THEN** returned config は同じ system name、remoting config、start time を持つ

#### Scenario: setup conversion は runtime settings を保持する

- **GIVEN** `ActorSystemSetup` に tick driver、scheduler config、extension installers、provider installer、
  dispatcher factory、mailbox factory、circuit breaker config が設定されている
- **WHEN** `into_actor_system_config` を呼ぶ
- **THEN** returned config はそれらの runtime settings を保持する

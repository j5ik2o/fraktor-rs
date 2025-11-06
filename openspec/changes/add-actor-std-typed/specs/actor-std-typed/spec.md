## ADDED Requirements
### Requirement: Std Typed Module Layout
`modules/actor-std/src/typed` に core 版 typed と同等のモジュール階層（`actor_prim`, `behaviors`, `props`, `system` など）と公開 API 群を追加しなければならない (MUST)。

#### Scenario: std crate exposes typed namespace
- **WHEN** `actor-std::typed::*` を参照する
- **THEN** typed アクター DSL（Behaviors/TypedActorSystem/TypedProps 等）が利用できる

#### Scenario: all core typed components have std counterparts
- **WHEN** `modules/actor-core/src/typed` に存在する公開モジュール・型を列挙する
- **THEN** std 側にも同名もしくは等価な型（Behavior, Behaviors, TypedProps, TypedActorRef, TypedActorSystem, behavior runner など）が実装されている

### Requirement: Std Typed Actor Trait & Adapter
std 向けに `TypedActor` トレイトとアダプターを実装し、既存 `actor_prim::Actor` へ接続しなければならない (MUST)。

#### Scenario: typed actor adapts to std Actor
- **WHEN** `TypedProps::new` に std typed actor を渡して actor を spawn する
- **THEN** adapter が `modules/actor-std/src/actor_prim/actor.rs` の `Actor` トレイトへ橋渡しし、メッセージ downcast を安全に処理する

### Requirement: Std Typed Actor System Wrapper
`TypedActorSystem` を `CoreTypedActorSystemGeneric<StdToolbox>` のラッパーとして実装し、std 向け API を提供しなければならない (MUST)。

#### Scenario: std typed actor system mirrors ActorSystem API
- **WHEN** ユーザーが `TypedActorSystem::new` や `when_terminated` を呼び出す
- **THEN** `modules/actor-std/src/system/base.rs` の `ActorSystem` と同等の ergonomics で利用できる

### Requirement: Std Typed Examples & Tests
std typed API を示す example/test を追加し、CI で検証しなければならない (MUST)。

#### Scenario: example demonstrates Behaviors::setup/receiveSignal
- **WHEN** `cargo run --package cellactor-actor-std-rs --example typed_...` を実行する
- **THEN** std typed API を使った挙動（setup, receiveMessage, receiveSignal など）が動作する
- **AND** README もしくはコードコメントで実行方法を記載する

#### Scenario: CI validates std typed module
- **WHEN** `./scripts/ci-check.sh all` を実行する
- **THEN** std typed 実装を含む全テスト・lint が成功する

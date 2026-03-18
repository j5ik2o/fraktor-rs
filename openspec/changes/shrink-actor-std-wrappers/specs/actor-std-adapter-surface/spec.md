## ADDED Requirements

### Requirement: `std` 公開面は adapter と std 固有 helper のみに限定される
`modules/actor/src/std` は、`core` の port を std/tokio/tracing に接続する adapter 実装、または std 固有 helper のみを公開しなければならない。`core` 型を包み直しただけの façade / wrapper は外部公開してはならない。

#### Scenario: 純粋 wrapper が `std` 公開面から除外される
- **WHEN** 利用者が `crate::std` 配下の actor / typed / props / system API を参照する
- **THEN** `TypedActorContext`、`TypedActorContextRef`、`TypedActorRef`、`TypedChildRef`、`TypedProps`、`TypedActorSystem`、`ActorContext`、`Props`、`ActorSystemConfig` のような pure wrapper は公開面に現れない

### Requirement: examples と tests は pure wrapper ではなく core API に依存する
`modules/actor/examples` と `modules/actor/src/std/tests.rs`、および削除対象 wrapper に依存している内部コードは、pure wrapper ではなく `core` API に依存しなければならない。

#### Scenario: typed examples が core typed API を使う
- **WHEN** `modules/actor/examples/*_std/main.rs` の typed example を確認する
- **THEN** typed actor system、typed props、typed actor ref、typed actor context には `core::typed` 側の型が使われ、`std::typed::actor::*` への依存は存在しない

#### Scenario: `std/tests.rs` が残すべき std API だけを固定する
- **WHEN** `modules/actor/src/std/tests.rs` を実行する
- **THEN** adapter subsystem と std 固有 helper だけが公開面として確認され、pure wrapper の公開を前提とした assertion は存在しない

### Requirement: façade 依存の shim は依存消滅後に削除される
pure wrapper のみを成立させるために存在していた shim は、その依存が `core` 側に置き換わった後に削除されなければならない。

#### Scenario: `ActorAdapter` と `TypedActorAdapter` が façade 依存消滅後に不要化される
- **WHEN** `std::actor::Actor` / `std::props::Props` および `std::typed::TypedProps` / `std::typed::actor::TypedActor` への依存がなくなる
- **THEN** `ActorAdapter` と `TypedActorAdapter` は `std` から削除され、runtime adapter として残存しない

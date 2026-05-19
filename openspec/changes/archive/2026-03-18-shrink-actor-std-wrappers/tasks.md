## 1. 公開面の縮小

- [x] 1.1 `modules/actor/src/std.rs` から `std::typed::actor::*` の公開 re-export を外す
- [x] 1.2 `std::typed::actor` 配下の pure wrapper を外部公開せず `std` 内部専用に縮退させる
- [x] 1.3 `std::typed::{TypedProps, TypedActorSystem}` の公開面を縮退させる
- [x] 1.4 `std::actor::ActorContext`、`std::props::Props`、`std::system::ActorSystemConfig` の公開面を縮退させる

## 2. 依存の `core` 付け替え

- [x] 2.1 `modules/actor/examples/*_std` の typed 依存を `std::typed` から `core::typed` へ付け替える
- [x] 2.2 `modules/actor/src/std/tests.rs` の公開面確認を、新しい `std` 公開 API に合わせて更新する
- [x] 2.3 `std` 内部実装のうち façade に依存している箇所を `core` 型へ付け替える
- [x] 2.4 `std::system::base.rs` と `std::typed::Behaviors` が縮退後の内部 API だけで成立するように整理する

## 3. façade 起因 shim の削除

- [x] 3.1 `std::actor::{Actor, ActorContext, ActorAdapter}` を不要化し削除する
- [x] 3.2 `std::typed::{TypedProps, TypedActorSystem}` と `std::typed::actor::{TypedActor, TypedActorAdapter}` を不要化し削除する
- [x] 3.3 `std::typed::actor::{TypedActorContext, TypedActorContextRef, TypedActorRef, TypedChildRef}` の実体ファイルを削除する
- [x] 3.4 `std::props::Props` と `std::system::ActorSystemConfig` の実体ファイルを削除する

## 4. テストと固定化

- [x] 4.1 `modules/actor/src/std/tests.rs` で、pure wrapper が復活していないことを固定する
- [x] 4.2 `modules/actor/examples` と `modules/actor` の関連テストが `core` 依存へ置き換わったことを確認する
- [x] 4.3 `cargo test -p fraktor-actor-rs std::tests` を通す
- [x] 4.4 `./scripts/ci-check.sh ai all` を通し、残す adapter subsystem に副作用がないことを確認する

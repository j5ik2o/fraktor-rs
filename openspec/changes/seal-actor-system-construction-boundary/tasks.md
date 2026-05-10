## Phase 1: 現状確認と失敗条件の固定

- [x] 1.1 `rg -n "ActorSystem::from_state|create_started_from_config|new_empty_actor_system" modules tests showcases` で現行 caller を全数確認する。
- [x] 1.2 `modules/actor-core-kernel/src/system/base.rs` の public constructor surface を確認し、残す public constructor を `create_from_props` / `create_with_noop_guardian` / `create_from_props_with_init` に限定する。
- [x] 1.3 `modules/actor-core-typed/src/system.rs` の public constructor surface を確認し、typed constructor を `create_from_props` / `create_from_behavior_factory` / `create_with_noop_guardian` / `create_from_props_with_init` に揃える。
- [x] 1.4 `modules/actor-core-kernel/tests/kernel_public_surface.rs` に `ActorSystem::from_state` compile-fail fixture を追加する。
- [x] 1.5 `modules/actor-core-kernel/tests/kernel_public_surface.rs` に `ActorSystem::create_started_from_config` compile-fail fixture を追加する。
- [x] 1.6 caller 数を互換 API 維持の根拠にせず、すべての test-only construction seam を削除対象として記録する。

## Phase 2: actor-core-kernel construction seam 撤廃

- [x] 2.1 `ActorSystem::from_state` を削除し、必要な internal reconstruction 用に `pub(crate) const fn from_system_state(SystemStateShared) -> ActorSystem` を追加する。
- [x] 2.2 `ActorSystemWeak::upgrade` を `from_system_state` へ移行する。
- [x] 2.3 `ActorSelection::resolve_actor_ref` を `from_system_state` へ移行する。
- [x] 2.4 `ActorCell::make_context` を `from_system_state` へ移行する。
- [x] 2.5 actor-core-kernel 内部 tests の `ActorSystem::from_state` caller を `from_system_state` または lower-level `SystemState` test へ移行する。
- [x] 2.6 `ActorSystem::create_started_from_config` を削除する。

## Phase 3: actor-core-typed construction surface を bootstrap 経由へ揃える

- [x] 3.1 `TypedActorSystem::create_from_props_with_init<F>` を追加し、system receptionist install と caller callback を同じ kernel bootstrap callback 内で実行する。
- [x] 3.2 `TypedActorSystem::create_from_props` を `create_from_props_with_init(..., |_| Ok(()))` に委譲する。
- [x] 3.3 `TypedActorSystem::create_with_noop_guardian` を追加し、typed no-op guardian props から bootstrapped typed system を作る。
- [x] 3.4 `TypedActorSystem::create_from_behavior_factory` が `create_from_props` 経由の convenience constructor であり続けることを確認する。
- [x] 3.5 typed constructor tests を追加し、`create_with_noop_guardian` と `create_from_props_with_init` が receptionist / actor-ref resolver / event stream facade を欠落させないことを確認する。

## Phase 4: actor-core-kernel test helper を bootstrap 経由へ寄せる

- [x] 4.1 `modules/actor-core-kernel/src/system/base/tests.rs` の test-only helper を `create_with_noop_guardian` 経由へ変更し、pre-bootstrap 検証だけ crate-private unbootstrapped helper に分離する。
- [x] 4.2 test-only helper 名を必要に応じて `new_noop` / `new_noop_with` へ改名し、inline tests の caller を更新する。
- [x] 4.3 root started flag だけを直接立てる helper usage が actor-core-kernel tests に残らないことを確認する。

## Phase 5: actor-adaptor-std helper の再設計

- [x] 5.1 `modules/actor-adaptor-std/src/system/empty_system.rs` を no-op system helper として整理し、`new_noop_actor_system` を追加する。
- [x] 5.2 `new_noop_actor_system_with<F>` を追加し、`TestTickDriver`、std mailbox clock、caller config、`ActorSystem::create_with_noop_guardian` の順で構築する。
- [x] 5.3 `new_empty_actor_system` / `new_empty_actor_system_with` を削除し、互換 alias を残さない。
- [x] 5.4 `modules/actor-adaptor-std/src/system.rs` の re-export を `new_noop_actor_system*` に更新する。
- [x] 5.5 actor-adaptor-std 自身の tests を `new_noop_actor_system*` へ移行する。

## Phase 6: downstream tests の移行

- [x] 6.1 大量の test rewrite を許容し、削除 API の代替として compatibility helper / deprecated alias / test-only public API を追加しないことを確認する。
- [x] 6.2 `persistence-core` tests の `ActorSystem::from_state(SystemStateShared::new(SystemState::new()))` を `new_noop_actor_system` または purpose-specific lower-level state setup へ移行する。
- [x] 6.3 `persistent_actor` / `persistent_fsm` / `persistent_actor_adapter` の `ActorContext` helper は bootstrapped no-op system から pid を確保する形に変える。
- [x] 6.4 `journal_actor` / `snapshot_actor` tests の synthetic cell setup は no-op guardian と衝突しない pid allocation に変える。
- [x] 6.5 `remote-core` tests の `ActorSystem::from_state` caller を no-op system へ移行する。
- [x] 6.6 `stream-core-kernel` tests の `create_started_from_config` caller を `new_noop_actor_system_with` または `create_with_noop_guardian` へ移行する。
- [x] 6.7 `actor-core-typed` / `cluster-core` / その他 workspace tests の `new_empty_actor_system*` import を `new_noop_actor_system*` へ移行する。
- [x] 6.8 typed no-op system が必要な tests を `TypedActorSystem::create_with_noop_guardian` に移行し、`TypedActorSystem::from_untyped(ActorSystem::create_with_noop_guardian(...))` は typed bootstrap 欠落を検証する目的に限定する。

## Phase 7: setup conversion coverage

- [x] 7.1 `ActorSystemSetup::into_actor_system_config` が bootstrap settings (`system_name`, remoting config, start time) を保持する unit test を追加する。
- [x] 7.2 `ActorSystemSetup::into_actor_system_config` が runtime settings (tick driver, scheduler, extension installers, provider installer, dispatcher, mailbox, circuit breaker config) を保持する unit test を追加する。
- [x] 7.3 `with_bootstrap_setup` が runtime settings を落とさず bootstrap portion だけ置換する既存 test を `into_actor_system_config` 経由でも検証する。

## Phase 8: spec と public surface の整合確認

- [x] 8.1 `openspec/changes/seal-actor-system-construction-boundary/specs/actor-system-construction-boundary/spec.md` と実装差分が一致していることを確認する。
- [x] 8.2 `openspec/changes/seal-actor-system-construction-boundary/specs/actor-test-driver-placement/spec.md` と helper 配置が一致していることを確認する。
- [x] 8.3 `rg -n "ActorSystem::from_state|create_started_from_config|new_empty_actor_system" modules tests showcases` が compile-fail fixtures / harness を除く source code 上で 0 件であることを確認する。
- [x] 8.4 `rg -n "pub .*from_state|create_started_from_config" modules/actor-core-kernel/src/system/base.rs` が 0 件であることを確認する。
- [x] 8.5 `rg -n "create_with_noop_guardian|create_from_props_with_init" modules/actor-core-typed/src/system.rs modules/actor-core-typed/src/system/tests.rs` で typed constructor surface と tests が存在することを確認する。

## Phase 9: targeted verification

- [x] 9.1 `cargo test -p fraktor-actor-core-kernel-rs --test kernel_public_surface` を実行する。
- [x] 9.2 `cargo test -p fraktor-actor-core-kernel-rs actor_system_setup` を実行する。
- [x] 9.3 `cargo test -p fraktor-actor-core-typed-rs system` を実行する。
- [x] 9.4 `cargo test -p fraktor-actor-adaptor-std-rs` を実行する。
- [x] 9.5 `cargo test -p fraktor-persistence-core-rs` を実行する。
- [x] 9.6 `cargo test -p fraktor-stream-core-kernel-rs` を実行する。

## Phase 10: final verification

- [x] 10.1 `./scripts/ci-check.sh ai all` を実行する。
- [x] 10.2 `mise exec -- openspec status --change seal-actor-system-construction-boundary` で proposal / design / specs / tasks が done になっていることを確認する。

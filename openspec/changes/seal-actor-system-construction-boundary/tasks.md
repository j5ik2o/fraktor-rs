## Phase 1: 現状確認と失敗条件の固定

- [ ] 1.1 `rg -n "ActorSystem::from_state|create_started_from_config|new_empty_actor_system" modules tests showcases` で現行 caller を全数確認する。
- [ ] 1.2 `modules/actor-core-kernel/src/system/base.rs` の public constructor surface を確認し、残す public constructor を `create_from_props` / `create_with_noop_guardian` / `create_from_props_with_init` に限定する。
- [ ] 1.3 `modules/actor-core-kernel/tests/kernel_public_surface.rs` に `ActorSystem::from_state` compile-fail fixture を追加する。
- [ ] 1.4 `modules/actor-core-kernel/tests/kernel_public_surface.rs` に `ActorSystem::create_started_from_config` compile-fail fixture を追加する。
- [ ] 1.5 caller 数を互換 API 維持の根拠にせず、すべての test-only construction seam を削除対象として記録する。

## Phase 2: actor-core-kernel construction seam 撤廃

- [ ] 2.1 `ActorSystem::from_state` を削除し、必要な internal reconstruction 用に `pub(crate) const fn from_system_state(SystemStateShared) -> ActorSystem` を追加する。
- [ ] 2.2 `ActorSystemWeak::upgrade` を `from_system_state` へ移行する。
- [ ] 2.3 `ActorSelection::resolve_actor_ref` を `from_system_state` へ移行する。
- [ ] 2.4 `ActorCell::make_context` を `from_system_state` へ移行する。
- [ ] 2.5 actor-core-kernel 内部 tests の `ActorSystem::from_state` caller を `from_system_state` または lower-level `SystemState` test へ移行する。
- [ ] 2.6 `ActorSystem::create_started_from_config` を削除する。

## Phase 3: actor-core-kernel test helper を bootstrap 経由へ寄せる

- [ ] 3.1 `modules/actor-core-kernel/src/system/base/tests.rs` の test-only helper を `create_with_noop_guardian` 経由へ変更する。
- [ ] 3.2 test-only helper 名を必要に応じて `new_noop` / `new_noop_with` へ改名し、inline tests の caller を更新する。
- [ ] 3.3 root started flag だけを直接立てる helper usage が actor-core-kernel tests に残らないことを確認する。

## Phase 4: actor-adaptor-std helper の再設計

- [ ] 4.1 `modules/actor-adaptor-std/src/system/empty_system.rs` を no-op system helper として整理し、`new_noop_actor_system` を追加する。
- [ ] 4.2 `new_noop_actor_system_with<F>` を追加し、`TestTickDriver`、std mailbox clock、caller config、`ActorSystem::create_with_noop_guardian` の順で構築する。
- [ ] 4.3 `new_empty_actor_system` / `new_empty_actor_system_with` を削除し、互換 alias を残さない。
- [ ] 4.4 `modules/actor-adaptor-std/src/system.rs` の re-export を `new_noop_actor_system*` に更新する。
- [ ] 4.5 actor-adaptor-std 自身の tests を `new_noop_actor_system*` へ移行する。

## Phase 5: downstream tests の移行

- [ ] 5.1 大量の test rewrite を許容し、削除 API の代替として compatibility helper / deprecated alias / test-only public API を追加しないことを確認する。
- [ ] 5.2 `persistence-core` tests の `ActorSystem::from_state(SystemStateShared::new(SystemState::new()))` を `new_noop_actor_system` または purpose-specific lower-level state setup へ移行する。
- [ ] 5.3 `persistent_actor` / `persistent_fsm` / `persistent_actor_adapter` の `ActorContext` helper は bootstrapped no-op system から pid を確保する形に変える。
- [ ] 5.4 `journal_actor` / `snapshot_actor` tests の synthetic cell setup は no-op guardian と衝突しない pid allocation に変える。
- [ ] 5.5 `remote-core` tests の `ActorSystem::from_state` caller を no-op system へ移行する。
- [ ] 5.6 `stream-core-kernel` tests の `create_started_from_config` caller を `new_noop_actor_system_with` または `create_with_noop_guardian` へ移行する。
- [ ] 5.7 `actor-core-typed` / `cluster-core` / その他 workspace tests の `new_empty_actor_system*` import を `new_noop_actor_system*` へ移行する。

## Phase 6: setup conversion coverage

- [ ] 6.1 `ActorSystemSetup::into_actor_system_config` が bootstrap settings (`system_name`, remoting config, start time) を保持する unit test を追加する。
- [ ] 6.2 `ActorSystemSetup::into_actor_system_config` が runtime settings (tick driver, scheduler, extension installers, provider installer, dispatcher, mailbox, circuit breaker config) を保持する unit test を追加する。
- [ ] 6.3 `with_bootstrap_setup` が runtime settings を落とさず bootstrap portion だけ置換する既存 test を `into_actor_system_config` 経由でも検証する。

## Phase 7: spec と public surface の整合確認

- [ ] 7.1 `openspec/changes/seal-actor-system-construction-boundary/specs/actor-system-construction-boundary/spec.md` と実装差分が一致していることを確認する。
- [ ] 7.2 `openspec/changes/seal-actor-system-construction-boundary/specs/actor-test-driver-placement/spec.md` と helper 配置が一致していることを確認する。
- [ ] 7.3 `rg -n "ActorSystem::from_state|create_started_from_config|new_empty_actor_system" modules tests showcases` が source code 上で 0 件であることを確認する。
- [ ] 7.4 `rg -n "pub .*from_state|create_started_from_config" modules/actor-core-kernel/src/system/base.rs` が 0 件であることを確認する。

## Phase 8: targeted verification

- [ ] 8.1 `cargo test -p fraktor-actor-core-kernel-rs kernel_public_surface` を実行する。
- [ ] 8.2 `cargo test -p fraktor-actor-core-kernel-rs actor_system_setup` を実行する。
- [ ] 8.3 `cargo test -p fraktor-actor-adaptor-std-rs` を実行する。
- [ ] 8.4 `cargo test -p fraktor-persistence-core-rs` を実行する。
- [ ] 8.5 `cargo test -p fraktor-stream-core-kernel-rs` を実行する。

## Phase 9: final verification

- [ ] 9.1 `./scripts/ci-check.sh ai all` を実行する。
- [ ] 9.2 `mise exec -- openspec status --change seal-actor-system-construction-boundary` で proposal / design / specs / tasks が done になっていることを確認する。

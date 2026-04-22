## 1. 事前確認

- [ ] 1.1 `Grep "feature = \"test-support\"" modules/actor-core/src/` を実行し、design.md のシンボル一覧と差異がないことを再確認（14 箇所、11 シンボル）
- [ ] 1.2 全シンボルの caller を再確認 (`rtk grep -rn "<symbol>" modules/ showcases/`)。外部 caller が 0 件であることを再確認
- [ ] 1.3 ベースライン記録: `cargo test --workspace` と `cargo test --workspace --features test-support` 両方 pass を確認

## 2. Phase 1 — Behavior::handle_* (3 シンボル)

- [ ] 2.1 `modules/actor-core/src/core/typed/behavior.rs` の `handle_message` の dual-cfg pattern を削除し `pub(crate)` 一本化
- [ ] 2.2 同 `handle_start` を `pub(crate)` 一本化
- [ ] 2.3 同 `handle_signal` を `pub(crate)` 一本化
- [ ] 2.4 `cargo test -p fraktor-actor-core-rs --lib` で pass 確認

## 3. Phase 2 — TypedActorContext::from_untyped

- [ ] 3.1 `modules/actor-core/src/core/typed/actor/actor_context.rs` の `from_untyped` を `pub(crate)` 一本化
- [ ] 3.2 `cargo test -p fraktor-actor-core-rs --lib` で pass 確認

## 4. Phase 3 — ActorRef::new_with_builtin_lock (impl と impl_helper の 2 つ)

- [ ] 4.1 `modules/actor-core/src/core/kernel/actor/actor_ref/base.rs` の `new_with_builtin_lock` を `pub(crate)` 一本化（cfg gate 削除）
- [ ] 4.2 同ファイル内の `new_with_builtin_lock_impl` の cfg gate 削除（`pub(crate)` 維持）
- [ ] 4.3 `cargo test -p fraktor-actor-core-rs --lib` で pass 確認

## 5. Phase 4 — SchedulerRunner::manual

- [ ] 5.1 `modules/actor-core/src/core/kernel/actor/scheduler/scheduler_runner.rs` の `manual` を `pub(crate)` 一本化
- [ ] 5.2 `cargo test -p fraktor-actor-core-rs --lib` で pass 確認

## 6. Phase 5 — TickDriverBootstrap (struct + provision + re-export)

- [ ] 6.1 `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/bootstrap.rs` の `TickDriverBootstrap` struct 定義を `pub(crate)` 一本化（dual-cfg 削除）
- [ ] 6.2 同ファイル `provision` メソッドを `pub(crate)` 一本化
- [ ] 6.3 `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver.rs` の `pub use bootstrap::TickDriverBootstrap;` および `pub(crate) use ...;` の dual-cfg を `pub(crate) use bootstrap::TickDriverBootstrap;` に統一
- [ ] 6.4 `cargo test -p fraktor-actor-core-rs --lib` で pass 確認

## 7. Phase 6 — register_guardian_pid (SystemState / SystemStateShared)

- [ ] 7.1 `modules/actor-core/src/core/kernel/system/state/system_state.rs` の `register_guardian_pid` の cfg gate を削除（`pub(crate)` 維持、常時存在）
- [ ] 7.2 `modules/actor-core/src/core/kernel/system/state/system_state_shared.rs` の `register_guardian_pid` の cfg gate を削除
- [ ] 7.3 `cargo test -p fraktor-actor-core-rs --lib` で pass 確認

## 8. Phase 7 — booting_state / running_state mod 宣言

- [ ] 8.1 `modules/actor-core/src/core/kernel/system/state.rs` の `#[cfg(any(test, feature = "test-support"))] mod booting_state;` の cfg gate を削除（`mod booting_state;` のみに）
- [ ] 8.2 同 `running_state` の cfg gate を削除
- [ ] 8.3 `cargo test -p fraktor-actor-core-rs --lib` で pass 確認
- [ ] 8.4 dead code 警告が出る場合、`pub(crate)` items を `#[allow(dead_code)]` でマーク（または別 change で safe_delete を検討）

## 9. 全体検証

- [ ] 9.1 `Grep "feature = \"test-support\"" modules/actor-core/src/` で 0 件を確認
- [ ] 9.2 `Grep "cfg(any(test, feature = \"test-support\"))" modules/actor-core/src/` で 0 件を確認
- [ ] 9.3 `cargo test --workspace` で全テスト pass
- [ ] 9.4 `cargo test --workspace --features test-support` で全テスト pass（test-support feature が空 になっていることを確認）
- [ ] 9.5 `cargo build --workspace --no-default-features` で workspace 全体ビルド成功
- [ ] 9.6 `./scripts/ci-check.sh dylint` で lint pass
- [ ] 9.7 `./scripts/ci-check.sh ai all` で全 CI 緑

## 10. spec / docs 整合

- [ ] 10.1 `openspec validate step05-hide-actor-core-internal-test-api --strict` で artifact 整合確認
- [ ] 10.2 `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 1 責務 B-2 残および責務 C を「解消済み」に更新
- [ ] 10.3 step06 proposal を「actor-core/test-support feature が空 `[]` になった想定で削除」と整合する形で更新（必要なら）

## 11. コミット・PR

- [ ] 11.1 シンボルごとの小さな commit（Phase 1-7 で約 7-14 commit、論理単位を意識）
- [ ] 11.2 ブランチ作成: `step05-hide-actor-core-internal-test-api`
- [ ] 11.3 push + PR 作成（base: main）
- [ ] 11.4 CI 全 pass + レビュー対応 + マージ
- [ ] 11.5 archive (`/opsx:archive` または skill 経由)

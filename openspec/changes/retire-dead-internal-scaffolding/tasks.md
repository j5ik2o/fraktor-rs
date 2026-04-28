## 1. 事前確認

- [ ] 1.1 `cargo test -p fraktor-actor-core-rs` のベースラインを確認する
- [ ] 1.2 `BootingSystemState` / `RunningSystemState` の参照が `state.rs`、`booting_state.rs`、`running_state.rs`、`system_state/tests.rs` の専用テストに閉じていることを `grep` で確認する
- [ ] 1.3 `register_guardian_pid` が wrapper 以外のテストからも使われていることを確認し、本 change では削除対象に含めない
- [ ] 1.4 `openspec/specs/` に `BootingSystemState` / `RunningSystemState` が登場しないことを確認する

## 2. wrapper 専用テスト削除

- [ ] 2.1 `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` から `BootingSystemState` import を削除する
- [ ] 2.2 `booting_into_running_requires_all_guardians` テストを削除する
- [ ] 2.3 `booting_into_running_fails_when_guardian_missing` テストを削除する
- [ ] 2.4 `cargo test -p fraktor-actor-core-rs system_state` を実行し、削除対象以外の system state テストが通ることを確認する

## 3. wrapper 本体削除

- [ ] 3.1 `modules/actor-core/src/core/kernel/system/state/booting_state.rs` を削除する
- [ ] 3.2 `modules/actor-core/src/core/kernel/system/state/running_state.rs` を削除する
- [ ] 3.3 `modules/actor-core/src/core/kernel/system/state.rs` から `mod booting_state;` と `mod running_state;` を削除する
- [ ] 3.4 `cargo build -p fraktor-actor-core-rs` を実行し、module wiring の破綻がないことを確認する
- [ ] 3.5 `cargo test -p fraktor-actor-core-rs` を実行し、actor-core 全体が通ることを確認する

## 4. 完全退役確認

- [ ] 4.1 `grep -rn "BootingSystemState\\|RunningSystemState\\|booting_state\\|running_state" modules/actor-core/src` で 0 件を確認する
- [ ] 4.2 `grep -rn "register_guardian_pid" modules/actor-core/src/core/kernel/system/state` で wrapper 由来の caller が消え、既存テスト由来の caller だけが残ることを確認する
- [ ] 4.3 `openspec validate retire-dead-internal-scaffolding --strict` を実行する

## 5. 最終検証

- [ ] 5.1 `./scripts/ci-check.sh ai all` を実行し、エラーがないことを確認する

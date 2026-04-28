## 1. 事前確認

- [x] 1.1 `cargo test -p fraktor-actor-core-rs` のベースラインを確認する
- [x] 1.2 `BootingSystemState` / `RunningSystemState` の参照が `state.rs`、`booting_state.rs`、`running_state.rs`、`system_state/tests.rs` の専用テストに閉じていることを `grep` で確認する
- [x] 1.3 `register_guardian_pid` が wrapper 以外のテストからも使われていることを確認し、wrapper 削除後の production dead API として同 change で削除対象に含める
- [x] 1.4 `openspec/specs/` に `BootingSystemState` / `RunningSystemState` が登場しないことを確認する

## 2. wrapper 専用テスト削除

- [x] 2.1 `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` から `BootingSystemState` import を削除する
- [x] 2.2 `booting_into_running_requires_all_guardians` テストを削除する
- [x] 2.3 `booting_into_running_fails_when_guardian_missing` テストを削除する
- [x] 2.4 `cargo test -p fraktor-actor-core-rs system_state` を実行し、削除対象以外の system state テストが通ることを確認する

## 3. wrapper 本体削除

- [x] 3.1 `modules/actor-core/src/core/kernel/system/state/booting_state.rs` を削除する
- [x] 3.2 `modules/actor-core/src/core/kernel/system/state/running_state.rs` を削除する
- [x] 3.3 `modules/actor-core/src/core/kernel/system/state.rs` から `mod booting_state;` と `mod running_state;` を削除する
- [x] 3.4 `register_guardian_pid` 利用テストを production API へ置き換えるか、test-only API でしか表現できない unreachable case を削除する
- [x] 3.5 `SystemState::register_guardian_pid` / `SystemStateShared::register_guardian_pid` を削除する
- [x] 3.6 `cargo build -p fraktor-actor-core-rs` を実行し、module wiring の破綻がないことを確認する
- [x] 3.7 `cargo test -p fraktor-actor-core-rs` を実行し、actor-core 全体が通ることを確認する

## 4. 完全退役確認

- [x] 4.1 `grep -rn "BootingSystemState\\|RunningSystemState\\|booting_state\\|running_state" modules/actor-core/src` で 0 件を確認する
- [x] 4.2 `grep -rn "register_guardian_pid" modules/actor-core/src/core/kernel/system/state` で 0 件を確認する
- [x] 4.3 `openspec validate retire-dead-internal-scaffolding --strict` を実行する

## 5. 最終検証

- [x] 5.1 `./scripts/ci-check.sh ai all` を実行し、エラーがないことを確認する

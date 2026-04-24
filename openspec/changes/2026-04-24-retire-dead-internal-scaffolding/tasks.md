## 1. 事前確認

- [ ] 1.1 `cargo test -p fraktor-actor-core-rs` ベースライン pass 確認 (削除前の pass 件数を記録)
- [ ] 1.2 `grep -rn --include="*.rs" -E "(BootingSystemState|RunningSystemState|booting_state::|running_state::|mod booting_state|mod running_state)" modules/ src/` で参照が以下の範囲に限定されることを再確認
  - `modules/actor-core/src/core/kernel/system/state.rs` の `mod booting_state;` (L21) と `mod running_state;` (L22)
  - `modules/actor-core/src/core/kernel/system/state/booting_state.rs` 内 (self 定義 + `running_state::RunningSystemState` 参照)
  - `modules/actor-core/src/core/kernel/system/state/running_state.rs` 内 (self 定義のみ)
  - `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs:9` の `use ... booting_state::BootingSystemState` import
  - `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` L543-572 (`booting_into_running_requires_all_guardians`) / L574-587 (`booting_into_running_fails_when_guardian_missing`) の 2 テスト関数
- [ ] 1.3 `grep -rn "\.register_guardian_pid\|fn register_guardian_pid" modules/ src/` で caller が `booting_state.rs:19` の 1 箇所のみであることを再確認 (定義 2 箇所 + 1 caller の計 3 ヒット)
- [ ] 1.4 `grep -rn "guardian_alive_flag" modules/` で `mark_guardian_stopped` / `guardian_alive` からの利用が残ることを確認 (`register_guardian_pid` 削除後も helper が dead にならない根拠)
- [ ] 1.5 `grep -rn "BootingSystemState\|RunningSystemState" openspec/specs/` で 0 件 (spec 未記述) を確認

## 2. Phase 1 — tests から削除 (参照側を先に落とす)

> 依存関係逆順で進める: tests → booting_state → running_state の順。tests を残したまま本体を消すとコンパイルエラーが連鎖するため、tests から落とす。

- [ ] 2.1 `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs`:
  - L9 の `use super::{super::booting_state::BootingSystemState, SystemState};` から `super::super::booting_state::BootingSystemState,` フラグメントのみ除去 (残り `use super::SystemState;` あるいは `use super::{SystemState};` と整形)
  - L543 (`#[test]`) - L572 (`}`) の `fn booting_into_running_requires_all_guardians` 関数全体を削除 (`#[test]` 属性を含む)
  - L574 (`#[test]`) - L587 (`}`) の `fn booting_into_running_fails_when_guardian_missing` 関数全体を削除 (`#[test]` 属性を含む)
  - 削除後、削除箇所前後の空行を 1 行に詰める (Rust fmt 規約に合わせる)
- [ ] 2.2 `cargo test -p fraktor-actor-core-rs` pass 確認 (削除した 2 テストを除いた件数で pass)

## 3. Phase 2 — wrapper 本体削除

- [ ] 3.1 `modules/actor-core/src/core/kernel/system/state/booting_state.rs` を `git rm`
- [ ] 3.2 `modules/actor-core/src/core/kernel/system/state/running_state.rs` を `git rm`
- [ ] 3.3 `modules/actor-core/src/core/kernel/system/state.rs` から以下 2 行を削除:
  - `mod booting_state;` (L21)
  - `mod running_state;` (L22)
  - (※ `mod authority_state;` / `pub mod system_state;` / `mod system_state_shared;` / `mod system_state_weak;` は残す。`pub use` も無変更)
- [ ] 3.4 `cargo build -p fraktor-actor-core-rs` pass 確認 (この時点で `register_guardian_pid` 2 箇所が dead_code 警告になることを確認 — Phase 3 で解消)
- [ ] 3.5 `cargo test -p fraktor-actor-core-rs` pass 確認 (新規失敗なし)

## 4. Phase 3 — 連動 dead 化した `register_guardian_pid` 削除

- [ ] 4.1 `modules/actor-core/src/core/kernel/system/state/system_state.rs`: L512 付近の `pub(crate) fn register_guardian_pid(&mut self, kind: GuardianKind, pid: Pid) { ... }` メソッド全体を削除 (4 行程度、直前直後の空行も整理)
- [ ] 4.2 `modules/actor-core/src/core/kernel/system/state/system_state_shared.rs`: L428 付近の `pub(crate) fn register_guardian_pid(&self, kind: GuardianKind, pid: Pid) { ... }` wrapper 全体および直前の `/// Registers a PID for the specified guardian kind.` doc コメント削除 (4 行程度)
- [ ] 4.3 `cargo build -p fraktor-actor-core-rs` pass 確認 (dead_code 警告解消)
- [ ] 4.4 `cargo test -p fraktor-actor-core-rs` pass 確認

## 5. Phase 4 — workspace 全体検証

- [ ] 5.1 `cargo build --workspace` pass 確認
- [ ] 5.2 `cargo test --workspace` pass 確認
- [ ] 5.3 `grep -rn "BootingSystemState\|RunningSystemState\|booting_state\|running_state" modules/ src/` で 0 件を確認 (主対象の完全退役)
- [ ] 5.4 `grep -rn "register_guardian_pid" modules/ src/` で 0 件を確認 (連動 dead API の完全退役)
- [ ] 5.5 `./scripts/ci-check.sh dylint` で lint pass
- [ ] 5.6 `./scripts/ci-check.sh ai all` で全 CI 緑

## 6. artifact / docs 整合

- [ ] 6.1 `openspec validate 2026-04-24-retire-dead-internal-scaffolding --strict` で artifact 整合確認
- [ ] 6.2 `docs/plan/` に関連メモがあるか確認 (なければスキップ、あれば後続 hand-off 欄を更新)

## 7. コミット・PR

- [ ] 7.1 ブランチ作成: `refactor/retire-dead-internal-scaffolding` または `impl/retire-dead-internal-scaffolding`
- [ ] 7.2 論理単位での commit (Phase 1-2-3 を 1 commit でも分割でも可)
- [ ] 7.3 push + PR 作成 (base: main、title prefix `refactor(actor-core):` 推奨)
- [ ] 7.4 CI 全 pass + レビュー対応 + マージ
- [ ] 7.5 archive

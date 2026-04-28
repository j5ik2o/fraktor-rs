# source-test-layout-hygiene Specification

## Purpose
TBD - created by archiving change 2026-04-21-repo-wide-src-test-cleanup. Update Purpose after archive.

## Requirements

### Requirement: no_std-sensitive な production source tree は std 依存 test logic を含んではならない
no_std-sensitive な crate の production source tree は、`std::*` 依存の test-only code を `src/` 配下へ保持してはならない（MUST NOT）。std 依存の test logic は `tests/` 配下へ移すか、production path と分離された test fixture に切り出さなければならない（MUST）。

#### Scenario: core crate の std 依存 test は `tests/` へ移される
- **WHEN** no_std-sensitive な crate で `std::panic` や `std::thread` を必要とする test を追加または整理する
- **THEN** その test は `src/` 配下ではなく `tests/` 配下に置かれる

#### Scenario: production module には implementation だけが残る
- **WHEN** `src/` 配下の `tests.rs` が integration test へ移設可能と判断される
- **THEN** production module には runtime implementation だけが残り、std 依存 test helper は残されない

### Requirement: test-only helper は production module に居座ってはならない
test-only helper / type / method は、production path から見て未使用な状態で module に残されてはならない（MUST NOT）。test のためだけに必要な helper は `tests/` 配下の fixture へ移すか、test 専用 module に閉じ込めなければならない（MUST）。

#### Scenario: integration test へ移した helper は production 公開面に露出しない
- **WHEN** `src/` 配下の test helper を `tests/` 側へ移設する
- **THEN** helper を使うためだけに production API の可視性を広げない

#### Scenario: cleanup は runtime semantics を変えない
- **WHEN** test module の配置整理や dead code 整理を行う
- **THEN** 既存テストの assertion と対象 runtime behavior は維持される

### Requirement: file-level dead-code scaffolding は production source tree に残さない
production source tree にある内部型は、production callsite を持たない状態でファイル冒頭 `#![allow(dead_code)]` により丸ごと警告抑制されていてはならない（MUST NOT）。test-only に閉じた古い scaffolding は、対象テストと一緒に削除するか、必要性を再評価して明示的な production contract に昇格しなければならない（MUST）。

#### Scenario: production 未使用の boot/running wrapper を削除する
- **WHEN** `modules/actor-core/src/core/kernel/system/state/` を確認する
- **THEN** `BootingSystemState` と `RunningSystemState` は production source tree に存在しない
- **AND** `booting_state.rs` / `running_state.rs` の file-level `#![allow(dead_code)]` は存在しない

#### Scenario: wrapper 専用テストを残さない
- **WHEN** `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` を確認する
- **THEN** `BootingSystemState` / `RunningSystemState` のみを検証するテストは存在しない

#### Scenario: test-only guardian PID registration API を残さない
- **WHEN** `register_guardian_pid` の利用箇所を確認する
- **THEN** `SystemState::register_guardian_pid` と `SystemStateShared::register_guardian_pid` は存在しない
- **AND** guardian registration を必要とするテストは実 `ActorCell` を `set_*_guardian` に渡す production API 経由で表現されている

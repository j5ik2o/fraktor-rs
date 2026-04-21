# source-test-layout-hygiene Specification

## Purpose
TBD - created by archiving change 2026-04-21-repo-wide-src-test-cleanup. Update Purpose after archive.

## Requirements

### Requirement: no_std-sensitive な production source tree は std 依存 test logic を含んではならない
no_std-sensitive な crate の production source tree は、`std::*` 依存の test-only code を `src/` 配下へ保持してはならない。std 依存の test logic は `tests/` 配下へ移すか、production path と分離された test fixture に切り出さなければならない。

#### Scenario: core crate の std 依存 test は `tests/` へ移される
- **WHEN** no_std-sensitive な crate で `std::panic` や `std::thread` を必要とする test を追加または整理する
- **THEN** その test は `src/` 配下ではなく `tests/` 配下に置かれる

#### Scenario: production module には implementation だけが残る
- **WHEN** `src/` 配下の `tests.rs` が integration test へ移設可能と判断される
- **THEN** production module には runtime implementation だけが残り、std 依存 test helper は残されない

### Requirement: test-only helper は production module に居座ってはならない
test-only helper / type / method は、production path から見て未使用な状態で module に残されてはならない。test のためだけに必要な helper は `tests/` 配下の fixture へ移すか、test 専用 module に閉じ込めなければならない。

#### Scenario: integration test へ移した helper は production 公開面に露出しない
- **WHEN** `src/` 配下の test helper を `tests/` 側へ移設する
- **THEN** helper を使うためだけに production API の可視性を広げない

#### Scenario: cleanup は runtime semantics を変えない
- **WHEN** test module の配置整理や dead code 整理を行う
- **THEN** 既存テストの assertion と対象 runtime behavior は維持される

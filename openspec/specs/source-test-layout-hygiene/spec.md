# source-test-layout-hygiene Specification

## Purpose
TBD - created by archiving change 2026-04-21-repo-wide-src-test-cleanup. Update Purpose after archive.
## Requirements
### Requirement: no_std-sensitive な production source tree は std 依存 test logic を含んではならない
no_std-sensitive な crate の production source tree は、`std::*` 依存の test-only code を `src/` 配下へ保持してはならない（MUST NOT）。std 依存の test logic は `tests/` 配下へ移すか、production path と分離された test fixture に切り出さなければならない（MUST）。

#### Scenario: core crate の std 依存 test は `tests/` へ移される
- **WHEN** no_std-sensitive な crate で `std::panic` や `std::thread` を必要とする test を追加または整理する
- **THEN** その test は `src/` 配下ではなく `tests/` 配下に置かれる

#### Scenario: production module には実装と test hook だけが残る
- **WHEN** `src/` 配下の crate 内 unit test を production file から分離する
- **THEN** production module には runtime implementation と `#[cfg(test)] #[path = "<module>_test.rs"] mod tests;` hook だけが残る
- **AND** test body / test helper / test fixture は production file 本体へ書かれない

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

### Requirement: crate 内 unit test は sibling `_test.rs` に置く
`src/` 配下に残す crate 内 unit test は、対象 production file と同じディレクトリの sibling `<module>_test.rs` に置かなければならない（MUST）。`<module>/tests.rs` のためだけにディレクトリを作ってはならない（MUST NOT）。production file は test module を `tests` という module name で宣言し、物理ファイルだけを `#[path = "<module>_test.rs"]` で指定しなければならない（MUST）。

#### Scenario: leaf module の unit test は sibling file に置かれる
- **WHEN** `hoge.rs` の crate 内 unit test を追加または移設する
- **THEN** test body は `hoge_test.rs` に置かれる
- **AND** `hoge.rs` は `#[cfg(test)] #[path = "hoge_test.rs"] mod tests;` で test module を有効化する

#### Scenario: test-only directory は作られない
- **WHEN** `hoge.rs` に実サブモジュールがない状態で unit test だけを追加する
- **THEN** `hoge/` directory は作られない
- **AND** `hoge/tests.rs` は作られない

#### Scenario: crate root の unit test は `lib_test.rs` に置かれる
- **WHEN** `src/lib.rs` の crate 内 unit test を追加または移設する
- **THEN** test body は `src/lib_test.rs` に置かれる
- **AND** `src/lib.rs` は `#[cfg(test)] #[path = "lib_test.rs"] mod tests;` で test module を有効化する

### Requirement: Dylint は sibling test layout を強制する
custom Dylint は、production file 内の inline test を禁止し続けなければならない（MUST）。同時に、test-only `#[path = "..._test.rs"] mod tests;` だけを許可し、その他の `#[path = ...]` module wiring は禁止し続けなければならない（MUST）。

#### Scenario: production file の inline test は拒否される
- **WHEN** production file に `#[cfg(test)] mod tests { ... }` または `#[test]` function が書かれる
- **THEN** `tests-location-lint` は sibling `<module>_test.rs` へ移すよう報告する

#### Scenario: 制約付き test path attribute は許可される
- **WHEN** `hoge.rs` に `#[cfg(test)] #[path = "hoge_test.rs"] mod tests;` が書かれる
- **THEN** `tests-location-lint` は `#[cfg(test)]` item として報告しない
- **AND** `module-wiring-lint` は `#[path = "hoge_test.rs"]` を報告しない

#### Scenario: 任意の path attribute は拒否される
- **WHEN** `hoge.rs` に `#[path = "helper.rs"] mod helper;` または `#[cfg(test)] #[path = "shared/hoge_test.rs"] mod tests;` が書かれる
- **THEN** `module-wiring-lint` は path attribute の module wiring を報告する

#### Scenario: `_test.rs` file は production-oriented lint から test-only として扱われる
- **WHEN** `hoge_test.rs` に test helper type や test helper name が存在する
- **THEN** `tests.rs` を既に無視している production-oriented lint は `*_test.rs` も無視する（MUST）
- **AND** `type-per-file-lint` と `ambiguous-suffix-lint` は `*_test.rs` から production naming または type-placement violation を報告してはならない（MUST NOT）


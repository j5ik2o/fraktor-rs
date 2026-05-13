## MODIFIED Requirements

### Requirement: no_std-sensitive な production source tree は std 依存 test logic を含んではならない
no_std-sensitive な crate の production source tree は、`std::*` 依存の test-only code を `src/` 配下へ保持してはならない（MUST NOT）。std 依存の test logic は `tests/` 配下へ移すか、production path と分離された test fixture に切り出さなければならない（MUST）。

#### Scenario: core crate の std 依存 test は `tests/` へ移される
- **WHEN** no_std-sensitive な crate で `std::panic` や `std::thread` を必要とする test を追加または整理する
- **THEN** その test は `src/` 配下ではなく `tests/` 配下に置かれる

#### Scenario: production module には実装と test hook だけが残る
- **WHEN** `src/` 配下の crate 内 unit test を production file から分離する
- **THEN** production module には runtime implementation と `#[cfg(test)] #[path = "<module>_test.rs"] mod tests;` hook だけが残る
- **AND** test body / test helper / test fixture は production file 本体へ書かれない

## ADDED Requirements

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

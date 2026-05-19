## 1. ルールとドキュメント

- [x] 1.1 `.agents/rules/**/*.md` の `<module>/tests.rs` 参照を sibling `<module>_test.rs` へ更新する。
- [x] 1.2 rule example を `#[cfg(test)] #[path = "<module>_test.rs"] mod tests;` に更新する。
- [x] 1.3 fraktor の module / test layout を生成・レビューする skill documentation を更新する。対象には `creating-fraktor-modules`、`designing-fraktor-shared-types`、`reviewing-fraktor-types`、`tests.rs` に触れる package / refactoring reference を含める。
- [x] 1.4 test-file ignore pattern または test-location guidance を説明している lint README / SPEC を更新する。

## 2. Dylint 挙動

- [x] 2.1 `tests-location-lint` を更新し、`*_test.rs` file を test-only file として扱う。
- [x] 2.2 `tests-location-lint` を更新し、`#[cfg(test)] #[path = "<module>_test.rs"] mod tests;` を production-side hook として許可する。
- [x] 2.3 `tests-location-lint` の diagnostic を更新し、inline test を `<module>_test.rs` へ移すよう AI エージェントへ指示する。
- [x] 2.4 `module-wiring-lint` を更新し、module が `tests`、item が `#[cfg(test)]`、path が `<declaring_file_stem>_test.rs` と一致する制約付き test path attribute だけを許可する。
- [x] 2.5 `module-wiring-lint` は、non-test module、directory path、file stem 不一致、`#[cfg(test)]` 欠落を含む任意の `#[path = ...]` を引き続き拒否する。
- [x] 2.6 `type-per-file-lint` を更新し、`tests.rs` と `*_tests.rs` と同様に `*_test.rs` を無視する。
- [x] 2.7 `ambiguous-suffix-lint` を更新し、`tests.rs` と `*_tests.rs` と同様に `*_test.rs` を無視する。

## 3. Dylint テスト

- [x] 3.1 `tests-location-lint` の UI fixture を追加または更新し、許可される sibling `_test.rs` layout と拒否される inline test を検証する。
- [x] 3.2 `module-wiring-lint` の UI fixture を追加または更新し、`#[cfg(test)] #[path = "hoge_test.rs"] mod tests;` が許可されることを検証する。
- [x] 3.3 `module-wiring-lint` の UI fixture を追加または更新し、拒否される path attribute variant を検証する。
- [x] 3.4 `type-per-file-lint` の UI fixture を追加または更新し、`*_test.rs` 内の public test helper type が無視されることを検証する。
- [x] 3.5 `ambiguous-suffix-lint` の UI fixture を追加または更新し、`*_test.rs` 内の test helper name が無視されることを検証する。
- [x] 3.6 変更したすべての lint について targeted Dylint UI test を実行する。

## 4. Source 移行

- [x] 4.1 現在の `modules/**/src/**/tests.rs` file を棚卸しし、crate ごとに migration batch を分ける。
- [x] 4.2 nested `hoge/tests.rs` を sibling `hoge_test.rs` へ、小さな crate / module batch 単位で移行する。
- [x] 4.3 各 parent production file を `#[cfg(test)] mod tests;` から `#[cfg(test)] #[path = "<module>_test.rs"] mod tests;` へ更新する。
- [x] 4.4 root `src/tests.rs` を `src/lib_test.rs` へ移行し、対応する `lib.rs` hook を更新する。
- [x] 4.5 migration 中、既存 test assertion、helper visibility、runtime behavior を維持する。
- [x] 4.6 no_std-sensitive crate で安全に crate-internal `_test.rs` として残せない std 依存 test は、引き続き `tests/` 配下に置く。

## 5. Verification

- [x] 5.1 移行した batch ごとに crate-level test を実行する。
- [x] 5.2 `./scripts/ci-check.sh dylint tests-location-lint` を実行する。
- [x] 5.3 `./scripts/ci-check.sh dylint module-wiring-lint` を実行する。
- [x] 5.4 `./scripts/ci-check.sh dylint type-per-file-lint` を実行する。
- [x] 5.5 `./scripts/ci-check.sh dylint ambiguous-suffix-lint` を実行する。
- [x] 5.6 最終確認として `./scripts/ci-check.sh ai all` を実行する。

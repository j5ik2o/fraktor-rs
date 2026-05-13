## Why

現在の unit test 配置は `hoge.rs` と `hoge/tests.rs` の組み合わせであり、`hoge/` が実サブモジュール用ディレクトリなのか、テストファイルだけを置くためのディレクトリなのか判別しづらい。この曖昧さによりナビゲーション性が落ち、AI エージェントが編集時に production file と test file のリスク差を見誤りやすくなる。

## What Changes

- **BREAKING**: crate 内 unit test の配置を `<module>/tests.rs` から sibling の `<module>_test.rs` へ変更する。
- production file は test module を次の形で宣言する。
  ```rust
  #[cfg(test)]
  #[path = "hoge_test.rs"]
  mod tests;
  ```
- Rust module 名は `tests` のまま維持し、既存の `use super::*;` ベースの test file に必要な変更を最小化する。
- `.agents/rules/**/*.md` と関連 skill 参照を更新し、AI エージェントが sibling `_test.rs` layout で test を作成・移行するようにする。
- Dylint を更新し、test は別ファイル必須のまま、test 専用の制約付き `#[path = "..._test.rs"] mod tests;` だけを許可する。
- 既存の `src/**/tests.rs` を sibling `*_test.rs` へ移行し、runtime behavior は変更しない。

## Capabilities

### New Capabilities

### Modified Capabilities
- `source-test-layout-hygiene`: crate 内 unit test を nested `tests.rs` から production file の sibling `*_test.rs` へ移し、Dylint で新 layout を強制する。

## Impact

- 影響する rule:
  - `.agents/rules/rust/*.md`
  - `.agents/rules/*.md` references that mention `tests.rs`
  - fraktor の module / test layout を生成・レビューする skill documentation
- 影響する lint:
  - `lints/tests-location-lint`
  - `lints/module-wiring-lint`
  - `lints/type-per-file-lint`
  - `lints/ambiguous-suffix-lint`
  - 上記 lint の README / SPEC / UI fixture
- 影響する source:
  - `modules/**/src/**/tests.rs`
  - 現在 `#[cfg(test)] mod tests;` を宣言している parent production file
- 検証:
  - 変更した各 lint の targeted Dylint UI test
  - 代表的な migration batch ごとの対象 crate test
  - 最終 `./scripts/ci-check.sh ai all`

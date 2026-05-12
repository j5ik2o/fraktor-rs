---
paths:
  - "**/*.rs"
---
# Rust Rules

このファイルは Rust ルールの入口であり、詳細本文は専用ファイルを正とする。

## 詳細ルール

| 領域 | 正となるファイル |
|------|------------------|
| 内部可変性 / Shared ラッパー | `./immutability-policy.md` |
| CQS | `./cqs-principle.md` |
| 1ファイル1公開型 | `./type-organization.md` |
| 命名 / rustdoc 言語 / Shared と Handle | `./naming-conventions.md` |
| 参照実装からの逆輸入 | `./reference-implementation.md` |
| 戻り値の握りつぶし禁止 | `../ignored-return-values.md` |
| プロジェクト固有の Rust パターン | `./local.md` |

## Dylint で機械的に強制される構造ルール

- `mod-file-lint`: `mod.rs` 禁止。`foo.rs` + `foo/bar.rs` の階層配置にする
- `module-wiring-lint`: 親モジュールは `mod` と `pub use` に絞り、子の公開型を親へ集約しない
- `tests-location-lint`: テストは対象ファイルの sibling `*_test.rs` に分離する
- `use-placement-lint`: `use` 宣言はファイル先頭に集約する
- `redundant-fqcn-lint`: コード本体では FQCN を直接書かず `use` で取り込む
- `module-examples-lint`: `modules/**/examples` を作らず、実行可能サンプルは `showcases/std/examples/` に置く
- `rustdoc-lint`: rustdoc は英語、Markdown と通常コメントは日本語にする
- `cfg-std-forbid-lint`: `*-core` クレートから `std::*` へ直接依存しない

## 運用ルール

- `#[allow]` による lint 回避は人間の許可を得る
- TOCTOU 回避のため、read-then-act は `with_write` クロージャ等で原子化する
- `*-core` クレートは `#![cfg_attr(not(test), no_std)]` と `#![deny(cfg_std_forbid)]` を維持する

## Examples

迷ったら `./examples.md` を見る。

---
paths:
  - "**/*.rs"
---
# Rust Rules

## Principles

- 内部可変性禁止（`&mut self` 第1選択、`RefCell`/`Cell`/直接ロック型を
  `&self` で隠さない、共有が必要な場合のみ Shared ラッパー経由）
- CQS 厳守（`&self` で読み取り戻り値あり、`&mut self` で更新かつ
  戻り値なし or `Result<(), E>`、違反は人間許可必須）
- 1ファイル1公開型（`type-per-file-lint` で機械的強制、エラー型 /
  Shared / Handle / ドメインプリミティブは常に独立、例外は ≤20行 +
  親型限定 + ファイル ≤200行）
- mod.rs 禁止（`mod-file-lint`、`foo.rs` + `foo/bar.rs` の階層配置、
  wiring は `foo.rs` に書く）
- 親モジュールでのバレル的な再エクスポート集約は禁止
  （`module-wiring-lint`）。親で子の公開型をまとめて API 化する
  `pub use` 集約は書かない。
- 親モジュールの `pub use` は `module-wiring-lint` の wiring 用途に限る。
  子が公開型を定義し、親は必要な子型・シンボルだけを最小限露出する。
- テストは sibling `*_test.rs` 分離（`tests-location-lint`、インライン
  `#[cfg(test)] mod tests {}` 禁止、対象ファイル隣に `<module>_test.rs` を置き
  `#[cfg(test)] #[path = "<module>_test.rs"] mod tests;` で取り込む）
- `use` 宣言はファイル先頭に集約（`use-placement-lint`、関数内 `use` /
  宣言の合間挿入禁止）
- コード本体での FQCN 禁止（`redundant-fqcn-lint`、`use` で取り込んでから
  短い名前で参照、`use` 宣言内の FQCN は許可）
- `modules/**/examples` 禁止（`module-examples-lint`、実行可能サンプルは
  `showcases/std/examples/` 配下のみ）
- 曖昧サフィックス禁止（`ambiguous-suffix-lint`、Manager/Util/Service/
  Runtime/Engine/Facade、責務別命名表から具体名を選ぶ）
- ドキュメント言語の使い分け（`rustdoc-lint`、`///` `//!` は英語、
  それ以外のコメント・Markdown は日本語）
- 戻り値の握りつぶし禁止（`Result` / `Option` / `#[must_use]` を
  `let _ =` / `.ok()` で捨てない、fire-and-forget でも理由をコメントで明示）
- 参照実装の命名優先（Pekko / protoactor-go のドメイン用語が責務別命名と
  衝突した場合は参照実装に合わせる、例: `SupervisorStrategy`/`Behavior`/`Props`）
- `*-core` クレートは `#![cfg_attr(not(test), no_std)]` +
  `#![deny(cfg_std_forbid)]`（`std::*` 直接依存禁止、`extern crate alloc` で
  `alloc` を取り込む）
- TOCTOU 回避設計（read-then-act を `with_write` クロージャ等で原子化、
  外部にロックガードを返さない）
- lint の `#[allow]` による回避は人間許可必須（外部 API 由来の名称等の
  例外のみ `#[allow(ambiguous_suffix::ambiguous_suffix)]` のような明示形を使う）
- `&self` メソッド + 戻り値の Builder / Iterator パターン以外で
  `&mut self` + 戻り値を返す CQS 違反は分離する（`Vec::pop` 相当のみ
  人間許可前提で許容）
- ファイル / ディレクトリ / Cargo features / クレート名の命名
  （ファイル・ディレクトリ `snake_case`、型 `PascalCase`、
  Cargo features `kebab-case`、クレート `fraktor-<domain>-rs`）

## Examples

When in doubt: ./rust.examples.md

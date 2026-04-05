# module-examples-lint

## 概要

- `module_examples_forbid` リンターは、`modules/**/examples/*.rs` 配下に runnable example を置くことを禁止します。
- 実行例は各モジュールへ分散させず、`showcases/std` に集約するこのリポジトリの運用を機械的に強制するための lint です。

## チェック内容

- `modules/<crate>/examples/` 配下にある Rust ソースファイルを検出すると警告します。
- 同じファイルへの重複報告は行いません。
- `showcases/std` 配下や `target/` 配下のファイルは対象外です。

## 違反例

```text
modules/actor-adaptor/examples/classic_logging.rs
modules/actor/examples/typed_event_stream.rs
```

## 修正ガイド

1. `modules/<crate>/examples/*.rs` を `showcases/std/<example-name>/main.rs` へ移動する。
2. 対応する module crate の `Cargo.toml` から `[[example]]` エントリを削除する。
3. `showcases/std/Cargo.toml` に example エントリを追加し、必要なら feature 条件を移す。
4. 移動後に `./scripts/ci-check.sh ai examples` または `./scripts/ci-check.sh ai all` を実行して確認する。

## 例外指定

- 一時的に無効化する場合は `#![allow(module_examples_forbid)]` または `#![allow(module_examples_lint::module_examples_forbid)]` を使えます。
- ただし恒久運用は想定しません。例外ではなく `showcases/std` への移動を優先してください。

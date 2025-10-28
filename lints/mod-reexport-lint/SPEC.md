# mod-reexport-lint 仕様

## 概要
- 目的: `foo_detail::FooDetail` のように、モジュール名と型名が同じトークン列を共有する冗長な `use` を警告し、`mod` + `pub use` の再エクスポートに誘導する。
- 対象: すべての Rust ソースファイル。マクロ展開されたコードは対象外。

## ルール
1. `use` ツリーの末尾が `<module_name>::ModuleType` であり、`ModuleType` が `<module_name>` を UpperCamelCase 化した識別子と一致する場合に警告する。
2. `as` によるリネームを伴う場合は対象外とする。`{ ... }` によるグループ指定の場合は各要素を個別に判定する。

## 推奨修正例
- 警告箇所のあるファイル内で `mod <module_name>;` と `pub use <module_name>::ModuleType;` を宣言し、`use` を削除する。
- AI 用メモ: 同名モジュールは同ファイル内でモジュール宣言 + 再エクスポートに置き換え、呼び出し側はそのまま `use crate::foo::ModuleType;` で利用できる状態に整える。

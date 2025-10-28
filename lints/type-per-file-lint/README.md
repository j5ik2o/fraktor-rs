# type-per-file-lint

## 概要
- `type_per_file::multiple_type_definitions` リンターは、1 ファイルに複数の公開構造体・列挙型・トレイトを定義することを防ぎます。
- ファイルと型の対応を 1:1 に保ち、モジュール構造・リンター群・テスト配置との整合性を高める目的で導入されています。

## チェック内容
- `pub` な `struct` / `enum` / `trait` をファイル単位で追跡し、同じファイルで 2 つ目以降の公開型を検出すると警告します。
- テストファイル（`tests.rs`、`*_tests.rs`、`tests/` 配下）や生成コード、`target/` ディレクトリはスキップされます。
- 非公開型やマクロ展開由来の型は対象外です。

## 違反例
```rust
pub struct ActorRef;
pub enum MailboxState {
  Idle,
  Busy,
}
```

## 修正ガイド
1. 追加で定義している型ごとに新しいファイルを用意する。例: `mailbox_state.rs`。
2. 親モジュールで `mod mailbox_state;` を宣言し、必要に応じて `pub use mailbox_state::MailboxState;` を追加する。
3. 元ファイルから余分な型定義を削除し、関連する `use` / `pub use` を更新する。
4. 実装移動後に `cargo check` を走らせ、公開パスの破損がないか確認する。

## 例外指定
- 一時的に許容する場合は `#![allow(multiple_type_definitions)]` または `#![allow(type_per_file::multiple_type_definitions)]` を追加する。
- 局所的な回避は `#[allow(multiple_type_definitions)]` を当該アイテムの直前に書く。ただし恒久的な利用は避けること。

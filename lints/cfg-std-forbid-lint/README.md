# cfg-std-forbid-lint

## 概要
- `cfg_std_forbid` リンターは `no_std` なコンポーネントから `std` 依存を排除するためのチェックです。
- `#[cfg(feature = "std")]` の分岐や `use std::...` を発見し、`std` 対応クレートへ切り出すか最小範囲で許可指定することを促します。
- `protoactor-go` や `pekko` のように実装とランタイムを分離する設計を前提に、セルアクターランタイムのポータビリティを確保する目的で導入されています。

## チェック内容
- ソース全体・モジュール・アイテムに付与された `#[cfg(feature = "std")]`。
- `use std::` を起点としたインポート（ネストした `use foo::{bar, std::baz}` なども含む）。
- `#[cfg(test)]` 付き要素や `#[allow(cfg_std_forbid)]` が明示された範囲は検査対象外です。

## 違反例
```rust
#[cfg(feature = "std")]
fn current_time() -> std::time::SystemTime {
  std::time::SystemTime::now()
}

use std::sync::Arc;
```

## 修正ガイド
1. `std` 依存コードを `alloc` や `core` ベースへ書き換えられないか検討する。
2. どうしても `std` が必要な処理は `std` 対応クレート（例: `cellactor-std`）へ移動し、境界を明示する。
3. 一時的に例外が必要な場合は、影響範囲を最小限にして `#[allow(cfg_std_forbid)]` を付ける。ファイル全体を許可する場合は先頭に `#![allow(cfg_std_forbid)]` を置く。
4. 移動後は `cargo check` を実行して依存関係が崩れていないことを確認する。

## 利用方法
- 対象クレートで `#![warn(cfg_std_forbid)]` または `#![deny(cfg_std_forbid)]` を設定する。
- デバッグ用途で一時無効化する場合は `#[allow(cfg_std_forbid)]` を違反箇所の直前に付与する。長期的な恒久措置としては推奨されない。

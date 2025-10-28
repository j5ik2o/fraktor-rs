# module-wiring-lint

## 概要
- `module_wiring::no_parent_reexport` リンターは、モジュールの公開規約と再エクスポートの経路を統一するための複合ルールです。
- 末端モジュール（子を持たないモジュール）は `mod` 宣言でのみ親へ接続し、公開したいシンボルは親モジュールで `pub use` する、という運用を強制します。
- モジュール構造の可視性を明確にし、依存グラフを `protoactor-go` / `pekko` で採用されている「親が API を束ねる」形に寄せることが目的です。

## 基本ルール
- **葉モジュールの宣言は常に非公開**：`pub mod foo;` や `pub(crate) mod foo;` は警告対象です。宣言から可視性修飾子を外し、親で必要なシンボルを再エクスポートしてください。
- **再エクスポートは直属の親のみ許可**：`pub use child::Type;` は `child` を直接 `mod` 宣言しているモジュールでのみ実行できます。祖父モジュールから `pub use grandchild::Type;` を行うと違反になります。
- **特殊パスと別名は禁止**：`pub use self::child::Type;`、`pub use super::child::Type;`、`pub use crate::child::Type;`、および `pub use child::Type as Alias;` は検出対象です。
- **葉でないモジュールの再エクスポート禁止**：さらに子モジュールを持つモジュールを丸ごと再エクスポートすることは認められません。葉モジュールまで降りてから公開してください。
- **`prelude` は特例**：`prelude` モジュール内の再エクスポートは許容されています。

## 違反例
```rust
// 親モジュール(parent.rs)
pub mod child; // × 葉モジュールを直接公開している
pub use child::Service; // この組み合わせで警告

// 祖父モジュール(grand.rs)
pub mod parent;
pub use parent::child::Service; // × 直属親以外からの再エクスポート
```

## 修正ガイド
1. 葉モジュールの宣言から `pub` / `pub(crate)` を取り除き、単に `mod child;` と宣言する。
2. 葉モジュール内で公開したい型・関数に `pub` を付与する。
3. 親モジュールで `pub use child::Service;` のように再エクスポートし、祖父モジュールから直接再エクスポートしていた場合は呼び出し元を親モジュール経由へ書き換える。
4. `pub use child::Service as Alias;` といった別名付けが必要なら、呼び出し側で `use` する際に `as` を使う。
5. 変更後は `cargo check` を実行し、公開 API が意図通り露出しているか確認する。

## 例外指定
- 一時的に抑制したい場合はファイル先頭で `#![allow(module_wiring::no_parent_reexport)]` を指定する。
- 個別の行を許可する場合は対象行に `// allow module_wiring::no_parent_reexport` を付ける。コメントの行内・直前どちらでも検出されます。
- 長期的な恒久措置としての許可は避け、ルールに沿った再配線を優先してください。

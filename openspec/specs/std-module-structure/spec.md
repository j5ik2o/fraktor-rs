# std-module-structure Specification

## Purpose
TBD - created by archiving change normalize-std-module-structure. Update Purpose after archive.

## Requirements
### Requirement: std 側のモジュール構造が core 側と同じパターンに従う

std.rs の各 `pub mod xxx` 宣言に対応する `std/xxx.rs` ファイルが存在し、インラインモジュール定義を使用しては MUST NOT。

#### Scenario: std.rs が外部ファイル参照のみで構成される

- **WHEN** `std.rs` の内容を確認する
- **THEN** すべてのモジュール宣言が `pub mod xxx;` 形式である
- **AND** インラインの `pub mod xxx { ... }` が存在しない

## Why

`modules/actor/src/std.rs` の5つのモジュール（`dispatch`, `event`, `scheduler`, `system`, `typed`）がインラインモジュール定義で宣言されており、対応する `.rs` ファイルが存在しない。core 側は全モジュールが `core.rs` で `pub mod xxx;` + `core/xxx.rs` + `core/xxx/` の Rust 2018 正規構造に従っている。

`learning-before-coding.md` ルール（「既存の実装を分析してから書け」）に違反した状態であり、`mod-file-lint` の期待する構造とも不整合。`pattern` だけが正しい構造で、残り5つが不統一。

## What Changes

- `std.rs` のインラインモジュール定義5つを外部ファイル参照（`pub mod xxx;`）に変更
- 対応する `.rs` ファイル8つを新規作成（2段ネストの `dispatch`, `event` は中間ファイルも必要）
- ロジック変更なし、モジュール構造の正規化のみ

## Capabilities

### Modified Capabilities

- `std-module-structure`: std 側のモジュール構造を core 側と同じ Rust 2018 正規パターンに統一する

## Impact

- 影響コード: `modules/actor/src/std.rs` + 新規 `.rs` ファイル8つ
- 影響 API: なし（公開 API は変更なし）
- リスク: ゼロ（機械的な抽出のみ）

## Non-goals

- std 側のモジュール内容・ロジックの変更
- 公開 API の変更

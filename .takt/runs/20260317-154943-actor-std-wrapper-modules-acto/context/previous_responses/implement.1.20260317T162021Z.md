## 作業結果
- order.md で要求された全要件（wrapper ファイル11件の削除、std.rs への再エクスポート集約、テストでの削除固定）が既に実装済みであることを確認
- 対象11ファイル全てが削除済み
- `std.rs` にインラインモジュール宣言として吸収済み
- `std/tests.rs` の `REMOVED_STD_ALIAS_FILES` に全ファイルが含まれ、復活防止テストが機能
- 追加のコード変更は不要

## 変更内容
- プロダクションコードの変更なし（全要件が既に実装済み）
- レポートファイルのみ作成: `coder-scope.md`, `coder-decisions.md`

## ビルド結果
- `cargo build -p fraktor-actor-rs --features std,tokio-executor` → 成功（既知のwarning 1件のみ: `new_with_clock` 未使用）

## テスト結果
- `cargo test -p fraktor-actor-rs --features std,tokio-executor -- std::tests` → 2テスト全てパス
  - `removed_std_alias_files_stay_deleted` — OK
  - `std_public_modules_expose_only_live_entry_points` — OK
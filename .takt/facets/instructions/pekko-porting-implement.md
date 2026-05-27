{extends:implement-after-tests}

## Pekko porting 固有の補足

Pekko の契約意図を Rust / fraktor-rs の設計原則を壊さずに実装すること。
見た目だけ Pekko に似せる実装は失敗とみなす。

## タスク分解（team_leader として実行される場合）

このステップは team_leader モードで実行されます。計画レポート（`00-plan.md`）の
「並行実装マップ」セクションを参照し、以下のルールでサブタスクに分解してください。

1. 計画レポートの**並行グループ**に従ってサブタスクを分ける
2. **同一ファイルを変更するタスクは同一サブタスクに割り当てる**（ファイル競合回避）
3. 依存関係がある場合は依存元を先のサブタスクに含める
4. max_parts（最大3）を超える場合は、ファイル競合しないグループにまとめる

## Fraktor/Pekko 実装前チェック

- 新規作成したクラス・関数・公開APIには単体テストを追加する
- 既存コードを変更した場合は該当するテストを更新する
- `write_tests` が「テスト対象が未実装のためテスト作成をスキップする」で `implement` に遷移した場合、実装と同時に対応するテストを必ず追加する
- 本家 `implement-after-tests` の「テストは既に作成済み」という前提より、この workflow のテスト追加必須ルールを優先する
- wrapper / alias を追加しただけで互換 API を実装したことにしていないか確認する
- `ignore()` / `empty()` / `self` を返すだけの fallback を public API に露出していないか確認する
- no-op / placeholder のまま Pekko互換名を public にしていないか確認する
- `public API` と `internal implementation` の境界が悪化していないか確認する
- no_std/std分離、CQS、1ファイル1公開型、Dylint lint に違反していないか確認する

## 完了条件の追加

変更範囲に対応する lint / 型チェックと最小限のテストの成功ログを、coder-decisionsレポートの「実行結果」セクションに含めること。

## Fake Gap チェック（必須出力）

- wrapper/alias 偽装: なし / あり（内容）
- fallback/no-op 公開API: なし / あり（内容）
- public/internal 境界悪化: なし / あり（内容）

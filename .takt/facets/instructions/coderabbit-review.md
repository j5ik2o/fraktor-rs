## Coderabbitローカルレビュー (`coderabbit --prompt-only`)

以下を必ず実行してください。

- リポジトリルートで `coderabbit --prompt-only` を実行し、出力を保存する
- ビルドを伴うコマンド（`cargo check`, `cargo build`, `cargo test`）は実行しない
- 指摘内容を以下の判定に分ける
  - `approved`: 重要/中程度の指摘がなく、実装に進むに足る状態
  - `needs_fix`: 修正が必要な指摘がある

## 実行時の必須要件

- 実行結果の要点（検出指摘、対象ファイル、優先度、推奨対応）をレポートに反映すること
- `approved` の場合、検出件数ゼロか、未解決でブロッキングにならないことを明記
- `needs_fix` の場合、次アクションは `qa-fix` であることを明確化すること
- `coderabbit` が見つからない/実行不能な場合は、原因と対処を明記し、`needs_fix` 判定を出すこと

## 出力ルール

- レビューは必ず簡潔に `07-coderabbit-review.md` 形式で保存する
- 既存の`coder-decisions.md`に実行ログ（標準出力/標準エラーの要約）を添付すること

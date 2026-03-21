最終ゲートとして `./scripts/ci-check.sh ai all` を1回だけ実行してください。

このムーブメントではコード編集を行わず、CIの成否確認だけを行います。
失敗時は、最初の失敗箇所（コマンドと要点）を短くまとめて報告してください。
**重要**: 失敗時は Report Directory 内の `coder-decisions.md` にエラー詳細を追記すること（差し戻し先の fix ムーブメントがエラー内容を参照できるようにするため）。

**実行方法（重要 — タイムアウト・誤中断回避）:**

`ci-check.sh ai all` は全モジュールのビルド・テスト・lintを実行するため **15〜30分** かかります。

**絶対にやってはいけないこと:**
- 実行中に「出力が止まった」「フリーズした」と判断して中断すること。テストやlintのフェーズ間で数分間出力がないのは正常
- バックグラウンドタスクの状態を繰り返し確認すること
- 途中で再実行すること

**実行手順:**
1. 以下のコマンドを **`run_in_background: true`** で実行する:
   ```
   ./scripts/ci-check.sh ai all > /tmp/ci-check-result.txt 2>&1; echo "EXIT_CODE=$?" >> /tmp/ci-check-result.txt
   ```
2. **完了通知が来るまで何もせず待つ**。ポーリングしない。確認しない。待つだけ
3. 完了通知を受け取ったら、結果を確認:
   ```
   tail -20 /tmp/ci-check-result.txt
   ```
4. 失敗した場合はエラー箇所を `grep` で特定:
   ```
   grep -E "^error|FAILED|Exit code" /tmp/ci-check-result.txt | head -20
   ```

**必須出力（見出しを含める）**
## 実行コマンド
- `./scripts/ci-check.sh ai all`
## 実行結果
- {成功 / 失敗}
## 失敗時メモ
- {失敗した場合のみ、最初の失敗箇所と修正方針}

最後に必ず以下のどちらか1行をそのまま記載してください:
- `ci-check.sh ai all が成功`
- `ci-check.sh ai all が失敗`

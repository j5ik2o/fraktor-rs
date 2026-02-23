# タスク仕様

## 目的

`.takt/pieces/pekko-porting.yaml` の `fix` ムーブメントに `session: refresh` を追加し、コード編集フェーズのムーブメントで新しいAIセッションを開始するようにする。

## 要件

- [ ] `fix` ムーブメントに `session: refresh` を追加する
- [ ] `implement` および `ai_fix` ムーブメントと同様のパターンであることを確認する

## 受け入れ基準

- `fix` ムーブメントに `session: refresh` が設定されている
- `validate-takt-files.sh` がエラーなしで通る

## 参考情報

- GitHub Issue: #136
- 対象ファイル: `.takt/pieces/pekko-porting.yaml`
- 参考: `implement`（Line 92）と `ai_fix`（Line 155）には既に `session: refresh` がある

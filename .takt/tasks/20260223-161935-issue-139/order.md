# タスク仕様

## 目的

`.takt/pieces/pekko-porting.yaml` で参照されている未定義の instruction 識別子（`ai-review`, `ai-fix`, `arbitrate`）を解決する。

## 要件

- [ ] `ai_review` ムーブメントの `instruction: ai-review` を解決する（ビルトインに存在するか確認し、なければカスタム instruction を作成するか既存のものに変更する）
- [ ] `ai_fix` ムーブメントの `instruction: ai-fix` を解決する
- [ ] `ai_no_fix` ムーブメントの `instruction: arbitrate` を解決する
- [ ] ビルトイン instruction セットに含まれる場合はそのまま、含まれない場合は `.takt/facets/instructions/` にカスタムファイルを作成しセクションマップに追加する

## 受け入れ基準

- `pekko-porting.yaml` で参照されるすべての instruction がビルトインまたはカスタムファセットとして存在する
- `validate-takt-files.sh` がエラーなしで通る
- 実行時に instruction 未定義エラーが発生しない

## 参考情報

- GitHub Issue: #139
- 対象ファイル: `.takt/pieces/pekko-porting.yaml`
- ビルトイン instructions: `references/takt/builtins/ja/instructions/`

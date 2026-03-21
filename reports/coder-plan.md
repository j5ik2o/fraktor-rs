# 実装計画

1. `04-ai-review.md` の open finding を整理し、A4 スコープ外として除去すべき差分を確定する
2. `flow.rs`、`json_framing.rs`、関連テストから A5/B1/B5/B6 に相当する混入差分だけを最小変更で除去する
3. `SubstreamCancelStrategy` の A4 実装が残っていることを確認する
4. 対象 finding に対応する最小限のテストとチェックだけを実行し、結果を報告できる状態にする

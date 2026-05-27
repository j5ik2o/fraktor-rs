{extends:arbitrate}

## Pekko porting 固有の読み替え

- 本家の `ai-antipattern-review-1st` は、この workflow では `ai_review` と読み替える。
- 本家の `ai-antipattern-fix` は、この workflow では `ai_fix` と読み替える。
- 本家の `ai-antipattern-review.md` は、この workflow では `04-ai-review.md` と読み替える。

## やらないこと

- `cargo check` / `cargo build` / `cargo test` など、ビルドを伴うコマンドを実行しないこと。このステップはビルド権限がなく `Operation not permitted` で失敗する。ビルド検証は `fix` / `implement` ステップの責務。

## 判定基準

- `04-ai-review.md` の指摘が妥当で修正すべき場合 → 「ai_reviewの指摘が妥当（修正すべき）」
- `ai_fix` の修正不要判断が妥当な場合 → 「ai_fixの判断が妥当（修正不要）」

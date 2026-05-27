{extends:ai-antipattern-review}

## Pekko porting 固有の補足

- `cargo check` / `cargo build` / `cargo test` など、ビルドを伴うコマンドを実行しないこと。このステップはビルド権限がなく `Operation not permitted` で失敗する。ビルド検証は `fix` / `implement` ステップの責務。
- Previous Response から前回の open findings を抽出し、各 finding に `finding_id` を付与する。
- 各 finding を `new / persists / resolved / reopened` で判定する。`persists` または `reopened` の場合は、未解決または再発の根拠（ファイル/行）を示す。
- ブロッキング問題（`new`、`persists`、または `reopened`）が 1 件でもある場合は REJECT、0 件なら APPROVE と判定する。

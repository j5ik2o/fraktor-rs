# AIレビュー指示

## やること (Do)
1. AI生成コード特有の問題を対象ファイルで確認してください。主に、幻覚API、ファントムインポート、パターン補完エラー、過度な抽象化、未使用デッドコード、フォールバック濫用、指示外の後方互換追加をチェックしてください。
2. Previous Response から前回の open findings を抽出して、各 finding に `finding_id` を付与してください。
3. 各 finding を `new / persists / resolved` で判定してください。`persists` の場合は、未解決の根拠（ファイル/行）を示してください。
4. ブロッキング問題（`new` または `persists`）が1件でもある場合は REJECT、0件なら APPROVE を判定してください。

## 必須出力 (Required Output)
1. 変更した点とその根拠を、finding ごとに明記してください。
2. 最終判定を `REJECT` または `APPROVE` で示してください。
3. `REJECT` の場合は、必ずブロッキング issue の file/line 付きで修正方針を示してください。

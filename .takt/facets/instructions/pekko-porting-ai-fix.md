{extends:ai-antipattern-fix}

## Pekko porting 固有の補足

- AI レビュー結果はレポートディレクトリの `ai-antipattern-review.md` を一次情報として扱う
- `pass_previous_response: false` のため、Previous Response ではなくレポートディレクトリと実ファイル内容を優先する
- 複数の open finding を扱う場合は、`family_tag` 単位で重複を統合し、同時対応できる範囲を一括修正する
- 重い一括実行ではなく、対象 finding 群に対応する最小限の確認コマンドで検証する

## 判定基準

- open finding 群を修正・整理し、再レビュー可能な状態にした → 「AI問題の修正完了」
- 指摘が誤りであると根拠を示せる → 「修正不要（指摘対象ファイル/仕様の確認済み）」
- 修正すべきか判断できない → 「判断できない、情報不足」

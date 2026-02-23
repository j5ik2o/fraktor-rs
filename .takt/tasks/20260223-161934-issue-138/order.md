# タスク仕様

## 目的

`.takt/facets/output-contracts/pekko-compat-review.md` を30行以内に簡素化し、Output Contract の品質基準に準拠させる。

## 要件

- [ ] テンプレートを30行以内に削減する（現状46行）
- [ ] 「継続指摘」「解消済み」テーブルのサンプル行を削除し、ヘッダのみにする
- [ ] 「REJECT判定条件」を認知負荷軽減ルールに統合する
- [ ] 認知負荷軽減ルールをコードブロック内に含めるか、テンプレート本体に統合する

## 受け入れ基準

- Output Contract が30行以内に収まっている
- テンプレートとしての一貫性が保たれている（コードブロック内外の構造）
- レビュアーが必要な情報を出力できるテンプレート構造が維持されている

## 参考情報

- GitHub Issue: #138
- 対象ファイル: `.takt/facets/output-contracts/pekko-compat-review.md`

# タスク仕様

## 目的

`.takt/facets/personas/pekko-compat-reviewer.md` の REJECT判定基準テーブルを Policy ファセットに移動し、Persona ファセットの品質基準（ポリシー詳細ルールを混入させない）に準拠させる。

## 要件

- [ ] `pekko-compat-reviewer.md` から REJECT判定基準テーブルを削除し、「REJECT判定はポリシーに従う」程度の参照に置き換える
- [ ] REJECT判定基準テーブルを `fraktor-coding.md` ポリシーまたは専用の Pekko互換性ポリシーファセットに移動する
- [ ] 移動先でテーブルの内容（Pekko APIに対応するメソッドが欠落、型パラメータの対応が不正確 等）がそのまま保持されることを確認する
- [ ] `pekko-porting.yaml` のセクションマップ参照に矛盾がないことを確認する

## 受け入れ基準

- Persona ファセットにポリシー詳細ルール（テーブル）が含まれていない
- REJECT判定基準が Policy ファセットに存在し、レビュームーブメントから参照可能である
- `validate-takt-files.sh` がエラーなしで通る

## 参考情報

- GitHub Issue: #135
- 対象ファイル: `.takt/facets/personas/pekko-compat-reviewer.md`
- 関連ポリシー: `.takt/facets/policies/fraktor-coding.md`
